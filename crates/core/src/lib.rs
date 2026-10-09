//! Composition root of `wisp-core`: wires the database, event bus, dispatcher and transport.

pub mod agents;
pub mod ai;
pub mod config;
pub mod context;
pub mod conversations;
pub mod dispatch;
pub mod events;
pub mod logging;
pub mod memory;
pub mod skills;
pub mod teacher;
pub mod tools;
pub mod transport;
pub mod voice;

pub use config::Config;
pub use dispatch::Core;
