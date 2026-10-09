import AppKit
import WebKit
import WispIPC

/// The compact floating assistant (spec §91): a non-activating panel that floats over every app
/// and Space without stealing focus from what the user is working on.
@MainActor
public final class FloatingAssistantPanel: NSPanel {
    private let surface: WebSurface
    public var onDismiss: (() -> Void)?
    /// Called when the panel loses key status (the user clicked elsewhere) — auto-hide.
    public var onResignKey: (() -> Void)?
    public private(set) var isExpanded = false

    public init(router: IPCRouter) {
        surface = WebSurface(router: router, route: "/assistant", transparent: true)
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 440, height: 300),
            styleMask: [.nonactivatingPanel, .titled, .fullSizeContentView, .resizable],
            backing: .buffered,
            defer: false
        )
        titleVisibility = .hidden
        titlebarAppearsTransparent = true
        standardWindowButton(.closeButton)?.isHidden = true
        standardWindowButton(.miniaturizeButton)?.isHidden = true
        standardWindowButton(.zoomButton)?.isHidden = true
        isFloatingPanel = true
        level = .floating
        hidesOnDeactivate = false
        becomesKeyOnlyIfNeeded = true
        isMovableByWindowBackground = true
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        minSize = NSSize(width: 360, height: 200)
        maxSize = NSSize(width: 720, height: 900)
        animationBehavior = .utilityWindow

        let effect = NSVisualEffectView()
        effect.material = .popover
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.wantsLayer = true
        effect.layer?.cornerRadius = 14
        effect.layer?.masksToBounds = true
        surface.webView.translatesAutoresizingMaskIntoConstraints = false
        effect.addSubview(surface.webView)
        NSLayoutConstraint.activate([
            surface.webView.leadingAnchor.constraint(equalTo: effect.leadingAnchor),
            surface.webView.trailingAnchor.constraint(equalTo: effect.trailingAnchor),
            surface.webView.topAnchor.constraint(equalTo: effect.topAnchor),
            surface.webView.bottomAnchor.constraint(equalTo: effect.bottomAnchor),
        ])
        contentView = effect

        // Size is remembered; position always follows the buddy.
        setFrameAutosaveName("WispAssistantPanel")
        _ = setFrameUsingName("WispAssistantPanel")
        surface.load()
    }

    public override var canBecomeKey: Bool { true }
    public override var canBecomeMain: Bool { false }

    /// Expand out of the buddy at `anchor` (screen rect), taking keyboard focus.
    public func expand(from anchor: NSRect) {
        let target = Self.frame(size: frame.size, beside: anchor)
        isExpanded = true
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            setFrame(target, display: true)
            alphaValue = 1
            makeKeyAndOrderFront(nil)
            return
        }
        setFrame(NSRect(x: anchor.midX - 20, y: anchor.midY - 14, width: 40, height: 28), display: false)
        alphaValue = 0
        makeKeyAndOrderFront(nil)
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.22
            ctx.timingFunction = CAMediaTimingFunction(controlPoints: 0.22, 1, 0.36, 1)
            animator().setFrame(target, display: true)
            animator().alphaValue = 1
        }
    }

    /// Shrink back into the buddy at `anchor` and hide.
    public func collapse(to anchor: NSRect?) {
        guard isExpanded else { return }
        isExpanded = false
        saveFrame(usingName: "WispAssistantPanel")
        let restore = frame.size
        guard let anchor, !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion else {
            orderOut(nil)
            return
        }
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.16
            animator().setFrame(NSRect(x: anchor.midX - 20, y: anchor.midY - 14, width: 40, height: 28), display: true)
            animator().alphaValue = 0
        } completionHandler: { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !self.isExpanded else { return }
                self.orderOut(nil)
                self.setContentSize(restore)
            }
        }
    }

    public override func resignKey() {
        super.resignKey()
        onResignKey?()
    }

    /// Place a panel of `size` beside `anchor`, inside the visible frame of the anchor's display.
    static func frame(size: NSSize, beside anchor: NSRect) -> NSRect {
        let center = NSPoint(x: anchor.midX, y: anchor.midY)
        let screen = NSScreen.screens.first { NSMouseInRect(center, $0.frame, false) } ?? NSScreen.main
        let v = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        var x = anchor.maxX + 6
        if x + size.width > v.maxX { x = anchor.minX - 6 - size.width }
        var y = anchor.maxY + 6 - size.height
        if y < v.minY { y = v.minY + 6 }
        if y + size.height > v.maxY { y = v.maxY - size.height - 6 }
        x = min(max(x, v.minX + 6), v.maxX - size.width - 6)
        return NSRect(x: x.rounded(), y: y.rounded(), width: size.width, height: size.height)
    }

    public override func cancelOperation(_ sender: Any?) {
        onDismiss?()
    }

}
