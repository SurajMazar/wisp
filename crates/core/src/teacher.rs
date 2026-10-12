//! Teacher mode (spec §53): observe → work out the next step → highlight and say it → wait for
//! the user → observe → confirm → next. Wisp never performs a step itself; "Do it for me" goes
//! through the tool pipeline with the user's approval.

use std::{sync::Mutex, time::Duration};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wisp_ipc::{
    ErrorCode, IpcError,
    methods::{MessageUpdated, TeachUpdated},
    types::{
        app::Empty,
        conversations::{Message, MessageRole, MessageStatus, SendParams},
        focus::FocusRegion,
        teacher::{
            CurrentLesson, Lesson, LessonAction, LessonControl, LessonStatus, LessonStep,
            StepStatus,
        },
        voice::SpeakAnswers,
    },
};

use crate::Core;

/// A lesson stops after this many steps (spec §61: maximum steps).
pub const MAX_STEPS: usize = 15;
/// Waiting for the user to do a step ends after this long.
const STEP_WAIT: Duration = Duration::from_secs(15 * 60);
const POLL: Duration = Duration::from_millis(1500);

struct Active {
    lesson: Lesson,
    control: Option<mpsc::UnboundedSender<LessonAction>>,
    cancel: CancellationToken,
}

#[derive(Default)]
pub struct Teacher {
    current: Mutex<Option<Active>>,
}

/// "teach me how to make a pivot table" → "make a pivot table".
pub fn lesson_goal(text: &str) -> Option<String> {
    let t = text.trim();
    let lower = t.to_lowercase();
    for prefix in [
        "teach me how to ",
        "teach me how ",
        "teach me to ",
        "teach me ",
        "show me how to ",
        "walk me through ",
        "guide me through ",
        "guide me to ",
    ] {
        if lower.starts_with(prefix) {
            let goal = t[prefix.len()..]
                .trim()
                .trim_end_matches(['?', '.', '!'])
                .trim();
            return (!goal.is_empty()).then(|| goal.to_owned());
        }
    }
    None
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

enum Wait {
    Advanced(String),
    Stop,
}

impl Core {
    fn teacher(&self) -> &Teacher {
        self.teacher_state()
    }

    fn update_lesson(&self, f: impl FnOnce(&mut Lesson)) {
        let snapshot = self.teacher().current.lock().ok().and_then(|mut g| {
            g.as_mut().map(|a| {
                f(&mut a.lesson);
                a.lesson.clone()
            })
        });
        if let Some(l) = snapshot {
            self.events().publish::<TeachUpdated>(&l);
        }
    }

    pub(crate) fn current_lesson(&self) -> CurrentLesson {
        CurrentLesson {
            lesson: self
                .teacher()
                .current
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|a| a.lesson.clone())),
        }
    }

    pub(crate) fn control_lesson(&self, c: LessonControl) -> Result<Empty, IpcError> {
        let tx = self
            .teacher()
            .current
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .filter(|a| a.lesson.id == c.lesson_id)
                    .and_then(|a| a.control.clone())
            })
            .ok_or_else(|| IpcError::new(ErrorCode::NotFound, "That lesson has ended."))?;
        tx.send(c.action)
            .map_err(|_| IpcError::new(ErrorCode::NotFound, "That lesson has ended."))?;
        Ok(Empty {})
    }

    /// Start a lesson whose first step fills `first` (an empty streaming assistant message).
    pub(crate) fn start_lesson(&self, goal: String, first: Message, p: SendParams) {
        // One lesson at a time: a new one replaces the old.
        if let Ok(mut g) = self.teacher().current.lock()
            && let Some(old) = g.take()
        {
            old.cancel.cancel();
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let lesson = Lesson {
            id: wisp_common::ids::new_id(),
            conversation_id: first.conversation_id.clone(),
            goal: goal.clone(),
            status: LessonStatus::Thinking,
            steps: vec![],
            watching: false,
            note: None,
            started_at_ms: wisp_common::time::now_ms(),
        };
        if let Ok(mut g) = self.teacher().current.lock() {
            *g = Some(Active {
                lesson: lesson.clone(),
                control: Some(tx),
                cancel: cancel.clone(),
            });
        }
        self.events().publish::<TeachUpdated>(&lesson);
        let core = self.clone();
        tokio::spawn(async move {
            let status = core.run_lesson(goal, first, p, rx, cancel).await;
            core.update_lesson(|l| {
                if !matches!(l.status, LessonStatus::Done | LessonStatus::Failed) {
                    l.status = status;
                }
                for s in &mut l.steps {
                    if s.status == StepStatus::Current && status == LessonStatus::Done {
                        s.status = StepStatus::Done;
                    }
                }
            });
            if let Ok(mut g) = core.teacher().current.lock()
                && let Some(a) = g.as_mut()
            {
                a.control = None;
            }
            if let Some(host) = core.ai().host() {
                let _ = host
                    .call("host.guidance.clear", serde_json::json!({}))
                    .await;
            }
        });
    }

    async fn observe(&self) -> Result<FocusRegion, IpcError> {
        let host = self
            .ai()
            .host()
            .ok_or_else(|| IpcError::new(ErrorCode::Unavailable, "The app isn't connected."))?;
        let region: FocusRegion = serde_json::from_value(
            host.call("host.screen.observe", serde_json::json!({}))
                .await?,
        )
        .map_err(|e| IpcError::internal(e.to_string()))?;
        wisp_screen::validate(&region).map_err(|e| IpcError::internal(e.to_string()))?;
        Ok(region)
    }

    async fn new_step_message(&self, conversation_id: &str) -> Result<Message, IpcError> {
        let m = crate::conversations::new_message(
            conversation_id,
            MessageRole::Assistant,
            String::new(),
            MessageStatus::Streaming,
        );
        let stored = m.clone();
        self.blocking(move |db| db.insert_message(&stored)).await?;
        self.events().publish::<MessageUpdated>(&m);
        Ok(m)
    }

    async fn run_lesson(
        &self,
        goal: String,
        first: Message,
        p: SendParams,
        mut rx: mpsc::UnboundedReceiver<LessonAction>,
        cancel: CancellationToken,
    ) -> LessonStatus {
        let speak = self
            .blocking(|db| db.load_settings())
            .await
            .map(|s| s.voice.enabled && s.voice.speak_answers != SpeakAnswers::Never)
            .unwrap_or(false);
        let mut pending = Some(first);
        let mut done: Vec<String> = vec![];
        let mut repeats = 0;
        for _ in 0..MAX_STEPS {
            if cancel.is_cancelled() {
                return LessonStatus::Stopped;
            }
            self.update_lesson(|l| l.status = LessonStatus::Thinking);
            let msg = match pending.take() {
                Some(m) => m,
                None => match self
                    .new_step_message(
                        &self
                            .current_lesson()
                            .lesson
                            .map(|l| l.conversation_id)
                            .unwrap_or_default(),
                    )
                    .await
                {
                    Ok(m) => m,
                    Err(_) => return LessonStatus::Failed,
                },
            };
            let screen = match self.observe().await {
                Ok(r) => r,
                Err(e) => {
                    let mut m = msg;
                    m.status = MessageStatus::Error;
                    m.error = Some(if e.code == ErrorCode::PermissionDenied {
                        "To teach you, Wisp needs to see your screen. Allow Screen Recording for Wisp, then try again.".into()
                    } else {
                        format!("Wisp couldn't look at your screen: {}", e.message)
                    });
                    self.finish_step(m).await;
                    self.update_lesson(|l| l.note = Some("Wisp couldn't see your screen.".into()));
                    return LessonStatus::Failed;
                }
            };
            if speak {
                self.speak_message(&msg.id);
            }
            let question = format!(
                "Goal: {goal}\nSteps already done:\n{}\nWhat's the very next step?",
                if done.is_empty() {
                    "(none yet)".to_owned()
                } else {
                    done.iter()
                        .enumerate()
                        .map(|(i, s)| format!("{}. {s}", i + 1))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            );
            let token = cancel.child_token();
            self.inner_streams().insert(&msg.id, token.clone());
            let answered = self
                .answer(msg, question, Some(screen), p.clone(), token, true)
                .await;
            if cancel.is_cancelled() {
                return LessonStatus::Stopped;
            }
            if answered.status != MessageStatus::Complete {
                self.update_lesson(|l| l.note = answered.error.clone());
                return if answered.status == MessageStatus::Cancelled {
                    LessonStatus::Stopped
                } else {
                    LessonStatus::Failed
                };
            }
            let text = answered.content.trim().to_owned();
            if let Some(rest) = text.strip_prefix("DONE:") {
                self.update_lesson(|l| l.note = Some(rest.trim().to_owned()));
                return LessonStatus::Done;
            }
            let stuck = text.strip_prefix("STUCK:").map(|s| s.trim().to_owned());
            let instruction = stuck.clone().unwrap_or_else(|| text.clone());
            // Loop detection (spec §61): the same instruction three times means we're stuck.
            if done
                .last()
                .is_some_and(|d| normalize(d) == normalize(&instruction))
                || self
                    .current_lesson()
                    .lesson
                    .and_then(|l| l.steps.last().map(|s| normalize(&s.instruction)))
                    == Some(normalize(&instruction))
            {
                repeats += 1;
            } else {
                repeats = 0;
            }
            let id = answered.id.clone();
            let target = answered.guidance.last().cloned();
            // What's in front of the user now, to notice when they've done the step.
            let before = if stuck.is_some() {
                None
            } else {
                self.fingerprint().await
            };
            let watching = before.is_some();
            self.update_lesson(|l| {
                l.watching = watching;
                for s in &mut l.steps {
                    s.status = StepStatus::Done;
                }
                l.steps.push(LessonStep { instruction: instruction.clone(), target, status: StepStatus::Current, confirmed: None, message_id: id });
                l.status = if stuck.is_some() || repeats >= 2 { LessonStatus::Stuck } else { LessonStatus::WaitingForUser };
                l.note = if repeats >= 2 {
                    Some("It looks like we're going in circles. Try the step yourself, or tell me more.".into())
                } else {
                    None
                };
            });
            match self
                .wait_for_step(&answered, &instruction, &mut rx, &cancel, before)
                .await
            {
                Wait::Stop => return LessonStatus::Stopped,
                Wait::Advanced(how) => {
                    self.update_lesson(|l| {
                        if let Some(s) = l.steps.last_mut() {
                            s.status = StepStatus::Done;
                            s.confirmed = Some(how);
                        }
                    });
                    if stuck.is_none() {
                        done.push(instruction);
                    }
                }
            }
        }
        self.update_lesson(|l| {
            l.note = Some(format!(
                "That's {MAX_STEPS} steps — let's pause here. Ask again to keep going."
            ))
        });
        LessonStatus::Stopped
    }

    async fn finish_step(&self, mut m: Message) {
        m.stage = None;
        self.inner_streams().remove(&m.id);
        let stored = m.clone();
        let _ = self
            .blocking(move |db| db.update_message(&stored, (None, None)))
            .await;
        self.events().publish::<MessageUpdated>(&m);
    }

    /// A cheap signature of what's in front of the user (no capture): app, window, focus, selection.
    async fn fingerprint(&self) -> Option<String> {
        let host = self.ai().host()?;
        let v = host
            .call("host.ax.snapshot", serde_json::json!({ "maxNodes": 1 }))
            .await
            .ok()?;
        let f = &v["focused"];
        Some(format!(
            "{}|{}|{}|{}|{}|{}",
            v["application"]["name"].as_str().unwrap_or(""),
            v["window"]["title"].as_str().unwrap_or(""),
            f["role"].as_str().unwrap_or(""),
            f["title"].as_str().unwrap_or(""),
            f["value"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect::<String>(),
            v["selectedText"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(50)
                .collect::<String>(),
        ))
    }

    async fn wait_for_step(
        &self,
        step: &Message,
        instruction: &str,
        rx: &mut mpsc::UnboundedReceiver<LessonAction>,
        cancel: &CancellationToken,
        before: Option<String>,
    ) -> Wait {
        let watching = before.is_some();
        let deadline = tokio::time::Instant::now() + STEP_WAIT;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Wait::Stop,
                _ = tokio::time::sleep_until(deadline) => {
                    self.update_lesson(|l| l.note = Some("The lesson paused after a long wait.".into()));
                    return Wait::Stop;
                }
                a = rx.recv() => match a {
                    Some(LessonAction::Next) => return Wait::Advanced("You said you did it".into()),
                    Some(LessonAction::Repeat) => {
                        let _ = self.show_guidance(&step.id).await;
                        let _ = self
                            .speak_text(&wisp_screen::guide::strip_markers(instruction))
                            .await;
                    }
                    Some(LessonAction::Stop) | None => return Wait::Stop,
                },
                _ = tokio::time::sleep(POLL), if watching => {
                    if let Some(now) = self.fingerprint().await
                        && Some(&now) != before.as_ref()
                    {
                        // Let the app settle before looking again.
                        tokio::time::sleep(Duration::from_millis(700)).await;
                        return Wait::Advanced("Wisp saw the screen change".into());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_lesson_requests() {
        assert_eq!(
            lesson_goal("Teach me how to make a pivot table?").as_deref(),
            Some("make a pivot table")
        );
        assert_eq!(
            lesson_goal("walk me through setting up Time Machine").as_deref(),
            Some("setting up Time Machine")
        );
        assert_eq!(
            lesson_goal("show me how to crop a photo").as_deref(),
            Some("crop a photo")
        );
        assert_eq!(lesson_goal("What does this button do?"), None);
        assert_eq!(lesson_goal("teach me"), None);
    }
}
