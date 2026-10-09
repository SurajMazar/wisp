//! The conversation engine (spec §50–51): context → route → stream → persist.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use wisp_ai::{ChatMessage, Chunk, ImagePart, Role};
use wisp_ipc::{
    IpcError,
    methods::{ConversationDeltaEvent, ConversationUpdated, MessageUpdated, ReasoningDelta},
    types::{
        conversations::{
            FocusAttachment, Message, MessageDelta, MessageRole, MessageStage, MessageStatus,
            SendParams, SendResult,
        },
        focus::{FocusKind, FocusRegion},
    },
};

use crate::Core;

/// How Wisp behaves. Short, spoken-friendly answers grounded in what the user is pointing at.
pub const SYSTEM_PROMPT: &str = "You are Wisp, a calm, helpful assistant that lives on the user's Mac beside their cursor. \
The user can point at part of their screen; when an image is attached it shows exactly the area they selected, \
and a list of the app's UI elements in that area may follow their question. \
Answer conversationally and concisely — usually one to four sentences — because answers may be read aloud. \
When they ask where to click or what something does, name the control by its label and describe where it is. \
Use plain text; avoid tables. Never claim you clicked, typed or changed anything. \
If you can't see or don't know something, say so plainly.";

const CONTEXT_MESSAGES: u32 = 16;
const MAX_TEXT: usize = 8000;

#[derive(Default)]
pub struct Streams {
    cancels: Mutex<HashMap<String, CancellationToken>>,
}

impl Streams {
    fn insert(&self, id: &str, t: CancellationToken) {
        if let Ok(mut m) = self.cancels.lock() {
            m.insert(id.to_owned(), t);
        }
    }
    fn remove(&self, id: &str) {
        if let Ok(mut m) = self.cancels.lock() {
            m.remove(id);
        }
    }
    pub fn cancel(&self, id: &str) -> bool {
        self.cancels
            .lock()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .map(|t| t.cancel())
            .is_some()
    }
}

fn title_from(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("New conversation")
        .trim();
    let mut t: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        t.push('…');
    }
    t
}

fn focus_label(r: &FocusRegion) -> String {
    let size = format!("{:.0} × {:.0} pt", r.selection.width, r.selection.height);
    let place = r
        .window
        .as_ref()
        .and_then(|w| w.title.clone())
        .or_else(|| r.application.as_ref().map(|a| a.name.clone()));
    [Some(size), place]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Load the model-sized image for a focus capture from the cache, confined to it.
fn load_focus_image(data_dir: &Path, focus_id: &str) -> Option<ImagePart> {
    if !wisp_common::ids::is_valid_id(focus_id) {
        return None;
    }
    let dir = data_dir.join("cache/focus");
    for (name, mime) in [
        (format!("{focus_id}.model.jpg"), "image/jpeg"),
        (format!("{focus_id}.png"), "image/png"),
    ] {
        if let Ok(data) = std::fs::read(dir.join(name)) {
            return Some(ImagePart {
                mime: mime.into(),
                data,
            });
        }
    }
    None
}

fn new_message(
    conversation_id: &str,
    role: MessageRole,
    content: String,
    status: MessageStatus,
) -> Message {
    Message {
        id: wisp_common::ids::new_id(),
        conversation_id: conversation_id.to_owned(),
        role,
        content,
        status,
        model: None,
        model_name: None,
        locality: None,
        focus: None,
        error: None,
        error_details: None,
        latency_ms: None,
        reasoning: None,
        reasoning_ms: None,
        stage: None,
        created_at_ms: wisp_common::time::now_ms(),
    }
}

impl Core {
    pub(crate) async fn send(&self, p: SendParams) -> Result<SendResult, IpcError> {
        let text = p.text.trim().to_owned();
        if text.is_empty() || text.chars().count() > MAX_TEXT {
            return Err(IpcError::invalid_params(format!(
                "text must be 1–{MAX_TEXT} characters"
            )));
        }
        let conversation_id = match p.conversation_id.clone() {
            Some(id) => {
                let check = id.clone();
                if !wisp_common::ids::is_valid_id(&id)
                    || !self
                        .blocking(move |db| db.conversation_exists(&check))
                        .await?
                {
                    return Err(IpcError::new(
                        wisp_ipc::ErrorCode::NotFound,
                        "That conversation no longer exists.",
                    ));
                }
                id
            }
            None => {
                let title = title_from(&text);
                self.blocking(move |db| db.create_conversation(&title))
                    .await?
            }
        };

        let focus: Option<FocusRegion> = if p.include_focus {
            self.latest_focus()
        } else {
            None
        };
        let mut user = new_message(
            &conversation_id,
            MessageRole::User,
            text.clone(),
            MessageStatus::Complete,
        );
        user.focus = focus.as_ref().map(|f| FocusAttachment {
            focus_id: f.id.clone(),
            kind: f.kind,
            image_url: f.image.as_ref().map(|i| i.url.clone()),
            label: focus_label(f),
        });
        let mut assistant = new_message(
            &conversation_id,
            MessageRole::Assistant,
            String::new(),
            MessageStatus::Streaming,
        );
        assistant.created_at_ms = user.created_at_ms + 1;

        let (u, a) = (user.clone(), assistant.clone());
        self.blocking(move |db| {
            db.insert_message(&u)?;
            db.insert_message(&a)
        })
        .await?;
        self.events().publish::<MessageUpdated>(&user);
        self.events().publish::<MessageUpdated>(&assistant);
        self.publish_summary(&conversation_id).await;

        let token = CancellationToken::new();
        self.inner_streams().insert(&assistant.id, token.clone());
        let core = self.clone();
        let (a, f) = (assistant.clone(), focus);
        tokio::spawn(async move {
            core.answer(a, text, f, p, token).await;
        });

        Ok(SendResult {
            conversation_id,
            user_message: user,
            assistant_message: assistant,
        })
    }

    /// Re-answer the user question that preceded `message_id`, replacing that answer.
    pub(crate) async fn regenerate(
        &self,
        p: wisp_ipc::types::conversations::RegenerateParams,
    ) -> Result<Message, IpcError> {
        if !wisp_common::ids::is_valid_id(&p.message_id) {
            return Err(IpcError::invalid_params("invalid id"));
        }
        let id = p.message_id.clone();
        let old = self
            .blocking(move |db| db.get_message(&id))
            .await?
            .filter(|m| m.role == MessageRole::Assistant && m.status != MessageStatus::Streaming)
            .ok_or_else(|| {
                IpcError::new(
                    wisp_ipc::ErrorCode::NotFound,
                    "That answer can't be regenerated.",
                )
            })?;
        let conv = old.conversation_id.clone();
        let conversation = self
            .blocking(move |db| db.get_conversation(&conv))
            .await?
            .ok_or_else(|| {
                IpcError::new(
                    wisp_ipc::ErrorCode::NotFound,
                    "That conversation no longer exists.",
                )
            })?;
        let question = conversation
            .messages
            .iter()
            .take_while(|m| m.id != old.id)
            .filter(|m| m.role == MessageRole::User)
            .last()
            .cloned()
            .ok_or_else(|| {
                IpcError::new(
                    wisp_ipc::ErrorCode::NotFound,
                    "There's no question to answer again.",
                )
            })?;
        let old_id = old.id.clone();
        self.blocking(move |db| db.delete_message(&old_id)).await?;
        self.events()
            .publish::<wisp_ipc::methods::MessageRemoved>(&old);

        let mut assistant = new_message(
            &old.conversation_id,
            MessageRole::Assistant,
            String::new(),
            MessageStatus::Streaming,
        );
        assistant.created_at_ms = old.created_at_ms;
        let a = assistant.clone();
        self.blocking(move |db| db.insert_message(&a)).await?;
        self.events().publish::<MessageUpdated>(&assistant);

        // Re-attach the focus only if it's still the current one.
        let focus = question
            .focus
            .as_ref()
            .and_then(|f| self.latest_focus().filter(|l| l.id == f.focus_id));
        let token = CancellationToken::new();
        self.inner_streams().insert(&assistant.id, token.clone());
        let send = SendParams {
            conversation_id: Some(old.conversation_id.clone()),
            text: question.content.clone(),
            include_focus: focus.is_some(),
            allow_cloud: false,
            model: p.model,
        };
        let core = self.clone();
        let a = assistant.clone();
        tokio::spawn(async move { core.answer(a, question.content, focus, send, token).await });
        Ok(assistant)
    }

    async fn publish_summary(&self, id: &str) {
        let id = id.to_owned();
        if let Ok(Some(s)) = self.blocking(move |db| db.conversation_summary(&id)).await {
            self.events().publish::<ConversationUpdated>(&s);
        }
    }

    /// Build context, generate and stream into `msg`, persisting as it goes.
    async fn answer(
        &self,
        mut msg: Message,
        question: String,
        focus: Option<FocusRegion>,
        p: SendParams,
        cancel: CancellationToken,
    ) {
        let started = Instant::now();
        let stage = |msg: &mut Message, s: MessageStage, core: &Core| {
            if msg.stage != Some(s) {
                msg.stage = Some(s);
                core.events().publish::<MessageUpdated>(msg);
            }
        };
        stage(
            &mut msg,
            if focus.is_some() {
                MessageStage::ReadingScreen
            } else {
                MessageStage::Preparing
            },
            self,
        );
        let snapshot = msg.clone();
        let result = async {
            let settings = self.blocking(|db| db.load_settings()).await?;
            let conv = msg.conversation_id.clone();
            let history = self
                .blocking(move |db| db.recent_messages(&conv, CONTEXT_MESSAGES + 1))
                .await?;

            let mut messages: Vec<ChatMessage> = history
                .iter()
                .filter(|m| m.id != msg.id && !m.content.trim().is_empty())
                .map(|m| {
                    ChatMessage::text(
                        if m.role == MessageRole::User {
                            Role::User
                        } else {
                            Role::Assistant
                        },
                        m.content.clone(),
                    )
                })
                .collect();
            // The question itself is the last user message; enrich it with what they're pointing at.
            if messages
                .last()
                .is_some_and(|m| m.role == Role::User && m.content == question)
            {
                messages.pop();
            }
            let mut content = question.clone();
            let mut images = vec![];
            if let Some(f) = &focus {
                let what = match f.kind {
                    FocusKind::Window => "the window",
                    FocusKind::Screen => "the whole screen",
                    FocusKind::Point => "the area around where I clicked",
                    _ => "the area I selected",
                };
                content.push_str(&format!("\n\n[The attached image shows {what}.]"));
                if let Some(ax) = f
                    .accessibility
                    .as_ref()
                    .filter(|a| !a.summary.trim().is_empty())
                {
                    content.push_str(&format!("\n[UI elements there]\n{}", ax.summary.trim()));
                } else if let Some(app) = &f.application {
                    content.push_str(&format!("\n[App: {}]", app.name));
                }
                if let Some(img) = load_focus_image(&self.data_dir(), &f.id) {
                    images.push(img);
                }
            }
            messages.push(ChatMessage {
                role: Role::User,
                content,
                images,
            });

            let events = self.events().clone();
            let mut announced = snapshot.clone();
            announced.stage = Some(MessageStage::ChoosingModel);
            events.publish::<MessageUpdated>(&announced);
            let on_route = move |route: &wisp_ipc::types::models::RouteInfo,
                                 loaded: Option<bool>| {
                let mut m = announced.clone();
                m.model = Some(route.model.clone());
                m.model_name = Some(route.model_name.clone());
                m.locality = Some(route.locality);
                m.stage = Some(if loaded == Some(false) {
                    MessageStage::LoadingModel
                } else {
                    MessageStage::Waiting
                });
                events.publish::<MessageUpdated>(&m);
            };
            self.ai()
                .generate(
                    &settings.models,
                    settings.privacy.mode,
                    p.allow_cloud,
                    p.model.as_ref(),
                    Some(SYSTEM_PROMPT.into()),
                    messages,
                    cancel.clone(),
                    &on_route,
                )
                .await
        }
        .await;

        let prepared = match result {
            Ok(prep) => prep,
            Err(e) => {
                msg.status = if e.code == wisp_ipc::ErrorCode::Cancelled {
                    MessageStatus::Cancelled
                } else {
                    MessageStatus::Error
                };
                msg.error = Some(e.message.clone());
                msg.error_details = e.details.clone();
                self.finish(msg, (None, None)).await;
                return;
            }
        };

        msg.model = Some(prepared.route.model.clone());
        msg.model_name = Some(prepared.route.model_name.clone());
        msg.locality = Some(prepared.route.locality);
        msg.stage = Some(MessageStage::Waiting);
        self.events().publish::<MessageUpdated>(&msg);
        let mut reasoning_started: Option<Instant> = None;

        let mut stream = wisp_ai::think::separate_reasoning(prepared.stream);
        let mut last_save = Instant::now();
        let mut usage = (None, None);
        let mut first_token: Option<Duration> = None;
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => { msg.status = MessageStatus::Cancelled; break; }
                n = stream.next() => n,
            };
            match next {
                Some(Ok(Chunk::Reasoning(t))) => {
                    reasoning_started.get_or_insert_with(Instant::now);
                    stage(&mut msg, MessageStage::Reasoning, self);
                    msg.reasoning.get_or_insert_with(String::new).push_str(&t);
                    self.events().publish::<ReasoningDelta>(&MessageDelta {
                        conversation_id: msg.conversation_id.clone(),
                        message_id: msg.id.clone(),
                        delta: t,
                    });
                }
                Some(Ok(Chunk::Text(t))) => {
                    first_token.get_or_insert(started.elapsed());
                    if msg.stage != Some(MessageStage::Writing) {
                        if let Some(r) = reasoning_started {
                            msg.reasoning_ms
                                .get_or_insert(r.elapsed().as_millis() as u32);
                        }
                        stage(&mut msg, MessageStage::Writing, self);
                    }
                    msg.content.push_str(&t);
                    self.events()
                        .publish::<ConversationDeltaEvent>(&MessageDelta {
                            conversation_id: msg.conversation_id.clone(),
                            message_id: msg.id.clone(),
                            delta: t,
                        });
                    if last_save.elapsed() > Duration::from_millis(500) {
                        last_save = Instant::now();
                        let m = msg.clone();
                        let _ = self
                            .blocking(move |db| db.update_message(&m, (None, None)))
                            .await;
                    }
                }
                Some(Ok(Chunk::Done(u))) => {
                    usage = (u.prompt_tokens, u.completion_tokens);
                    msg.status = MessageStatus::Complete;
                    break;
                }
                Some(Err(wisp_ai::AiError::Cancelled)) => {
                    msg.status = MessageStatus::Cancelled;
                    break;
                }
                Some(Err(e)) => {
                    let ipc: IpcError = e.into();
                    msg.status = MessageStatus::Error;
                    msg.error = Some(ipc.message);
                    msg.error_details = ipc.details;
                    break;
                }
                None => {
                    msg.status = MessageStatus::Complete;
                    break;
                }
            }
        }
        if let (Some(r), None) = (reasoning_started, msg.reasoning_ms) {
            msg.reasoning_ms = Some(r.elapsed().as_millis() as u32);
        }
        msg.reasoning = msg
            .reasoning
            .map(|r| r.trim().to_owned())
            .filter(|r| !r.is_empty());
        msg.stage = None;
        msg.content = msg.content.trim().to_owned();
        if msg.status == MessageStatus::Complete && msg.content.is_empty() {
            msg.status = MessageStatus::Error;
            msg.error =
                Some("The model didn't say anything. Try again or pick another model.".into());
        }
        msg.latency_ms = Some(started.elapsed().as_millis().min(u32::MAX as u128) as u32);
        let route = msg.model.clone();
        let locality = msg.locality;
        let first_ms = first_token.map(|d| d.as_millis() as i64);
        let total_ms = started.elapsed().as_millis() as i64;
        let _ = self
            .blocking(move |db| {
                db.record_diagnostic(
                    "model",
                    "generate",
                    route.as_ref().map(|r| r.provider_id.as_str()),
                    route.as_ref().map(|r| r.model.as_str()),
                    Some(total_ms),
                    usage,
                    serde_json::json!({ "firstTokenMs": first_ms, "locality": locality }),
                )
            })
            .await;
        self.finish(msg, usage).await;
    }

    async fn finish(&self, mut msg: Message, usage: (Option<u32>, Option<u32>)) {
        msg.stage = None;
        self.inner_streams().remove(&msg.id);
        let m = msg.clone();
        if let Err(e) = self.blocking(move |db| db.update_message(&m, usage)).await {
            tracing::warn!(error = %e, "failed to persist message");
        }
        self.events().publish::<MessageUpdated>(&msg);
        self.publish_summary(&msg.conversation_id).await;
    }

    pub(crate) fn data_dir(&self) -> PathBuf {
        PathBuf::from(&self.info().data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_first_line_clipped() {
        assert_eq!(
            title_from("\n  Explain this error please \nmore"),
            "Explain this error please"
        );
        assert_eq!(title_from(&"x".repeat(80)).chars().count(), 61);
    }

    #[test]
    fn focus_images_are_confined_to_the_cache() {
        let dir = std::env::temp_dir();
        assert!(load_focus_image(&dir, "../../etc/passwd").is_none());
        assert!(load_focus_image(&dir, "0199c2a8-0000-7000-8000-00000000dead").is_none());
    }
}
