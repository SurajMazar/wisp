//! Conversations (spec §50, §73).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    focus::FocusKind,
    models::{Locality, ModelRef},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MessageStatus {
    Streaming,
    Complete,
    Cancelled,
    Error,
}

/// What the user was pointing at when they asked. Metadata only; the image stays in the cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusAttachment {
    pub focus_id: String,
    pub kind: FocusKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// e.g. "320 × 180 pt · Safari · Checkout".
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub status: MessageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locality: Option<Locality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<FocusAttachment>,
    /// User-facing error when `status` is `error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
    /// The model's streamed reasoning, shown collapsed above the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// How long the model reasoned before answering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_ms: Option<u32>,
    /// Your files this answer drew on ([n] in the text).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<super::files::Citation>,
    /// Memories saved from the question this answers (shown with Undo).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saved_memories: Vec<super::memory::MemoryRef>,
    /// Things on screen this answer pointed at (shown with the guidance overlay).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guidance: Vec<super::guidance::GuidanceTarget>,
    /// What was sent to the model with this answer's question (privacy transparency).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextInfo>,
    /// What's happening right now while `status` is `streaming` (live progress for the UI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<MessageStage>,
    pub created_at_ms: i64,
}

/// What context an answer was given (spec §36, §97) — shown under the answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextInfo {
    /// Size of the compressed screenshot sent, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_bytes: Option<u32>,
    /// Lines of on-screen text included.
    pub screen_text_lines: u32,
    pub ui_elements: u32,
    pub selected_text: bool,
    /// The screen from an earlier question was reused (no new capture or vision pass).
    pub reused_screen: bool,
    /// Earlier messages included verbatim.
    pub history_messages: u32,
    /// Older messages condensed into a short summary to fit the model's context.
    pub summarized_messages: u32,
    /// Rough size of the prompt.
    pub estimated_tokens: u32,
    /// Skills whose instructions were included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// Saved memories included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<String>,
    /// Passages from your files included, as citation labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Why the screenshot wasn't sent even though one was attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_only_reason: Option<String>,
}

/// Live progress of an answer, so the UI can say what Wisp is doing instead of a bare spinner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MessageStage {
    /// Gathering the conversation and attachments.
    Preparing,
    /// Reading the attached screen region (image + UI elements).
    ReadingScreen,
    /// Picking the model to use.
    ChoosingModel,
    /// The chosen model isn't in memory yet and is loading.
    LoadingModel,
    /// The model is working on it (no output yet).
    Waiting,
    /// The model is streaming its reasoning.
    Reasoning,
    /// The answer is streaming.
    Writing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub message_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub messages: Vec<Message>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// 1–500.
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationList {
    pub items: Vec<ConversationSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdParams {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameParams {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PinParams {
    pub id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendParams {
    /// Continue this conversation; omit to start a new one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub text: String,
    /// Attach the current focus region (screenshot + UI elements).
    #[serde(default)]
    pub include_focus: bool,
    /// The user approved cloud for this message (Local Preferred).
    #[serde(default)]
    pub allow_cloud: bool,
    /// Use this model instead of the router's choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    /// Start a guided lesson for this goal instead of answering (spec §53).
    #[serde(default)]
    pub teach: bool,
    /// Carry out this goal on the computer as an agent run instead of answering (spec §60).
    #[serde(default)]
    pub act: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendResult {
    pub conversation_id: String,
    pub user_message: Message,
    pub assistant_message: Message,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageDelta {
    pub conversation_id: String,
    pub message_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegenerateParams {
    /// The assistant message to replace.
    pub message_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardText {
    pub text: String,
}
