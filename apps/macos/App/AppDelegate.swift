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
    private var assistantPanel: FloatingAssistantPanel?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let deps = DependencyContainer()
        self.deps = deps
        NSApp.mainMenu = MainMenu.build(settingsTarget: self, settingsAction: #selector(openSettings))

        let menuBar = MenuBarController(actions: .init(
            summon: { deps.invocation.toggle(from: .menuBar) },
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
        HostMethods.register(on: deps.router, deps: deps) { visible, source in
            if visible {
                if !deps.invocation.isActive { deps.invocation.toggle(from: source ?? .commandPalette) }
            } else {
                deps.invocation.dismissed()
            }
        }

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
        // Wisp keeps running in the menu bar.
        false
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let core = deps?.core else { return .terminateNow }
        Task {
            await core.stop()
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
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
    }

    private func panel() -> FloatingAssistantPanel? {
        guard let deps else { return nil }
        if assistantPanel == nil {
            let p = FloatingAssistantPanel(router: deps.router)
            p.onDismiss = { [weak deps] in deps?.invocation.dismissed() }
            assistantPanel = p
        }
        return assistantPanel
    }
}

extension AppDelegate: InvocationDelegate {
    func invocationStarted(source: InvocationSource, behavior: InvocationBehavior) {
        // Hold keeps the user's app focused; toggle and menu invocations take focus for typing.
        panel()?.present(takeFocus: behavior == .toggle)
        deps?.router.emit(IPCEvents.invocationStarted, InvocationStarted(
            atMs: Int64(Date().timeIntervalSince1970 * 1000), behavior: behavior, source: source
        ))
    }

    func invocationEnded(source: InvocationSource, reason: InvocationEndReason, duration: Duration) {
        assistantPanel?.dismiss()
        let ms = duration.components.seconds * 1000 + duration.components.attoseconds / 1_000_000_000_000_000
        deps?.router.emit(IPCEvents.invocationEnded, InvocationEnded(durationMs: ms, reason: reason, source: source))
    }

    func invocationRequestsMainWindow() {
        showMain()
    }

    func invocationStatusChanged(_ status: InvocationStatus) {
        deps?.router.emit(IPCEvents.invocationStatusChanged, status)
    }
}
