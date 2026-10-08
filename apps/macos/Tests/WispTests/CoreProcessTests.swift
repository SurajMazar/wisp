import Foundation
import Testing
@testable import WispIPC

/// Exercises the real `wisp-core` binary (cargo build -p wisp-core) through the same supervisor
/// the app uses: Swift → stdio IPC → Rust → SQLite.
@Suite(.serialized) struct CoreProcessTests {
    static var coreBinary: URL {
        if let p = ProcessInfo.processInfo.environment["WISP_CORE_BIN"] { return URL(fileURLWithPath: p) }
        return URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("target/debug/wisp-core")
    }

    private func tempDir() throws -> URL {
        let d = FileManager.default.temporaryDirectory.appendingPathComponent("wisp-swift-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d
    }

    private func waitRunning(_ core: CoreProcess) async throws {
        for _ in 0..<200 {
            if case .running = await core.state { return }
            try await Task.sleep(for: .milliseconds(25))
        }
        Issue.record("core never reached running state")
    }

    @Test func settingsPersistAcrossCoreRestart() async throws {
        let dir = try tempDir()
        defer { try? FileManager.default.removeItem(at: dir) }

        let core = CoreProcess(executable: Self.coreBinary, dataDirectory: dir)
        await core.start()
        try await waitRunning(core)
        let info = try await core.call(IPCMethods.appInfo, Empty())
        #expect(info.protocolVersion == ipcProtocolVersion)

        let updated = try await core.call(
            IPCMethods.settingsUpdate,
            SettingsUpdateParams(update: .appearance(AppearanceSettings(accent: .green, theme: .light)))
        )
        #expect(updated.appearance.accent == .green)
        await core.stop()

        let again = CoreProcess(executable: Self.coreBinary, dataDirectory: dir)
        await again.start()
        try await waitRunning(again)
        let s = try await again.call(IPCMethods.settingsGet, Empty())
        #expect(s.appearance.accent == .green)
        #expect(s.appearance.theme == .light)
        await again.stop()
    }

    @Test func coreErrorsSurfaceAsIpcErrors() async throws {
        let dir = try tempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let core = CoreProcess(executable: Self.coreBinary, dataDirectory: dir)
        await core.start()
        try await waitRunning(core)
        await #expect(throws: IpcError.self) {
            _ = try await core.request(method: "no.such", params: nil)
        }
        await core.stop()
    }

    @Test func restartsAfterCrash() async throws {
        let dir = try tempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let core = CoreProcess(executable: Self.coreBinary, dataDirectory: dir)
        await core.start()
        try await waitRunning(core)
        guard case .running(let pid) = await core.state else { return }

        kill(pid, SIGKILL)
        // Wait for a *new* pid.
        var newPid: Int32 = pid
        for _ in 0..<200 {
            if case .running(let p) = await core.state, p != pid { newPid = p; break }
            try await Task.sleep(for: .milliseconds(25))
        }
        #expect(newPid != pid, "core was not restarted")
        let info = try await core.call(IPCMethods.appInfo, Empty())
        #expect(info.schemaVersion >= 7)
        await core.stop()
    }

    @Test func missingBinaryFailsGracefully() async {
        let core = CoreProcess(executable: URL(fileURLWithPath: "/nonexistent/wisp-core"), dataDirectory: nil)
        await core.start()
        guard case .failed = await core.state else {
            Issue.record("expected failed state")
            return
        }
        await #expect(throws: IpcError.self) { _ = try await core.request(method: "app.info", params: nil) }
    }
}

@Suite @MainActor struct RouterTests {
    @Test func rejectsUnknownMethodsWithoutForwarding() async {
        let router = IPCRouter(core: CoreProcess(executable: URL(fileURLWithPath: "/nonexistent"), dataDirectory: nil))
        let r = await router.dispatch(method: "shell.exec", params: nil)
        guard case .failure(let e) = r else { Issue.record("expected failure"); return }
        #expect(e.code == .methodNotFound)
    }
}
