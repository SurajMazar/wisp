@preconcurrency import AVFoundation
import Foundation
import WispIPC

/// The voices built into macOS (AVSpeechSynthesizer): near-instant, nothing to download. Speech is
/// written to a WAV file so it plays through the same queue as Wisp's own voice models.
public enum SystemSpeech {
    public static func voices() -> [SystemVoice] {
        AVSpeechSynthesisVoice.speechVoices()
            .map { v in
                SystemVoice(id: v.identifier, language: v.language, name: v.name, quality: quality(v.quality))
            }
            .sorted { ($0.language, rank($0.quality), $0.name) < ($1.language, rank($1.quality), $1.name) }
    }

    private static func quality(_ q: AVSpeechSynthesisVoiceQuality) -> String {
        switch q {
        case .premium: "premium"
        case .enhanced: "enhanced"
        default: "default"
        }
    }

    private static func rank(_ q: String) -> Int { q == "premium" ? 0 : q == "enhanced" ? 1 : 2 }

    public enum Failure: Error, LocalizedError {
        case unknownVoice, nothingWritten, write(String)
        public var errorDescription: String? {
            switch self {
            case .unknownVoice: "That macOS voice isn't installed."
            case .nothingWritten: "macOS didn't produce any speech."
            case .write(let m): "Couldn't save the speech: \(m)"
            }
        }
    }

    /// Speak `text` into a WAV file at `url`. `speed`: 0.5–2 (1 = normal).
    /// Nonisolated: the synthesizer calls back on its own thread (a main-actor closure would trap).
    public nonisolated static func synthesize(text: String, voice: String, speed: Double, to url: URL) async throws {
        // Some macOS versions hand back a different voice for an unknown identifier.
        guard let v = AVSpeechSynthesisVoice(identifier: voice), v.identifier == voice else { throw Failure.unknownVoice }
        let utterance = AVSpeechUtterance(string: text)
        utterance.voice = v
        let rate = AVSpeechUtteranceDefaultSpeechRate * Float(min(max(speed, 0.5), 2))
        utterance.rate = min(max(rate, AVSpeechUtteranceMinimumSpeechRate), AVSpeechUtteranceMaximumSpeechRate)
        let job = WriteJob(url: url)
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
            job.continuation = c
            job.synthesizer.write(utterance, toBufferCallback: job.receive)
        }
    }
}

/// Collects buffers from the synthesizer into a file; finishes on the empty end-of-speech buffer.
private final class WriteJob: @unchecked Sendable {
    let url: URL
    let synthesizer = AVSpeechSynthesizer()
    var continuation: CheckedContinuation<Void, Error>?
    private var file: AVAudioFile?
    private let lock = NSLock()

    init(url: URL) { self.url = url }

    func receive(_ buffer: AVAudioBuffer) {
        lock.lock()
        defer { lock.unlock() }
        guard let pcm = buffer as? AVAudioPCMBuffer else { return }
        if pcm.frameLength == 0 {
            file = nil  // closes the file
            let ok = FileManager.default.fileExists(atPath: url.path)
            continuation?.resume(with: ok ? .success(()) : .failure(SystemSpeech.Failure.nothingWritten))
            continuation = nil
            return
        }
        do {
            if file == nil {
                file = try AVAudioFile(forWriting: url, settings: pcm.format.settings,
                                       commonFormat: pcm.format.commonFormat, interleaved: pcm.format.isInterleaved)
            }
            try file?.write(from: pcm)
        } catch {
            continuation?.resume(throwing: SystemSpeech.Failure.write(error.localizedDescription))
            continuation = nil
        }
    }
}
