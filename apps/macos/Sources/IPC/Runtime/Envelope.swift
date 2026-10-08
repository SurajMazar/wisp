import Foundation

public enum IPCTarget: String, Sendable, Codable {
    case core
    case host
}

/// Typed descriptor for a method (generated in `IPCMethods`).
public struct IPCMethod<Params: Codable & Sendable, Result: Codable & Sendable>: Sendable {
    public let name: String
    public let target: IPCTarget
    public init(name: String, target: IPCTarget) {
        self.name = name
        self.target = target
    }
}

/// Typed descriptor for an event (generated in `IPCEvents`).
public struct IPCEvent<Payload: Codable & Sendable>: Sendable {
    public let name: String
    public init(name: String) { self.name = name }
}

/// Wire envelope. Mirrors `crates/ipc/src/envelope.rs`; unknown keys are rejected.
public struct Envelope: Codable, Sendable, Equatable {
    public enum Kind: String, Codable, Sendable { case request, response, event }

    public var kind: Kind
    public var v: UInt32
    public var id: String?
    public var method: String?
    public var params: JSONValue?
    public var result: JSONValue?
    public var error: IpcError?
    public var event: String?
    public var payload: JSONValue?

    public static func request(id: String, method: String, params: JSONValue?) -> Envelope {
        Envelope(kind: .request, v: ipcProtocolVersion, id: id, method: method, params: params)
    }

    public static func response(id: String, result: JSONValue) -> Envelope {
        Envelope(kind: .response, v: ipcProtocolVersion, id: id, result: result)
    }

    public static func response(id: String, error: IpcError) -> Envelope {
        Envelope(kind: .response, v: ipcProtocolVersion, id: id, error: error)
    }

    public static func event(_ name: String, payload: JSONValue?) -> Envelope {
        Envelope(kind: .event, v: ipcProtocolVersion, event: name, payload: payload)
    }

    init(kind: Kind, v: UInt32, id: String? = nil, method: String? = nil, params: JSONValue? = nil,
         result: JSONValue? = nil, error: IpcError? = nil, event: String? = nil, payload: JSONValue? = nil) {
        self.kind = kind; self.v = v; self.id = id; self.method = method; self.params = params
        self.result = result; self.error = error; self.event = event; self.payload = payload
    }

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case kind, v, id, method, params, result, error, event, payload
    }

    private struct AnyKey: CodingKey {
        var stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }

    public init(from decoder: Decoder) throws {
        let keys = try decoder.container(keyedBy: AnyKey.self).allKeys.map(\.stringValue)
        let allowed = Set(CodingKeys.allCases.map(\.rawValue))
        if let bad = keys.first(where: { !allowed.contains($0) }) {
            throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "unknown envelope field \(bad)"))
        }
        let c = try decoder.container(keyedBy: CodingKeys.self)
        kind = try c.decode(Kind.self, forKey: .kind)
        v = try c.decode(UInt32.self, forKey: .v)
        id = try c.decodeIfPresent(String.self, forKey: .id)
        method = try c.decodeIfPresent(String.self, forKey: .method)
        params = try c.decodeIfPresent(JSONValue.self, forKey: .params)
        result = try c.decodeIfPresent(JSONValue.self, forKey: .result)
        error = try c.decodeIfPresent(IpcError.self, forKey: .error)
        event = try c.decodeIfPresent(String.self, forKey: .event)
        payload = try c.decodeIfPresent(JSONValue.self, forKey: .payload)
        guard v == ipcProtocolVersion else {
            throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "protocol version \(v) unsupported"))
        }
        switch kind {
        case .request where id == nil || method == nil,
             .response where id == nil,
             .event where event == nil:
            throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "incomplete \(kind) envelope"))
        default: break
        }
    }

    /// One line of newline-delimited JSON.
    public func encodedLine() throws -> Data {
        var data = try JSONEncoder().encode(self)
        data.append(0x0A)
        return data
    }
}

extension IpcError: Error {}

public extension IpcError {
    static func unavailable(_ details: String) -> IpcError {
        IpcError(code: .unavailable, details: details, message: "Wisp's engine isn't running right now.")
    }
    static func methodNotFound(_ method: String) -> IpcError {
        IpcError(code: .methodNotFound, details: "unknown method `\(method)`", message: "That action isn't supported.")
    }
    static func invalidRequest(_ details: String) -> IpcError {
        IpcError(code: .invalidRequest, details: details, message: "The request was malformed.")
    }
    static func timeout(_ method: String) -> IpcError {
        IpcError(code: .timeout, details: method, message: "Wisp took too long to respond.")
    }
}
