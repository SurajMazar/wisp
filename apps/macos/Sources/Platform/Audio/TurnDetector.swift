import Foundation

/// Talk mode's turn-taking (plan Step 6), from the microphone level alone: a turn ends after a
/// pause once the user has spoken; a long silence before any speech gives up; a very long turn is
/// cut off. Pure, so it's tested with synthetic levels.
public struct TurnDetector: Sendable {
    public enum Decision: Equatable, Sendable {
        case keepListening
        /// The user finished speaking: send the turn.
        case endTurn
        /// Nobody spoke: end talk mode.
        case giveUp
    }

    /// Level (0–1) above which it's speech.
    public var speechLevel: Double
    public var endOfTurnPause: TimeInterval = 0.9
    public var giveUpAfter: TimeInterval = 20
    public var longestTurn: TimeInterval = 45

    private var startedAt: Date
    private var heardSpeech = false
    private var quietSince: Date?
    private var decided = false

    /// `sensitivity` 0–1 (Voice settings): higher hears quieter speech.
    public init(sensitivity: Double, startedAt: Date = Date()) {
        speechLevel = 0.13 - 0.09 * min(max(sensitivity, 0), 1)
        self.startedAt = startedAt
    }

    public var hasHeardSpeech: Bool { heardSpeech }

    public mutating func feed(_ level: Double, at now: Date = Date()) -> Decision {
        guard !decided else { return .keepListening }
        if level > speechLevel {
            heardSpeech = true
            quietSince = nil
        } else if heardSpeech {
            if let q = quietSince {
                if now.timeIntervalSince(q) >= endOfTurnPause { return decide(.endTurn) }
            } else {
                quietSince = now
            }
        }
        let elapsed = now.timeIntervalSince(startedAt)
        if !heardSpeech, elapsed > giveUpAfter { return decide(.giveUp) }
        if elapsed > longestTurn { return decide(heardSpeech ? .endTurn : .giveUp) }
        return .keepListening
    }

    private mutating func decide(_ d: Decision) -> Decision {
        decided = true
        return d
    }
}
