import AppKit
import WispIPC
import WispPlatform
import WispUtilities

/// Presents the focus overlay on every display and reports what the user selected (spec §33, §93).
///
/// The overlay is excluded from screen capture, removed on every exit path (finish, cancel,
/// display change, timeout) so no stale overlay can remain.
@MainActor
public final class FocusOverlayController {
    /// A finished selection in global CG points (top-left origin).
    public struct Result {
        public let kind: FocusKind
        public let selection: CGRect
        public let path: [CGPoint]
        public let screen: NSScreen
        /// Pen strokes drawn while holding the shortcut (global CG points), burned into the capture
        /// so the model sees what was marked.
        public let strokes: [[CGPoint]]

        public init(kind: FocusKind, selection: CGRect, path: [CGPoint], screen: NSScreen, strokes: [[CGPoint]] = []) {
            self.kind = kind
            self.selection = selection
            self.path = path
            self.screen = screen
            self.strokes = strokes
        }
    }

    public var onPhase: ((AssistantState) -> Void)?
    private var windows: [NSPanel] = []
    private var completion: ((Result?) -> Void)?
    private var timeout: Task<Void, Never>?
    private var screenObserver: NSObjectProtocol?
    private var activationObserver: NSObjectProtocol?
    /// The app the user was in when drawing started; drawing must never take focus from it.
    private var previousApp: NSRunningApplication?

    public init() {}

    public var isActive: Bool { !windows.isEmpty }
    /// The user has started dragging (mouse is down).
    public private(set) var isDrawing = false

    /// Show the overlay. `completion` receives the selection, or nil if cancelled.
    public func begin(style: DrawStyle, dim: Bool, accent: NSColor, showHint: Bool, ink: Bool = false, completion: @escaping (Result?) -> Void) {
        cancel()
        self.completion = completion
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let pointer = NSEvent.mouseLocation
        for screen in NSScreen.screens {
            let panel = OverlayPanel(contentRect: screen.frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
            // While drawing, stay out of the way: never key, so clicking never activates Wisp
            // (which would bring its windows to the front over the app you're drawing on).
            panel.keyable = !ink
            panel.becomesKeyOnlyIfNeeded = true
            panel.isOpaque = false
            panel.backgroundColor = .clear
            panel.hasShadow = false
            panel.level = .popUpMenu
            panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
            panel.hidesOnDeactivate = false
            panel.isReleasedWhenClosed = false
            panel.sharingType = .none
            panel.animationBehavior = .none
            // Claim every click, even over transparent pixels, so drags draw instead of reaching
            // the app underneath.
            panel.ignoresMouseEvents = false
            panel.acceptsMouseMovedEvents = true
            panel.setFrame(screen.frame, display: false)

            let view = FocusOverlayView(frame: NSRect(origin: .zero, size: screen.frame.size))
            view.style = style
            view.dimSurroundings = dim
            view.accent = accent
            view.reduceMotion = reduceMotion
            view.ink = ink
            view.onCancel = { [weak self] in self?.cancel() }
            view.onPhase = { [weak self] phase in
                guard let self else { return }
                if phase == .drawing {
                    self.isDrawing = true
                    self.onPhase?(.drawing)
                    if !ink {
                        for w in self.windows where w !== panel { (w.contentView as? FocusOverlayView)?.dimAll(true) }
                    }
                }
            }
            view.onFinish = { [weak self, weak panel] sel in
                guard let self, let panel else { return }
                self.finish(sel, on: screen, panel: panel)
            }
            panel.contentView = view
            view.prepare(showHint: showHint && NSMouseInRect(pointer, screen.frame, false))
            panel.orderFrontRegardless()
            if !ink, NSMouseInRect(pointer, screen.frame, false) {
                panel.makeKey()
                panel.makeFirstResponder(view)
            }
            windows.append(panel)
        }
        onPhase?(ink ? .listening : .selecting)
        NSCursor.crosshair.set()
        if ink {
            let me = NSRunningApplication.current
            previousApp = NSWorkspace.shared.frontmostApplication.flatMap { $0 == me ? nil : $0 }
            // Belt and braces: if anything activates Wisp mid-drawing, hand focus straight back.
            activationObserver = NotificationCenter.default.addObserver(
                forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self, self.completion != nil, let app = self.previousApp, !app.isTerminated else { return }
                    app.activate()
                }
            }
        }
        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
        ) { [weak self] _ in MainActor.assumeIsolated { self?.cancel() } }
        timeout = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(60))
            self?.cancel()
        }
    }

    /// Ink mode: the keys were released. Send what was drawn, or put the overlay away if nothing was.
    public func finishInk() {
        guard completion != nil else { return }
        var best: (FocusOverlayView, NSPanel, FocusOverlayView.Selection)?
        for w in windows {
            guard let v = w.contentView as? FocusOverlayView else { continue }
            let count = v.strokes.reduce(0) { $0 + $1.count }
            if let sel = v.finishInk(), best == nil || count > (best?.0.strokes.reduce(0) { $0 + $1.count } ?? 0) {
                best = (v, w, sel)
            }
        }
        guard let (view, panel, sel) = best, let screen = panel.screen ?? NSScreen.screens.first(where: { $0.frame == panel.frame }) else {
            return cancel()
        }
        finish(sel, on: screen, panel: panel, strokes: view.strokes)
    }

    /// Remove the overlay without a selection.
    public func cancel() {
        guard isActive || completion != nil else { return }
        let done = completion
        completion = nil
        teardown()
        done?(nil)
    }

    private func finish(_ sel: FocusOverlayView.Selection, on screen: NSScreen, panel: NSPanel, strokes: [[CGPoint]] = []) {
        let done = completion
        completion = nil
        timeout?.cancel()
        let h = Displays.primaryHeight
        let origin = screen.frame.origin
        let toGlobalCG: (CGPoint) -> CGPoint = {
            FocusGeometry.cgPoint(fromAppKit: CGPoint(x: $0.x + origin.x, y: $0.y + origin.y), primaryHeight: h)
        }
        let rectCG = FocusGeometry.cgRect(fromAppKit: sel.rect.offsetBy(dx: origin.x, dy: origin.y), primaryHeight: h)
        let result = Result(kind: sel.kind, selection: rectCG, path: sel.path.map(toGlobalCG), screen: screen,
                            strokes: strokes.map { $0.map(toGlobalCG) })

        // Hand the selection over immediately (capture excludes Wisp's windows), then animate out.
        done?(result)
        for w in windows where w !== panel { w.orderOut(nil) }
        if sel.kind == .point {
            teardown()
        } else {
            (panel.contentView as? FocusOverlayView)?.showFocused(rect: sel.rect, path: sel.path, kind: sel.kind) { [weak self] in
                MainActor.assumeIsolated { self?.teardown() }
            }
            // Belt and braces: never leave an overlay behind.
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(2))
                self?.teardown()
            }
        }
    }

    private func teardown() {
        isDrawing = false
        NSCursor.arrow.set()
        timeout?.cancel()
        timeout = nil
        if let screenObserver { NotificationCenter.default.removeObserver(screenObserver) }
        screenObserver = nil
        if let activationObserver { NotificationCenter.default.removeObserver(activationObserver) }
        activationObserver = nil
        previousApp = nil
        for w in windows {
            w.orderOut(nil)
            w.contentView = nil
        }
        windows.removeAll()
    }
}

/// A borderless overlay panel that becomes key only when keyboard input is wanted (Esc to cancel
/// a box/lasso selection), never while the pointer is a pen.
final class OverlayPanel: NSPanel {
    var keyable = true
    override var canBecomeKey: Bool { keyable }
    override var canBecomeMain: Bool { false }
}
