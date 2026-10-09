//! AI providers, models and routing (spec §10–14, §75, §110–111).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Apple,
    Ollama,
    OpenaiCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Locality {
    Local,
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ModelCapability {
    Text,
    Vision,
    AudioInput,
    AudioOutput,
    ToolCalling,
    StructuredOutput,
    Streaming,
    Reasoning,
}

/// Provider health as observed right now (spec §111). Never assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProviderState {
    /// Ready to answer.
    Available,
    /// Installed but its server isn't running.
    NotRunning,
    /// Not installed on this Mac.
    NotInstalled,
    /// Present but unusable (e.g. Apple Intelligence off, unreachable endpoint).
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInfo {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    pub locality: Locality,
    pub state: ProviderState,
    /// User-facing explanation when not available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelInfo {
    pub provider_id: String,
    /// Provider-native identifier, e.g. `qwen3-vl:8b`.
    pub id: String,
    pub name: String,
    pub locality: Locality,
    pub capabilities: Vec<ModelCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRef {
    pub provider_id: String,
    pub model: String,
}

/// A user-configured OpenAI-compatible endpoint (LM Studio, llama.cpp server, or a cloud API).
/// Its API key lives in the Keychain under `keychain_account`, never here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndpointConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keychain_account: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ModelSettings {
    /// Preferred model for text. `None` lets the router choose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub general: Option<ModelRef>,
    /// Preferred model when an image (focus region) is included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vision: Option<ModelRef>,
    /// 0–2.
    pub temperature: f64,
    /// 64–8192.
    pub max_tokens: u32,
    /// 1024–131072.
    pub context_length: u32,
    pub ollama_url: String,
    pub endpoints: Vec<EndpointConfig>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            general: None,
            vision: None,
            temperature: 0.4,
            max_tokens: 1024,
            context_length: 8192,
            ollama_url: "http://127.0.0.1:11434".into(),
            endpoints: vec![],
        }
    }
}

impl ModelSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err("temperature must be 0–2".into());
        }
        if !(64..=8192).contains(&self.max_tokens) {
            return Err("maxTokens must be 64–8192".into());
        }
        if !(1024..=131_072).contains(&self.context_length) {
            return Err("contextLength must be 1024–131072".into());
        }
        let local = |u: &str| {
            u.strip_prefix("http://")
                .map(|r| {
                    r.starts_with("127.0.0.1")
                        || r.starts_with("localhost")
                        || r.starts_with("[::1]")
                })
                .unwrap_or(false)
        };
        if !local(&self.ollama_url) {
            return Err("ollamaUrl must be a local http:// address".into());
        }
        for e in &self.endpoints {
            if !(e.base_url.starts_with("https://") || local(&e.base_url)) {
                return Err(format!(
                    "endpoint {} must use https or a local http address",
                    e.name
                ));
            }
            if e.id.is_empty()
                || e.id.len() > 64
                || !e.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err("endpoint id must be 1–64 letters, digits or dashes".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelsList {
    pub providers: Vec<ProviderInfo>,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelNameParams {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullProgress {
    pub name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelTestParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelTestResult {
    pub model: ModelRef,
    pub locality: Locality,
    pub text: String,
    pub first_token_ms: u32,
    pub total_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteParams {
    pub needs_vision: bool,
    /// The user approved cloud use for this one request (Local Preferred).
    #[serde(default)]
    pub allow_cloud: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteInfo {
    pub model: ModelRef,
    pub model_name: String,
    pub provider_name: String,
    pub locality: Locality,
    /// Why this model was chosen, in plain words.
    pub reason: String,
}

// ── Host-side Apple Foundation Models ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppleStatus {
    pub state: ProviderState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppleGenerateParams {
    pub request_id: String,
    pub instructions: String,
    pub prompt: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppleGenerateResult {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppleDelta {
    pub request_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestIdParams {
    pub request_id: String,
}

// ── Keychain (host) ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeychainAccount {
    pub account: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeychainSetParams {
    pub account: String,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeychainSecret {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}
