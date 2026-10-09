//! Files & local semantic search (spec §68–69). Documents are copied into Wisp's data folder,
//! read and indexed on this Mac, and cited in answers. Never uploaded in Local Only.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Pending,
    Indexing,
    Indexed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileItem {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size_bytes: u64,
    pub status: FileStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub chunks: u32,
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileList {
    pub items: Vec<FileItem>,
}

/// A dropped file's bytes (drag and drop), base64.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileImportData {
    pub name: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileSearchParams {
    pub query: String,
    /// 1–20.
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileHit {
    pub file_id: String,
    pub name: String,
    /// e.g. "report.pdf, page 3".
    pub citation: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileSearchResults {
    pub hits: Vec<FileHit>,
}

/// A source an answer drew on (shown under the answer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Citation {
    /// The [n] used in the answer.
    pub n: u32,
    pub file_id: String,
    pub label: String,
}

/// Files the host copied into Wisp's data folder after the user picked them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StagedFile {
    /// Relative to the data folder (always under `files/`).
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StagedFiles {
    pub files: Vec<StagedFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePathParams {
    /// Relative to the data folder (under `files/`).
    pub path: String,
}

/// Text the host read from a rich file, one entry per page (PDF) or a single entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtractedText {
    pub pages: Vec<String>,
    /// True when pages are real PDF pages (for "page N" citations).
    pub paged: bool,
}
