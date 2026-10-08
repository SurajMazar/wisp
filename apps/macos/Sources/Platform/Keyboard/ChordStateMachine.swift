import Foundation
import WispIPC

/// Pure state machine for a modifier-only chord (e.g. ⌘⌥) under hold or toggle behavior.
///
/// Rules (DECISIONS.md D-006):
/// * The chord is "down" only when the held modifiers are *exactly* the target set.
/// * Pressing any other key while the chord is down **taints** it: the user is typing an app
///   shortcut such as ⌘⌥I, so Wisp must not activate (or, if already active in hold mode,
///   cancels). Mouse clicks never taint — drawing while holding is the point.
/// * Hold: activates after `holdDelay` of untainted holding; release ends it.
/// * Toggle: a quick untainted press-and-release (≤ `tapMaxDuration`) flips the active state.
public struct ChordStateMachine: Sendable {
    public enum Output: Equatable, Sendable {
        case none
        /// Start a delay timer; call `holdTimerFired(token:)` when it elapses.
        case scheduleHoldTimer(token: Int, after: Duration)
        case activate
        case deactivate(InvocationEndReason)
    }

    public var target: Set<Modifier>
    public var behavior: InvocationBehavior
    public var holdDelay: Duration
    public var tapMaxDuration: Duration = .milliseconds(450)

    public private(set) var isActive = false
    private var chordDown = false
    private var tainted = false
    private var downAt: ContinuousClock.Instant?
    private var timerToken = 0

    public init(target: Set<Modifier>, behavior: InvocationBehavior, holdDelay: Duration) {
        self.target = target
        self.behavior = behavior
        self.holdDelay = holdDelay
    }

    /// Modifier state changed.
    public mutating func flagsChanged(_ held: Set<Modifier>, at now: ContinuousClock.Instant = .now) -> Output {
        let exact = held == target
        if exact && !chordDown {
            chordDown = true
            tainted = false
            downAt = now
            if behavior == .hold || behavior == .pushToTalk {
                timerToken += 1
                return .scheduleHoldTimer(token: timerToken, after: holdDelay)
            }
            return .none
        }
        if !exact && chordDown {
            // Adding a modifier to the chord (e.g. ⌘⌥⇧) is not the chord any more.
            let released = held.isSubset(of: target)
            chordDown = false
            timerToken += 1  // invalidate pending timer
            let heldFor = downAt.map { now - $0 } ?? .zero
            defer { downAt = nil }
            switch behavior {
            case .hold, .pushToTalk:
                if isActive {
                    isActive = false
                    return .deactivate(released && !tainted ? .released : .cancelled)
                }
                return .none
            case .toggle:
                guard released, !tainted, heldFor <= tapMaxDuration else { return .none }
                isActive.toggle()
                return isActive ? .activate : .deactivate(.toggled)
            }
        }
        return .none
    }

    /// A non-modifier key was pressed.
    public mutating func keyDown() -> Output {
        guard chordDown else { return .none }
        tainted = true
        timerToken += 1
        if (behavior == .hold || behavior == .pushToTalk) && isActive {
            isActive = false
            return .deactivate(.cancelled)
        }
        return .none
    }

    public mutating func holdTimerFired(token: Int) -> Output {
        guard token == timerToken, chordDown, !tainted, !isActive else { return .none }
        isActive = true
        return .activate
    }

    /// The assistant was dismissed some other way (Esc, close button).
    public mutating func dismissed() {
        isActive = false
    }
}
