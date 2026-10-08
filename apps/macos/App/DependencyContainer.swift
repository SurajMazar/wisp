import Foundation
import WispIPC
import WispPlatform
import WispUI
import WispUtilities

/// Owns long-lived services. Created once by the app delegate; no global singletons elsewhere.
@MainActor
final class DependencyContainer {
    let core: CoreProcess
    let router: IPCRouter
    let state: AppState
    let invocation: InvocationController
    let recorder: ShortcutRecorder

    init() {
        core = CoreProcess(executable: AppPaths.coreExecutable, dataDirectory: AppPaths.dataDirectory)
        router = IPCRouter(core: core)
        state = AppState()
        invocation = InvocationController()
        recorder = ShortcutRecorder()
    }
}
