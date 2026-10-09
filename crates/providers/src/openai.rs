//! OpenAI-compatible chat completions (LM Studio, llama.cpp server, vLLM, or a cloud API).
//! Locality is derived from the endpoint address: loopback is local, anything else is cloud.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::Engine;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use wisp_ai::{AiError, AiProvider, Chunk, ChunkStream, GenerateRequest, Role, Usage};
use wisp_ipc::types::models::{
    EndpointConfig, Locality, ModelCapability, ModelInfo, ProviderInfo, ProviderKind, ProviderState,
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
    let rest = base_url.split("://").nth(1).unwrap_or("");
    let host = rest.split(['/', ':']).next().unwrap_or("");
    if matches!(host, "127.0.0.1" | "localhost") || rest.starts_with("[::1]") {
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
    Ok(v.pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .map(|t| vec![Chunk::Text(t.to_owned())])
        .unwrap_or_default())
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
            .filter_map(|m| m.get("id").and_then(Value::as_str))
            .map(|id| ModelInfo {
                provider_id: self.id.clone(),
                id: id.to_owned(),
                name: id.to_owned(),
                locality: self.locality,
                // The models endpoint doesn't report modalities; images are attempted only when the
                // user explicitly picks this model for vision.
                capabilities: vec![ModelCapability::Text, ModelCapability::Streaming],
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
        let body = json!({ "model": req.model, "messages": messages, "stream": true, "temperature": req.temperature, "max_tokens": req.max_tokens });
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
