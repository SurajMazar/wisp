//! Memory (spec §67, §96): explicit extraction, and ranking for retrieval. Pure.
//!
//! Wisp only remembers what the user clearly states — "remember that…", "my name is…",
//! "I prefer…", "call me…", "from now on…" — and says so in the conversation. No hidden
//! summarizing of chats into memory.

use wisp_ipc::types::memory::{MemoryCategory, MemoryLevel};

/// The most memories kept (spec §67: don't silently store unlimited information).
pub const MAX_MEMORIES: u32 = 500;
pub const MAX_CONTENT: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub struct Extracted {
    pub content: String,
    pub category: MemoryCategory,
    pub level: MemoryLevel,
}

fn sentence(s: &str) -> String {
    let s = s.trim().trim_end_matches(['.', '!']).trim();
    let mut c = s.chars();
    match c.next() {
        Some(f) => format!("{}{}.", f.to_uppercase(), c.as_str()),
        None => String::new(),
    }
}

/// "prefer tea" → "prefers tea" (the statement is rewritten about "the user").
fn third_person(rest: &str) -> String {
    let rest = rest.trim();
    let (verb, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    let v = match verb {
        "prefer" | "like" | "love" | "use" | "work" | "live" | "want" | "need" | "hate" => {
            format!("{verb}s")
        }
        other => other.to_owned(),
    };
    if tail.is_empty() {
        v
    } else {
        format!("{v} {tail}")
    }
}

/// Things worth remembering in `text`, if the user stated any.
pub fn extract(text: &str) -> Vec<Extracted> {
    let mut out = vec![];
    // One statement per sentence.
    for raw in text.split(['.', '!', '\n', ';']) {
        let t = raw.trim();
        let lower = t.to_lowercase();
        if t.len() < 6
            || t.len() > MAX_CONTENT
            || lower.ends_with('?')
            || lower.starts_with("do you ")
        {
            continue;
        }
        let level = if ["for today", "today only", "for now", "this week"]
            .iter()
            .any(|w| lower.contains(w))
        {
            MemoryLevel::Temporary
        } else {
            MemoryLevel::Persistent
        };
        let after = |prefixes: &[&str]| {
            prefixes.iter().find_map(|p| {
                lower
                    .strip_prefix(p)
                    .map(|_| t[p.len()..].trim().to_owned())
            })
        };
        let hit = if let Some(rest) = after(&[
            "please remember that ",
            "remember that ",
            "please remember ",
            "remember: ",
            "remember ",
            "note that ",
            "keep in mind that ",
            "don't forget that ",
        ]) {
            let cat = if rest.to_lowercase().starts_with("i prefer")
                || rest.to_lowercase().starts_with("i like")
            {
                MemoryCategory::Preference
            } else {
                MemoryCategory::Fact
            };
            Some((sentence(&rest), cat))
        } else if let Some(name) = after(&["my name is ", "call me "]) {
            let name = name.trim_matches(['"', '\'']);
            (!name.is_empty() && name.split_whitespace().count() <= 4).then(|| {
                (
                    format!("The user's name is {}.", name),
                    MemoryCategory::UserProfile,
                )
            })
        } else if lower.starts_with("i prefer ")
            || lower.starts_with("i always ")
            || lower.starts_with("i never ")
            || lower.starts_with("i like ") && lower.contains(" better")
        {
            Some((
                sentence(&format!("The user {}", third_person(&t[2..]))),
                MemoryCategory::Preference,
            ))
        } else if let Some(rest) = after(&["my favorite ", "my favourite "]) {
            Some((
                sentence(&format!("The user's favorite {rest}")),
                MemoryCategory::Preference,
            ))
        } else if let Some(rest) = after(&["from now on, ", "from now on "]) {
            Some((
                sentence(&format!("From now on, {rest}")),
                MemoryCategory::Preference,
            ))
        } else if let Some(rest) = after(&[
            "i work at ",
            "i work as ",
            "i live in ",
            "i'm a ",
            "i am a ",
        ]) {
            let verb = &t[..t.len() - rest.len()];
            Some((
                sentence(&format!(
                    "The user {}{rest}",
                    verb.trim_start_matches(['I', 'i'])
                        .trim_start()
                        .replace("'m", "is")
                        .replace("am", "is")
                )),
                MemoryCategory::UserProfile,
            ))
        } else {
            None
        };
        if let Some((content, category)) = hit
            && content.len() > 4
        {
            out.push(Extracted {
                content,
                category,
                level,
            });
        }
    }
    out
}

/// "forget that I like coffee" → "I like coffee"; "forget my name" → "my name".
pub fn forget_request(text: &str) -> Option<String> {
    let lower = text.trim().to_lowercase();
    for p in [
        "please forget that ",
        "forget that ",
        "please forget ",
        "forget ",
    ] {
        if let Some(rest) = lower.strip_prefix(p) {
            let rest = rest.trim().trim_end_matches(['.', '!']);
            if rest.len() >= 3 && !["it", "this", "that", "everything"].contains(&rest) {
                return Some(rest.to_owned());
            }
        }
    }
    None
}

fn tokens(s: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "that", "this", "are", "was", "you", "your", "user", "user's",
        "what", "when", "how", "have", "has", "from", "into", "about", "does", "can", "should",
        "would", "they", "them",
    ];
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !STOP.contains(w))
        // Light stemming so "prefers" ~ "prefer", "coffees" ~ "coffee".
        .map(|w| w.trim_end_matches("ing").trim_end_matches('s').to_owned())
        .collect()
}

/// Share of the memory's words found in the query (0–1).
pub fn lexical(query: &str, memory: &str) -> f32 {
    let q = tokens(query);
    let m = tokens(memory);
    if q.is_empty() || m.is_empty() {
        return 0.0;
    }
    let hits = m.iter().filter(|w| q.contains(w)).count() as f32;
    (hits / (m.len() as f32).min(6.0)).min(1.0)
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

pub struct Candidate<'a> {
    pub content: &'a str,
    pub category: MemoryCategory,
    pub importance: f32,
    pub embedding: Option<&'a [f32]>,
}

/// Relevance of a memory to a query (0–1): an even blend of semantic similarity and shared
/// keywords. Calibrated on Apple's on-device sentence embeddings, whose cosines are low and
/// compressed (related ≈ 0.25–0.40, unrelated ≈ 0.0–0.29). Profile facts (like the user's name)
/// and standing instructions ("From now on…") apply broadly, so they get a boost.
pub fn score(query: &str, query_embedding: Option<&[f32]>, c: &Candidate<'_>) -> f32 {
    let lex = lexical(query, c.content);
    let base = match (query_embedding, c.embedding) {
        (Some(q), Some(m)) => (((cosine(q, m) - 0.2) / 0.25).clamp(0.0, 1.0) * 0.5) + lex * 0.5,
        _ => lex,
    };
    let broad = if c.category == MemoryCategory::UserProfile {
        0.25
    } else if c.content.to_lowercase().starts_with("from now on") {
        0.3
    } else {
        0.0
    };
    (base + broad + c.importance * 0.1).min(1.0)
}

/// Calibration points measured on this Mac (cosine of Apple NL sentence embeddings).
#[cfg(test)]
const MEASURED: &[(&str, &str, f32, bool)] = &[
    (
        "The user prefers dark roast coffee.",
        "Which coffee should I buy?",
        0.394,
        true,
    ),
    (
        "My dentist appointment is on Friday.",
        "Which coffee should I buy?",
        0.290,
        false,
    ),
    (
        "The user lives in Kathmandu.",
        "What's the weather like where I live?",
        0.247,
        true,
    ),
    (
        "The user's favorite editor is Zed.",
        "Which code editor should I use?",
        0.299,
        true,
    ),
    (
        "The user's favorite editor is Zed.",
        "Book a flight to Delhi",
        0.017,
        false,
    ),
    (
        "From now on, answer in bullet points.",
        "Explain how DNS works",
        0.242,
        true,
    ),
];

/// Minimum score to include a memory in a prompt.
pub const THRESHOLD: f32 = 0.3;

pub fn encode(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn decode(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents(t: &str) -> Vec<String> {
        extract(t).into_iter().map(|e| e.content).collect()
    }

    #[test]
    fn extracts_only_clear_statements() {
        assert_eq!(
            contents("Remember that my dentist is on Fridays."),
            ["My dentist is on Fridays."]
        );
        assert_eq!(
            contents("I prefer dark roast coffee"),
            ["The user prefers dark roast coffee."]
        );
        assert_eq!(
            extract("I prefer dark roast coffee")[0].category,
            MemoryCategory::Preference
        );
        assert_eq!(
            contents("My name is Suraj. What's 2+2?"),
            ["The user's name is Suraj."]
        );
        assert_eq!(
            extract("call me Sam")[0].category,
            MemoryCategory::UserProfile
        );
        assert_eq!(
            contents("From now on, answer in bullet points"),
            ["From now on, answer in bullet points."]
        );
        assert_eq!(
            contents("My favorite editor is Zed"),
            ["The user's favorite editor is Zed."]
        );
        assert_eq!(
            extract("Remember I'm out of office for today")[0].level,
            MemoryLevel::Temporary
        );
        // Questions and ordinary chat are never stored.
        for t in [
            "What's my name?",
            "Do you remember my name?",
            "How do I prefer to work?",
            "The weather is nice",
            "ok",
        ] {
            assert!(extract(t).is_empty(), "{t}");
        }
    }

    #[test]
    fn recognizes_forget_requests() {
        assert_eq!(
            forget_request("Forget that I like coffee.").as_deref(),
            Some("i like coffee")
        );
        assert_eq!(forget_request("forget it"), None);
        assert_eq!(forget_request("please don't forget"), None);
    }

    #[test]
    fn ranks_relevant_memories_above_unrelated_ones() {
        let coffee = Candidate {
            content: "The user prefers dark roast coffee.",
            category: MemoryCategory::Preference,
            importance: 0.5,
            embedding: None,
        };
        let dentist = Candidate {
            content: "My dentist is on Fridays.",
            category: MemoryCategory::Fact,
            importance: 0.5,
            embedding: None,
        };
        let q = "Which coffee should I order?";
        assert!(score(q, None, &coffee) >= THRESHOLD);
        assert!(score(q, None, &dentist) < THRESHOLD);
        // With embeddings, similarity dominates.
        let (qv, near, far) = ([1.0, 0.0, 0.2], [0.95, 0.05, 0.2], [0.0, 1.0, 0.0]);
        let a = Candidate {
            embedding: Some(&near),
            ..dentist
        };
        let b = Candidate {
            content: "x",
            category: MemoryCategory::Fact,
            importance: 0.5,
            embedding: Some(&far),
        };
        assert!(score("unrelated words", Some(&qv), &a) > score("unrelated words", Some(&qv), &b));
    }

    #[test]
    fn calibrated_on_measured_embeddings() {
        // Build vector pairs with the measured cosine and check include/exclude decisions.
        for (memory, query, cos, include) in MEASURED {
            let q = [1.0f32, 0.0];
            let m = [*cos, (1.0 - cos * cos).sqrt()];
            let category = if memory.contains("lives in") {
                MemoryCategory::UserProfile
            } else {
                MemoryCategory::Preference
            };
            let c = Candidate {
                content: memory,
                category,
                importance: 0.5,
                embedding: Some(&m),
            };
            let s = score(query, Some(&q), &c);
            assert_eq!(s >= THRESHOLD, *include, "{memory} | {query}: {s}");
        }
    }

    #[test]
    fn vectors_round_trip() {
        let v = vec![0.25f32, -1.5, 3.0];
        assert_eq!(decode(&encode(&v)), v);
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }
}
