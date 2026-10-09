import AppKit
import WispIPC
import WispPlatform
import WispUI
import WispUtilities

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var deps: DependencyContainer?
    private var mainWindow: MainWindowController?
    private var menuBar: MenuBarController?
    private var assistant: AssistantCoordinator?
    private var focus: FocusCoordinator?
    private var voice: VoiceCoordinator?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let deps = DependencyContainer()
        self.deps = deps
        NSApp.mainMenu = MainMenu.build(settingsTarget: self, settingsAction: #selector(openSettings))

        let assistant = AssistantCoordinator(router: deps.router, invocation: deps.invocation)
        self.assistant = assistant
        let voice = VoiceCoordinator(router: deps.router, assistant: assistant)
        self.voice = voice
        let focus = FocusCoordinator(router: deps.router, assistant: assistant)
        focus.onCompleted = { [weak voice] in voice?.lastFocusAt = Date() }
        self.focus = focus

        let menuBar = MenuBarController(actions: .init(
            summon: { assistant.toggleExpanded() },
            openMain: { [weak self] in self?.showMain() },
            openSettings: { [weak self] in self?.openSettings() },
            quit: { NSApp.terminate(nil) }
        ))
        menuBar.setVisible(true)
        self.menuBar = menuBar

        deps.router.subscribe(IPCEvents.settingsChanged) { [weak self] settings in
            self?.apply(settings)
        }

        deps.invocation.delegate = self
        HostMethods.register(on: deps.router, deps: deps, assistant: assistant, focus: focus, voice: voice)

        Task { @MainActor in
            await deps.router.connect()
            await deps.core.setStateHandler { [weak self] state in
                await self?.engineStateChanged(state)
            }
            await deps.core.start()
            await self.loadInitialSettings()
        }

        showMain()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { showMain() }
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        // Wisp keeps running in the menu bar and beside the pointer.
        false
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        // Quit immediately. When this process exits, the engine's stdin closes and it shuts down
        // cleanly on that EOF. Never `.terminateLater` waiting on a main-actor Task: `terminate`
        // is often called from a main-queue block, which keeps the main queue busy, so the Task
        // would never run and the app would hang (and every UI→engine call with it).
        .terminateNow
    }

    // MARK: Actions

    private func showMain(route: String? = nil) {
        guard let deps else { return }
        if mainWindow == nil { mainWindow = MainWindowController(router: deps.router) }
        mainWindow?.show(route: route)
    }

    @objc func openSettings() {
        showMain(route: "/settings")
    }

    // MARK: State

    private func engineStateChanged(_ state: CoreProcess.State) {
        deps?.state.engine = state
        menuBar?.update(engine: state)
    }

    private func loadInitialSettings() async {
        guard let deps else { return }
        do {
            apply(try await deps.core.call(IPCMethods.settingsGet, Empty()))
        } catch {
            Log.app.error("initial settings load failed: \(String(describing: error), privacy: .public)")
        }
    }

    private func apply(_ settings: AppSettings) {
        let previous = deps?.state.settings
        deps?.state.settings = settings
        menuBar?.setVisible(settings.general.showMenuBarIcon)
        menuBar?.setShortcutHint(KeyNames.display(settings.invocation.shortcut))
        if previous?.invocation != settings.invocation {
            deps?.invocation.apply(settings.invocation)
        }
        if previous?.cursor != settings.cursor {
            assistant?.apply(settings.cursor)
            focus?.apply(cursor: settings.cursor)
        }
        if previous?.focus != settings.focus {
            focus?.apply(settings.focus)
        }
        if previous?.voice != settings.voice {
            voice?.apply(settings.voice)
        }
    }
}

extension AppDelegate: InvocationDelegate {
    func invocationStarted(source: InvocationSource, behavior: InvocationBehavior) {
        assistant?.invocationStarted(behavior: behavior)
        if behavior == .hold { focus?.holdStarted() }
        if behavior != .toggle { voice?.holdStarted() }
        deps?.router.emit(IPCEvents.invocationStarted, InvocationStarted(
            atMs: Int64(Date().timeIntervalSince1970 * 1000), behavior: behavior, source: source
        ))
    }

    func invocationEnded(source: InvocationSource, reason: InvocationEndReason, duration: Duration) {
        focus?.holdEnded()
        if reason == .tapped {
            // A quick tap: nothing was said or drawn — just open/close the panel.
            voice?.cancelHold()
            assistant?.invocationEnded()
            assistant?.toggleExpanded()
        } else {
            voice?.holdEnded()
            assistant?.invocationEnded()
        }
        let ms = duration.components.seconds * 1000 + duration.components.attoseconds / 1_000_000_000_000_000
        deps?.router.emit(IPCEvents.invocationEnded, InvocationEnded(durationMs: ms, reason: reason, source: source))
    }

    func invocationTapped() {
        assistant?.toggleExpanded()
    }

    func invocationRequestsMainWindow() {
        showMain()
    }

    func invocationStatusChanged(_ status: InvocationStatus) {
        deps?.router.emit(IPCEvents.invocationStatusChanged, status)
    }
}
