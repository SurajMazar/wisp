import Foundation
import Testing
@testable import WispIPC

@Suite struct RouterEndToEndTests {
    /// The exact message the first-run "Start using Wisp" button sends, through the real router
    /// and the real engine.
    @Test @MainActor func uiSettingsUpdateReachesTheEngine() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("wisp-router-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let core = CoreProcess(executable: CoreProcessTests.coreBinary, dataDirectory: dir)
        let router = IPCRouter(core: core)
        await core.start()
        for _ in 0..<200 {
            if case .running = await core.state { break }
            try await Task.sleep(for: .milliseconds(25))
        }
        let sink = CapturingSink()
        let json = #"{"kind":"request","v":1,"id":"7","method":"settings.update","params":{"update":{"section":"onboarding","value":{"completed":true,"completedAtMs":1760000000000}}}}"#
        router.receive(json: Data(json.utf8), from: sink)
        for _ in 0..<300 where sink.received.isEmpty { try await Task.sleep(for: .milliseconds(20)) }
        let reply = try #require(sink.received.first)
        #expect(reply.error == nil, "\(String(describing: reply.error))")
        await core.stop()
    }
}
