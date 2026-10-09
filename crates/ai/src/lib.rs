//! AI orchestration primitives: the provider interface, generation types and the model router.
//!
//! Provider implementations live in `wisp-providers`; this crate knows nothing about any specific
//! provider (architectural rule: no provider logic in the agent runtime or UI).

pub mod provider;
pub mod router;

pub use provider::{
    AiError, AiProvider, ChatMessage, Chunk, ChunkStream, GenerateRequest, ImagePart, Role, Usage,
};
pub use router::{RouteInput, route};
