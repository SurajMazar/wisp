//! Composition root of `wisp-core`: wires the database, event bus, dispatcher and transport.

pub mod agents;
pub mod ai;
pub mod background;
pub mod config;
pub mod connections;
pub mod context;
pub mod conversations;
pub mod dispatch;
pub mod events;
pub mod files;
pub mod logging;
pub mod memory;
pub mod prefill;
pub mod release;
pub mod routines;
pub mod skills;
pub mod teacher;
pub mod tools;
pub mod transport;
pub mod voice;

pub use config::Config;
pub use dispatch::Core;
