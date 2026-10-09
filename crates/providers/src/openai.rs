//! OpenAI-compatible chat completions (LM Studio, llama.cpp server, vLLM, or a cloud API).
//! Locality is derived from the endpoint address: loopback is local, anything else is cloud.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::Engine;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use wisp_ai::{AiError, AiProvider, Chunk, ChunkStream, GenerateRequest, Role, Usage};
use wisp_ipc::types::models::{
    CloudPreset, EndpointConfig, Locality, ModelCapability, ModelInfo, ProviderInfo, ProviderKind,
    ProviderState,
};

use crate::{HostBridge, stream::lines_to_chunks};

pub struct OpenAiCompatibleProvider {
    config: EndpointConfig,
    id: String,
    locality: Locality,
    client: reqwest::Client,
    host: Arc<dyn HostBridge>,
}

pub fn locality_of(base_url: &str) -> Locality {
    if wisp_common::net::is_loopback_url(base_url) {
        Locality::Local
    } else {
        Locality::Cloud
    }
}

impl OpenAiCompatibleProvider {
    pub fn new(config: EndpointConfig, host: Arc<dyn HostBridge>) -> Self {
        let id = format!("endpoint:{}", config.id);
        let locality = locality_of(&config.base_url);
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            config,
            id,
            locality,
            host,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.config.base_url.trim_end_matches('/'), path)
    }

    /// Fetch the API key from the Keychain for this request only; it is never stored by the core.
    async fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let Some(account) = &self.config.keychain_account else {
            return req;
        };
        match self
            .host
            .call("host.keychain.get", json!({ "account": account }))
            .await
        {
            Ok(v) => match v.get("secret").and_then(Value::as_str) {
                // Anthropic's model list wants its own headers; its chat endpoint takes either.
                Some(key) if self.config.preset == Some(CloudPreset::Anthropic) => req
                    .bearer_auth(key)
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01"),
                Some(key) => req.bearer_auth(key),
                None => req,
            },
            Err(e) => {
                tracing::warn!(endpoint = %self.config.name, code = ?e.code, "keychain lookup failed");
                req
            }
        }
    }
}

/// Chat models only: providers also list embedding, speech, image and moderation models.
fn preset_keeps(preset: Option<CloudPreset>, id: &str) -> bool {
    const NOT_CHAT: [&str; 12] = [
        "embed",
        "tts",
        "whisper",
        "transcribe",
        "audio",
        "realtime",
        "image",
        "dall-e",
        "moderation",
        "search",
        "aqa",
        "imagen",
    ];
    if NOT_CHAT.iter().any(|w| id.contains(w)) {
        return false;
    }
    match preset {
        None => true,
        Some(CloudPreset::Openai) => {
            ["gpt-", "o1", "o3", "o4", "chatgpt-"]
                .iter()
                .any(|p| id.starts_with(p))
                && !id.contains("instruct")
                && !id.starts_with("gpt-3.5")
        }
        Some(CloudPreset::Anthropic) => id.starts_with("claude-"),
        Some(CloudPreset::Gemini) => id.starts_with("gemini-") || id.starts_with("gemma-"),
    }
}

/// Current models from these services understand images; unknown endpoints are text-only
/// unless the user picks the model for vision.
fn preset_capabilities(preset: Option<CloudPreset>, id: &str) -> Vec<ModelCapability> {
    let mut caps = vec![ModelCapability::Text, ModelCapability::Streaming];
    let vision = match preset {
        None => false,
        Some(CloudPreset::Openai) => !id.starts_with("o1-mini") && !id.starts_with("o3-mini"),
        Some(CloudPreset::Anthropic) => true,
        Some(CloudPreset::Gemini) => id.starts_with("gemini-"),
    };
    if vision {
        caps.push(ModelCapability::Vision);
    }
    caps
}

fn openai_is_reasoning(model: &str) -> bool {
    ["o1", "o3", "o4", "gpt-5"]
        .iter()
        .any(|p| model.starts_with(p))
}

pub(crate) fn parse_sse_line(line: &str) -> Result<Vec<Chunk>, AiError> {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(vec![]);
    };
    if data == "[DONE]" {
        return Ok(vec![Chunk::Done(Usage::default())]);
    }
    let v: Value = serde_json::from_str(data).map_err(|e| AiError::Protocol(e.to_string()))?;
    if let Some(err) = v.pointer("/error/message").and_then(Value::as_str) {
        return Err(AiError::unavailable(
            "The model couldn't answer.",
            err.to_owned(),
        ));
    }
    let field = |k: &str| {
        v.pointer(&format!("/choices/0/delta/{k}"))
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    };
    let mut out = vec![];
    // Servers name streamed reasoning differently (llama.cpp/DeepSeek vs. vLLM/OpenRouter).
    if let Some(r) = field("reasoning_content").or_else(|| field("reasoning")) {
        out.push(Chunk::Reasoning(r));
    }
    if let Some(t) = field("content") {
        out.push(Chunk::Text(t));
    }
    Ok(out)
}

#[async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenaiCompatible
    }

    fn locality(&self) -> Locality {
        self.locality
    }

    async fn info(&self) -> ProviderInfo {
        let ok = self
            .auth(
                self.client
                    .get(self.url("models"))
                    .timeout(Duration::from_secs(3)),
            )
            .await
            .send()
            .await;
        let (state, detail) = match ok {
            Ok(r) if r.status().is_success() => (ProviderState::Available, None),
            Ok(r) => (
                ProviderState::Unavailable,
                Some(format!(
                    "The endpoint answered HTTP {}. Check the address and key.",
                    r.status()
                )),
            ),
            Err(_) => (
                ProviderState::Unavailable,
                Some("Couldn't reach this endpoint.".to_owned()),
            ),
        };
        ProviderInfo {
            id: self.id.clone(),
            kind: ProviderKind::OpenaiCompatible,
            name: self.config.name.clone(),
            locality: self.locality,
            state,
            detail,
            version: None,
        }
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        let v: Value = self
            .auth(
                self.client
                    .get(self.url("models"))
                    .timeout(Duration::from_secs(5)),
            )
            .await
            .send()
            .await
            .map_err(|e| AiError::unavailable("Couldn't reach this endpoint.", e.to_string()))?
            .json()
            .await
            .map_err(|e| AiError::Protocol(e.to_string()))?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let id = m.get("id").and_then(Value::as_str)?;
                // Gemini lists `models/gemini-…`; requests take the bare name.
                let id = id.strip_prefix("models/").unwrap_or(id);
                let name = m
                    .get("display_name")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned();
                preset_keeps(self.config.preset, id).then(|| (id.to_owned(), name))
            })
            .map(|(id, name)| ModelInfo {
                provider_id: self.id.clone(),
                capabilities: preset_capabilities(self.config.preset, &id),
                id,
                name,
                locality: self.locality,
                size_bytes: None,
                context_length: None,
                parameter_size: None,
                family: None,
            })
            .collect())
    }

    async fn generate(
        &self,
        req: GenerateRequest,
        cancel: CancellationToken,
    ) -> Result<ChunkStream, AiError> {
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut messages: Vec<Value> = vec![];
        if let Some(s) = &req.system {
            messages.push(json!({ "role": "system", "content": s }));
        }
        for m in &req.messages {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            if m.images.is_empty() {
                messages.push(json!({ "role": role, "content": m.content }));
            } else {
                let mut parts = vec![json!({ "type": "text", "text": m.content })];
                for i in &m.images {
                    parts.push(json!({ "type": "image_url", "image_url": { "url": format!("data:{};base64,{}", i.mime, b64.encode(&i.data)) } }));
                }
                messages.push(json!({ "role": role, "content": parts }));
            }
        }
        let mut body = json!({ "model": req.model, "messages": messages, "stream": true, "temperature": req.temperature, "max_tokens": req.max_tokens });
        if self.config.preset == Some(CloudPreset::Openai)
            && let Some(o) = body.as_object_mut()
        {
            // Current OpenAI models take `max_completion_tokens`; reasoning models reject a
            // custom temperature.
            if let Some(m) = o.remove("max_tokens") {
                o.insert("max_completion_tokens".into(), m);
            }
            if openai_is_reasoning(&req.model) {
                o.remove("temperature");
                o.insert(
                    "reasoning_effort".into(),
                    json!(if req.think { "medium" } else { "low" }),
                );
            }
        }
        let resp = self
            .auth(self.client.post(self.url("chat/completions")).json(&body))
            .await
            .send()
            .await
            .map_err(|e| AiError::unavailable("Couldn't reach this endpoint.", e.to_string()))?;
        if !resp.status().is_success() {
            let s = resp.status();
            return Err(AiError::unavailable(
                "The model couldn't answer.",
                format!("HTTP {s}: {}", resp.text().await.unwrap_or_default()),
            ));
        }
        Ok(lines_to_chunks(resp, cancel, parse_sse_line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_keep_only_chat_models() {
        use CloudPreset::*;
        assert!(preset_keeps(Some(Openai), "gpt-4.1-mini"));
        assert!(preset_keeps(Some(Openai), "o4-mini"));
        assert!(!preset_keeps(Some(Openai), "text-embedding-3-small"));
        assert!(!preset_keeps(Some(Openai), "gpt-4o-mini-tts"));
        assert!(!preset_keeps(Some(Openai), "gpt-4o-realtime-preview"));
        assert!(preset_keeps(Some(Anthropic), "claude-sonnet-4-5"));
        assert!(preset_keeps(Some(Gemini), "gemini-2.5-flash"));
        assert!(!preset_keeps(Some(Gemini), "gemini-embedding-001"));
        assert!(
            preset_capabilities(Some(Anthropic), "claude-sonnet-4-5")
                .contains(&ModelCapability::Vision)
        );
        assert!(!preset_capabilities(None, "local-model").contains(&ModelCapability::Vision));
        assert!(openai_is_reasoning("gpt-5-mini") && !openai_is_reasoning("gpt-4.1"));
    }

    #[test]
    fn locality_from_address() {
        assert_eq!(locality_of("http://127.0.0.1:1234/v1"), Locality::Local);
        assert_eq!(locality_of("http://localhost:8080/v1"), Locality::Local);
        assert_eq!(locality_of("http://[::1]:8080/v1"), Locality::Local);
        assert_eq!(locality_of("https://api.example.com/v1"), Locality::Cloud);
        assert_eq!(
            locality_of("https://localhost.evil.com/v1"),
            Locality::Cloud
        );
    }

    #[test]
    fn parses_sse() {
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"content":"4"}}]}"#).expect("ok"),
            vec![Chunk::Text("4".into())]
        );
        assert_eq!(
            parse_sse_line("data: [DONE]").expect("ok"),
            vec![Chunk::Done(Usage::default())]
        );
        assert!(parse_sse_line(": keep-alive").expect("ok").is_empty());
    }
}
