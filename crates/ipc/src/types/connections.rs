//! Connections to other services through MCP servers (Model Context Protocol): configuration,
//! per-tool rules, and live status.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ConnectionsSettings {
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerConfig {
    /// Short identifier (letters, digits, dashes), used in tool names ("notion/search").
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub transport: McpTransport,
    /// The user confirmed this server works only on this Mac (sends nothing elsewhere). Only such
    /// servers can be used in Local Only.
    #[serde(default)]
    pub local_only: bool,
    /// Per-tool rules; tools without one ask every time.
    #[serde(default)]
    pub tools: Vec<McpToolRule>,
}

/// How to reach a server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "config", rename_all = "camelCase")]
pub enum McpTransport {
    /// A program on this Mac, started by Wisp.
    Stdio(McpStdioConfig),
    /// A server on the web (Streamable HTTP).
    Http(McpHttpConfig),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStdioConfig {
    /// The program, e.g. "npx".
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<McpEnvVar>,
}

/// An environment variable for a local server. Secrets live in the Keychain, never in settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpEnvVar {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keychain_account: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpHttpConfig {
    /// https (or a local http address).
    pub url: String,
    /// Keychain account holding a bearer token, if the server needs one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keychain_account: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum McpRule {
    /// Ask every time (the default).
    Ask,
    /// Run without asking.
    Allow,
    /// Never run.
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolRule {
    pub tool: String,
    pub rule: McpRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum McpState {
    Off,
    Connecting,
    Ready,
    Error,
    /// Can't be used while privacy is Local Only.
    BlockedByPrivacy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub read_only: bool,
    pub destructive: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerStatus {
    pub id: String,
    pub state: McpState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// What the server calls itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    pub tools: Vec<McpToolInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStatusList {
    pub servers: Vec<McpServerStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerId {
    pub id: String,
}

impl ConnectionsSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.servers.len() > 20 {
            return Err("at most 20 connections".into());
        }
        let mut ids = std::collections::HashSet::new();
        for s in &self.servers {
            let id_ok = !s.id.is_empty()
                && s.id.len() <= 32
                && s.id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !id_ok || !ids.insert(s.id.as_str()) {
                return Err(format!(
                    "connection id {:?} must be unique: a-z, 0-9, -",
                    s.id
                ));
            }
            if s.name.trim().is_empty() || s.name.chars().count() > 60 {
                return Err("a connection needs a name (≤ 60 characters)".into());
            }
            match &s.transport {
                McpTransport::Stdio(c) => {
                    if c.command.trim().is_empty() || c.command.len() > 512 || c.args.len() > 50 {
                        return Err(format!("{}: invalid command", s.name));
                    }
                    if c.env.iter().any(|e| {
                        e.name.is_empty()
                            || !e
                                .name
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    }) {
                        return Err(format!("{}: invalid environment variable name", s.name));
                    }
                }
                McpTransport::Http(h) => {
                    let lower = h.url.to_ascii_lowercase();
                    let local = wisp_common_is_loopback(&lower);
                    if !(lower.starts_with("https://") || (lower.starts_with("http://") && local))
                        || h.url.len() > 2048
                    {
                        return Err(format!(
                            "{}: use an https:// address (http only for this Mac)",
                            s.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// `http://localhost…`, `http://127.0.0.1…`, `http://[::1]…` (kept local to avoid a dependency).
fn wisp_common_is_loopback(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = if host.starts_with('[') {
        host.split(']')
            .next()
            .map(|h| format!("{h}]"))
            .unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_owned()
    };
    matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]")
}
