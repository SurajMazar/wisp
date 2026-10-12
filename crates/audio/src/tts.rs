//! Text-to-speech with Kokoro-82M via ONNX Runtime (spec §45–48).
//!
//! Phonemes come from espeak-ng, run as a separate process (GPL-3.0 — invoked, never linked;
//! DECISIONS.md D-021). Kokoro's tokenizer maps IPA symbols to ids; each voice is a style table
//! indexed by token count.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

use ort::{session::Session, value::Tensor};

pub const SAMPLE_RATE: u32 = 24_000;
const MAX_TOKENS: usize = 510;
const STYLE_DIM: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("espeak-ng isn't installed (needed for Kokoro voices)")]
    NoPhonemizer,
    #[error("the voice model couldn't be loaded: {0}")]
    Load(String),
    #[error("unknown voice {0}")]
    UnknownVoice(String),
    #[error("speech synthesis failed: {0}")]
    Run(String),
}

pub fn find_espeak() -> Option<PathBuf> {
    [
        "/opt/homebrew/bin/espeak-ng",
        "/usr/local/bin/espeak-ng",
        "/usr/bin/espeak-ng",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

/// Performance cores on Apple Silicon (falls back to 4).
fn performance_cores() -> usize {
    std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.perflevel0.physicalcpu"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .filter(|n: &usize| (1..=16).contains(n))
        .unwrap_or(4)
}

pub struct KokoroTts {
    session: Mutex<Session>,
    vocab: HashMap<char, i64>,
    dir: PathBuf,
    voices: Mutex<HashMap<String, Vec<f32>>>,
    espeak: PathBuf,
}

impl KokoroTts {
    pub fn load(dir: &Path) -> Result<Self, TtsError> {
        let espeak = find_espeak().ok_or(TtsError::NoPhonemizer)?;
        let tok: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join("tokenizer.json"))
                .map_err(|e| TtsError::Load(e.to_string()))?,
        )
        .map_err(|e| TtsError::Load(e.to_string()))?;
        let vocab: HashMap<char, i64> = tok
            .pointer("/model/vocab")
            .and_then(|v| v.as_object())
            .ok_or_else(|| TtsError::Load("tokenizer has no vocab".into()))?
            .iter()
            .filter_map(|(k, v)| Some((k.chars().next()?, v.as_i64()?)))
            .collect();
        let load = |e: &dyn std::fmt::Display| TtsError::Load(e.to_string());
        let builder = Session::builder().map_err(|e| load(&e))?;
        // One thread per performance core: measured ~20% faster than 4 on an M1 Pro (6 P-cores);
        // more threads (efficiency cores) don't help, and Core ML was slower for this model.
        let mut builder = builder
            .with_intra_threads(performance_cores())
            .map_err(|e| load(&e))?;
        let session = builder
            .commit_from_file(dir.join("model_quantized.onnx"))
            .map_err(|e| load(&e))?;
        Ok(Self {
            session: Mutex::new(session),
            vocab,
            dir: dir.to_owned(),
            voices: Mutex::new(HashMap::new()),
            espeak,
        })
    }

    fn voice(&self, name: &str) -> Result<Vec<f32>, TtsError> {
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(TtsError::UnknownVoice(name.into()));
        }
        if let Some(v) = self.voices.lock().ok().and_then(|m| m.get(name).cloned()) {
            return Ok(v);
        }
        let bytes = std::fs::read(self.dir.join("voices").join(format!("{name}.bin")))
            .map_err(|_| TtsError::UnknownVoice(name.into()))?;
        let v: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        if !v.len().is_multiple_of(STYLE_DIM) || v.is_empty() {
            return Err(TtsError::Load(format!(
                "voice {name} has an unexpected size"
            )));
        }
        if let Ok(mut m) = self.voices.lock() {
            m.insert(name.into(), v.clone());
        }
        Ok(v)
    }

    /// IPA phonemes for `text` via espeak-ng, keeping punctuation for prosody.
    pub fn phonemize(&self, text: &str) -> Result<String, TtsError> {
        let mut out = String::new();
        // Phonemize clause by clause and re-insert the punctuation espeak would drop.
        let mut clause = String::new();
        let flush = |clause: &mut String, out: &mut String| -> Result<(), TtsError> {
            let c = clause.trim();
            if !c.is_empty() {
                let mut child = Command::new(&self.espeak)
                    .args(["-q", "--ipa", "-v", "en-us", "--stdin"])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| TtsError::Run(e.to_string()))?;
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    stdin
                        .write_all(c.as_bytes())
                        .map_err(|e| TtsError::Run(e.to_string()))?;
                }
                let o = child
                    .wait_with_output()
                    .map_err(|e| TtsError::Run(e.to_string()))?;
                let ps = String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&ps);
            }
            clause.clear();
            Ok(())
        };
        for ch in text.chars() {
            if matches!(ch, ',' | '.' | '!' | '?' | ';' | ':' | '—' | '…') {
                flush(&mut clause, &mut out)?;
                out.push(ch);
            } else {
                clause.push(ch);
            }
        }
        flush(&mut clause, &mut out)?;
        Ok(normalize(&out))
    }

    pub fn tokenize(&self, phonemes: &str) -> Vec<i64> {
        phonemes
            .chars()
            .filter_map(|c| self.vocab.get(&c).copied())
            .collect()
    }

    /// Synthesize mono f32 audio at 24 kHz. Long text is split to fit the model's context.
    pub fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<f32>, TtsError> {
        let style_table = self.voice(voice)?;
        let ps = self.phonemize(text)?;
        let ids = self.tokenize(&ps);
        let mut audio = vec![];
        for chunk in ids.chunks(MAX_TOKENS - 2) {
            let n = chunk.len();
            let row = n.min(style_table.len() / STYLE_DIM - 1);
            let style = style_table[row * STYLE_DIM..(row + 1) * STYLE_DIM].to_vec();
            let mut input = Vec::with_capacity(n + 2);
            input.push(0);
            input.extend_from_slice(chunk);
            input.push(0);
            let t_ids = Tensor::from_array(([1usize, n + 2], input))
                .map_err(|e| TtsError::Run(e.to_string()))?;
            let t_style = Tensor::from_array(([1usize, STYLE_DIM], style))
                .map_err(|e| TtsError::Run(e.to_string()))?;
            let t_speed = Tensor::from_array(([1usize], vec![speed.clamp(0.5, 2.0)]))
                .map_err(|e| TtsError::Run(e.to_string()))?;
            let mut session = self
                .session
                .lock()
                .map_err(|_| TtsError::Run("session lock".into()))?;
            let outputs = session
                .run(ort::inputs!["input_ids" => t_ids, "style" => t_style, "speed" => t_speed])
                .map_err(|e| TtsError::Run(e.to_string()))?;
            let (_, data) = outputs["waveform"]
                .try_extract_tensor::<f32>()
                .map_err(|e| TtsError::Run(e.to_string()))?;
            audio.extend_from_slice(data);
        }
        Ok(audio)
    }
}

/// espeak IPA → Kokoro's phoneme set (ported from Kokoro's reference phonemizer).
pub fn normalize(ps: &str) -> String {
    ps.replace("kəkˈoːɹoʊ", "kˈoʊkəɹoʊ")
        .replace('ʲ', "j")
        .replace('r', "ɹ")
        .replace('x', "k")
        .replace('ɬ', "l")
        .trim()
        .to_owned()
}

/// Write mono f32 samples as a 16-bit WAV.
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), TtsError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| TtsError::Run(e.to_string()))?;
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .map_err(|e| TtsError::Run(e.to_string()))?;
    }
    w.finalize().map_err(|e| TtsError::Run(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_espeak_symbols() {
        assert_eq!(normalize(" ɹˈɛd r x "), "ɹˈɛd ɹ k");
    }
}
