import AppKit
import WispIPC
import WispPlatform
import WispUI
import WispUtilities

/// Owns the focus flow (spec §30–37): select → capture → describe → publish.
///
/// * Hold the shortcut, then drag (box / lasso / circle per settings) or click a point.
/// * Or use the panel's buttons: area, point, window under the pointer, whole screen.
/// Captures happen only when a selection completes — never continuously.
@MainActor
final class FocusCoordinator {
    private let router: IPCRouter
    private let assistant: AssistantCoordinator
    private let overlay = FocusOverlayController()
    private var settings = FocusSettings(dimSurroundings: true, drawStyle: .rectangle, margin: 24, pointSize: 480)
    private var accent = BuddyTheme.color(for: .default, custom: nil)
    private(set) var latest: FocusRegion?
    private var holdSession = false
    /// A capture started by releasing the keys; voice waits for it so the drawing is attached.
    private var pending: Task<Void, Never>?
    var onCompleted: (() -> Void)?

    init(router: IPCRouter, assistant: AssistantCoordinator) {
        self.router = router
        self.assistant = assistant
        overlay.onPhase = { [weak assistant] state in assistant?.set(state: state) }
    }

    func apply(_ s: FocusSettings) { settings = s }
    func apply(cursor: CursorSettings) { accent = BuddyTheme.color(for: cursor.theme, custom: cursor.customColor) }

    // MARK: Entry points

    /// Holding the invocation shortcut arms drawing.
    func holdStarted() {
        guard !overlay.isActive else { return }
        // Without screen access, don't prompt or open the panel mid-hold: that steals focus and
        // ends the hold, cutting off listening. Keep listening; the panel explains what's missing.
        guard ScreenPermission.status == .enabled else {
            router.emit(IPCEvents.focusEnded, FocusEnded(
                detail: "Screen access is off, so Wisp can't see what you draw.", reason: .permissionDenied
            ))
            return
        }
        holdSession = true
        router.emit(IPCEvents.focusStarted, FocusStarted(kind: .lasso))
        // While the shortcut is held, the pointer is a pen: draw, circle, underline or click as
        // many times as you like while talking. Releasing the keys sends it with what you said.
        overlay.begin(style: .lasso, dim: false, accent: accent, showHint: true, ink: true) { [weak self] result in
            guard let self else { return }
            if let result {
                self.pending = Task { await self.complete(result) }
            } else {
                self.end(.cancelled)
            }
        }
    }

    /// Releasing the shortcut sends what was drawn (nothing drawn: the overlay just goes away).
    func holdEnded() {
        guard holdSession else { return }
        holdSession = false
        if overlay.isActive { overlay.finishInk() }
    }

    /// Wait for a capture that releasing the keys started (bounded, so voice never hangs on it).
    func settle() async {
        guard let pending else { return }
        let timeout = Task { try? await Task.sleep(for: .seconds(3)); pending.cancel() }
        await pending.value
        timeout.cancel()
        self.pending = nil
    }

    func start(kind: FocusKind, showHint: Bool = true) {
        router.emit(IPCEvents.focusStarted, FocusStarted(kind: kind))
        guard ensurePermission() else { return }
        switch kind {
        case .window, .screen:
            Task { await self.captureImmediate(kind) }
        case .point, .rectangle, .lasso, .circle:
            let style: DrawStyle = switch kind {
            case .lasso: .lasso
            case .circle: .circle
            case .rectangle: .rectangle
            default: settings.drawStyle
            }
            overlay.begin(style: style, dim: settings.dimSurroundings, accent: accent, showHint: showHint) { [weak self] result in
                guard let self else { return }
                if let result {
                    Task { await self.complete(result) }
                } else {
                    self.end(.cancelled)
                }
            }
        }
    }

    func cancel() {
        overlay.cancel()
    }

    func clear() {
        if let id = latest?.id {
            try? FileManager.default.removeItem(at: AppPaths.focusCacheDirectory.appendingPathComponent("\(id).png"))
            try? FileManager.default.removeItem(at: AppPaths.focusCacheDirectory.appendingPathComponent("\(id).model.jpg"))
            try? FileManager.default.removeItem(at: AppPaths.focusCacheDirectory.appendingPathComponent("\(id).context.txt"))
        }
        latest = nil
        router.emit(IPCEvents.focusCleared, Empty())
    }

    // MARK: Flow

    private func ensurePermission() -> Bool {
        if ScreenPermission.status == .enabled { return true }
        if ScreenPermission.request() == .enabled { return true }
        assistant.set(state: .waitingForPermission)
        end(.permissionDenied, detail: "Allow Wisp under System Settings → Privacy & Security → Screen & System Audio Recording.")
        assistant.expand()
        return false
    }

    private func captureImmediate(_ kind: FocusKind) async {
        let pointer = FocusGeometry.cgPoint(fromAppKit: NSEvent.mouseLocation, primaryHeight: Displays.primaryHeight)
        guard let screen = Displays.screen(containingCG: pointer) else { return end(.captureFailed, detail: "No display under the pointer.") }
        let display = Displays.info(for: screen).frame.cg
        let rect: CGRect
        if kind == .window {
            guard let found = WindowLocator.window(at: pointer) else {
                return end(.nothingSelected, detail: "There's no window under the pointer.")
            }
            rect = found.window.frame.cg.intersection(display)
        } else {
            rect = display
        }
        await complete(FocusOverlayController.Result(kind: kind, selection: rect, path: [], screen: screen))
    }

    private func complete(_ r: FocusOverlayController.Result) async {
        assistant.set(state: .analyzing)
        let display = Displays.info(for: r.screen)
        let displayRect = display.frame.cg
        var selection = r.selection.intersection(displayRect)
        var path = r.path
        if r.kind == .point, let p = r.path.first {
            selection = FocusGeometry.pointRegion(center: p, side: CGFloat(settings.pointSize), display: displayRect)
            path = [p]
        }
        guard !selection.isEmpty else { return end(.nothingSelected) }
        let crop: CGRect = [.point, .window, .screen].contains(r.kind)
            ? selection.integral.intersection(displayRect)
            : FocusGeometry.crop(selection: selection, margin: CGFloat(settings.margin), display: displayRect).integral.intersection(displayRect)

        let id = UUID().uuidString.lowercased()
        let cursor = FocusGeometry.cgPoint(fromAppKit: NSEvent.mouseLocation, primaryHeight: Displays.primaryHeight)
        let center = CGPoint(x: selection.midX, y: selection.midY)
        let found = WindowLocator.window(at: center)

        do {
            var image = try await ScreenCapturer.capture(rect: crop, display: display)
            if !r.strokes.isEmpty, let inked = InkRenderer.burn(r.strokes, into: image, crop: crop, color: accent) {
                image = inked
            }
            _ = try ScreenCapturer.save(image, id: id, directory: AppPaths.focusCacheDirectory)
            // Read the text and the UI elements concurrently.
            async let recognized = TextRecognizer.recognize(image, crop: crop)
            let accessibility = await accessibilityContext(selection: selection, cursor: r.kind == .point ? path.first ?? cursor : cursor)
            let text = await recognized
            let region = FocusRegion(
                accessibility: accessibility,
                application: found?.application ?? WindowLocator.frontmostApplication(),
                createdAtMs: Int64(Date().timeIntervalSince1970 * 1000),
                crop: Rect(crop),
                cursor: Point(cursor),
                display: display,
                id: id,
                image: FocusImage(heightPx: UInt32(image.height), url: "wisp://focus/\(id).png", widthPx: UInt32(image.width)),
                kind: r.kind,
                path: path.map(Point.init),
                selection: Rect(selection),
                text: text,
                window: found?.window
            )
            latest = region
            onCompleted?()
            router.emit(IPCEvents.focusCompleted, region)
            assistant.set(state: .success)
            assistant.expand()
            try? await Task.sleep(for: .milliseconds(900))
            if assistant.state == .success { assistant.set(state: .idle) }
        } catch ScreenCapturer.CaptureError.permissionDenied {
            assistant.set(state: .waitingForPermission)
            end(.permissionDenied, detail: "Screen access is disabled.")
        } catch {
            Log.app.error("focus capture failed: \(error.localizedDescription, privacy: .public)")
            assistant.set(state: .error)
            end(.captureFailed, detail: error.localizedDescription)
        }
    }

    /// Look at the window the user is working in, without any selection UI (teacher mode).
    func observeFrontWindow() async throws -> FocusRegion {
        guard ScreenPermission.status == .enabled else {
            throw IpcError(code: .permissionDenied, details: nil, message: "Wisp needs Screen Recording permission to watch the lesson.")
        }
        guard let found = WindowLocator.frontmostWindow() else {
            throw IpcError(code: .unavailable, details: nil, message: "There's no window to look at.")
        }
        let frame = found.window.frame.cg
        let center = CGPoint(x: frame.midX, y: frame.midY)
        guard let screen = Displays.screen(containingCG: center) else {
            throw IpcError(code: .unavailable, details: nil, message: "That window isn't on a display.")
        }
        let display = Displays.info(for: screen)
        let crop = frame.intersection(display.frame.cg).integral
        guard !crop.isEmpty else { throw IpcError(code: .unavailable, details: nil, message: "That window is off screen.") }
        let id = UUID().uuidString.lowercased()
        let image = try await ScreenCapturer.capture(rect: crop, display: display)
        _ = try ScreenCapturer.save(image, id: id, directory: AppPaths.focusCacheDirectory)
        async let recognized = TextRecognizer.recognize(image, crop: crop)
        let cursor = FocusGeometry.cgPoint(fromAppKit: NSEvent.mouseLocation, primaryHeight: Displays.primaryHeight)
        let accessibility = await accessibilityContext(selection: crop, cursor: crop.contains(cursor) ? cursor : center)
        let text = await recognized
        return FocusRegion(
            accessibility: accessibility,
            application: found.application,
            createdAtMs: Int64(Date().timeIntervalSince1970 * 1000),
            crop: Rect(crop),
            cursor: Point(cursor),
            display: display,
            id: id,
            image: FocusImage(heightPx: UInt32(image.height), url: "wisp://focus/\(id).png", widthPx: UInt32(image.width)),
            kind: .window,
            path: [],
            selection: Rect(crop),
            text: text,
            window: found.window
        )
    }

    /// The UI elements inside the selection, ranked by the core (spec §35). Nil without permission.
    private func accessibilityContext(selection: CGRect, cursor: CGPoint) async -> AxContext? {
        guard AccessibilityReader.isTrusted else { return nil }
        let point = selection.contains(cursor) ? cursor : CGPoint(x: selection.midX, y: selection.midY)
        let snapshot = await AccessibilityReader.snapshot(
            point: point, region: selection, limits: .init(maxNodes: 400),
            excludingPID: ProcessInfo.processInfo.processIdentifier
        )
        do {
            return try await router.core.call(
                IPCMethods.accessibilityRank,
                AxRankParams(cursor: Point(point), limit: 40, region: Rect(selection), snapshot: snapshot)
            )
        } catch {
            Log.app.error("accessibility ranking failed: \(String(describing: error), privacy: .public)")
            return nil
        }
    }

    private func end(_ reason: FocusEndReason, detail: String? = nil) {
        if reason == .cancelled || reason == .nothingSelected, [.selecting, .drawing].contains(assistant.state) {
            assistant.set(state: .idle)
        }
        router.emit(IPCEvents.focusEnded, FocusEnded(detail: detail, reason: reason))
    }
}

extension FocusKind {
    init(_ style: DrawStyle) {
        self = switch style {
        case .rectangle: .rectangle
        case .lasso: .lasso
        case .circle: .circle
        }
    }
}
