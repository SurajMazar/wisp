//! Typed, versioned IPC contracts shared by React (Zod), Swift (Codable) and Rust (serde).
//!
//! Rust types in this crate are the single source of truth (DECISIONS.md D-003).
//! `cargo run -p wisp-ipc --bin contracts-gen` emits JSON Schema that is turned into
//! TypeScript and Swift code by `scripts/validation/gen-contracts.ts`.

pub mod envelope;
pub mod error;
pub mod methods;
pub mod schema;
pub mod types;

pub use envelope::{Envelope, FrameError, MAX_FRAME_BYTES, PROTOCOL_VERSION};
pub use error::{ErrorCode, IpcError};
pub use methods::{EventKind, Method, Target};
