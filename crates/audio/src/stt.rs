//! Speech-to-text with whisper.cpp (spec §40–42). Runs on Metal; blocking — call from a
//! blocking thread. whisper.cpp's logging is routed into `tracing` so it never reaches stdout
//! (which carries IPC frames).

use std::{path::Path, sync::Once};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

static LOGGING: Once = Once::new();

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("the speech model couldn't be loaded: {0}")]
    Load(String),
    #[error("transcription failed: {0}")]
    Run(String),
}

pub struct WhisperStt {
    ctx: WhisperContext,
    english_only: bool,
}

impl WhisperStt {
    pub fn load(model: &Path) -> Result<Self, SttError> {
        LOGGING.call_once(whisper_rs::install_logging_hooks);
        let path = model
            .to_str()
            .ok_or_else(|| SttError::Load("non-UTF-8 path".into()))?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| SttError::Load(e.to_string()))?;
        let english_only = model
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".en."));
        Ok(Self { ctx, english_only })
    }

    /// Transcribe 16 kHz mono PCM. `language`: BCP-47 code or "auto".
    pub fn transcribe(&self, pcm: &[i16], language: &str) -> Result<String, SttError> {
        let audio: Vec<f32> = pcm.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| SttError::Run(e.to_string()))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let lang = if self.english_only {
            Some("en")
        } else if language == "auto" {
            None
        } else {
            Some(language)
        };
        params.set_language(lang);
        params.set_n_threads(
            std::thread::available_parallelism()
                .map(|n| n.get().min(8) as i32)
                .unwrap_or(4),
        );
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        state
            .full(params, &audio)
            .map_err(|e| SttError::Run(e.to_string()))?;
        let mut text = String::new();
        for seg in state.as_iter() {
            text.push_str(
                &seg.to_str_lossy()
                    .map_err(|e| SttError::Run(e.to_string()))?,
            );
        }
        Ok(clean(&text))
    }
}

/// Remove whisper's non-speech annotations like "[BLANK_AUDIO]" or "(music)".
pub fn clean(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn strips_annotations() {
        assert_eq!(
            super::clean(" [BLANK_AUDIO] What is this (music) button? "),
            "What is this button?"
        );
    }
}
