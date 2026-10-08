//! Composition root of `wisp-core`: wires the database, event bus, dispatcher and transport.

pub mod config;
pub mod dispatch;
pub mod events;
pub mod logging;
pub mod transport;

pub use config::Config;
pub use dispatch::Core;
