import Foundation
import Testing
@testable import WispPlatform

@Suite struct TurnDetectorTests {
    /// Feed levels every 66 ms (the capture's level rate) from `start`.
    private func run(_ d: inout TurnDetector, _ levels: [Double], from start: Date) -> [TurnDetector.Decision] {
        levels.enumerated().map { i, l in d.feed(l, at: start.addingTimeInterval(Double(i) * 0.066)) }
    }

    @Test func aPauseAfterSpeechEndsTheTurn() {
        let t0 = Date()
        var d = TurnDetector(sensitivity: 0.5, startedAt: t0)
        let speech = Array(repeating: 0.3, count: 20)   // ~1.3 s talking
        let briefPause = Array(repeating: 0.01, count: 6) // 0.4 s: a breath, not the end
        let pause = Array(repeating: 0.01, count: 16)     // ~1 s: done
        let out = run(&d, speech + briefPause + speech + pause, from: t0)
        #expect(out.firstIndex(of: .endTurn) == speech.count * 2 + briefPause.count + 14,
                "ends ~0.9 s into the final pause, not at the breath")
        #expect(out.filter { $0 == .endTurn }.count == 1, "decides once")
    }

    @Test func silenceBeforeSpeakingGivesUpAfterTwentySeconds() {
        let t0 = Date()
        var d = TurnDetector(sensitivity: 0.5, startedAt: t0)
        #expect(d.feed(0.01, at: t0.addingTimeInterval(19)) == .keepListening)
        #expect(d.feed(0.01, at: t0.addingTimeInterval(20.5)) == .giveUp)
    }

    @Test func backgroundNoiseBelowTheThresholdIsntSpeech() {
        let t0 = Date()
        var d = TurnDetector(sensitivity: 0.5, startedAt: t0)  // threshold 0.085
        _ = run(&d, Array(repeating: 0.06, count: 60), from: t0)
        #expect(!d.hasHeardSpeech)
        var keen = TurnDetector(sensitivity: 1, startedAt: t0)  // threshold 0.04
        _ = run(&keen, Array(repeating: 0.06, count: 3), from: t0)
        #expect(keen.hasHeardSpeech, "higher sensitivity hears quieter speech")
    }

    @Test func aVeryLongTurnIsCutOff() {
        let t0 = Date()
        var d = TurnDetector(sensitivity: 0.5, startedAt: t0)
        #expect(d.feed(0.3, at: t0) == .keepListening)
        #expect(d.feed(0.3, at: t0.addingTimeInterval(46)) == .endTurn)
    }
}
