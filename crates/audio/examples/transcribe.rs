//! Check narration by ear, without ears: transcribe each WAV with Whisper and compare it with the
//! script, word by word. A mispronounced word usually comes back as a different word.
//!
//!   cargo run --release -p wisp-audio --example transcribe -- <whisper model> <script.tsv> <wav dir>
//!
//! Given a Kokoro model folder instead of a WAV folder, it speaks each line itself first.

#![allow(clippy::expect_used)] // a developer tool

use std::path::Path;

use wisp_audio::{
    stt::WhisperStt,
    tts::{KokoroTts, SAMPLE_RATE},
};

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word-level edit distance, with the script words that weren't heard as written.
fn compare(script: &[String], heard: &[String]) -> (usize, Vec<String>) {
    let (n, m) = (script.len(), heard.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = d[i - 1][j - 1] + usize::from(script[i - 1] != heard[j - 1]);
            d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
        }
    }
    let (mut i, mut j, mut missed) = (n, m, vec![]);
    while i > 0 && j > 0 {
        if d[i][j] == d[i - 1][j - 1] + usize::from(script[i - 1] != heard[j - 1]) {
            if script[i - 1] != heard[j - 1] {
                missed.push(format!("{}→{}", script[i - 1], heard[j - 1]));
            }
            i -= 1;
            j -= 1;
        } else if d[i][j] == d[i - 1][j] + 1 {
            missed.push(format!("{}→∅", script[i - 1]));
            i -= 1;
        } else {
            j -= 1;
        }
    }
    missed.reverse();
    (d[n][m], missed)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stt = WhisperStt::load(Path::new(&args[1])).expect("load Whisper");
    let source = Path::new(&args[3]);
    let kokoro = source
        .join("model.onnx")
        .exists()
        .then(|| KokoroTts::load(source).expect("load Kokoro"));
    let (mut errors, mut total) = (0, 0);
    for line in std::fs::read_to_string(&args[2]).expect("script").lines() {
        let Some((id, text)) = line.split_once('\t') else {
            continue;
        };
        let (src, rate): (Vec<f64>, f64) = if let Some(tts) = &kokoro {
            let audio = tts.synthesize(text, "af_heart", 1.0).expect("synthesize");
            (
                audio.iter().map(|&s| f64::from(s) * 32767.0).collect(),
                f64::from(SAMPLE_RATE),
            )
        } else {
            let mut reader = hound::WavReader::open(source.join(format!("{id}.wav"))).expect("wav");
            let rate = f64::from(reader.spec().sample_rate);
            (
                reader
                    .samples::<i16>()
                    .map(|s| f64::from(s.expect("sample")))
                    .collect(),
                rate,
            )
        };
        // Linear resample to Whisper's 16 kHz.
        let len = (src.len() as f64 * 16_000.0 / rate) as usize;
        let pcm: Vec<i16> = (0..len)
            .map(|k| {
                let x = k as f64 * rate / 16_000.0;
                let (i, f) = (x as usize, x.fract());
                let b = src.get(i + 1).copied().unwrap_or(src[i]);
                (src[i] * (1.0 - f) + b * f) as i16
            })
            .collect();
        let heard = stt.transcribe(&pcm, "en").expect("transcribe");
        let (e, missed) = compare(&words(text), &words(&heard));
        errors += e;
        total += words(text).len();
        println!("{id}  {e} errors  {}", missed.join("  "));
    }
    println!(
        "\nword error rate: {:.1}% ({errors}/{total})",
        100.0 * errors as f64 / total as f64
    );
}
