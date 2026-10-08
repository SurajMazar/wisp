import AppKit
import Carbon.HIToolbox
import WispIPC

/// Captures the next shortcut pressed while Wisp is frontmost.
///
/// * A key with ≥1 modifier records a key chord (⌥Space).
/// * Holding ≥2 modifiers and releasing them without a key records a modifier-only chord (⌘⌥).
/// * Esc alone cancels. Events are swallowed so they don't reach the web view.
@MainActor
public final class ShortcutRecorder {
    public enum Outcome: Sendable {
        case recorded(Shortcut)
        case cancelled
        case timedOut
    }

    private var monitor: Any?
    private var peak = Set<Modifier>()
    private var continuation: CheckedContinuation<Outcome, Never>?
    private var timeoutTask: Task<Void, Never>?

    public init() {}

    public var isRecording: Bool { monitor != nil }

    public func record(timeout: Duration) async -> Outcome {
        finish(.cancelled)  // only one recording at a time
        peak = []
        return await withCheckedContinuation { cont in
            continuation = cont
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.flagsChanged, .keyDown]) { [weak self] event in
                self?.handle(event)
                return nil
            }
            timeoutTask = Task { [weak self] in
                try? await Task.sleep(for: timeout)
                self?.finish(.timedOut)
            }
        }
    }

    public func cancel() { finish(.cancelled) }

    private func handle(_ event: NSEvent) {
        let held = KeyNames.modifiers(from: event.modifierFlags)
        switch event.type {
        case .flagsChanged:
            if held.count > peak.count || !held.isSubset(of: peak) { peak.formUnion(held) }
            if held.isEmpty && peak.count >= 2 {
                finish(.recorded(Shortcut(keyCode: nil, keyLabel: nil, modifiers: KeyNames.order.filter(peak.contains))))
            } else if held.isEmpty {
                peak = []
            }
        case .keyDown:
            let code = Int(event.keyCode)
            if code == kVK_Escape && held.isEmpty { finish(.cancelled); return }
            guard !held.isEmpty, !KeyNames.modifierKeyCodes.contains(code) else { return }
            finish(.recorded(Shortcut(
                keyCode: Int(code),
                keyLabel: KeyNames.label(forKeyCode: code),
                modifiers: KeyNames.order.filter(held.contains)
            )))
        default:
            break
        }
    }

    private func finish(_ outcome: Outcome) {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        timeoutTask?.cancel()
        timeoutTask = nil
        continuation?.resume(returning: outcome)
        continuation = nil
    }
}
