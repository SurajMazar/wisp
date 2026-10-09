//! Context builder (spec §51, §96–97): decides what the model sees for one question, within a
//! token budget sized for small local models. Older turns are condensed instead of dropped, and
//! the screen is described compactly instead of dumping everything.

use wisp_ai::{ChatMessage, Role};
use wisp_ipc::types::conversations::{Message, MessageRole};

/// Rough tokens for a piece of text (≈4 characters per token, plus per-message overhead).
pub fn tokens(s: &str) -> u32 {
    (s.len() / 4) as u32 + 4
}

pub struct History {
    /// Most recent turns, verbatim, oldest first.
    pub messages: Vec<ChatMessage>,
    /// Condensed older turns, to append to the system prompt.
    pub summary: Option<String>,
    pub included: u32,
    pub summarized: u32,
}

fn excerpt(s: &str, max: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max).collect::<String>().trim_end())
    }
}

fn turn_text(m: &Message) -> String {
    match (&m.focus, m.role) {
        (Some(f), MessageRole::User) => {
            format!("{}\n[They were pointing at: {}]", m.content, f.label)
        }
        _ => m.content.clone(),
    }
}

/// Fit finished `history` (oldest first) into `budget` tokens: newest turns verbatim, older ones
/// condensed to one line each (most recent first), anything beyond that dropped.
pub fn fit_history(history: &[Message], budget: u32) -> History {
    let usable: Vec<&Message> = history
        .iter()
        .filter(|m| !m.content.trim().is_empty())
        .collect();
    let mut used = 0u32;
    let mut keep_from = usable.len();
    for (i, m) in usable.iter().enumerate().rev() {
        let t = tokens(&turn_text(m));
        if used + t > budget * 3 / 4 {
            break;
        }
        used += t;
        keep_from = i;
    }
    // A verbatim window should start with the user's turn.
    while keep_from < usable.len() && usable[keep_from].role != MessageRole::User {
        keep_from += 1;
    }
    let messages: Vec<ChatMessage> = usable[keep_from..]
        .iter()
        .map(|m| {
            ChatMessage::text(
                if m.role == MessageRole::User {
                    Role::User
                } else {
                    Role::Assistant
                },
                turn_text(m),
            )
        })
        .collect();

    let older = &usable[..keep_from];
    let mut lines: Vec<String> = vec![];
    let mut room = budget.saturating_sub(used) as usize * 4;
    for m in older.iter().rev() {
        let line = match m.role {
            MessageRole::User => format!("- They asked: {}", excerpt(&m.content, 140)),
            MessageRole::Assistant => format!("- You answered: {}", excerpt(&m.content, 180)),
        };
        if line.len() + 1 > room {
            break;
        }
        room -= line.len() + 1;
        lines.push(line);
    }
    lines.reverse();
    let summarized = lines.len() as u32;
    History {
        included: messages.len() as u32,
        messages,
        summary: (!lines.is_empty()).then(|| {
            format!(
                "Earlier in this conversation (condensed):\n{}",
                lines.join("\n")
            )
        }),
        summarized,
    }
}

/// How a model's context is split between the conversation and the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub history: u32,
    pub screen_chars: usize,
}

/// Split `context_length` after reserving room for the answer and the system prompt. Long prompts
/// make local models slow, so history is capped even when the window is large.
pub fn budget(context_length: u32, max_tokens: u32, has_screen: bool) -> Budget {
    let room = context_length
        .saturating_sub(max_tokens.min(context_length / 2) + 700)
        .max(800);
    let screen = if has_screen {
        (room * 2 / 5).min(1500)
    } else {
        0
    };
    Budget {
        history: (room - screen).min(3000),
        screen_chars: screen as usize * 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_ipc::types::conversations::MessageStatus;

    fn m(role: MessageRole, text: &str) -> Message {
        Message {
            id: wisp_common::ids::new_id(),
            conversation_id: "c".into(),
            role,
            content: text.into(),
            status: MessageStatus::Complete,
            model: None,
            model_name: None,
            locality: None,
            focus: None,
            error: None,
            error_details: None,
            latency_ms: None,
            reasoning: None,
            reasoning_ms: None,
            context: None,
            stage: None,
            created_at_ms: 0,
        }
    }

    fn convo(turns: usize, words: usize) -> Vec<Message> {
        (0..turns)
            .flat_map(|i| {
                [
                    m(
                        MessageRole::User,
                        &format!("question {i} {}", "word ".repeat(words)),
                    ),
                    m(
                        MessageRole::Assistant,
                        &format!("answer {i} {}", "word ".repeat(words)),
                    ),
                ]
            })
            .collect()
    }

    #[test]
    fn short_conversations_go_in_verbatim() {
        let h = fit_history(&convo(3, 5), 2000);
        assert_eq!((h.included, h.summarized), (6, 0));
        assert!(h.summary.is_none());
        assert_eq!(h.messages[0].role, Role::User);
    }

    #[test]
    fn long_conversations_condense_older_turns_and_keep_the_latest() {
        let h = fit_history(&convo(30, 60), 1200);
        assert!(h.included >= 2 && h.included < 60, "{}", h.included);
        assert!(h.summarized > 0);
        let summary = h.summary.expect("summary");
        assert!(summary.contains("They asked: question"));
        // The newest turn is always verbatim, and the window starts with the user.
        assert!(
            h.messages
                .last()
                .expect("last")
                .content
                .starts_with("answer 29")
        );
        assert_eq!(h.messages[0].role, Role::User);
        let total: u32 =
            h.messages.iter().map(|m| tokens(&m.content)).sum::<u32>() + tokens(&summary);
        assert!(total <= 1200 + 50, "{total}");
    }

    #[test]
    fn budget_reserves_room_for_the_answer_and_screen() {
        let b = budget(8192, 1024, true);
        assert_eq!(b.screen_chars, 1500 * 4);
        assert!(b.history <= 3000);
        let small = budget(4096, 1024, true);
        assert!(small.history > 0 && small.screen_chars > 0);
        assert_eq!(budget(4096, 1024, false).screen_chars, 0);
    }
}
