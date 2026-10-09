import AppKit
import WispIPC
import WispPlatform
import WispUI
import WispUtilities

/// Push-to-talk (spec §39, §48, §85): hold the shortcut → listen; release → transcribe and ask
/// (with whatever you drew while holding); the answer is spoken while the buddy shows it.
@MainActor
final class VoiceCoordinator {
    private let router: IPCRouter
    private let assistant: AssistantCoordinator
    private let capture = AudioCapture()
    private let player: SpeechPlayer
    private var settings = VoiceSettings(enabled: true, language: "auto", speakAnswers: .voiceQuestions, speed: 1, sttModel: "whisper-base.en", vadSensitivity: 0.5, voice: "af_heart", volume: 1)
    private var purpose: VoicePurpose = .ask
    private var startedAt = Date.distantPast
    /// When a focus region completed (to attach it if it happened during this hold).
    var lastFocusAt = Date.distantPast

    init(router: IPCRouter, assistant: AssistantCoordinator) {
        self.router = router
        self.assistant = assistant
        player = SpeechPlayer(root: AppPaths.dataDirectory.appendingPathComponent("cache/tts", isDirectory: true))
        capture.onChunk = { [weak router] session, seq, data in
            router?.sendToCore(IPCEvents.audioChunk, AudioChunk(pcm: data.base64EncodedString(), seq: seq, sessionId: session))
        }
        capture.onLevel = { [weak router] level in
            router?.broadcast(event: IPCEvents.voiceLevel.name, payload: .object(["level": .double(level)]))
        }
        player.onStart = { [weak assistant] _ in assistant?.set(state: .speaking) }
        player.onFinish = { [weak assistant] _ in
            if assistant?.state == .speaking { assistant?.set(state: .idle) }
        }
        router.subscribe(IPCEvents.voiceSpeechChunk) { [weak self] chunk in
            self?.player.enqueue(utterance: chunk.utteranceId, seq: chunk.seq, path: chunk.path, last: chunk.last)
        }
    }

    func apply(_ s: VoiceSettings) {
        settings = s
        player.volume = Float(s.volume)
    }

    var isListening: Bool { capture.isRunning }

    // MARK: Push-to-talk

    func holdStarted() {
        interrupt()
        guard settings.enabled else { return }
        switch MicrophonePermission.status {
        case .enabled:
            begin(.ask)
        case .notRequested:
            Task { _ = await MicrophonePermission.request() }  // ask once; next hold will listen
        case .needsAttention:
            break
        }
    }

    func holdEnded() {
        guard capture.isRunning, purpose == .ask else { return }
        Task { _ = try? await self.finish() }
    }

    /// Stop any speech right away (spec §85: user speaking interrupts playback).
    func interrupt() {
        if player.isSpeaking {
            player.stop()
            Task { _ = try? await router.core.request(method: IPCMethods.voiceStopSpeaking.name, params: .object([:])) }
        }
    }

    // MARK: Panel / tests

    func start(_ p: VoicePurpose) throws {
        interrupt()
        guard MicrophonePermission.status == .enabled else {
            throw IpcError(code: .permissionDenied, details: nil, message: "Wisp needs microphone access to hear you.")
        }
        begin(p)
        if !capture.isRunning {
            throw IpcError(code: .unavailable, details: nil, message: "The microphone couldn't start.")
        }
    }

    func stop() async throws -> VoiceFinishResult {
        guard capture.isRunning else {
            return VoiceFinishResult(audioMs: 0, noSpeech: .silence, sent: nil, sttMs: 0, transcript: nil)
        }
        return try await finish()
    }

    // MARK: Internals

    private func begin(_ p: VoicePurpose) {
        let id = UUID().uuidString.lowercased()
        do {
            try capture.start(sessionId: id)
            purpose = p
            startedAt = Date()
            assistant.set(state: .listening)
        } catch {
            Log.app.error("microphone failed: \(error.localizedDescription, privacy: .public)")
            assistant.set(state: .error)
        }
    }

    @discardableResult
    private func finish() async throws -> VoiceFinishResult {
        guard let session = capture.sessionId else {
            return VoiceFinishResult(audioMs: 0, noSpeech: .silence, sent: nil, sttMs: 0, transcript: nil)
        }
        let includeFocus = lastFocusAt >= startedAt
        let testOnly = purpose == .test
        capture.stop()
        assistant.set(state: .thinking)
        // Let the final chunk reach the core before asking it to finish.
        try? await Task.sleep(for: .milliseconds(60))
        do {
            let result = try await router.core.call(
                IPCMethods.voiceFinish,
                VoiceFinishParams(conversationId: nil, includeFocus: includeFocus, sessionId: session, transcribeOnly: testOnly),
                timeout: .seconds(120)
            )
            if let transcript = result.transcript, let sent = result.sent {
                router.broadcast(event: IPCEvents.voiceAsked.name, payload: try? JSONValue.from(VoiceAsked(conversationId: sent.conversationId, transcript: transcript)))
                assistant.expand()
            } else if result.transcript == nil {
                assistant.set(state: .idle)
            }
            if testOnly { assistant.set(state: .idle) }
            return result
        } catch {
            Log.app.error("voice finish failed: \(String(describing: error), privacy: .public)")
            assistant.set(state: .error)
            Task { try? await Task.sleep(for: .seconds(1.2)); if self.assistant.state == .error { self.assistant.set(state: .idle) } }
            throw error
        }
    }
}
