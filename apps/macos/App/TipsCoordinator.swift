import AppKit
import WispIPC
import WispPlatform
import WispUI
import WispUtilities

/// Opt-in tips (plan Step 7): after ten minutes in the same app, the island offers a quick tour of
/// it, once per app. Uses only which app is in front — never a screenshot — and stays quiet while
/// Wisp is busy.
@MainActor
final class TipsCoordinator {
    private let router: IPCRouter
    private let assistant: AssistantCoordinator
    var enabled = false
    private var current: (bundle: String, name: String, since: Date)?
    private var timer: Timer?
    private var observer: NSObjectProtocol?

    static let after: TimeInterval = 10 * 60
    private static let tippedKey = "wisp.tips.tippedApps"
    /// Never offered: Wisp itself, system plumbing and password managers.
    private static let skip: Set<String> = [
        "com.wisp.assistant", "com.apple.finder", "com.apple.systempreferences", "com.apple.loginwindow",
        "com.1password.1password", "com.agilebits.onepassword7", "com.bitwarden.desktop", "com.apple.Passwords",
        "com.apple.keychainaccess",
    ]

    init(router: IPCRouter, assistant: AssistantCoordinator) {
        self.router = router
        self.assistant = assistant
        observer = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [weak self] note in
            let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
            MainActor.assumeIsolated { self?.frontmost(app) }
        }
        frontmost(NSWorkspace.shared.frontmostApplication)
        timer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.check() }
        }
    }

    private func frontmost(_ app: NSRunningApplication?) {
        // Get web content ready to read before the user asks about it.
        if let pid = app?.processIdentifier, app?.activationPolicy == .regular {
            AccessibilityReader.prepare(pid: pid)
        }
        guard let app, let bundle = app.bundleIdentifier, app.activationPolicy == .regular else {
            current = nil
            return
        }
        if current?.bundle != bundle {
            current = (bundle, app.localizedName ?? bundle, Date())
        }
    }

    private var tipped: Set<String> {
        get { Set(UserDefaults.standard.stringArray(forKey: Self.tippedKey) ?? []) }
        set { UserDefaults.standard.set(Array(newValue), forKey: Self.tippedKey) }
    }

    /// Whether a tip is due now (pure, for tests).
    static func due(bundle: String, since: Date, now: Date, tipped: Set<String>, idle: Bool) -> Bool {
        idle && !skip.contains(bundle) && !tipped.contains(bundle) && now.timeIntervalSince(since) >= after
    }

    private func check() {
        guard enabled, let c = current,
              Self.due(bundle: c.bundle, since: c.since, now: Date(), tipped: tipped,
                       idle: assistant.state == .idle && !assistant.isExpanded)
        else { return }
        tipped.insert(c.bundle)
        let name = c.name
        assistant.island.showTip("Want a quick tour of \(name)?") { [weak self] in self?.tour(name) }
    }

    private func tour(_ app: String) {
        assistant.expand()
        Task {
            do {
                _ = try await router.core.call(IPCMethods.conversationSend, SendParams(
                    act: false, allowCloud: false, background: false, conversationId: nil,
                    includeFocus: false, model: nil, teach: true, text: "Give me a quick tour of \(app)"
                ))
            } catch {
                Log.app.error("tour failed to start: \(String(describing: error), privacy: .public)")
            }
        }
    }
}
