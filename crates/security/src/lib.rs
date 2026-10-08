//! Security utilities shared by every subsystem: redaction and path validation.
//!
//! Policy decisions (what a tool may do) live in `wisp-permissions`; this crate provides the
//! mechanical checks those policies rely on.

pub mod paths;
pub mod redact;
