//! Ollama (spec §12): detection, model discovery with real capabilities, streaming chat with
//! images, model installation and removal.

use std::{collections::HashMap, path::Path, sync::Mutex, time::Duration};

use async_trait::async_trait;
use base64::Engine;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use wisp_ai::{AiError, AiProvider, Chunk, ChunkStream, GenerateRequest, Role, Usage};
use wisp_ipc::types::models::{
    Locality, ModelCapability, ModelInfo, ProviderInfo, ProviderKind, ProviderState, PullProgress,
};

use crate::stream::lines_to_chunks;

pub const ID: &str = "ollama";

/// Capabilities and context length reported by `/api/show`.
type ShowInfo = (Vec<ModelCapability>, Option<u32>);

pub struct OllamaProvider {
    base: String,
    client: reqwest::Client,
    probe: reqwest::Client,
    show_cache: Mutex<HashMap<String, ShowInfo>>,
}

#[derive(Deserialize)]
struct Tags {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    digest: String,
    #[serde(default)]
    details: Option<TagDetails>,
}

#[derive(Deserialize, Default)]
struct TagDetails {
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    parameter_size: Option<String>,
}

impl OllamaProvider {
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into().trim_end_matches('/').to_owned();
        Self {
            base,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .build()
                .unwrap_or_default(),
            probe: reqwest::Client::builder()
                .timeout(Duration::from_millis(1500))
                .build()
                .unwrap_or_default(),
            show_cache: Mutex::new(HashMap::new()),
        }
    }

    fn not_running() -> AiError {
        AiError::unavailable(
            "I couldn't connect to Ollama. Make sure Ollama is running.",
            "connection to the Ollama server failed",
        )
    }

    fn installed() -> bool {
        [
            "/opt/homebrew/bin/ollama",
            "/usr/local/bin/ollama",
            "/Applications/Ollama.app",
        ]
        .iter()
        .any(|p| Path::new(p).exists())
    }

    async fn show(&self, name: &str, digest: &str) -> ShowInfo {
        let key = format!("{name}@{digest}");
        if let Some(hit) = self
            .show_cache
            .lock()
            .ok()
            .and_then(|c| c.get(&key).cloned())
        {
            return hit;
        }
        let parsed = async {
            let v: Value = self
                .client
                .post(format!("{}/api/show", self.base))
                .json(&json!({ "model": name }))
                .send()
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            Some(parse_show(&v))
        }
        .await
        .unwrap_or((
            vec![ModelCapability::Text, ModelCapability::Streaming],
            None,
        ));
        if let Ok(mut c) = self.show_cache.lock() {
            c.insert(key, parsed.clone());
        }
        parsed
    }

    /// Download a model, reporting progress. Cancellable.
    pub async fn pull(
        &self,
        name: &str,
        cancel: CancellationToken,
        mut progress: impl FnMut(PullProgress) + Send,
    ) -> Result<(), AiError> {
        validate_model_name(name)?;
        let resp = self
            .client
            .post(format!("{}/api/pull", self.base))
            .json(&json!({ "model": name, "stream": true }))
            .send()
            .await
            .map_err(|_| Self::not_running())?;
        if !resp.status().is_success() {
            return Err(AiError::unavailable(
                "Ollama couldn't download that model.",
                format!("HTTP {}", resp.status()),
            ));
        }
        let mut bytes = resp.bytes_stream();
        let mut buf = Vec::new();
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => return Err(AiError::Cancelled),
                n = bytes.next() => n,
            };
            let Some(chunk) = next else { break };
            buf.extend_from_slice(&chunk.map_err(|e| {
                AiError::unavailable("The download was interrupted.", e.to_string())
            })?);
            while let Some(pos) = buf.iter().position(|&c| c == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let Ok(v) = serde_json::from_slice::<Value>(&line) else {
                    continue;
                };
                if let Some(err) = v.get("error").and_then(Value::as_str) {
                    return Err(AiError::unavailable(
                        format!("Ollama couldn't download {name}."),
                        err.to_owned(),
                    ));
                }
                let status = v
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let done = status == "success";
                progress(PullProgress {
                    name: name.to_owned(),
                    status,
                    completed: v.get("completed").and_then(Value::as_u64),
                    total: v.get("total").and_then(Value::as_u64),
                    done,
                    error: None,
                });
            }
        }
        if let Ok(mut c) = self.show_cache.lock() {
            c.retain(|k, _| !k.starts_with(&format!("{name}@")));
        }
        Ok(())
    }

    pub async fn delete(&self, name: &str) -> Result<(), AiError> {
        validate_model_name(name)?;
        let resp = self
            .client
            .delete(format!("{}/api/delete", self.base))
            .json(&json!({ "model": name }))
            .send()
            .await
            .map_err(|_| Self::not_running())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AiError::unavailable(
                "Ollama couldn't remove that model.",
                format!("HTTP {}", resp.status()),
            ))
        }
    }
}

/// Entries Ollama lists for its own bookkeeping rather than as user-facing models.
pub fn is_internal_alias(name: &str) -> bool {
    name.starts_with("llamacpp:") || name.starts_with("sha256-") || name.starts_with("sha256:")
}

/// Ollama model names: `name[:tag]` with a conservative character set (no paths, no URLs).
pub fn validate_model_name(name: &str) -> Result<(), AiError> {
    let ok = !name.is_empty()
        && name.len() <= 120
        && !name.contains("..")
        && !name.contains("//")
        && !name.starts_with('/')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_' | '/'));
    if ok {
        Ok(())
    } else {
        Err(AiError::NoModel(format!(
            "“{name}” isn't a valid model name."
        )))
    }
}

pub(crate) fn parse_show(v: &Value) -> ShowInfo {
    let mut caps = vec![ModelCapability::Streaming];
    for c in v
        .get("capabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        match c {
            "completion" => caps.push(ModelCapability::Text),
            "vision" => caps.push(ModelCapability::Vision),
            "tools" => caps.push(ModelCapability::ToolCalling),
            "thinking" => caps.push(ModelCapability::Reasoning),
            _ => {}
        }
    }
    if !caps.contains(&ModelCapability::Text) && v.get("capabilities").is_none() {
        caps.push(ModelCapability::Text); // older Ollama without capability reporting
    }
    let ctx = v
        .get("model_info")
        .and_then(Value::as_object)
        .and_then(|o| {
            o.iter()
                .find(|(k, _)| k.ends_with(".context_length"))
                .and_then(|(_, v)| v.as_u64())
        })
        .map(|c| c.min(u32::MAX as u64) as u32);
    (caps, ctx)
}

pub(crate) fn parse_chat_line(line: &str) -> Result<Vec<Chunk>, AiError> {
    let v: Value = serde_json::from_str(line).map_err(|e| AiError::Protocol(e.to_string()))?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(AiError::unavailable(
            "The model couldn't answer.",
            err.to_owned(),
        ));
    }
    let mut out = vec![];
    if let Some(t) = v
        .pointer("/message/thinking")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
    {
        out.push(Chunk::Reasoning(t.to_owned()));
    }
    if let Some(t) = v
        .pointer("/message/content")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
    {
        out.push(Chunk::Text(t.to_owned()));
    }
    if v.get("done").and_then(Value::as_bool) == Some(true) {
        out.push(Chunk::Done(Usage {
            prompt_tokens: v
                .get("prompt_eval_count")
                .and_then(Value::as_u64)
                .map(|n| n as u32),
            completion_tokens: v
                .get("eval_count")
                .and_then(Value::as_u64)
                .map(|n| n as u32),
        }));
    }
    Ok(out)
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn id(&self) -> &str {
        ID
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    fn locality(&self) -> Locality {
        Locality::Local
    }

    async fn info(&self) -> ProviderInfo {
        let version = async {
            let v: Value = self
                .probe
                .get(format!("{}/api/version", self.base))
                .send()
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            v.get("version").and_then(Value::as_str).map(str::to_owned)
        }
        .await;
        let (state, detail) = match &version {
            Some(_) => (ProviderState::Available, None),
            None if Self::installed() => (
                ProviderState::NotRunning,
                Some("Ollama is installed but not running.".to_owned()),
            ),
            None => (
                ProviderState::NotInstalled,
                Some("Ollama isn't installed. Get it from ollama.com.".to_owned()),
            ),
        };
        ProviderInfo {
            id: ID.into(),
            kind: ProviderKind::Ollama,
            name: "Ollama".into(),
            locality: Locality::Local,
            state,
            detail,
            version,
        }
    }

    async fn is_loaded(&self, model: &str) -> Option<bool> {
        let v: Value = self
            .client
            .get(format!("{}/api/ps", self.base))
            .timeout(std::time::Duration::from_millis(400))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let loaded = v.get("models")?.as_array()?.iter().any(|m| {
            m.get("name").and_then(Value::as_str) == Some(model)
                || m.get("model").and_then(Value::as_str) == Some(model)
        });
        Some(loaded)
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        let tags: Tags = self
            .probe
            .get(format!("{}/api/tags", self.base))
            .send()
            .await
            .map_err(|_| Self::not_running())?
            .json()
            .await
            .map_err(|e| AiError::Protocol(e.to_string()))?;
        // Ollama can list internal aliases (`llamacpp:<digest>`, `sha256-…`) and duplicate names;
        // show each real model once.
        let mut seen = std::collections::HashSet::new();
        let tags = Tags {
            models: tags
                .models
                .into_iter()
                .filter(|m| !is_internal_alias(&m.name))
                .filter(|m| seen.insert(m.name.clone()))
                .collect(),
        };
        let shows =
            futures::future::join_all(tags.models.iter().map(|m| self.show(&m.name, &m.digest)))
                .await;
        Ok(tags
            .models
            .into_iter()
            .zip(shows)
            .map(|(m, (capabilities, context_length))| {
                let details = m.details.unwrap_or_default();
                ModelInfo {
                    provider_id: ID.into(),
                    id: m.name.clone(),
                    name: m.name,
                    locality: Locality::Local,
                    capabilities,
                    size_bytes: m.size,
                    context_length,
                    parameter_size: details.parameter_size,
                    family: details.family,
                }
            })
            .collect())
    }

    async fn generate(
        &self,
        req: GenerateRequest,
        cancel: CancellationToken,
    ) -> Result<ChunkStream, AiError> {
        validate_model_name(&req.model)?;
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
            let mut msg = json!({ "role": role, "content": m.content });
            if !m.images.is_empty() {
                msg["images"] = Value::Array(
                    m.images
                        .iter()
                        .map(|i| Value::String(b64.encode(&i.data)))
                        .collect(),
                );
            }
            messages.push(msg);
        }
        let body = json!({
            "model": req.model,
            "messages": messages,
            "stream": true,
            "think": false,
            "keep_alive": "15m",
            "options": { "temperature": req.temperature, "num_predict": req.max_tokens, "num_ctx": req.context_length },
        });
        let resp = self
            .client
            .post(format!("{}/api/chat", self.base))
            .json(&body)
            .send()
            .await
            .map_err(|_| Self::not_running())?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if text.contains("think") && text.contains("not support") {
                // Model doesn't accept the `think` flag; retry without it.
                let mut body = body;
                if let Some(o) = body.as_object_mut() {
                    o.remove("think");
                }
                let resp = self
                    .client
                    .post(format!("{}/api/chat", self.base))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|_| Self::not_running())?;
                return Ok(lines_to_chunks(resp, cancel, parse_chat_line));
            }
            return Err(AiError::unavailable(
                "The model couldn't answer.",
                format!("HTTP {status}: {text}"),
            ));
        }
        Ok(lines_to_chunks(resp, cancel, parse_chat_line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_capabilities_and_context_from_show() {
        let v = json!({
            "capabilities": ["completion", "vision"],
            "model_info": { "qwen3vl.context_length": 262144, "general.architecture": "qwen3vl" }
        });
        let (caps, ctx) = parse_show(&v);
        assert!(caps.contains(&ModelCapability::Text) && caps.contains(&ModelCapability::Vision));
        assert_eq!(ctx, Some(262144));
    }

    #[test]
    fn parses_chat_stream_lines() {
        assert_eq!(
            parse_chat_line(r#"{"message":{"content":"Hi"},"done":false}"#).expect("ok"),
            vec![Chunk::Text("Hi".into())]
        );
        let done = parse_chat_line(
            r#"{"message":{"content":""},"done":true,"prompt_eval_count":12,"eval_count":3}"#,
        )
        .expect("ok");
        assert_eq!(
            done,
            vec![Chunk::Done(Usage {
                prompt_tokens: Some(12),
                completion_tokens: Some(3)
            })]
        );
        assert!(parse_chat_line(r#"{"error":"model not found"}"#).is_err());
        assert_eq!(
            parse_chat_line(r#"{"message":{"content":"","thinking":"Hmm"},"done":false}"#)
                .expect("ok"),
            vec![Chunk::Reasoning("Hmm".into())]
        );
    }

    #[test]
    fn hides_internal_aliases() {
        assert!(is_internal_alias("llamacpp:e707b2c67b41"));
        assert!(is_internal_alias("sha256-e707b2c67b41:latest"));
        assert!(!is_internal_alias("qwen3-vl:8b-instruct"));
    }

    #[test]
    fn rejects_hostile_model_names() {
        for bad in ["", "../etc/passwd", "/abs", "http://x", "a b", "x;rm -rf"] {
            assert!(validate_model_name(bad).is_err(), "{bad}");
        }
        for good in ["qwen3-vl:8b", "library/llama3.2:3b", "gemma3:4b-it-qat"] {
            assert!(validate_model_name(good).is_ok(), "{good}");
        }
    }
}
