//! Text for speech: streaming sentence detection (spec §48) and markdown cleanup.

/// Accumulates streamed text and yields complete sentences as soon as they end.
#[derive(Default)]
pub struct SentenceSplitter {
    buf: String,
}

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
        let mut in_code = false;
        for (i, w) in self.buf.char_indices() {
            if self.buf[i..].starts_with("```") {
                in_code = !in_code;
            }
            if in_code {
                continue;
            }
            let end = i + w.len_utf8();
            let next_ws = b.get(end).is_some_and(|c| c.is_ascii_whitespace());
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
        assert!(s.push("Use version 3.5 of it, e.g. the latest").is_empty());
        let mut tiny = SentenceSplitter::default();
        assert!(tiny.push("Yes. ").is_empty(), "too short to speak alone");
        assert_eq!(
            tiny.push("That is the right button. "),
            vec!["Yes. That is the right button."]
        );
    }

    #[test]
    fn makes_markdown_speakable_and_skips_code() {
        assert_eq!(
            speakable("## Steps\n- Click **Save**\n```\nrm -rf /\n```\nThen `quit`."),
            "Steps Click Save Then quit."
        );
    }
}
