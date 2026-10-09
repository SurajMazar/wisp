//! Audio pipeline (spec §39–48). Capture and playback live in the macOS host; this crate turns
//! captured PCM into text (VAD + whisper.cpp) and text into speech (Kokoro via ONNX Runtime).

pub mod models;
pub mod stt;
pub mod text;
pub mod tts;
pub mod vad;
