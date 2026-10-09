use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use tokio_util::sync::CancellationToken;
use wisp_ipc::{
    ErrorCode, IpcError,
    types::models::{Locality, ModelInfo, ProviderInfo, ProviderKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePart {
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub images: Vec<ImagePart>,
}

impl ChatMessage {
    pub fn text(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            images: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: f64,
    pub max_tokens: u32,
    pub context_length: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Chunk {
    Text(String),
    /// The model's visible reasoning ("thinking"), streamed before the answer.
    Reasoning(String),
    Done(Usage),
}

pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<Chunk, AiError>> + Send>>;

/// Errors carry a user-facing sentence plus technical detail (spec §83).
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum AiError {
    #[error("{message}")]
    Unavailable { message: String, detail: String },
    #[error("{0}")]
    NoModel(String),
    #[error("{0}")]
    PrivacyBlocked(String),
    #[error("The model returned something unexpected.")]
    Protocol(String),
    #[error("Cancelled.")]
    Cancelled,
}

impl AiError {
    pub fn unavailable(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Unavailable {
            message: message.into(),
            detail: detail.into(),
        }
    }
}

impl From<AiError> for IpcError {
    fn from(e: AiError) -> Self {
        match &e {
            AiError::Unavailable { message, detail } => {
                IpcError::new(ErrorCode::Unavailable, message.clone()).with_details(detail.clone())
            }
            AiError::NoModel(m) => IpcError::new(ErrorCode::NotFound, m.clone()),
            AiError::PrivacyBlocked(m) => IpcError::new(ErrorCode::PermissionDenied, m.clone()),
            AiError::Protocol(d) => {
                IpcError::new(ErrorCode::Internal, e.to_string()).with_details(d.clone())
            }
            AiError::Cancelled => IpcError::new(ErrorCode::Cancelled, "Cancelled."),
        }
    }
}

/// A source of models (spec §10). Implementations must never fake availability.
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> ProviderKind;
    fn locality(&self) -> Locality;
    /// Live status — probes the backend.
    async fn info(&self) -> ProviderInfo;
    async fn models(&self) -> Result<Vec<ModelInfo>, AiError>;
    /// Stream a response. Dropping the stream or cancelling the token stops generation.
    async fn generate(
        &self,
        req: GenerateRequest,
        cancel: CancellationToken,
    ) -> Result<ChunkStream, AiError>;
    /// Whether the model is already in memory (`None` = unknown). Lets the UI say
    /// "Loading the model…" instead of an unexplained wait.
    async fn is_loaded(&self, _model: &str) -> Option<bool> {
        None
    }
}
