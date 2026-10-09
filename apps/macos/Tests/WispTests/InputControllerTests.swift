import AppKit
import Carbon.HIToolbox
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite struct InputControllerTests {
    @Test @MainActor func mapsKeysForShortcuts() {
        #expect(InputController.keyCode(for: "s") == kVK_ANSI_S)
        #expect(InputController.keyCode(for: "S") == kVK_ANSI_S)
        #expect(InputController.keyCode(for: "return") == kVK_Return)
        #expect(InputController.keyCode(for: "5") == kVK_ANSI_5)
        #expect(InputController.keyCode(for: "nope") == nil)
    }

    @Test @MainActor func clipboardRoundTripRestoresTheUsersClipboard() async throws {
        let pb = NSPasteboard.general
        let saved = pb.string(forType: .string)
        defer {
            pb.clearContents()
            if let saved { pb.setString(saved, forType: .string) }
        }
        let w = try await InputController.perform(.writeClipboard(TextArgs(text: "wisp-test-123")))
        #expect(w.verification.status == .verified)
        let r = try await InputController.perform(.readClipboard(Empty()))
        #expect(r.output == "wisp-test-123")
    }

    @Test @MainActor func refusesNonWebURLs() async {
        await #expect(throws: InputController.Failure.self) {
            _ = try await InputController.perform(.openUrl(UrlArgs(url: "file:///etc/hosts")))
        }
    }

    @Test @MainActor func observesWithoutPermissionAndNeverCrashes() {
        let o = InputController.observe(at: CGPoint(x: 10, y: 10))
        #expect(o.frontPID != nil)
    }
}
