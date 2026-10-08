//! IPC error model. `message` is safe to show users; `details` is technical and shown only
//! behind "Show Technical Details" (spec §83). Neither may contain secrets.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    Internal,
    Unavailable,
    PermissionDenied,
    Cancelled,
    Timeout,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl IpcError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(ErrorCode::MethodNotFound, "That action isn't supported.")
            .with_details(format!("unknown method `{method}`"))
    }

    pub fn invalid_params(details: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidParams, "The request was malformed.").with_details(details)
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, "Something went wrong inside Wisp.").with_details(details)
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)?;
        if let Some(d) = &self.details {
            write!(f, " ({d})")?;
        }
        Ok(())
    }
}

impl std::error::Error for IpcError {}
