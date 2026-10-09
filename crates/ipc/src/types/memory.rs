//! Memory (spec §67, §96): things the user asked Wisp to remember, always inspectable and
//! deletable. Nothing is stored silently: every save is shown in the conversation with Undo.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MemoryCategory {
    UserProfile,
    Preference,
    Project,
    Fact,
    Conversation,
    Agent,
    Skill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MemoryLevel {
    /// Forgotten after a day.
    Temporary,
    /// Forgotten when Wisp restarts.
    Session,
    Project,
    Persistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MemorySource {
    /// Added or edited by the user on the Memory page.
    User,
    /// Taken from something the user said ("remember that…").
    Extracted,
    /// Saved by an agent.
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryItem {
    pub id: String,
    pub category: MemoryCategory,
    pub level: MemoryLevel,
    pub content: String,
    pub source: MemorySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub importance: f32,
    pub access_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_accessed_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryListParams {
    /// Filter by words in the content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<MemoryCategory>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryList {
    pub items: Vec<MemoryItem>,
    /// The cap on stored memories.
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemorySaveParams {
    /// Omit to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub content: String,
    pub category: MemoryCategory,
    pub level: MemoryLevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryCleared {
    pub deleted: u32,
}

/// A memory saved or used by an answer (shown in the chat).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryRef {
    pub id: String,
    pub content: String,
}

/// Memory preferences (spec §67).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct MemorySettings {
    /// Use saved memories in answers.
    pub enabled: bool,
    /// Save things the user clearly states ("remember that…", "I prefer…").
    pub learn_from_chats: bool,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            learn_from_chats: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedParams {
    /// 1–64 texts, each up to 2 000 characters.
    pub texts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Embeddings {
    /// e.g. `apple-nl-sentence-en`.
    pub model: String,
    pub dims: u32,
    /// One per text; empty when a text couldn't be embedded.
    pub vectors: Vec<Vec<f32>>,
}
