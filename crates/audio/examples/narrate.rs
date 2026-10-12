//! Speak lines with Kokoro into WAV files (used to narrate the product demo).
//!
//!   cargo run --release -p wisp-audio --example narrate -- <model dir> <script.tsv> <out dir> [voice]
//!
//! script.tsv: one `id<TAB>text` per line → <out dir>/<id>.wav

#![allow(clippy::expect_used)] // a developer tool

use std::path::Path;

use wisp_audio::tts::{KokoroTts, SAMPLE_RATE, write_wav};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (model, script, out) = (&args[1], &args[2], &args[3]);
    let voice = args.get(4).map_or("af_heart", String::as_str);
    let tts = KokoroTts::load(Path::new(model)).expect("load Kokoro");
    std::fs::create_dir_all(out).expect("out dir");
    for line in std::fs::read_to_string(script).expect("script").lines() {
        let Some((id, text)) = line.split_once('\t') else {
            continue;
        };
        // Sentence by sentence (Kokoro sounds best on short inputs), with a short breath between.
        let mut audio: Vec<f32> = vec![];
        for sentence in text.split_inclusive(['.', '!', '?']) {
            let s = sentence.trim();
            if s.is_empty() {
                continue;
            }
            audio.extend(tts.synthesize(s, voice, 1.0).expect("synthesize"));
            audio.extend(std::iter::repeat_n(0.0, SAMPLE_RATE as usize / 6));
        }
        let path = Path::new(out).join(format!("{id}.wav"));
        write_wav(&path, &audio, SAMPLE_RATE).expect("write");
        println!("{id}\t{:.2}", audio.len() as f32 / SAMPLE_RATE as f32);
    }
}
