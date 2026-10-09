//! Apple Foundation Models (spec §11). The model runs in the Swift host; this provider proxies
//! requests to `host.apple.generate` and receives streamed text as `apple.delta` events.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wisp_ai::{AiError, AiProvider, Chunk, ChunkStream, GenerateRequest, Role, Usage};
use wisp_ipc::types::models::{
    AppleStatus, Locality, ModelCapability, ModelInfo, ProviderInfo, ProviderKind, ProviderState,
};

use crate::HostBridge;

pub const ID: &str = "apple";
pub const MODEL: &str = "system";

/// Routes `apple.delta` events from the host to the request that is waiting for them.
#[derive(Clone, Default)]
pub struct AppleStreams {
    inner: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<String>>>>,
}

impl AppleStreams {
    pub fn deliver(&self, request_id: &str, delta: String) {
        if let Some(tx) = self
            .inner
            .lock()
            .ok()
            .and_then(|m| m.get(request_id).cloned())
        {
            let _ = tx.send(delta);
        }
    }

    fn register(&self, id: &str) -> mpsc::UnboundedReceiver<String> {
        let (tx, rx) = mpsc::unbounded_channel();
        if let Ok(mut m) = self.inner.lock() {
            m.insert(id.to_owned(), tx);
        }
        rx
    }

    fn remove(&self, id: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.remove(id);
        }
    }
}

pub struct AppleProvider {
    host: Arc<dyn HostBridge>,
    streams: AppleStreams,
}

impl AppleProvider {
    pub fn new(host: Arc<dyn HostBridge>, streams: AppleStreams) -> Self {
        Self { host, streams }
    }

    async fn status(&self) -> AppleStatus {
        match self
            .host
            .call("host.apple.status", json!({}))
            .await
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
        {
            Some(s) => s,
            None => AppleStatus {
                state: ProviderState::Unavailable,
                detail: Some("The Wisp app isn't connected.".into()),
            },
        }
    }
}

/// Apple's API takes instructions and a single prompt; render the conversation into it.
pub(crate) fn render_prompt(req: &GenerateRequest) -> (String, String) {
    let instructions = req.system.clone().unwrap_or_default();
    let mut prompt = String::new();
    let n = req.messages.len();
    for (i, m) in req.messages.iter().enumerate() {
        if i + 1 == n && m.role == Role::User {
            if n > 1 {
                prompt.push_str("\nUser: ");
            }
            prompt.push_str(&m.content);
        } else {
            let who = match m.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "Note",
            };
            if i == 0 {
                prompt.push_str("Conversation so far:\n");
            }
            prompt.push_str(&format!("{who}: {}\n", m.content));
        }
    }
    (instructions, prompt)
}

#[async_trait]
impl AiProvider for AppleProvider {
    fn id(&self) -> &str {
        ID
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Apple
    }

    fn locality(&self) -> Locality {
        Locality::Local
    }

    async fn info(&self) -> ProviderInfo {
        let s = self.status().await;
        ProviderInfo {
            id: ID.into(),
            kind: ProviderKind::Apple,
            name: "Apple Foundation Model".into(),
            locality: Locality::Local,
            state: s.state,
            detail: s.detail,
            version: None,
        }
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        if self.status().await.state != ProviderState::Available {
            return Ok(vec![]);
        }
        Ok(vec![ModelInfo {
            provider_id: ID.into(),
            id: MODEL.into(),
            name: "Apple on-device model".into(),
            locality: Locality::Local,
            capabilities: vec![
                ModelCapability::Text,
                ModelCapability::Streaming,
                ModelCapability::StructuredOutput,
            ],
            size_bytes: None,
            context_length: Some(4096),
            parameter_size: None,
            family: Some("apple".into()),
        }])
    }

    async fn generate(
        &self,
        req: GenerateRequest,
        cancel: CancellationToken,
    ) -> Result<ChunkStream, AiError> {
        if req.messages.iter().any(|m| !m.images.is_empty()) {
            return Err(AiError::NoModel(
                "Apple's on-device model can't see images.".into(),
            ));
        }
        let request_id = wisp_common::ids::new_id();
        let mut deltas = self.streams.register(&request_id);
        let (instructions, prompt) = render_prompt(&req);
        let host = self.host.clone();
        let params = json!({
            "requestId": request_id, "instructions": instructions, "prompt": prompt,
            "temperature": req.temperature, "maxTokens": req.max_tokens,
        });
        let (tx, rx) = mpsc::channel::<Result<Chunk, AiError>>(64);
        let streams = self.streams.clone();
        tokio::spawn(async move {
            let call = host.call("host.apple.generate", params);
            tokio::pin!(call);
            let outcome = loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        let _ = host.call("host.apple.cancel", json!({ "requestId": request_id })).await;
                        break Err(AiError::Cancelled);
                    }
                    Some(d) = deltas.recv() => {
                        if tx.send(Ok(Chunk::Text(d))).await.is_err() { break Err(AiError::Cancelled); }
                    }
                    r = &mut call => break r.map_err(|e| AiError::unavailable(e.message.clone(), e.details.unwrap_or_default())),
                }
            };
            // Flush deltas that arrived before the final response.
            while let Ok(d) = deltas.try_recv() {
                let _ = tx.send(Ok(Chunk::Text(d))).await;
            }
            streams.remove(&request_id);
            let _ = match outcome {
                Ok(_) => tx.send(Ok(Chunk::Done(Usage::default()))).await,
                Err(e) => tx.send(Err(e)).await,
            };
        });
        Ok(Box::pin(futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|x| (x, rx))
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_ai::ChatMessage;

    #[test]
    fn renders_conversation_into_prompt() {
        let req = GenerateRequest {
            model: MODEL.into(),
            system: Some("Be brief.".into()),
            messages: vec![
                ChatMessage::text(Role::User, "Hi"),
                ChatMessage::text(Role::Assistant, "Hello!"),
                ChatMessage::text(Role::User, "What is 2+2?"),
            ],
            temperature: 0.2,
            max_tokens: 64,
            context_length: 4096,
            think: false,
        };
        let (i, p) = render_prompt(&req);
        assert_eq!(i, "Be brief.");
        assert!(p.starts_with("Conversation so far:\nUser: Hi\nAssistant: Hello!\n"));
        assert!(p.ends_with("User: What is 2+2?"));
    }
}
