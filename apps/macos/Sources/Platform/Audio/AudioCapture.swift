@preconcurrency import AVFoundation
import Foundation
import WispUtilities

/// Microphone capture for push-to-talk (spec §39): 16 kHz mono 16-bit PCM in ~100 ms chunks.
/// The microphone is open only between `start` and `stop`.
@MainActor
public final class AudioCapture {
    public var onChunk: ((_ sessionId: String, _ seq: UInt32, _ pcm: Data) -> Void)?
    public var onLevel: ((Double) -> Void)?
    public private(set) var sessionId: String?
    public private(set) var capturedMs: Int = 0

    private let engine = AVAudioEngine()
    private var sink: Sink?

    public init() {}

    public var isRunning: Bool { sessionId != nil }

    /// Thread-safe accumulator used on the audio thread.
    private final class Sink: @unchecked Sendable {
        let converter: AVAudioConverter
        let target: AVAudioFormat
        let lock = NSLock()
        var pending = Data()
        var seq: UInt32 = 0
        var lastLevel: Date = .distantPast
        let deliver: @Sendable (UInt32, Data) -> Void
        let level: @Sendable (Double) -> Void

        init(converter: AVAudioConverter, target: AVAudioFormat, deliver: @escaping @Sendable (UInt32, Data) -> Void, level: @escaping @Sendable (Double) -> Void) {
            self.converter = converter
            self.target = target
            self.deliver = deliver
            self.level = level
        }

        func process(_ buffer: AVAudioPCMBuffer) {
            if let ch = buffer.floatChannelData?[0], buffer.frameLength > 0, Date().timeIntervalSince(lastLevel) > 0.066 {
                var sum: Float = 0
                for i in 0..<Int(buffer.frameLength) { sum += ch[i] * ch[i] }
                let rms = sqrt(sum / Float(buffer.frameLength))
                lastLevel = Date()
                level(Double(min(1, rms * 8)))
            }
            let ratio = target.sampleRate / buffer.format.sampleRate
            let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio + 32)
            guard let out = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: capacity) else { return }
            // The input block runs synchronously inside `convert`; a box makes that explicit.
            final class Once: @unchecked Sendable { var fed = false }
            let once = Once()
            var error: NSError?
            nonisolated(unsafe) let input = buffer
            converter.convert(to: out, error: &error) { _, status in
                if once.fed { status.pointee = .noDataNow; return nil }
                once.fed = true
                status.pointee = .haveData
                return input
            }
            guard error == nil, out.frameLength > 0, let ptr = out.int16ChannelData?[0] else { return }
            let bytes = Data(bytes: ptr, count: Int(out.frameLength) * 2)
            lock.lock()
            pending.append(bytes)
            var ready: [(UInt32, Data)] = []
            while pending.count >= 3200 {
                ready.append((seq, pending.prefix(3200)))
                pending.removeFirst(3200)
                seq += 1
            }
            lock.unlock()
            for (s, d) in ready { deliver(s, d) }
        }

        func flush() -> (UInt32, Data)? {
            lock.lock()
            defer { lock.unlock() }
            guard !pending.isEmpty else { return nil }
            let d = pending
            pending.removeAll()
            seq += 1
            return (seq - 1, d)
        }
    }

    public func start(sessionId: String) throws {
        stop()
        let input = engine.inputNode
        let inFormat = input.outputFormat(forBus: 0)
        guard inFormat.sampleRate > 0, inFormat.channelCount > 0,
              let target = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000, channels: 1, interleaved: true),
              let mono = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: inFormat.sampleRate, channels: 1, interleaved: false),
              let converter = AVAudioConverter(from: mono, to: target) else {
            throw NSError(domain: "Wisp.Audio", code: 1, userInfo: [NSLocalizedDescriptionKey: "No microphone is available."])
        }
        let id = sessionId
        let sink = Sink(converter: converter, target: target, deliver: { [weak self] seq, data in
            Task { @MainActor in
                guard let self, self.sessionId == id else { return }
                self.capturedMs += data.count / 32
                self.onChunk?(id, seq, data)
            }
        }, level: { [weak self] l in
            Task { @MainActor in self?.onLevel?(l) }
        })
        self.sink = sink
        input.installTap(onBus: 0, bufferSize: 2048, format: mono) { buffer, _ in
            sink.process(buffer)
        }
        engine.prepare()
        try engine.start()
        self.sessionId = sessionId
        capturedMs = 0
    }

    /// Stop the microphone and deliver any remaining audio.
    public func stop() {
        guard engine.isRunning || sessionId != nil else { return }
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        if let id = sessionId, let (seq, data) = sink?.flush() {
            capturedMs += data.count / 32
            onChunk?(id, seq, data)
        }
        sessionId = nil
        sink = nil
        onLevel?(0)
    }
}
