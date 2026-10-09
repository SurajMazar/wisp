//! Separates a model's reasoning from its answer.
//!
//! Reasoning arrives either in a dedicated field (Ollama `thinking`, OpenAI-compatible
//! `reasoning_content`) or inline as `<think>…</think>` at the start of the text. Both end up as
//! [`Chunk::Reasoning`], with the tags removed, so the UI can fold it away.

use futures::StreamExt;

use crate::{Chunk, ChunkStream};

const OPEN: &str = "<think>";
const CLOSE: &str = "</think>";

#[derive(Debug, Default, PartialEq)]
enum State {
    /// Haven't seen enough text to know whether it opens with `<think>`.
    #[default]
    Start,
    InThink,
    Answer,
}

/// Incremental splitter; tags may be split across chunks.
#[derive(Debug, Default)]
pub struct ThinkSplitter {
    state: State,
    buf: String,
}

/// Length of the longest suffix of `s` that is a proper prefix of `tag`.
fn partial_suffix(s: &str, tag: &str) -> usize {
    (1..tag.len())
        .rev()
        .find(|&n| {
            s.len() >= n && s.is_char_boundary(s.len() - n) && tag.starts_with(&s[s.len() - n..])
        })
        .unwrap_or(0)
}

impl ThinkSplitter {
    pub fn push(&mut self, chunk: Chunk) -> Vec<Chunk> {
        match chunk {
            Chunk::Reasoning(r) => {
                let r = r.replace(OPEN, "").replace(CLOSE, "");
                if r.is_empty() {
                    vec![]
                } else {
                    vec![Chunk::Reasoning(r)]
                }
            }
            Chunk::Text(t) => self.text(&t),
            Chunk::Done(u) => {
                let mut out = self.flush();
                out.push(Chunk::Done(u));
                out
            }
        }
    }

    fn text(&mut self, t: &str) -> Vec<Chunk> {
        let mut out = vec![];
        self.buf.push_str(t);
        loop {
            match self.state {
                State::Answer => {
                    if !self.buf.is_empty() {
                        out.push(Chunk::Text(std::mem::take(&mut self.buf)));
                    }
                    return out;
                }
                State::Start => {
                    let lead = self.buf.trim_start();
                    if let Some(rest) = lead.strip_prefix(OPEN) {
                        self.buf = rest.to_owned();
                        self.state = State::InThink;
                    } else if OPEN.starts_with(lead) {
                        return out; // could still become "<think>"
                    } else {
                        self.state = State::Answer;
                    }
                }
                State::InThink => {
                    if let Some(i) = self.buf.find(CLOSE) {
                        let reasoning = self.buf[..i].to_owned();
                        let rest = self.buf[i + CLOSE.len()..].trim_start().to_owned();
                        if !reasoning.is_empty() {
                            out.push(Chunk::Reasoning(reasoning));
                        }
                        self.buf = rest;
                        self.state = State::Answer;
                    } else {
                        let keep = partial_suffix(&self.buf, CLOSE);
                        let emit: String = self.buf.drain(..self.buf.len() - keep).collect();
                        if !emit.is_empty() {
                            out.push(Chunk::Reasoning(emit));
                        }
                        return out;
                    }
                }
            }
        }
    }

    /// Whatever is still buffered at the end of the stream.
    pub fn flush(&mut self) -> Vec<Chunk> {
        let rest = std::mem::take(&mut self.buf);
        if rest.is_empty() {
            return vec![];
        }
        match self.state {
            State::InThink => vec![Chunk::Reasoning(rest)],
            _ => vec![Chunk::Text(rest)],
        }
    }
}

/// Wrap a provider stream so reasoning always arrives as [`Chunk::Reasoning`].
pub fn separate_reasoning(stream: ChunkStream) -> ChunkStream {
    Box::pin(
        stream
            .scan(ThinkSplitter::default(), |sp, item| {
                let items: Vec<_> = match item {
                    Ok(c) => sp.push(c).into_iter().map(Ok).collect(),
                    Err(e) => vec![Err(e)],
                };
                futures::future::ready(Some(futures::stream::iter(items)))
            })
            .flatten(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(parts: &[&str]) -> (String, String) {
        let mut sp = ThinkSplitter::default();
        let mut out = vec![];
        for p in parts {
            out.extend(sp.push(Chunk::Text((*p).into())));
        }
        out.extend(sp.push(Chunk::Done(Default::default())));
        let (mut r, mut t) = (String::new(), String::new());
        for c in out {
            match c {
                Chunk::Reasoning(x) => r.push_str(&x),
                Chunk::Text(x) => t.push_str(&x),
                Chunk::Done(_) => {}
            }
        }
        (r, t)
    }

    #[test]
    fn splits_inline_think_tags_even_when_split_across_chunks() {
        assert_eq!(
            run(&["<th", "ink>Let me ", "add.</th", "ink>\n\n2 + 2 = 4."]),
            ("Let me add.".into(), "2 + 2 = 4.".into())
        );
    }

    #[test]
    fn leaves_plain_answers_alone() {
        assert_eq!(run(&["<b>", "Hi"]), (String::new(), "<b>Hi".into()));
        assert_eq!(run(&["4"]), (String::new(), "4".into()));
        // A "<think>" later in the answer isn't reasoning.
        assert_eq!(
            run(&["Use <think> tags"]),
            (String::new(), "Use <think> tags".into())
        );
    }

    #[test]
    fn strips_tags_from_dedicated_reasoning() {
        let mut sp = ThinkSplitter::default();
        assert_eq!(sp.push(Chunk::Reasoning("<think>".into())), vec![]);
        assert_eq!(
            sp.push(Chunk::Reasoning("\nOkay".into())),
            vec![Chunk::Reasoning("\nOkay".into())]
        );
    }

    #[test]
    fn unfinished_reasoning_is_flushed_as_reasoning() {
        assert_eq!(
            run(&["<think>still going"]),
            ("still going".into(), String::new())
        );
    }
}
