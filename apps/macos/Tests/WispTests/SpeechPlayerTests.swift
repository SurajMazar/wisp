@preconcurrency import AVFoundation
import Foundation
import Testing
@testable import WispPlatform

@MainActor
@Suite struct SpeechPlayerTests {
    private func wav(_ url: URL, seconds: Double) throws {
        let format = try #require(AVAudioFormat(standardFormatWithSampleRate: 24_000, channels: 1))
        let file = try AVAudioFile(forWriting: url, settings: format.settings)
        let frames = AVAudioFrameCount(24_000 * seconds)
        guard frames > 0 else { return }  // a header-only file: zero samples
        let buf = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames))
        buf.frameLength = frames
        try file.write(from: buf)
    }

    /// The bug: the end-of-answer piece was zero-length audio, which never "finishes playing", so
    /// the app stayed in Speaking. Every way an answer can end must report finished.
    @Test(arguments: ["missing marker", "empty audio"])
    func finishesWhateverTheLastPieceIs(ending: String) async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("wisp-tts-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let player = SpeechPlayer(root: root)
        player.volume = 0
        var started = 0
        var finished = 0
        player.onStart = { _ in started += 1 }
        player.onFinish = { _ in finished += 1 }

        try wav(root.appendingPathComponent("u-0.wav"), seconds: 0.2)
        try wav(root.appendingPathComponent("u-1.wav"), seconds: 0.2)
        player.enqueue(utterance: "u", seq: 0, path: root.appendingPathComponent("u-0.wav").path, last: false)
        player.enqueue(utterance: "u", seq: 1, path: root.appendingPathComponent("u-1.wav").path, last: false)
        let end = root.appendingPathComponent("u-2.wav")
        if ending == "empty audio" { try wav(end, seconds: 0) }
        player.enqueue(utterance: "u", seq: 2, path: end.path, last: true)

        for _ in 0..<40 where finished == 0 { try await Task.sleep(for: .milliseconds(50)) }
        #expect(finished == 1, "reports finished (\(ending))")
        #expect(started == 1, "announces speaking once, not per piece")
        #expect(!player.isSpeaking)
    }
}
