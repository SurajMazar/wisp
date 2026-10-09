//! Release support (spec §117): backups, data export, update checks.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupInfo {
    pub name: String,
    pub size_bytes: u64,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupList {
    pub backups: Vec<BackupInfo>,
    /// Where backups are kept.
    pub folder: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportResult {
    /// The export file (JSON).
    pub path: String,
    pub conversations: u32,
    pub memories: u32,
    pub skills: u32,
    pub agents: u32,
    pub routines: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateInfo {
    pub current: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    pub update_available: bool,
    /// Release page to download from (never installed automatically).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}
