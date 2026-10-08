# Voice (design — Phase 8)

Mic (AVAudioEngine, host) → 16 kHz PCM frames → VAD (Silero, ONNX Runtime) → STT (whisper.cpp) →
LLM (streaming) → sentence splitter → TTS (Kokoro-82M, ONNX Runtime) → audio queue → speaker (host).
Barge-in: user speech during playback stops playback and switches to listening.
Open risk: Kokoro phonemization without GPL espeak-ng (DECISIONS.md D-009).
