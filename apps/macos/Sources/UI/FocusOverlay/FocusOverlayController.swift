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

        public init(kind: FocusKind, selection: CGRect, path: [CGPoint], screen: NSScreen) {
            self.kind = kind
            self.selection = selection
            self.path = path
            self.screen = screen
        }
    }

    public var onPhase: ((AssistantState) -> Void)?
    private var windows: [NSPanel] = []
    private var completion: ((Result?) -> Void)?
    private var timeout: Task<Void, Never>?
    private var screenObserver: NSObjectProtocol?

    public init() {}

    public var isActive: Bool { !windows.isEmpty }
    /// The user has started dragging (mouse is down).
    public private(set) var isDrawing = false

    /// Show the overlay. `completion` receives the selection, or nil if cancelled.
    public func begin(style: DrawStyle, dim: Bool, accent: NSColor, showHint: Bool, completion: @escaping (Result?) -> Void) {
        cancel()
        self.completion = completion
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let pointer = NSEvent.mouseLocation
        for screen in NSScreen.screens {
            let panel = NSPanel(contentRect: screen.frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
            panel.isOpaque = false
            panel.backgroundColor = .clear
            panel.hasShadow = false
            panel.level = .popUpMenu
            panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
            panel.hidesOnDeactivate = false
            panel.isReleasedWhenClosed = false
            panel.sharingType = .none
            panel.animationBehavior = .none
            panel.setFrame(screen.frame, display: false)

            let view = FocusOverlayView(frame: NSRect(origin: .zero, size: screen.frame.size))
            view.style = style
            view.dimSurroundings = dim
            view.accent = accent
            view.reduceMotion = reduceMotion
            view.onCancel = { [weak self] in self?.cancel() }
            view.onPhase = { [weak self] phase in
                guard let self else { return }
                if phase == .drawing {
                    self.isDrawing = true
                    self.onPhase?(.drawing)
                    for w in self.windows where w !== panel { (w.contentView as? FocusOverlayView)?.dimAll(true) }
                }
            }
            view.onFinish = { [weak self, weak panel] sel in
                guard let self, let panel else { return }
                self.finish(sel, on: screen, panel: panel)
            }
            panel.contentView = view
            view.prepare(showHint: showHint && NSMouseInRect(pointer, screen.frame, false))
            panel.orderFrontRegardless()
            if NSMouseInRect(pointer, screen.frame, false) {
                panel.makeKey()
                panel.makeFirstResponder(view)
            }
            windows.append(panel)
        }
        onPhase?(.selecting)
        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
        ) { [weak self] _ in MainActor.assumeIsolated { self?.cancel() } }
        timeout = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(60))
            self?.cancel()
        }
    }

    /// Remove the overlay without a selection.
    public func cancel() {
        guard isActive || completion != nil else { return }
        let done = completion
        completion = nil
        teardown()
        done?(nil)
    }

    private func finish(_ sel: FocusOverlayView.Selection, on screen: NSScreen, panel: NSPanel) {
        let done = completion
        completion = nil
        timeout?.cancel()
        let h = Displays.primaryHeight
        let origin = screen.frame.origin
        let toGlobalCG: (CGPoint) -> CGPoint = {
            FocusGeometry.cgPoint(fromAppKit: CGPoint(x: $0.x + origin.x, y: $0.y + origin.y), primaryHeight: h)
        }
        let rectCG = FocusGeometry.cgRect(fromAppKit: sel.rect.offsetBy(dx: origin.x, dy: origin.y), primaryHeight: h)
        let result = Result(kind: sel.kind, selection: rectCG, path: sel.path.map(toGlobalCG), screen: screen)

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
        timeout?.cancel()
        timeout = nil
        if let screenObserver { NotificationCenter.default.removeObserver(screenObserver) }
        screenObserver = nil
        for w in windows {
            w.orderOut(nil)
            w.contentView = nil
        }
        windows.removeAll()
    }
}
