import Foundation
import WispUtilities

/// Supervises the `wisp-core` child process and speaks the line-delimited IPC protocol with it.
///
/// * Requests to the core are correlated by id and time out.
/// * Requests *from* the core (`host.*`) are answered by `hostHandler`.
/// * Events from the core are delivered, in order, to `eventHandler`.
/// * If the core exits unexpectedly, pending requests fail with `unavailable` and the process is
///   restarted with exponential backoff (DECISIONS.md D-002).
public actor CoreProcess {
    public enum State: Sendable, Equatable {
        case stopped
        case starting
        case running(pid: Int32)
        case restarting(attempt: Int, delay: Duration)
        case failed(String)
    }

    public typealias EventHandler = @Sendable (_ name: String, _ payload: JSONValue?) async -> Void
    public typealias HostHandler = @Sendable (_ method: String, _ params: JSONValue?) async -> Result<JSONValue, IpcError>
    public typealias StateHandler = @Sendable (State) async -> Void

    private let executable: URL
    private let arguments: [String]
    private var process: Process?
    private var stdin: FileHandle?
    private var pending: [String: CheckedContinuation<JSONValue, Error>] = [:]
    private var nextId: UInt64 = 1
    private var stopping = false
    private var restartAttempt = 0
    private var startedAt: ContinuousClock.Instant?
    private var readerTask: Task<Void, Never>?
    private(set) public var state: State = .stopped

    private var eventHandler: EventHandler?
    private var hostHandler: HostHandler?
    private var stateHandler: StateHandler?

    public init(executable: URL, dataDirectory: URL?) {
        self.executable = executable
        var args: [String] = []
        if let dataDirectory { args += ["--data-dir", dataDirectory.path] }
        self.arguments = args
    }

    /// Install event and host-request handlers (done by `IPCRouter.connect`).
    public func setMessageHandlers(event: EventHandler?, host: HostHandler?) {
        eventHandler = event
        hostHandler = host
    }

    /// Observe lifecycle state changes (engine status in the UI).
    public func setStateHandler(_ handler: StateHandler?) {
        stateHandler = handler
    }

    // MARK: Lifecycle

    public func start() async {
        stopping = false
        await launch()
    }

    public func stop() async {
        stopping = true
        readerTask?.cancel()
        try? stdin?.close()  // EOF → core shuts down cleanly
        if let p = process, p.isRunning {
            let deadline = ContinuousClock.now + .seconds(2)
            while p.isRunning, ContinuousClock.now < deadline { try? await Task.sleep(for: .milliseconds(20)) }
            if p.isRunning { p.terminate() }
        }
        failAllPending(.unavailable("core stopped"))
        process = nil
        stdin = nil
        await setState(.stopped)
    }

    private func launch() async {
        await setState(.starting)
        guard FileManager.default.isExecutableFile(atPath: executable.path) else {
            Log.core.error("wisp-core not found at \(self.executable.path, privacy: .public)")
            await setState(.failed("The Wisp engine is missing from the app bundle."))
            return
        }

        let p = Process()
        p.executableURL = executable
        p.arguments = arguments
        let inPipe = Pipe(), outPipe = Pipe()
        p.standardInput = inPipe
        p.standardOutput = outPipe
        p.standardError = FileHandle.standardError

        // Ordered stream of stdout chunks → single consumer, so events keep their order.
        let (chunks, continuation) = AsyncStream<Data>.makeStream()
        outPipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
                continuation.finish()
            } else {
                continuation.yield(data)
            }
        }
        p.terminationHandler = { [weak self] proc in
            let status = proc.terminationStatus
            Task { await self?.processExited(pid: proc.processIdentifier, status: status) }
        }

        do {
            try p.run()
        } catch {
            Log.core.error("failed to launch wisp-core: \(error.localizedDescription, privacy: .public)")
            await scheduleRestart(reason: "launch failed")
            return
        }
        process = p
        stdin = inPipe.fileHandleForWriting
        startedAt = .now
        Log.core.info("wisp-core started pid=\(p.processIdentifier)")
        await setState(.running(pid: p.processIdentifier))

        readerTask = Task { [weak self] in
            var lines = LineBuffer()
            for await chunk in chunks {
                for line in lines.append(chunk) { await self?.handleLine(line) }
            }
        }
    }

    private func processExited(pid: Int32, status: Int32) async {
        guard process?.processIdentifier == pid else { return }
        process = nil
        stdin = nil
        failAllPending(.unavailable("wisp-core exited with status \(status)"))
        if stopping { return }
        Log.core.error("wisp-core exited unexpectedly (status \(status)); restarting")
        await scheduleRestart(reason: "exit \(status)")
    }

    private func scheduleRestart(reason: String) async {
        // Reset backoff after a healthy minute.
        if let startedAt, ContinuousClock.now - startedAt > .seconds(60) { restartAttempt = 0 }
        restartAttempt += 1
        let delay = Duration.milliseconds(min(30_000, 500 * (1 << min(restartAttempt - 1, 6))))
        await setState(.restarting(attempt: restartAttempt, delay: delay))
        try? await Task.sleep(for: delay)
        if !stopping { await launch() }
    }

    private func setState(_ s: State) async {
        state = s
        await stateHandler?(s)
    }

    // MARK: Requests to the core

    /// Send a request to the core and await its raw result.
    public func request(method: String, params: JSONValue?, timeout: Duration = .seconds(30)) async throws -> JSONValue {
        guard let stdin else { throw IpcError.unavailable("core not running") }
        let id = "h\(nextId)"
        nextId += 1
        let line = try Envelope.request(id: id, method: method, params: params).encodedLine()

        return try await withCheckedThrowingContinuation { cont in
            pending[id] = cont
            do {
                try stdin.write(contentsOf: line)
            } catch {
                pending.removeValue(forKey: id)
                cont.resume(throwing: IpcError.unavailable("write failed: \(error.localizedDescription)"))
                return
            }
            Task { [weak self] in
                try? await Task.sleep(for: timeout)
                await self?.expire(id: id, method: method)
            }
        }
    }

    /// Typed convenience wrapper.
    public func call<P, R>(_ method: IPCMethod<P, R>, _ params: P, timeout: Duration = .seconds(30)) async throws -> R {
        let raw = try await request(method: method.name, params: try JSONValue.from(params), timeout: timeout)
        return try raw.decode(as: R.self)
    }

    /// Send a host event to the core (fire-and-forget; dropped if the core isn't running).
    public func sendEvent(name: String, payload: JSONValue?) {
        guard let stdin, let line = try? Envelope.event(name, payload: payload).encodedLine() else { return }
        try? stdin.write(contentsOf: line)
    }

    private func expire(id: String, method: String) {
        pending.removeValue(forKey: id)?.resume(throwing: IpcError.timeout(method))
    }

    private func failAllPending(_ error: IpcError) {
        let waiters = pending
        pending.removeAll()
        for (_, c) in waiters { c.resume(throwing: error) }
    }

    // MARK: Inbound

    private func handleLine(_ line: Data) async {
        let env: Envelope
        do {
            env = try JSONDecoder().decode(Envelope.self, from: line)
        } catch {
            Log.ipc.error("dropping malformed frame from core: \(error.localizedDescription, privacy: .public)")
            return
        }
        switch env.kind {
        case .response:
            guard let id = env.id, let cont = pending.removeValue(forKey: id) else { return }
            if let error = env.error { cont.resume(throwing: error) } else { cont.resume(returning: env.result ?? .null) }
        case .event:
            if let name = env.event { await eventHandler?(name, env.payload) }
        case .request:
            guard let id = env.id, let method = env.method else { return }
            let outcome: Result<JSONValue, IpcError>
            if IPCMethods.targets[method] != .host {
                outcome = .failure(.methodNotFound(method))
            } else if let hostHandler {
                outcome = await hostHandler(method, env.params)
            } else {
                outcome = .failure(.unavailable("host handlers not installed"))
            }
            let reply: Envelope
            switch outcome {
            case .success(let v): reply = .response(id: id, result: v)
            case .failure(let e): reply = .response(id: id, error: e)
            }
            if let data = try? reply.encodedLine() { try? stdin?.write(contentsOf: data) }
        }
    }
}
