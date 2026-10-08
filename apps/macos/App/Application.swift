import AppKit

/// Entry point. AppKit lifecycle (rather than SwiftUI `App`) gives precise control over
/// activation policy, status items, panels and overlays.
@main
@MainActor
enum Application {
    private static let delegate = AppDelegate()

    static func main() {
        let app = NSApplication.shared
        app.delegate = delegate
        app.setActivationPolicy(.regular)
        app.run()
    }
}
