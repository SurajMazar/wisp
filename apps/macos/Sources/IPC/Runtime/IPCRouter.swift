import Foundation
import WispUtilities

/// A connected UI surface (a web view) that can receive envelopes.
@MainActor
public protocol EnvelopeSink: AnyObject {
    func deliver(_ envelope: Envelope)
}

/// Routes messages between UI surfaces, native Swift code and `wisp-core`.
///
/// UI requests are validated (envelope shape, protocol version, known method) before anything is
/// forwarded. Core-targeted requests are re-issued with fresh ids so ids from different surfaces
/// can never collide. Core events fan out to every surface and to native subscribers.
@MainActor
public final class IPCRouter {
    public let core: CoreProcess
    private var sinks: [ObjectIdentifier: WeakSink] = [:]
    private var subscribers: [String: [(JSONValue?) -> Void]] = [:]
    private var hostMethods: [String: @MainActor (JSONValue?) async -> Result<JSONValue, IpcError>] = [:]

    private struct WeakSink { weak var sink: EnvelopeSink? }

    public init(core: CoreProcess) {
        self.core = core
    }

    /// Wire the core's callbacks to this router. Call once before `core.start()`.
    public func connect() async {
        await core.setMessageHandlers(
            event: { [weak self] name, payload in
                await self?.broadcast(event: name, payload: payload)
            },
            host: { [weak self] method, params in
                guard let self else { return .failure(.unavailable("router gone")) }
                return await self.handleHost(method: method, params: params)
            }
        )
    }

    // MARK: Surfaces

    public func attach(_ sink: EnvelopeSink) {
        sinks[ObjectIdentifier(sink)] = WeakSink(sink: sink)
    }

    public func detach(_ sink: EnvelopeSink) {
        sinks.removeValue(forKey: ObjectIdentifier(sink))
    }

    /// Handle a raw message posted by a UI surface.
    public func receive(json: Data, from sink: EnvelopeSink) {
        let env: Envelope
        do {
            env = try JSONDecoder().decode(Envelope.self, from: json)
        } catch {
            Log.ipc.error("rejected malformed UI message: \(error.localizedDescription, privacy: .public)")
            return
        }
        guard env.kind == .request, let id = env.id, let method = env.method else {
            Log.ipc.error("UI may only send requests")
            return
        }
        Task { @MainActor in
            let reply: Envelope
            switch await self.dispatch(method: method, params: env.params) {
            case .success(let v): reply = .response(id: id, result: v)
            case .failure(let e): reply = .response(id: id, error: e)
            }
            sink.deliver(reply)
        }
    }

    /// Dispatch a request by target. Unknown methods are rejected here, never forwarded.
    public func dispatch(method: String, params: JSONValue?) async -> Result<JSONValue, IpcError> {
        switch IPCMethods.targets[method] {
        case .none:
            return .failure(.methodNotFound(method))
        case .host:
            return await handleHost(method: method, params: params)
        case .core:
            do {
                return .success(try await core.request(method: method, params: params))
            } catch let e as IpcError {
                return .failure(e)
            } catch {
                return .failure(.unavailable(error.localizedDescription))
            }
        }
    }

    // MARK: Host methods

    /// Register a Swift implementation of a `host.*` method.
    public func register<P, R>(_ method: IPCMethod<P, R>, handler: @escaping @MainActor (P) async throws -> R) {
        precondition(method.target == .host, "only host methods can be registered")
        hostMethods[method.name] = { raw in
            do {
                let params = try (raw ?? .object([:])).decode(as: P.self)
                return .success(try JSONValue.from(try await handler(params)))
            } catch let e as IpcError {
                return .failure(e)
            } catch is DecodingError {
                return .failure(.invalidRequest("invalid params for \(method.name)"))
            } catch {
                return .failure(IpcError(code: .internal, details: error.localizedDescription, message: "Something went wrong in Wisp."))
            }
        }
    }

    private func handleHost(method: String, params: JSONValue?) async -> Result<JSONValue, IpcError> {
        guard let h = hostMethods[method] else { return .failure(.methodNotFound(method)) }
        return await h(params)
    }

    // MARK: Events

    /// Native subscription to a core event.
    public func subscribe<P>(_ event: IPCEvent<P>, _ handler: @escaping @MainActor (P) -> Void) {
        subscribers[event.name, default: []].append { payload in
            guard let payload, let typed = try? payload.decode(as: P.self) else {
                Log.ipc.error("dropping malformed \(event.name, privacy: .public) event")
                return
            }
            handler(typed)
        }
    }

    /// Publish a typed host event to surfaces, native subscribers and the core.
    public func emit<P>(_ event: IPCEvent<P>, _ payload: P) {
        guard let value = try? JSONValue.from(payload) else { return }
        broadcast(event: event.name, payload: value)
        Task { await core.sendEvent(name: event.name, payload: value) }
    }

    /// Publish an event to all surfaces and native subscribers (used for core and host events).
    public func broadcast(event name: String, payload: JSONValue?) {
        for s in subscribers[name] ?? [] { s(payload) }
        let env = Envelope.event(name, payload: payload)
        sinks = sinks.filter { $0.value.sink != nil }
        for (_, w) in sinks { w.sink?.deliver(env) }
    }
}
