use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for methods that take none. Accepts `{}` or an absent/`null` params field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppInfo {
    pub core_version: String,
    pub protocol_version: u32,
    /// Highest applied database migration.
    pub schema_version: u32,
    pub data_dir: String,
    pub started_at_ms: i64,
}
