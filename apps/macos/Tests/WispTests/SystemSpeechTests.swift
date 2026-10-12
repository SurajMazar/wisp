import AVFoundation
import Foundation
import Testing
@testable import WispPlatform

@Suite struct SystemSpeechTests {
    @Test func listsVoicesAndSpeaksIntoAWavQuickly() async throws {
        let voices = SystemSpeech.voices()
        let english = try #require(voices.first { $0.language.hasPrefix("en") }, "an English voice is installed")
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("wisp-say-\(UUID().uuidString).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let started = ContinuousClock.now
        try await SystemSpeech.synthesize(text: "Click the Export button in the toolbar.", voice: english.id, speed: 1, to: url)
        let elapsed = ContinuousClock.now - started
        let file = try AVAudioFile(forReading: url)
        let seconds = Double(file.length) / file.fileFormat.sampleRate
        print("system voice \(english.name) (\(english.quality)): \(elapsed) for \(String(format: "%.1f", seconds)) s of audio")
        #expect(seconds > 1, "real speech was written")
        #expect(elapsed < .seconds(3), "fast (well under real time), even with other tests running")
        await #expect(throws: SystemSpeech.Failure.self) {
            try await SystemSpeech.synthesize(text: "Hi", voice: "com.example.nope", speed: 1, to: url)
        }
    }
}
