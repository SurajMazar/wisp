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
        let words: Vec<String> = self.reply.split_inclusive(' ').map(str::to_owned).collect();
        let saw_image = req.messages.iter().any(|m| !m.images.is_empty());
        Ok(Box::pin(futures::stream::unfold(
            (words, 0usize, cancel, saw_image),
            |(words, i, cancel, saw)| async move {
                if cancel.is_cancelled() {
                    return if i == usize::MAX {
                        None
                    } else {
                        Some((Err(AiError::Cancelled), (words, usize::MAX, cancel, saw)))
                    };
                }
                if i == usize::MAX {
                    return None;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                if i < words.len() {
                    let w = if i == 0 && saw {
                        format!("[saw image] {}", words[i])
                    } else {
                        words[i].clone()
                    };
                    Some((Ok(Chunk::Text(w)), (words, i + 1, cancel, saw)))
                } else {
                    Some((
                        Ok(Chunk::Done(Usage {
                            prompt_tokens: Some(1),
                            completion_tokens: Some(words.len() as u32),
                        })),
                        (words, usize::MAX, cancel, saw),
                    ))
                }
            },
        )))
    }
}
