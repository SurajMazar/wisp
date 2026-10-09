@preconcurrency import AVFoundation
import Foundation
import Testing
@testable import WispPlatform

@Suite struct AudioCaptureTests {
    /// CoreAudio calls the tap on its own thread. A main-actor tap traps there (crash on first use
    /// of push-to-talk), so it must be callable from any thread.
    @Test func tapRunsOffTheMainThread() async throws {
        let format = try #require(AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1))
        let buffer = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 512))
        buffer.frameLength = 512
        nonisolated(unsafe) let b = buffer
        final class Box: @unchecked Sendable { var frames: AVAudioFrameCount = 0; var main = true }
        let box = Box()
        nonisolated(unsafe) let tap = AudioCapture.tapBlock { buf in
            box.frames = buf.frameLength
            box.main = Thread.isMainThread
        }
        await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in
            DispatchQueue(label: "test.audio").async {
                tap(b, AVAudioTime(hostTime: 0))
                c.resume()
            }
        }
        #expect(box.frames == 512)
        #expect(box.main == false)
    }
}
