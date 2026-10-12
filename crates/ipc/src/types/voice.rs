//! Voice: push-to-talk capture, transcription and spoken answers (spec §39–49).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::conversations::SendResult;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SpeakAnswers {
    /// Speak answers to spoken questions.
    #[default]
    VoiceQuestions,
    Always,
    Never,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct VoiceSettings {
    /// Hold the shortcut to talk.
    pub enabled: bool,
    pub stt_model: String,
    /// BCP-47 code or "auto" (multilingual models only).
    pub language: String,
    /// 0 (least) – 1 (most sensitive).
    pub vad_sensitivity: f64,
    pub speak_answers: SpeakAnswers,
    /// What speaks: a downloaded voice model (e.g. "kokoro-82m") or "system" (the voices built into
    /// macOS: instant, nothing to download).
    pub tts_model: String,
    /// A voice of `tts_model` (Kokoro: "af_heart"; macOS: a voice identifier).
    pub voice: String,
    /// 0.5–2.0.
    pub speed: f64,
    /// 0–1.
    pub volume: f64,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            stt_model: "whisper-base.en".into(),
            language: "auto".into(),
            vad_sensitivity: 0.5,
            speak_answers: SpeakAnswers::VoiceQuestions,
            // Full precision synthesizes ~2.7× faster than the compressed model (D-042).
            tts_model: "kokoro-82m-full".into(),
            voice: "af_heart".into(),
            speed: 1.0,
            volume: 1.0,
        }
    }
}

impl VoiceSettings {
    pub fn validate(&self) -> Result<(), String> {
        let ident = |s: &str| {
            !s.is_empty()
                && s.len() <= 64
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        };
        if !ident(&self.stt_model) || !ident(&self.voice) || !ident(&self.tts_model) {
            return Err("invalid model or voice name".into());
        }
        if !(self.language == "auto"
            || (2..=8).contains(&self.language.len())
                && self
                    .language
                    .chars()
                    .all(|c| c.is_ascii_alphabetic() || c == '-'))
        {
            return Err("invalid language".into());
        }
        if !(0.0..=1.0).contains(&self.vad_sensitivity)
            || !(0.5..=2.0).contains(&self.speed)
            || !(0.0..=1.0).contains(&self.volume)
        {
            return Err("voice values out of range".into());
        }
        Ok(())
    }
}

/// 16 kHz mono signed 16-bit PCM, base64, ~100 ms per chunk (host → core).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioChunk {
    pub session_id: String,
    pub seq: u32,
    pub pcm: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceFinishParams {
    pub session_id: String,
    /// Ask about the current focus region too.
    #[serde(default)]
    pub include_focus: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// Transcribe only (microphone test) — don't ask anything.
    #[serde(default)]
    pub transcribe_only: bool,
    /// Dictation: transcribe (and clean up if enabled) to be typed, never asked.
    #[serde(default)]
    pub dictation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NoSpeechReason {
    Silence,
    TooShort,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceFinishResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech: Option<NoSpeechReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<SendResult>,
    pub audio_ms: u32,
    pub stt_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionParams {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeakParams {
    pub text: String,
}

/// A synthesized piece of speech ready to play (core → host). `path` is inside Wisp's cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechChunk {
    pub utterance_id: String,
    pub seq: u32,
    pub path: String,
    pub text: String,
    pub last: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechModelInfo {
    pub id: String,
    pub name: String,
    /// "stt" or "tts".
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    pub size_bytes: u64,
    pub installed: bool,
    pub languages: Vec<String>,
    pub license: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStatus {
    pub models: Vec<SpeechModelInfo>,
    pub stt_ready: bool,
    pub tts_ready: bool,
    /// espeak-ng found (needed for Kokoro).
    pub phonemizer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelIdParams {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechModelProgress {
    pub id: String,
    pub completed: u64,
    pub total: u64,
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Host → UI: microphone level 0–1 while listening.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceLevel {
    pub level: f64,
}

/// Host → UI: a spoken question was asked in this conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceAsked {
    pub conversation_id: String,
    pub transcript: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum VoicePurpose {
    Ask,
    Test,
    /// Type what's said where the cursor is.
    Dictate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostVoiceStartParams {
    pub purpose: VoicePurpose,
}

/// The voice engine built into macOS.
pub const SYSTEM_TTS: &str = "system";
/// OpenAI's voices, with the user's OpenAI key (cloud; never in Local Only).
pub const OPENAI_TTS: &str = "openai";
/// Voices OpenAI offers for speech.
pub const OPENAI_VOICES: [&str; 10] = [
    "alloy", "ash", "ballad", "coral", "echo", "fable", "nova", "onyx", "sage", "shimmer",
];

/// A voice installed in macOS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemVoice {
    pub id: String,
    pub name: String,
    /// BCP-47, e.g. "en-US".
    pub language: String,
    /// "default", "enhanced" or "premium" (download more in System Settings → Accessibility →
    /// Spoken Content).
    pub quality: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemVoices {
    pub voices: Vec<SystemVoice>,
}

/// Speak `text` with a macOS voice into a WAV file under the data folder's `cache/tts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechSynthesizeParams {
    pub text: String,
    pub voice: String,
    /// 0.5–2.0 (1 = normal).
    pub speed: f64,
    pub path: String,
}
