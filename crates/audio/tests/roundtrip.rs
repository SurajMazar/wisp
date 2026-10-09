#![allow(clippy::expect_used)] // test harness

//! Real models on this Mac: Kokoro speaks, Whisper transcribes Kokoro's audio back.
//! Skips (with a message) when the models aren't downloaded.

use std::path::PathBuf;

fn models_root() -> PathBuf {
    std::env::var_os("WISP_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").expect("HOME"))
                .join("Library/Application Support/Wisp/models")
        })
}

/// Linear resample 24 kHz → 16 kHz i16 (good enough for recognition).
fn to_16k(audio: &[f32]) -> Vec<i16> {
    let n = audio.len() * 2 / 3;
    (0..n)
        .map(|i| {
            let x = i as f32 * 1.5;
            let i0 = x as usize;
            let t = x - i0 as f32;
            let a = audio.get(i0).copied().unwrap_or(0.0);
            let b = audio.get(i0 + 1).copied().unwrap_or(a);
            ((a + (b - a) * t).clamp(-1.0, 1.0) * i16::MAX as f32) as i16
        })
        .collect()
}

#[test]
fn kokoro_speaks_and_whisper_hears_it() {
    let root = models_root();
    let kokoro = root.join("kokoro-82m");
    let whisper = root.join("whisper-base.en/ggml-base.en.bin");
    if !kokoro.join("model_quantized.onnx").exists()
        || !whisper.exists()
        || wisp_audio::tts::find_espeak().is_none()
    {
        eprintln!(
            "skipping: speech models or espeak-ng not installed under {}",
            root.display()
        );
        return;
    }

    let sentence = "Click the red Delete button to remove the files.";
    let tts = wisp_audio::tts::KokoroTts::load(&kokoro).expect("load kokoro");
    let ps = tts.phonemize(sentence).expect("phonemize");
    assert!(
        ps.contains('ˈ') && ps.ends_with('.'),
        "phonemes with stress and punctuation: {ps}"
    );
    let t0 = std::time::Instant::now();
    let audio = tts
        .synthesize(sentence, "af_heart", 1.0)
        .expect("synthesize");
    let synth_ms = t0.elapsed().as_millis();
    let secs = audio.len() as f32 / wisp_audio::tts::SAMPLE_RATE as f32;
    let peak = audio.iter().fold(0f32, |m, s| m.max(s.abs()));
    eprintln!("kokoro: {secs:.2}s of audio in {synth_ms} ms, peak {peak:.2}");
    assert!((1.5..8.0).contains(&secs), "duration {secs}");
    assert!(peak > 0.05, "audio is not silent");

    let dir = tempfile::tempdir().expect("tmp");
    wisp_audio::tts::write_wav(
        &dir.path().join("t.wav"),
        &audio,
        wisp_audio::tts::SAMPLE_RATE,
    )
    .expect("wav");

    let pcm = to_16k(&audio);
    let vad = wisp_audio::vad::detect(&pcm, Default::default());
    let range = vad.range.expect("VAD finds speech in synthesized audio");
    let stt = wisp_audio::stt::WhisperStt::load(&whisper).expect("load whisper");
    let t0 = std::time::Instant::now();
    let text = stt.transcribe(&pcm[range], "en").expect("transcribe");
    eprintln!("whisper: \"{text}\" in {} ms", t0.elapsed().as_millis());
    let lower = text.to_lowercase();
    assert!(
        lower.contains("delete") && lower.contains("button") && lower.contains("file"),
        "heard: {text}"
    );
}
