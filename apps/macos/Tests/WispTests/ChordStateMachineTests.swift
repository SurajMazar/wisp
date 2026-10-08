import Carbon.HIToolbox
import Foundation
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite struct ChordStateMachineTests {
    let cmdOpt: Set<Modifier> = [.command, .option]
    let t0 = ContinuousClock.now

    private func fireTimer(_ m: inout ChordStateMachine, _ out: ChordStateMachine.Output) -> ChordStateMachine.Output {
        guard case .scheduleHoldTimer(let token, _) = out else { return .none }
        return m.holdTimerFired(token: token)
    }

    @Test func holdActivatesAfterDelayAndReleaseEnds() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .hold, holdDelay: .milliseconds(150))
        #expect(m.flagsChanged([.command], at: t0) == .none)
        let sched = m.flagsChanged(cmdOpt, at: t0)
        #expect(fireTimer(&m, sched) == .activate)
        #expect(m.isActive)
        #expect(m.flagsChanged([.command], at: t0 + .seconds(2)) == .deactivate(.released))
        #expect(!m.isActive)
    }

    @Test func appShortcutDuringChordNeverActivates() {
        // ⌘⌥I (browser dev tools) must not summon Wisp.
        var m = ChordStateMachine(target: cmdOpt, behavior: .hold, holdDelay: .milliseconds(150))
        let sched = m.flagsChanged(cmdOpt, at: t0)
        #expect(m.keyDown() == .none)
        #expect(fireTimer(&m, sched) == .none, "stale timer after taint")
        #expect(m.flagsChanged([], at: t0 + .milliseconds(300)) == .none)
        #expect(!m.isActive)
    }

    @Test func keyPressWhileActiveCancelsHold() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .hold, holdDelay: .milliseconds(150))
        let sched = m.flagsChanged(cmdOpt, at: t0)
        #expect(fireTimer(&m, sched) == .activate)
        #expect(m.keyDown() == .deactivate(.cancelled))
    }

    @Test func releasingBeforeDelayDoesNothing() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .hold, holdDelay: .milliseconds(150))
        let sched = m.flagsChanged(cmdOpt, at: t0)
        #expect(m.flagsChanged([], at: t0 + .milliseconds(50)) == .none)
        #expect(fireTimer(&m, sched) == .none)
    }

    @Test func supersetChordIsNotTheChord() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .hold, holdDelay: .milliseconds(150))
        let sched = m.flagsChanged(cmdOpt, at: t0)
        #expect(fireTimer(&m, sched) == .activate)
        // Adding ⇧ while active cancels rather than "releases".
        #expect(m.flagsChanged([.command, .option, .shift], at: t0 + .seconds(1)) == .deactivate(.cancelled))
        // ⌘⌥⇧ pressed from scratch never starts.
        var n = ChordStateMachine(target: cmdOpt, behavior: .hold, holdDelay: .milliseconds(150))
        #expect(n.flagsChanged([.command, .option, .shift], at: t0) == .none)
    }

    @Test func toggleTapsFlipState() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .toggle, holdDelay: .milliseconds(150))
        #expect(m.flagsChanged(cmdOpt, at: t0) == .none)
        #expect(m.flagsChanged([], at: t0 + .milliseconds(120)) == .activate)
        #expect(m.flagsChanged(cmdOpt, at: t0 + .seconds(3)) == .none)
        #expect(m.flagsChanged([], at: t0 + .seconds(3) + .milliseconds(100)) == .deactivate(.toggled))
    }

    @Test func toggleIgnoresLongHoldsAndTaintedTaps() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .toggle, holdDelay: .milliseconds(150))
        _ = m.flagsChanged(cmdOpt, at: t0)
        #expect(m.flagsChanged([], at: t0 + .seconds(2)) == .none, "long hold isn't a tap")
        _ = m.flagsChanged(cmdOpt, at: t0 + .seconds(5))
        _ = m.keyDown()
        #expect(m.flagsChanged([], at: t0 + .seconds(5) + .milliseconds(100)) == .none, "⌘⌥-key isn't a tap")
        #expect(!m.isActive)
    }

    @Test func dismissResetsActiveState() {
        var m = ChordStateMachine(target: cmdOpt, behavior: .toggle, holdDelay: .milliseconds(150))
        _ = m.flagsChanged(cmdOpt, at: t0)
        #expect(m.flagsChanged([], at: t0 + .milliseconds(100)) == .activate)
        m.dismissed()
        _ = m.flagsChanged(cmdOpt, at: t0 + .seconds(1))
        #expect(m.flagsChanged([], at: t0 + .seconds(1) + .milliseconds(100)) == .activate)
    }
}

@Suite struct ShortcutConflictTests {
    private func sc(_ mods: [Modifier], _ code: Int?) -> Shortcut {
        Shortcut(keyCode: code, keyLabel: nil, modifiers: mods)
    }

    @Test func spotlightIsBlockingByDefault() {
        let c = ShortcutConflicts.check(sc([.command], kVK_Space), systemOverrides: [:])
        #expect(c.contains { $0.severity == .blocking && $0.with.contains("Spotlight") })
    }

    @Test func disabledSystemShortcutIsNotAConflict() {
        let overrides: [String: Any] = ["64": ["enabled": false]]
        #expect(ShortcutConflicts.check(sc([.command], kVK_Space), systemOverrides: overrides).isEmpty)
    }

    @Test func remappedSystemShortcutIsDetected() {
        // User moved Spotlight to ⌥Space (option = 524288).
        let overrides: [String: Any] = ["64": ["enabled": true, "value": ["parameters": [32, kVK_Space, 524_288], "type": "standard"]]]
        #expect(ShortcutConflicts.check(sc([.option], kVK_Space), systemOverrides: overrides).contains { $0.severity == .blocking })
        #expect(ShortcutConflicts.check(sc([.command], kVK_Space), systemOverrides: overrides).isEmpty)
    }

    @Test func commonAppShortcutsWarn() {
        let c = ShortcutConflicts.check(sc([.command], kVK_ANSI_C), systemOverrides: [:])
        #expect(c.count == 1 && c[0].severity == .warning)
    }

    @Test func defaultChordAndSuggestedAlternativesAreClean() {
        #expect(ShortcutConflicts.check(sc([.option, .command], nil), systemOverrides: [:]).isEmpty)
        #expect(ShortcutConflicts.check(sc([.option], kVK_Space), systemOverrides: [:]).isEmpty)
        #expect(!ShortcutConflicts.check(sc([.command, .shift], kVK_Space), systemOverrides: [:]).contains { $0.severity == .blocking })
    }

    @Test func displayUsesCanonicalOrder() {
        #expect(KeyNames.display(sc([.command, .option], nil)) == "⌥⌘")
        #expect(KeyNames.display(Shortcut(keyCode: kVK_Space, keyLabel: "Space", modifiers: [.control, .option])) == "⌃⌥Space")
    }
}
