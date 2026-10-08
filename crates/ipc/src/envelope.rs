//! Wire envelope and line framing.
//!
//! Every message is one UTF-8 JSON object on a single line. The envelope is deliberately
//! generic (method name + params value); typed decoding of params/results happens against
//! the [`crate::Method`] registry so that unknown methods and malformed params are rejected
//! with precise errors instead of panicking.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::IpcError;

/// Current protocol version. Bump on any breaking contract change.
pub const PROTOCOL_VERSION: u32 = 1;

/// Hard cap on a single frame. Screenshots are cropped/compressed before crossing IPC.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Envelope {
    Request {
        v: u32,
        id: String,
        method: String,
        #[serde(default)]
        params: Value,
    },
    Response {
        v: u32,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<IpcError>,
    },
    Event {
        v: u32,
        event: String,
        #[serde(default)]
        payload: Value,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    TooLarge,
    #[error("malformed frame: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("unsupported protocol version {0} (expected {PROTOCOL_VERSION})")]
    Version(u32),
}

impl Envelope {
    pub fn request(id: impl Into<String>, method: &str, params: Value) -> Self {
        Self::Request {
            v: PROTOCOL_VERSION,
            id: id.into(),
            method: method.to_owned(),
            params,
        }
    }

    pub fn ok(id: impl Into<String>, result: Value) -> Self {
        Self::Response {
            v: PROTOCOL_VERSION,
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: impl Into<String>, error: IpcError) -> Self {
        Self::Response {
            v: PROTOCOL_VERSION,
            id: id.into(),
            result: None,
            error: Some(error),
        }
    }

    pub fn event(event: &str, payload: Value) -> Self {
        Self::Event {
            v: PROTOCOL_VERSION,
            event: event.to_owned(),
            payload,
        }
    }

    fn version(&self) -> u32 {
        match self {
            Self::Request { v, .. } | Self::Response { v, .. } | Self::Event { v, .. } => *v,
        }
    }

    /// Encode as a single line terminated by `\n`.
    pub fn encode(&self) -> Result<String, FrameError> {
        let mut s = serde_json::to_string(self)?;
        if s.len() + 1 > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
        s.push('\n');
        Ok(s)
    }

    /// Decode one line (without or with trailing newline) and check the protocol version.
    pub fn decode(line: &str) -> Result<Self, FrameError> {
        if line.len() > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
        let env: Envelope = serde_json::from_str(line.trim_end_matches(['\n', '\r']))?;
        match env.version() {
            PROTOCOL_VERSION => Ok(env),
            other => Err(FrameError::Version(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_every_kind() {
        for env in [
            Envelope::request("1", "settings.get", Value::Null),
            Envelope::ok("1", json!({"a": 1})),
            Envelope::err("1", IpcError::method_not_found("nope")),
            Envelope::event("core.ready", json!({})),
        ] {
            let line = env.encode().expect("encode");
            assert!(line.ends_with('\n') && !line[..line.len() - 1].contains('\n'));
            assert_eq!(Envelope::decode(&line).expect("decode"), env);
        }
    }

    #[test]
    fn rejects_wrong_version_and_unknown_fields() {
        assert!(matches!(
            Envelope::decode(r#"{"kind":"event","v":99,"event":"x","payload":null}"#),
            Err(FrameError::Version(99))
        ));
        assert!(
            Envelope::decode(r#"{"kind":"event","v":1,"event":"x","payload":null,"evil":1}"#)
                .is_err()
        );
        assert!(Envelope::decode("not json").is_err());
    }
}
