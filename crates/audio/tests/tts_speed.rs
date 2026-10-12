#![allow(clippy::expect_used)] // test harness

//! How long the voice takes to say a typical first sentence (needs the downloaded model):
//!   WISP_KOKORO_DIR=~/Library/Application\ Support/Wisp/models/kokoro-82m \
//!   cargo test --release -p wisp-audio --test tts_speed -- --ignored --nocapture

use std::time::Instant;

use wisp_audio::tts::KokoroTts;

#[test]
#[ignore = "needs the downloaded Kokoro model"]
fn first_sentence_is_quick() {
    let dir = std::env::var("WISP_KOKORO_DIR").expect("WISP_KOKORO_DIR");
    let t = Instant::now();
    let tts = KokoroTts::load(std::path::Path::new(&dir)).expect("load");
    println!("load: {} ms", t.elapsed().as_millis());
    for text in [
        "Click the Export button in the toolbar.",
        "This page explains the wave equation and its solutions in one dimension.",
        "Sure!",
    ] {
        let t = Instant::now();
        let ph = tts.phonemize(text).expect("phonemize");
        let ph_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let audio = tts.synthesize(text, "af_heart", 1.0).expect("synth");
        let ms = t.elapsed().as_millis();
        println!(
            "{ms:>5} ms (phonemes {ph_ms} ms) for {:.1} s of audio · {text} · {ph}",
            audio.len() as f32 / wisp_audio::tts::SAMPLE_RATE as f32
        );
    }
}
