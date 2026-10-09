//! Deterministic provider for tests (spec §114). Never registered in production builds.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use wisp_ai::{AiError, AiProvider, Chunk, ChunkStream, GenerateRequest, Usage};
use wisp_ipc::types::models::{
    Locality, ModelCapability, ModelInfo, ProviderInfo, ProviderKind, ProviderState,
};

pub struct MockProvider {
    pub id: String,
    pub locality: Locality,
    pub vision: bool,
    pub reply: String,
}

#[async_trait]
impl AiProvider for MockProvider {
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
        ProviderInfo {
            id: self.id.clone(),
            kind: self.kind(),
            name: format!("Mock {}", self.id),
            locality: self.locality,
            state: ProviderState::Available,
            detail: None,
            version: None,
        }
    }
    async fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        let mut capabilities = vec![ModelCapability::Text, ModelCapability::Streaming];
        if self.vision {
            capabilities.push(ModelCapability::Vision);
        }
        Ok(vec![ModelInfo {
            provider_id: self.id.clone(),
            id: "mock".into(),
            name: "mock".into(),
            locality: self.locality,
            capabilities,
            size_bytes: None,
            context_length: Some(8192),
            parameter_size: None,
            family: None,
        }])
    }
    async fn generate(
        &self,
        req: GenerateRequest,
        cancel: CancellationToken,
    ) -> Result<ChunkStream, AiError> {
        // "<echo>" replies with the prompt's last message, so tests can inspect the context.
        let source = if self.reply == "<echo>" {
            req.messages
                .last()
                .map(|m| m.content.clone())
                .unwrap_or_default()
        } else {
            self.reply.clone()
        };
        // `<think>…</think>` at the start of the reply is streamed as reasoning.
        let (reasoning, reply) = match source
            .strip_prefix("<think>")
            .and_then(|r| r.split_once("</think>"))
        {
            Some((r, rest)) => (Some(r.to_owned()), rest.trim_start().to_owned()),
            None => (None, source.clone()),
        };
        let saw_image = req.messages.iter().any(|m| !m.images.is_empty());
        let mut items: Vec<Chunk> = reasoning.into_iter().map(Chunk::Reasoning).collect();
        for (i, w) in reply.split_inclusive(' ').enumerate() {
            items.push(Chunk::Text(if i == 0 && saw_image {
                format!("[saw image] {w}")
            } else {
                w.to_owned()
            }));
        }
        let words = items.iter().filter(|c| matches!(c, Chunk::Text(_))).count() as u32;
        items.push(Chunk::Done(Usage {
            prompt_tokens: Some(1),
            completion_tokens: Some(words),
        }));
        Ok(Box::pin(futures::stream::unfold(
            (items.into_iter(), cancel, false),
            |(mut items, cancel, stopped)| async move {
                if stopped {
                    return None;
                }
                if cancel.is_cancelled() {
                    return Some((Err(AiError::Cancelled), (items, cancel, true)));
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                let c = items.next()?;
                Some((Ok(c), (items, cancel, false)))
            },
        )))
    }
}
