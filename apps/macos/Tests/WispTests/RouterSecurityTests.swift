import Foundation
import Testing
@testable import WispIPC

@MainActor
final class CapturingSink: EnvelopeSink {
    var received: [Envelope] = []
    func deliver(_ envelope: Envelope) { received.append(envelope) }
}

@Suite struct RouterSecurityTests {
    /// The UI can't reach powerful host methods directly — only the core can, behind its policy.
    @Test @MainActor func uiCannotCallCoreOnlyHostMethods() async throws {
        let router = IPCRouter(core: CoreProcess(executable: URL(fileURLWithPath: "/nonexistent"), dataDirectory: FileManager.default.temporaryDirectory))
        var performed = false
        router.register(IPCMethods.hostInputPerform) { _ in
            performed = true
            return ToolExecution(verification: Verification(detail: "", status: .verified))
        }
        router.register(IPCMethods.hostClipboardWriteText) { _ in Empty() }
        let sink = CapturingSink()
        func send(_ id: String, _ method: String, _ params: String) {
            let json = #"{"kind":"request","v":1,"id":"\#(id)","method":"\#(method)","params":\#(params)}"#
            router.receive(json: Data(json.utf8), from: sink)
        }
        send("1", "host.input.perform", #"{"tool":"click","args":{"x":1,"y":1}}"#)
        send("2", "host.keychain.get", #"{"account":"x"}"#)
        send("3", "host.clipboard.writeText", #"{"text":"ok"}"#)
        for _ in 0..<50 where sink.received.count < 3 { try await Task.sleep(for: .milliseconds(10)) }
        #expect(!performed)
        let byId = Dictionary(uniqueKeysWithValues: sink.received.compactMap { e in e.id.map { ($0, e) } })
        #expect(byId["1"]?.error?.code == .methodNotFound)
        #expect(byId["2"]?.error?.code == .methodNotFound)
        #expect(byId["3"]?.error == nil)
    }
}
