import AppKit
import WebKit
import WispIPC

/// The compact floating assistant (spec §91): a non-activating panel that floats over every app
/// and Space without stealing focus from what the user is working on.
@MainActor
public final class FloatingAssistantPanel: NSPanel {
    private let surface: WebSurface
    public var onDismiss: (() -> Void)?

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

        setFrameAutosaveName("WispAssistantPanel")
        if !setFrameUsingName("WispAssistantPanel") { placeNearPointer() }
        surface.load()
    }

    public override var canBecomeKey: Bool { true }
    public override var canBecomeMain: Bool { false }

    /// Show without taking keyboard focus (hold mode keeps the user's app focused).
    public func present(takeFocus: Bool) {
        if !isVisible, !NSScreen.screens.contains(where: { $0.visibleFrame.intersects(frame) }) {
            placeNearPointer()
        }
        if takeFocus { makeKeyAndOrderFront(nil) } else { orderFrontRegardless() }
    }

    public func dismiss() {
        orderOut(nil)
    }

    public override func cancelOperation(_ sender: Any?) {
        onDismiss?()
    }

    /// Top-center of the display under the pointer.
    private func placeNearPointer() {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        guard let v = screen?.visibleFrame else { return }
        setFrameOrigin(NSPoint(x: v.midX - frame.width / 2, y: v.maxY - frame.height - 80))
    }
}
