import AppKit
import WispIPC
import WispPlatform
import WispUI

/// Swift implementations of `host.*` IPC methods.
@MainActor
enum HostMethods {
    static func register(on router: IPCRouter, deps: DependencyContainer, assistant: @escaping @MainActor (Bool, InvocationSource?) -> Void) {
        router.register(IPCMethods.hostInvocationStatus) { _ in
            deps.invocation.refresh()
            return deps.invocation.status
        }

        router.register(IPCMethods.hostShortcutCheck) { params in
            ShortcutCheckResult(conflicts: ShortcutConflicts.check(params.shortcut))
        }

        router.register(IPCMethods.hostShortcutRecord) { params in
            guard (1_000...30_000).contains(params.timeoutMs) else {
                throw IpcError.invalidRequest("timeoutMs must be 1000–30000")
            }
            deps.invocation.suspend()
            defer { deps.invocation.resume() }
            switch await deps.recorder.record(timeout: .milliseconds(Int(params.timeoutMs))) {
            case .recorded(let shortcut):
                return ShortcutRecordResult(conflicts: ShortcutConflicts.check(shortcut), shortcut: shortcut)
            case .cancelled:
                throw IpcError(code: .cancelled, details: nil, message: "Recording cancelled.")
            case .timedOut:
                throw IpcError(code: .timeout, details: nil, message: "No shortcut was pressed.")
            }
        }

        router.register(IPCMethods.hostSystemOpenSettingsPane) { params in
            SystemSettings.open(params.pane)
            return Empty()
        }

        router.register(IPCMethods.hostAssistantSetVisible) { params in
            assistant(params.visible, params.source)
            return Empty()
        }
    }
}
