//! Provider implementations behind `wisp_ai::AiProvider`.

pub mod apple;
pub mod mock;
pub mod ollama;
pub mod openai;
mod stream;

use async_trait::async_trait;
use serde_json::Value;
use wisp_ipc::IpcError;

/// Calls into the macOS host (Apple Foundation Models, Keychain). Implemented by the core's transport.
#[async_trait]
pub trait HostBridge: Send + Sync {
    async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError>;
}
