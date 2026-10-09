//! Composition root of `wisp-core`: wires the database, event bus, dispatcher and transport.

pub mod ai;
pub mod config;
pub mod context;
pub mod conversations;
pub mod dispatch;
pub mod events;
pub mod logging;
pub mod tools;
pub mod transport;
pub mod voice;

pub use config::Config;
pub use dispatch::Core;
