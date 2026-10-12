import AppKit
import Carbon.HIToolbox
import WispIPC
import WispUtilities

@MainActor
public protocol InvocationDelegate: AnyObject {
    func invocationStarted(source: InvocationSource, behavior: InvocationBehavior)
    func invocationEnded(source: InvocationSource, reason: InvocationEndReason, duration: Duration)
    func invocationRequestsMainWindow()
    /// Quick tap of the shortcut in hold mode.
    func invocationTapped()
    func invocationStatusChanged(_ status: InvocationStatus)
    /// The dictation chord was held: start recording to type.
    func dictationStarted()
    /// The dictation chord ended; `commit` is false when it was cancelled (another key pressed).
    func dictationEnded(commit: Bool)
    /// Esc was pressed anywhere (only reported while `watchEscape` is on, e.g. in talk mode).
    func escapePressed()
}

public extension InvocationDelegate {
    func dictationStarted() {}
    func dictationEnded(commit _: Bool) {}
    func escapePressed() {}
}

/// Owns every global invocation channel and turns raw input into start/end decisions.
///
/// * Modifier-only chords (⌘⌥) and mouse buttons use a listen-only event tap (needs permission).
/// * Key chords (⌥Space) use a Carbon hot key (no permission needed).
/// * Trackpad gestures are unsupported: macOS has no public API for global gesture listening
///   (DECISIONS.md D-017). The status reports this honestly.
@MainActor
public final class InvocationController {
    public weak var delegate: InvocationDelegate?

    private var settings = InvocationSettings(
        behavior: .hold, holdDelayMs: 150, mouseAction: .focusCursor, mouseButton: .none,
        shortcut: Shortcut(keyCode: nil, keyLabel: nil, modifiers: [.option, .command])
    )
    private var chord = ChordStateMachine(target: [.option, .command], behavior: .hold, holdDelay: .milliseconds(150))
    /// Dictation: its own hold chord (default ⌃⇧), independent of the assistant's.
    private var dictation = DictationSettings(
        cleanup: false, enabled: true, shortcut: Shortcut(keyCode: nil, keyLabel: nil, modifiers: [.control, .shift])
    )
    private var dictationChord = ChordStateMachine(target: [.control, .shift], behavior: .hold, holdDelay: .milliseconds(150))
    private(set) public var isDictating = false
    /// Report Esc presses to the delegate (talk mode turns this on).
    public var watchEscape = false
    private lazy var tap = EventTapMonitor { [weak self] event in self?.handle(event) }
    private lazy var hotKey = CarbonHotKey(
        onPress: { [weak self] in self?.keyChordPressed() },
        onRelease: { [weak self] in self?.keyChordReleased() }
    )
    private var hotKeyState: ChannelState = .disabled
    private var active: (source: InvocationSource, behavior: InvocationBehavior, since: ContinuousClock.Instant)?
    private var suspended = false
    private var permissionPoll: Task<Void, Never>?
    private var axObserver: NSObjectProtocol?
    private(set) public var status = InvocationStatus(
        detail: nil, eventTapPermitted: false, keyboard: .disabled, menuBar: .active, mouse: .disabled, trackpad: .unsupported
    )

    public init() {
        axObserver = DistributedNotificationCenter.default().addObserver(
            forName: NSNotification.Name("com.apple.accessibility.api"), object: nil, queue: .main
        ) { [weak self] _ in
            // The notification fires before the trust bit flips; re-check shortly after.
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .milliseconds(400))
                self?.refresh()
            }
        }
    }

    public var isActive: Bool { active != nil }

    // MARK: Configuration

    public func apply(_ newSettings: InvocationSettings) {
        if active != nil { end(reason: .cancelled) }
        settings = newSettings
        chord = ChordStateMachine(
            target: Set(newSettings.shortcut.modifiers),
            behavior: newSettings.behavior,
            holdDelay: .milliseconds(Int(newSettings.holdDelayMs))
        )
        hotKey.unregister()
        hotKeyState = .disabled
        if let code = newSettings.shortcut.keyCode {
            do {
                try hotKey.register(keyCode: UInt32(code), modifiers: KeyNames.carbonModifiers(newSettings.shortcut.modifiers))
                hotKeyState = .active
            } catch .taken {
                hotKeyState = .conflict
            } catch {
                Log.app.error("hot key registration failed: \(String(describing: error), privacy: .public)")
                hotKeyState = .conflict
            }
        }
        refresh()
    }

    public func applyDictation(_ d: DictationSettings) {
        if isDictating { endDictation(commit: false) }
        dictation = d
        dictationChord = ChordStateMachine(target: Set(d.shortcut.modifiers), behavior: .hold, holdDelay: .milliseconds(150))
        refresh()
    }

    /// Re-evaluate permissions and channel states; start the event tap when it becomes possible.
    public func refresh() {
        let permitted = EventTapMonitor.isPermitted
        let needsTap = settings.shortcut.keyCode == nil || settings.mouseButton != .none || dictation.enabled
        if needsTap && permitted && !tap.isRunning { tap.start() }
        if (!needsTap || !permitted) && tap.isRunning { tap.stop() }
        let tapRunning = tap.isRunning

        let keyboard: ChannelState = settings.shortcut.keyCode == nil
            ? (tapRunning ? .active : .needsPermission)
            : hotKeyState
        let mouse: ChannelState = settings.mouseButton == .none || settings.mouseAction == .disabled
            ? .disabled
            : (tapRunning ? .active : .needsPermission)
        var detail: String?
        if keyboard == .needsPermission {
            detail = "Allow Accessibility so Wisp can notice \(KeyNames.display(settings.shortcut)) in any app."
        } else if keyboard == .conflict {
            detail = "Another app is already using \(KeyNames.display(settings.shortcut))."
        }
        let next = InvocationStatus(
            detail: detail, eventTapPermitted: permitted, keyboard: keyboard, menuBar: .active,
            mouse: mouse, trackpad: .unsupported
        )
        if next != status {
            status = next
            delegate?.invocationStatusChanged(next)
        }
        updatePermissionPolling(waiting: needsTap && !permitted)
    }

    /// Pause global invocation (while recording a new shortcut).
    public func suspend() {
        suspended = true
        if active != nil { end(reason: .cancelled) }
        hotKey.unregister()
    }

    public func resume() {
        suspended = false
        apply(settings)
    }

    // MARK: External triggers

    /// Menu bar / Dock / command palette: always toggle semantics.
    public func toggle(from source: InvocationSource) {
        if active != nil { end(reason: .toggled) } else { start(source: source, behavior: .toggle) }
    }

    /// The assistant was closed by the user (Esc, close button, click-away).
    public func dismissed() {
        guard active != nil else { return }
        chord.dismissed()
        end(reason: .dismissed)
    }

    // MARK: Input

    private func handle(_ event: EventTapMonitor.Event) {
        guard !suspended else { return }
        switch event {
        case .flagsChanged(let flags):
            let held = KeyNames.modifiers(from: flags)
            if dictation.enabled { performDictation(dictationChord.flagsChanged(held)) }
            guard settings.shortcut.keyCode == nil else { return }
            perform(chord.flagsChanged(held), source: .keyboard)
        case .keyDown(let keyCode, let isRepeat):
            guard !isRepeat else { return }
            if watchEscape, keyCode == 53 { delegate?.escapePressed() }
            if dictation.enabled { performDictation(dictationChord.keyDown()) }
            guard settings.shortcut.keyCode == nil else { return }
            perform(chord.keyDown(), source: .keyboard)
        case .otherMouseDown(let button):
            guard button == mouseButtonNumber, settings.mouseAction != .disabled else { return }
            switch settings.mouseAction {
            case .openMainWindow: delegate?.invocationRequestsMainWindow()
            case .toggleAssistant, .focusCursor: toggle(from: .mouse)
            case .disabled: break
            }
        case .otherMouseUp:
            break
        }
    }

    private var mouseButtonNumber: Int64? {
        switch settings.mouseButton {
        case .none: nil
        case .middle: 2
        case .back: 3
        case .forward: 4
        }
    }

    private func perform(_ output: ChordStateMachine.Output, source: InvocationSource) {
        switch output {
        case .none:
            break
        case .scheduleHoldTimer(let token, let delay):
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: delay)
                guard let self else { return }
                self.perform(self.chord.holdTimerFired(token: token), source: source)
            }
        case .activate:
            start(source: source, behavior: settings.behavior)
        case .deactivate(let reason):
            end(reason: reason)
        case .tap:
            delegate?.invocationTapped()
        }
    }

    private func performDictation(_ output: ChordStateMachine.Output) {
        switch output {
        case .scheduleHoldTimer(let token, let delay):
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: delay)
                guard let self else { return }
                self.performDictation(self.dictationChord.holdTimerFired(token: token))
            }
        case .activate:
            // Not while the assistant is listening (and vice versa).
            guard active == nil else { dictationChord.dismissed(); return }
            isDictating = true
            delegate?.dictationStarted()
        case .deactivate(let reason):
            endDictation(commit: reason == .released)
        case .none, .tap:
            break
        }
    }

    private func endDictation(commit: Bool) {
        guard isDictating else { return }
        isDictating = false
        dictationChord.dismissed()
        delegate?.dictationEnded(commit: commit)
    }

    private func keyChordPressed() {
        guard !suspended else { return }
        switch settings.behavior {
        case .hold, .pushToTalk:
            if active == nil { start(source: .keyboard, behavior: settings.behavior) }
        case .toggle:
            toggle(from: .keyboard)
        }
    }

    private func keyChordReleased() {
        guard !suspended, settings.behavior != .toggle, active?.source == .keyboard else { return }
        end(reason: .released)
    }

    private func start(source: InvocationSource, behavior: InvocationBehavior) {
        guard active == nil, !isDictating else { return }
        active = (source, behavior, .now)
        delegate?.invocationStarted(source: source, behavior: behavior)
    }

    private func end(reason: InvocationEndReason) {
        guard let a = active else { return }
        active = nil
        chord.dismissed()
        delegate?.invocationEnded(source: a.source, reason: reason, duration: .now - a.since)
    }

    private func updatePermissionPolling(waiting: Bool) {
        if waiting, permissionPoll == nil {
            permissionPoll = Task { @MainActor [weak self] in
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(2))
                    guard let self, !Task.isCancelled else { return }
                    if EventTapMonitor.isPermitted { self.permissionPoll = nil; self.refresh(); return }
                }
            }
        } else if !waiting {
            permissionPoll?.cancel()
            permissionPoll = nil
        }
    }
}
