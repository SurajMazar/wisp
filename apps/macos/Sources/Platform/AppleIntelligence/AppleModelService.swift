import Foundation
import WispIPC
#if canImport(FoundationModels)
import FoundationModels
#endif

/// Apple Foundation Models (spec §11). Availability is read from the system every time — never
/// assumed. Generation streams text deltas through `onDelta`.
@MainActor
public final class AppleModelService {
    private var tasks: [String: Task<String, Error>] = [:]

    public init() {}

    public var status: AppleStatus {
        #if canImport(FoundationModels)
        if #available(macOS 26, *) {
            switch SystemLanguageModel.default.availability {
            case .available:
                return AppleStatus(detail: nil, state: .available)
            case .unavailable(let reason):
                let detail: String = switch reason {
                case .deviceNotEligible: "This Mac doesn't support Apple Intelligence."
                case .appleIntelligenceNotEnabled: "Turn on Apple Intelligence in System Settings to use Apple's on-device model."
                case .modelNotReady: "Apple's on-device model is still downloading."
                @unknown default: "Apple's on-device model isn't available right now."
                }
                return AppleStatus(detail: detail, state: reason == .deviceNotEligible ? .notInstalled : .unavailable)
            @unknown default:
                return AppleStatus(detail: "Apple's on-device model isn't available right now.", state: .unavailable)
            }
        }
        #endif
        return AppleStatus(detail: "Apple Foundation Models need macOS 26 or later.", state: .notInstalled)
    }

    public func generate(_ p: AppleGenerateParams, onDelta: @escaping @MainActor (String) -> Void) async throws -> String {
        #if canImport(FoundationModels)
        if #available(macOS 26, *) {
            guard case .available = SystemLanguageModel.default.availability else {
                throw IpcError(code: .unavailable, details: status.detail, message: "Apple's on-device model isn't available.")
            }
            let task = Task<String, Error> { @MainActor in
                let session = LanguageModelSession(instructions: p.instructions)
                let options = GenerationOptions(temperature: p.temperature, maximumResponseTokens: Int(p.maxTokens))
                var emitted = ""
                for try await snapshot in session.streamResponse(to: p.prompt, options: options) {
                    try Task.checkCancellation()
                    let text = snapshot.content
                    if text.hasPrefix(emitted), text.count > emitted.count {
                        onDelta(String(text.dropFirst(emitted.count)))
                    } else if !text.hasPrefix(emitted) {
                        // The model revised earlier text; send the remainder from the divergence.
                        onDelta(String(text.dropFirst(emitted.commonPrefix(with: text).count)))
                    }
                    emitted = text
                }
                return emitted
            }
            tasks[p.requestId] = task
            defer { tasks[p.requestId] = nil }
            do {
                return try await task.value
            } catch is CancellationError {
                throw IpcError(code: .cancelled, details: nil, message: "Cancelled.")
            } catch let e as IpcError {
                throw e
            } catch {
                // GenerationError and bridged system errors (e.g. the model service being busy).
                throw IpcError(code: .unavailable, details: String(describing: error), message: "Apple's on-device model couldn't answer that right now.")
            }
        }
        #endif
        throw IpcError(code: .unavailable, details: nil, message: "Apple Foundation Models need macOS 26 or later.")
    }

    public func cancel(_ requestId: String) {
        tasks[requestId]?.cancel()
    }
}
