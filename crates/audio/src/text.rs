//! Text for speech: streaming sentence detection (spec §48) and markdown cleanup.

/// Accumulates streamed text and yields complete sentences as soon as they end.
///
/// The first piece is cut shorter — at a clause break, or after a handful of words — because
/// speech takes about as long to synthesize as to play: a short first piece starts playing
/// sooner, and later pieces are synthesized while it plays.
#[derive(Default)]
pub struct SentenceSplitter {
    buf: String,
    emitted: usize,
}

/// Words in the first piece when no punctuation comes sooner.
const FIRST_PIECE_WORDS: usize = 6;

impl SentenceSplitter {
    /// Add streamed text; returns sentences completed by it.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.buf.push_str(delta);
        let mut out = vec![];
        while let Some(end) = self.boundary() {
            let s: String = self.buf.drain(..end).collect();
            let s = s.trim();
            if !s.is_empty() {
                out.push(s.to_owned());
                self.emitted += 1;
            }
        }
        out
    }

    /// Whatever remains when the stream ends.
    pub fn finish(&mut self) -> Option<String> {
        let s = std::mem::take(&mut self.buf);
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_owned())
    }

    /// Index just past a sentence end followed by whitespace; short fragments wait for more.
    fn boundary(&self) -> Option<usize> {
        let b = self.buf.as_bytes();
        let first = self.emitted == 0;
        let mut in_code = false;
        let mut in_marker = false;
        let mut words = 0usize;
        for (i, w) in self.buf.char_indices() {
            if self.buf[i..].starts_with("```") {
                in_code = !in_code;
            }
            if in_code {
                continue;
            }
            // Never cut inside a guidance marker ("[[point: Save]]").
            if self.buf[i..].starts_with("[[") {
                in_marker = true;
            }
            if in_marker {
                if self.buf[..i + w.len_utf8()].ends_with("]]") {
                    in_marker = false;
                }
                continue;
            }
            let end = i + w.len_utf8();
            if w.is_whitespace() && i > 0 && !self.buf[..i].ends_with(char::is_whitespace) {
                words += 1;
                if first && words >= FIRST_PIECE_WORDS {
                    return Some(i);
                }
            }
            let next_ws = b.get(end).is_some_and(|c| c.is_ascii_whitespace());
            if first && words >= 2 && next_ws && matches!(w, ',' | ';' | ':' | '—' | '–') {
                return Some(end);
            }
            let is_end = matches!(w, '.' | '!' | '?' | '…') && next_ws
                || w == '\n' && b.get(end) == Some(&b'\n');
            // Avoid splitting "e.g. " / "3.5" / very short fragments.
            if is_end
                && self.buf[..end].trim().chars().count() >= 12
                && !self.buf[..i].ends_with("e.g")
                && !self.buf[..i].ends_with("i.e")
            {
                return Some(end);
            }
        }
        None
    }
}

/// Turn markdown-ish model output into something sensible to say out loud.
pub fn speakable(s: &str) -> String {
    // Guidance markers ("[[point: Save]]") are for the overlay, not for reading aloud.
    let mut unmarked = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("[[") {
        unmarked.push_str(&rest[..i]);
        rest = rest[i..].find("]]").map_or("", |j| &rest[i + j + 2..]);
    }
    unmarked.push_str(rest);
    let s = unmarked.as_str();
    let mut out = String::new();
    let mut in_code = false;
    for line in s.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue; // never read code aloud
        }
        let l = line
            .trim_start_matches(['#', '>', ' '])
            .trim_start_matches("- ")
            .trim_start_matches("* ");
        out.push_str(l);
        out.push(' ');
    }
    out.replace("**", "")
        .replace('`', "")
        .replace("__", "")
        .replace('*', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_sentences_as_they_complete() {
        let mut s = SentenceSplitter::default();
        assert_eq!(
            s.push("Okay. This is a dialog box. "),
            vec!["Okay. This is a dialog box."]
        );
        assert!(s.push("This dialog asks").is_empty());
        assert_eq!(
            s.push(" whether to delete three files. Click "),
            vec!["This dialog asks whether to delete three files."]
        );
        assert!(s.push("the red").is_empty());
        assert_eq!(
            s.push(" Delete button! Done"),
            vec!["Click the red Delete button!"]
        );
        assert_eq!(s.finish().as_deref(), Some("Done"));
    }

    #[test]
    fn does_not_split_numbers_abbreviations_or_tiny_fragments() {
        let mut s = SentenceSplitter::default();
        assert_eq!(s.push("Here is the version you need. ").len(), 1);
        assert!(s.push("Use version 3.5 of it, e.g. the latest").is_empty());
        let mut tiny = SentenceSplitter::default();
        assert!(tiny.push("Yes. ").is_empty(), "too short to speak alone");
        assert_eq!(
            tiny.push("That is the right button. "),
            vec!["Yes. That is the right button."]
        );
    }

    #[test]
    fn the_first_piece_is_short_so_speech_starts_sooner() {
        // A clause break ends the first piece…
        let mut s = SentenceSplitter::default();
        assert_eq!(
            s.push("To export the file, open the File menu and choose Export. "),
            vec![
                "To export the file,",
                "open the File menu and choose Export."
            ]
        );
        // …later pieces are whole sentences again.
        assert!(s.push("Then pick a folder, name it").is_empty());
        // Without punctuation, it ends after a handful of words.
        let mut long = SentenceSplitter::default();
        assert_eq!(
            long.push("The button you want is the blue one in the top right corner of the window "),
            vec!["The button you want is the"]
        );
        // Short openers aren't cut ("Sure, …"), and markers are never cut.
        let mut short = SentenceSplitter::default();
        assert!(short.push("Sure, click Save").is_empty());
        let mut marker = SentenceSplitter::default();
        assert_eq!(
            marker.push(
                "Click the export button [[point: Export, as PDF]] near the top right of it "
            ),
            vec!["Click the export button [[point: Export, as PDF]] near"]
        );
    }

    #[test]
    fn makes_markdown_speakable_and_skips_code() {
        assert_eq!(
            speakable("## Steps\n- Click **Save**\n```\nrm -rf /\n```\nThen `quit`."),
            "Steps Click Save Then quit."
        );
    }

    #[test]
    fn never_reads_guidance_markers_aloud() {
        assert_eq!(
            speakable("Click Save [[point: Save]] now."),
            speakable("Click Save  now.")
        );
        assert!(!speakable("Open it [[point: 10,20]]").contains("point"));
    }
}
