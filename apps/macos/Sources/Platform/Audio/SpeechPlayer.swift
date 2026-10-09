@preconcurrency import AVFoundation
import Foundation
import WispUtilities

/// Plays synthesized speech chunks in order (spec §48, §85). A newer utterance replaces an older
/// one; `stop()` silences immediately (barge-in).
@MainActor
public final class SpeechPlayer: NSObject, AVAudioPlayerDelegate {
    public var onStart: ((String) -> Void)?
    public var onFinish: ((String) -> Void)?
    public var volume: Float = 1

    private var queue: [(utterance: String, seq: UInt32, url: URL, last: Bool)] = []
    private var player: AVAudioPlayer?
    private var current: String?
    private var lastReceived = false
    private let root: URL

    public init(root: URL) {
        self.root = root.standardizedFileURL.resolvingSymlinksInPath()
    }

    public var isSpeaking: Bool { player?.isPlaying == true || !queue.isEmpty }

    /// Queue a chunk. Paths outside Wisp's speech cache are refused.
    public func enqueue(utterance: String, seq: UInt32, path: String, last: Bool) {
        let url = URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath()
        guard url.path.hasPrefix(root.path + "/"), url.pathExtension == "wav" else {
            Log.app.error("refused speech file outside the cache")
            return
        }
        if utterance != current {
            stop()
            current = utterance
            lastReceived = false
        }
        if last { lastReceived = true }
        // An empty final marker has no audio file; only record that the utterance is complete.
        if FileManager.default.fileExists(atPath: url.path) {
            queue.append((utterance, seq, url, last))
            queue.sort { $0.seq < $1.seq }
        }
        if player?.isPlaying != true { playNext() }
    }

    public func stop() {
        player?.stop()
        player = nil
        queue.removeAll()
        if let c = current { onFinish?(c) }
        current = nil
    }

    private func playNext() {
        guard !queue.isEmpty else {
            if lastReceived, let c = current {
                current = nil
                onFinish?(c)
            }
            return
        }
        let next = queue.removeFirst()
        do {
            let p = try AVAudioPlayer(contentsOf: next.url)
            p.delegate = self
            p.volume = volume
            p.prepareToPlay()
            p.play()
            player = p
            if next.seq == 0 || queue.isEmpty { onStart?(next.utterance) }
        } catch {
            Log.app.error("speech playback failed: \(error.localizedDescription, privacy: .public)")
            playNext()
        }
    }

    nonisolated public func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        Task { @MainActor in self.playNext() }
    }
}
