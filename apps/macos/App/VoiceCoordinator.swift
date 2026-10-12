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
    private var settings = VoiceSettings(enabled: true, language: "auto", speakAnswers: .voiceQuestions, speed: 1, sttModel: "whisper-base.en", ttsModel: "kokoro-82m", vadSensitivity: 0.5, voice: "af_heart", volume: 1)
    private var purpose: VoicePurpose = .ask
    private var startedAt = Date.distantPast
    /// When a focus region completed (to attach it if it happened during this hold).
    var lastFocusAt = Date.distantPast
    /// Waits for a drawing the user just finished to be captured (set by the app delegate).
    var settleFocus: (() async -> Void)?
    /// The conversation a spoken question went to; its reply ends "Thinking…" by the pointer.
    private var awaiting: String?
    /// The conversation whose answer is captioned in the island.
    private var captionFor: String?

    init(router: IPCRouter, assistant: AssistantCoordinator) {
        self.router = router
        self.assistant = assistant
        player = SpeechPlayer(root: AppPaths.dataDirectory.appendingPathComponent("cache/tts", isDirectory: true))
        capture.onChunk = { [weak router] session, seq, data in
            router?.sendToCore(IPCEvents.audioChunk, AudioChunk(pcm: data.base64EncodedString(), seq: seq, sessionId: session))
        }
        capture.onLevel = { [weak router, weak assistant] level in
            assistant?.island.set(level: level)
            router?.broadcast(event: IPCEvents.voiceLevel.name, payload: .object(["level": .double(level)]))
        }
        player.onStart = { [weak assistant] _ in assistant?.set(state: .speaking) }
        player.onFinish = { [weak assistant] _ in
            if assistant?.state == .speaking { assistant?.set(state: .idle) }
        }
        router.subscribe(IPCEvents.conversationDelta) { [weak self] d in
            guard let self, d.conversationId == self.captionFor else { return }
            self.assistant.island.appendCaption(d.delta)
        }
        router.subscribe(IPCEvents.agentRunUpdated) { [weak self] run in
            self?.runUpdated(run)
        }
        router.subscribe(IPCEvents.conversationMessageUpdated) { [weak self] message in
            self?.replyUpdated(message)
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

    /// The hold turned out to be a tap: discard the recording without transcribing.
    func cancelHold() {
        guard let session = capture.sessionId, purpose == .ask else { return }
        capture.stop()
        if assistant.state == .listening { assistant.set(state: .idle) }
        Task { _ = try? await router.core.request(method: IPCMethods.voiceCancel.name, params: .object(["sessionId": .string(session)])) }
    }

    /// Stop any speech right away (spec §85: user speaking interrupts playback).
    func interrupt() {
        if player.isSpeaking {
            player.stop()
            Task { _ = try? await router.core.request(method: IPCMethods.voiceStopSpeaking.name, params: .object([:])) }
        }
    }

    // MARK: Dictation

    /// The dictation chord is held: record (the island says "Dictating").
    func dictationStarted() {
        interrupt()
        guard settings.enabled, !capture.isRunning else { return }
        switch MicrophonePermission.status {
        case .enabled:
            begin(.dictate)
            if purpose == .dictate, capture.isRunning { assistant.set(state: .dictating) }
        case .notRequested:
            Task { _ = await MicrophonePermission.request() }
        case .needsAttention:
            break
        }
    }

    /// Released: transcribe and type it where the cursor is. Cancelled (another key): drop it.
    func dictationEnded(commit: Bool) {
        guard let session = capture.sessionId, purpose == .dictate else { return }
        capture.stop()
        guard commit else {
            if assistant.state == .dictating { assistant.set(state: .idle) }
            Task { _ = try? await router.core.request(method: IPCMethods.voiceCancel.name, params: .object(["sessionId": .string(session)])) }
            return
        }
        Task {
            // Let the final chunk reach the core first.
            try? await Task.sleep(for: .milliseconds(60))
            do {
                let result = try await router.core.call(
                    IPCMethods.voiceFinish,
                    VoiceFinishParams(conversationId: nil, dictation: true, includeFocus: false, sessionId: session, transcribeOnly: true),
                    timeout: .seconds(60)
                )
                if assistant.state == .dictating { assistant.set(state: .idle) }
                guard let text = result.transcript?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { return }
                try await TextInserter.insert(text)
            } catch {
                Log.app.error("dictation failed: \(String(describing: error), privacy: .public)")
                assistant.set(state: .error)
                Task { try? await Task.sleep(for: .seconds(1.5)); if self.assistant.state == .error { self.assistant.set(state: .idle) } }
            }
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

    /// The reply to a spoken question finished: leave "Thinking…" once speech has had a chance to
    /// start (or right away if it failed or won't be spoken).
    private func replyUpdated(_ m: Message) {
        guard m.conversationId == awaiting, m.role == .assistant, m.status != .streaming else { return }
        awaiting = nil
        switch m.status {
        case .error:
            assistant.set(state: .error)
            Task { try? await Task.sleep(for: .seconds(2.5)); if self.assistant.state == .error { self.assistant.set(state: .idle) } }
        case .complete where settings.speakAnswers != .never && !m.content.isEmpty:
            Task {
                try? await Task.sleep(for: .seconds(8))
                if [.thinking, .executing].contains(self.assistant.state) { self.assistant.set(state: .idle) }
            }
        default:
            if [.thinking, .executing].contains(assistant.state) { assistant.set(state: .idle) }
        }
    }

    /// A background task started by voice ("Agent, …"): "Working" in the island while it runs; if
    /// it needs an answer, open the panel so the user can reply.
    private func runUpdated(_ r: AgentRun) {
        guard let conversation = r.conversationId, conversation == awaiting else { return }
        switch r.status {
        case .planning, .running, .waitingPermission:
            if assistant.state != .speaking { assistant.set(state: .executing) }
        case .needsInput, .stuck, .paused:
            assistant.set(state: .idle)
            assistant.expand()
        default:
            break
        }
    }

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
        let testOnly = purpose == .test
        capture.stop()
        await settleFocus?()
        let includeFocus = lastFocusAt >= startedAt
        assistant.set(state: .thinking)
        // Let the final chunk reach the core before asking it to finish.
        try? await Task.sleep(for: .milliseconds(60))
        do {
            let result = try await router.core.call(
                IPCMethods.voiceFinish,
                VoiceFinishParams(conversationId: nil, dictation: false, includeFocus: includeFocus, sessionId: session, transcribeOnly: testOnly),
                timeout: .seconds(120)
            )
            if let transcript = result.transcript, let sent = result.sent {
                router.broadcast(event: IPCEvents.voiceAsked.name, payload: try? JSONValue.from(VoiceAsked(conversationId: sent.conversationId, transcript: transcript)))
                // Answer out loud beside the pointer; the panel opens only when the user opens it
                // (or when answers aren't spoken, since there'd be no other way to see it).
                awaiting = sent.conversationId
                captionFor = sent.conversationId
                assistant.island.resetCaption()
                if settings.speakAnswers == .never { assistant.expand() }
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
