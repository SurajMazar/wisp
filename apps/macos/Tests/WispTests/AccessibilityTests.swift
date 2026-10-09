import AppKit
import ApplicationServices
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite(.serialized) @MainActor struct AccessibilityReaderTests {
    @Test func normalizesRealControlsAndNeverReadsPasswords() async throws {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 360, height: 160), styleMask: [.titled], backing: .buffered, defer: false)
        window.title = "AX Test Window"
        let button = NSButton(title: "Submit", target: nil, action: nil)
        button.frame = NSRect(x: 20, y: 20, width: 120, height: 32)
        let secure = NSSecureTextField(frame: NSRect(x: 20, y: 80, width: 200, height: 24))
        secure.stringValue = "hunter2"
        window.contentView?.addSubview(button)
        window.contentView?.addSubview(secure)
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }
        try await Task.sleep(for: .milliseconds(300))

        let app = AXUIElementCreateApplication(getpid())
        let windows: [AXUIElement]? = AccessibilityReader.copy(app, kAXWindowsAttribute)
        guard let axWindow = windows?.first(where: { AccessibilityReader.string($0, kAXTitleAttribute) == "AX Test Window" }) else {
            print("skipping: AX unavailable to the test runner (trusted: \(AXIsProcessTrusted()))")
            return
        }

        var found: [AxNode] = []
        var queue: [AXUIElement] = [axWindow]
        while let el = queue.popLast(), found.count < 50 {
            found.append(AccessibilityReader.node(el, depth: 0))
            queue.append(contentsOf: (AccessibilityReader.copy(el, kAXChildrenAttribute) as [AXUIElement]?) ?? [])
        }
        let submit = try #require(found.first { $0.role == "Button" && $0.title == "Submit" })
        #expect(submit.enabled)
        #expect(submit.actions?.contains("Press") == true)
        let frame = try #require(submit.frame)
        #expect(abs(frame.width - 120) < 2 && abs(frame.height - 32) < 2)
        // AX frames are global top-left points; compare with the window's converted frame.
        let expected = FocusGeometry.cgRect(fromAppKit: window.convertToScreen(button.frame), primaryHeight: NSScreen.screens[0].frame.height)
        #expect(abs(frame.x - expected.minX) < 2 && abs(frame.y - expected.minY) < 2, "AX frame \(frame) vs \(expected)")

        let pw = try #require(found.first { $0.role == "TextField" && $0.subrole == "SecureTextField" || $0.role == "SecureTextField" })
        #expect(pw.value == nil, "secure field value must never be read")
        #expect(!found.contains { $0.value?.contains("hunter2") == true })
    }
}
