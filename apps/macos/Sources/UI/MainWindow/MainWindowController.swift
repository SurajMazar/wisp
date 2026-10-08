import AppKit
import WispIPC

/// The main application window: a full-size-content window hosting the React app.
@MainActor
public final class MainWindowController: NSWindowController, NSWindowDelegate {
    private let surface: WebSurface

    public init(router: IPCRouter) {
        surface = WebSurface(router: router, route: "/home")
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1080, height: 720),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Wisp"
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.minSize = NSSize(width: 860, height: 560)
        window.isReleasedWhenClosed = false
        window.contentView = surface.webView
        window.setFrameAutosaveName("WispMainWindow")
        if !window.setFrameUsingName("WispMainWindow") { window.center() }
        super.init(window: window)
        window.delegate = self
        surface.load()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not supported") }

    public func show(route: String? = nil) {
        if let route { surface.navigate(to: route) }
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate()
    }
}
