//! Energy-based voice activity detection with an adaptive noise floor (spec §44).
//!
//! Push-to-talk bounds the recording; VAD trims leading/trailing silence and rejects recordings
//! with no real speech so silence is never sent to the recognizer.

pub const SAMPLE_RATE: usize = 16_000;
const FRAME: usize = SAMPLE_RATE * 30 / 1000; // 30 ms

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadConfig {
    /// 0 (least sensitive) – 1 (most sensitive).
    pub sensitivity: f32,
    /// Minimum speech to count as an utterance.
    pub min_speech_ms: u32,
    /// Padding kept around detected speech.
    pub pad_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sensitivity: 0.5,
            min_speech_ms: 250,
            pad_ms: 200,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VadResult {
    pub speech_ms: u32,
    /// Sample range containing speech (with padding), if any.
    pub range: Option<std::ops::Range<usize>>,
}

fn rms(frame: &[i16]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f64 = frame.iter().map(|&s| (s as f64) * (s as f64)).sum();
    ((sum / frame.len() as f64).sqrt() / i16::MAX as f64) as f32
}

pub fn detect(samples: &[i16], cfg: VadConfig) -> VadResult {
    let energies: Vec<f32> = samples.chunks(FRAME).map(rms).collect();
    if energies.is_empty() {
        return VadResult {
            speech_ms: 0,
            range: None,
        };
    }
    // Noise floor: 20th percentile of frame energy.
    let mut sorted = energies.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted[sorted.len() / 5].max(0.0005);
    // Sensitivity maps to a multiplier over the floor and an absolute minimum.
    let s = cfg.sensitivity.clamp(0.0, 1.0);
    let threshold = (floor * (4.0 - 2.5 * s)).max(0.012 - 0.009 * s);
    let speech: Vec<bool> = energies.iter().map(|&e| e > threshold).collect();
    let speech_frames = speech.iter().filter(|&&v| v).count();
    let speech_ms = (speech_frames * 30) as u32;
    if speech_ms < cfg.min_speech_ms {
        return VadResult {
            speech_ms,
            range: None,
        };
    }
    let first = speech.iter().position(|&v| v).unwrap_or(0);
    let last = speech.iter().rposition(|&v| v).unwrap_or(speech.len() - 1);
    let pad = cfg.pad_ms as usize * SAMPLE_RATE / 1000;
    let start = (first * FRAME).saturating_sub(pad);
    let end = ((last + 1) * FRAME + pad).min(samples.len());
    VadResult {
        speech_ms,
        range: Some(start..end),
    }
}

/// Normalized RMS level 0–1 for UI meters.
pub fn level(samples: &[i16]) -> f32 {
    (rms(samples) * 6.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: usize, amp: f32) -> Vec<i16> {
        (0..SAMPLE_RATE * ms / 1000)
            .map(|i| {
                ((i as f32 * 2.0 * std::f32::consts::PI * 220.0 / SAMPLE_RATE as f32).sin()
                    * amp
                    * i16::MAX as f32) as i16
            })
            .collect()
    }

    fn noise(ms: usize, amp: f32) -> Vec<i16> {
        let mut x: u32 = 12345;
        (0..SAMPLE_RATE * ms / 1000)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (((x >> 16) as f32 / 32768.0 - 1.0) * amp * i16::MAX as f32) as i16
            })
            .collect()
    }

    #[test]
    fn rejects_silence_and_background_noise() {
        assert_eq!(
            detect(&vec![0; SAMPLE_RATE], VadConfig::default()).range,
            None
        );
        assert_eq!(
            detect(&noise(2000, 0.004), VadConfig::default()).range,
            None
        );
    }

    #[test]
    fn finds_and_trims_speech() {
        let mut s = noise(600, 0.003);
        let speech_start = s.len();
        s.extend(tone(800, 0.3));
        s.extend(noise(900, 0.003));
        let r = detect(&s, VadConfig::default());
        let range = r.range.expect("speech");
        assert!(r.speech_ms >= 700 && r.speech_ms <= 900, "{}", r.speech_ms);
        assert!(
            range.start <= speech_start && range.start + 4000 > speech_start,
            "{range:?}"
        );
        assert!(range.end < s.len(), "trailing silence trimmed");
    }

    #[test]
    fn ignores_clicks_shorter_than_min_speech() {
        let mut s = noise(1000, 0.003);
        s.extend(tone(90, 0.5));
        s.extend(noise(1000, 0.003));
        assert_eq!(detect(&s, VadConfig::default()).range, None);
    }

    #[test]
    fn sensitivity_catches_quiet_speech() {
        let mut s = noise(500, 0.002);
        s.extend(tone(600, 0.02));
        s.extend(noise(500, 0.002));
        assert!(
            detect(
                &s,
                VadConfig {
                    sensitivity: 0.9,
                    ..Default::default()
                }
            )
            .range
            .is_some()
        );
    }
}
