//! Connected services (MCP servers, spec plan Step 4): keeps one live session per enabled server,
//! in line with settings and privacy, and calls their tools for agents.
//!
//! * Servers are only what the user configured; nothing the model says can add one.
//! * Secrets (env values, tokens) come from the Keychain at connect time and are never stored.
//! * In Local Only, only servers on this Mac the user marked as local are used.
//! * What a tool returns is fenced as data before a model sees it.

use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex as AsyncMutex;
use wisp_ipc::{
    IpcError,
    types::{
        connections::{
            McpRule, McpServerConfig, McpServerStatus, McpState, McpStatusList, McpToolInfo,
            McpTransport,
        },
        settings::PrivacyMode,
        tools::{McpCallArgs, ToolExecution, Verification, VerificationStatus},
    },
};
use wisp_mcp::McpSession;

use crate::Core;

#[derive(Default)]
pub struct Connections {
    live: AsyncMutex<HashMap<String, Live>>,
    status: std::sync::Mutex<HashMap<String, McpServerStatus>>,
}

struct Live {
    /// The configuration it was started with; a change reconnects.
    fingerprint: String,
    session: Arc<McpSession>,
}

fn fingerprint(c: &McpServerConfig) -> String {
    serde_json::to_string(&c.transport).unwrap_or_default()
}

/// Whether this server may be used under `privacy`.
fn allowed(c: &McpServerConfig, privacy: PrivacyMode) -> bool {
    if privacy != PrivacyMode::LocalOnly {
        return true;
    }
    match &c.transport {
        McpTransport::Stdio(_) => c.local_only,
        McpTransport::Http(h) => wisp_common::net::is_loopback_url(&h.url) && c.local_only,
    }
}

fn info(t: &wisp_mcp::McpTool) -> McpToolInfo {
    McpToolInfo {
        name: t.name.clone(),
        title: t.title.clone(),
        description: t.description.clone(),
        read_only: t.read_only,
        destructive: t.destructive,
    }
}

impl Core {
    fn connections(&self) -> &Connections {
        self.connections_state()
    }

    fn set_status(&self, s: McpServerStatus) {
        if let Ok(mut m) = self.connections().status.lock() {
            m.insert(s.id.clone(), s.clone());
        }
        self.events()
            .publish::<wisp_ipc::methods::McpStatusChanged>(&s);
    }

    /// Bring live connections in line with settings (in the background).
    pub(crate) fn sync_connections(&self) {
        let core = self.clone();
        tokio::spawn(async move {
            let Ok(settings) = core.blocking(|db| db.load_settings()).await else {
                return;
            };
            let wanted = settings.connections.servers;
            let privacy = settings.privacy.mode;
            // Close what's gone, disabled, changed or no longer allowed.
            {
                let mut live = core.connections().live.lock().await;
                let ids: Vec<String> = live.keys().cloned().collect();
                for id in ids {
                    let keep = wanted.iter().any(|c| {
                        c.id == id
                            && c.enabled
                            && allowed(c, privacy)
                            && live
                                .get(&id)
                                .is_some_and(|l| l.fingerprint == fingerprint(c))
                    });
                    if !keep && let Some(l) = live.remove(&id) {
                        l.session.close().await;
                    }
                }
            }
            if let Ok(mut m) = core.connections().status.lock() {
                m.retain(|id, _| wanted.iter().any(|c| &c.id == id));
            }
            for c in wanted {
                let off = |state: McpState, error: Option<String>| McpServerStatus {
                    id: c.id.clone(),
                    state,
                    error,
                    server_name: None,
                    tools: vec![],
                };
                if !c.enabled {
                    core.set_status(off(McpState::Off, None));
                } else if !allowed(&c, privacy) {
                    core.set_status(off(
                        McpState::BlockedByPrivacy,
                        Some("Privacy is set to Local Only.".into()),
                    ));
                } else if !core.connections().live.lock().await.contains_key(&c.id) {
                    let core = core.clone();
                    tokio::spawn(async move {
                        core.connect(&c).await;
                    });
                }
            }
        });
    }

    /// Start (or restart) one server and record what it offers.
    async fn connect(&self, c: &McpServerConfig) -> McpServerStatus {
        self.set_status(McpServerStatus {
            id: c.id.clone(),
            state: McpState::Connecting,
            error: None,
            server_name: None,
            tools: vec![],
        });
        let result = match &c.transport {
            McpTransport::Stdio(s) => {
                let mut env = vec![];
                for v in &s.env {
                    let value = match (&v.value, &v.keychain_account) {
                        (_, Some(account)) => self.secret(account).await,
                        (Some(value), None) => Some(value.clone()),
                        (None, None) => None,
                    };
                    if let Some(value) = value {
                        env.push((v.name.clone(), value));
                    }
                }
                McpSession::connect_stdio(&s.command, &s.args, &env).await
            }
            McpTransport::Http(h) => {
                let token = match &h.keychain_account {
                    Some(a) => self.secret(a).await,
                    None => None,
                };
                McpSession::connect_http(&h.url, token).await
            }
        };
        let status = match result {
            Ok(session) => {
                let status = McpServerStatus {
                    id: c.id.clone(),
                    state: McpState::Ready,
                    error: None,
                    server_name: session.server_name.clone(),
                    tools: session.tools.iter().map(info).collect(),
                };
                let old = self.connections().live.lock().await.insert(
                    c.id.clone(),
                    Live {
                        fingerprint: fingerprint(c),
                        session: Arc::new(session),
                    },
                );
                if let Some(old) = old {
                    old.session.close().await;
                }
                tracing::info!(server = %c.id, tools = status.tools.len(), "connected");
                status
            }
            Err(e) => {
                tracing::warn!(server = %c.id, error = %e, "connection failed");
                McpServerStatus {
                    id: c.id.clone(),
                    state: McpState::Error,
                    error: Some(e.to_string()),
                    server_name: None,
                    tools: vec![],
                }
            }
        };
        self.set_status(status.clone());
        status
    }

    async fn secret(&self, account: &str) -> Option<String> {
        let host = self.ai().host()?;
        host.call(
            "host.keychain.get",
            serde_json::json!({ "account": account }),
        )
        .await
        .ok()?
        .get("secret")?
        .as_str()
        .map(str::to_owned)
    }

    pub(crate) async fn mcp_status(&self) -> McpStatusList {
        let settings = self.blocking(|db| db.load_settings()).await.ok();
        let status = self
            .connections()
            .status
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        McpStatusList {
            servers: settings
                .map(|s| s.connections.servers)
                .unwrap_or_default()
                .into_iter()
                .map(|c| {
                    status.get(&c.id).cloned().unwrap_or(McpServerStatus {
                        id: c.id,
                        state: McpState::Off,
                        error: None,
                        server_name: None,
                        tools: vec![],
                    })
                })
                .collect(),
        }
    }

    pub(crate) async fn mcp_reconnect(&self, id: &str) -> Result<McpServerStatus, IpcError> {
        let settings = self.blocking(|db| db.load_settings()).await?;
        let c = settings
            .connections
            .servers
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| IpcError::new(wisp_ipc::ErrorCode::NotFound, "No such connection."))?;
        if !c.enabled || !allowed(&c, settings.privacy.mode) {
            return Err(IpcError::new(
                wisp_ipc::ErrorCode::PermissionDenied,
                "That connection is turned off or not allowed in Local Only.",
            ));
        }
        Ok(self.connect(&c).await)
    }

    /// What the user decided for this tool (Ask when they haven't).
    pub(crate) fn mcp_rule(
        settings: &wisp_ipc::types::settings::AppSettings,
        server: &str,
        tool: &str,
    ) -> McpRule {
        settings
            .connections
            .servers
            .iter()
            .find(|s| s.id == server)
            .and_then(|s| s.tools.iter().find(|t| t.tool == tool))
            .map_or(McpRule::Ask, |t| t.rule)
    }

    /// The server's description of a tool (for risk and prompts).
    pub(crate) fn mcp_tool(&self, server: &str, tool: &str) -> Option<McpToolInfo> {
        self.connections()
            .status
            .lock()
            .ok()?
            .get(server)?
            .tools
            .iter()
            .find(|t| t.name == tool)
            .cloned()
    }

    pub(crate) async fn mcp_call(&self, a: &McpCallArgs) -> Result<ToolExecution, IpcError> {
        let settings = self.blocking(|db| db.load_settings()).await?;
        let config = settings
            .connections
            .servers
            .iter()
            .find(|c| c.id == a.server)
            .ok_or_else(|| {
                IpcError::invalid_params(format!("there's no connection called {}", a.server))
            })?;
        if !config.enabled || !allowed(config, settings.privacy.mode) {
            return Err(IpcError::new(
                wisp_ipc::ErrorCode::PermissionDenied,
                format!(
                    "{} is turned off or not allowed in Local Only.",
                    config.name
                ),
            ));
        }
        let live = self
            .connections()
            .live
            .lock()
            .await
            .get(&a.server)
            .map(|l| l.session.clone());
        let session = match live {
            Some(s) => s,
            None => {
                let status = self.connect(config).await;
                if status.state != McpState::Ready {
                    return Err(IpcError::new(
                        wisp_ipc::ErrorCode::Unavailable,
                        format!(
                            "{} isn't connected: {}",
                            config.name,
                            status.error.unwrap_or_default()
                        ),
                    ));
                }
                self.connections()
                    .live
                    .lock()
                    .await
                    .get(&a.server)
                    .map(|l| l.session.clone())
                    .ok_or_else(|| IpcError::internal("connection vanished"))?
            }
        };
        if !session.tools.iter().any(|t| t.name == a.tool) {
            return Err(IpcError::invalid_params(format!(
                "{} has no tool called {}",
                config.name, a.tool
            )));
        }
        let arguments = if a.arguments.is_null() {
            serde_json::json!({})
        } else {
            a.arguments.clone()
        };
        let out = session.call_tool(&a.tool, arguments).await.map_err(|e| {
            IpcError::new(
                wisp_ipc::ErrorCode::Unavailable,
                format!("{}: {e}", config.name),
            )
        })?;
        Ok(ToolExecution {
            // What a service returns is data, never instructions.
            output: Some(format!("\"\"\"\n{}\n\"\"\"", out.text)),
            verification: Verification {
                status: if out.is_error {
                    VerificationStatus::Failed
                } else {
                    VerificationStatus::Verified
                },
                detail: if out.is_error {
                    out.text.chars().take(300).collect()
                } else {
                    format!("{} answered", config.name)
                },
            },
        })
    }

    /// The connected tools an agent may use, for its prompt (blocked tools left out).
    pub(crate) async fn mcp_prompt_section(&self) -> Option<String> {
        let settings = self.blocking(|db| db.load_settings()).await.ok()?;
        let status = self.connections().status.lock().ok()?.clone();
        let mut lines = vec![];
        for c in &settings.connections.servers {
            let Some(s) = status.get(&c.id).filter(|s| s.state == McpState::Ready) else {
                continue;
            };
            for t in &s.tools {
                if Self::mcp_rule(&settings, &c.id, &t.name) == McpRule::Block {
                    continue;
                }
                let desc: String = t
                    .description
                    .as_deref()
                    .or(t.title.as_deref())
                    .unwrap_or("")
                    .chars()
                    .take(160)
                    .collect();
                let schema = self
                    .connections()
                    .live
                    .lock()
                    .await
                    .get(&c.id)
                    .and_then(|l| {
                        l.session
                            .tools
                            .iter()
                            .find(|x| x.name == t.name)
                            .map(|x| arguments_hint(&x.input_schema))
                    })
                    .unwrap_or_default();
                lines.push(format!("- {}/{} — {desc}{schema}", c.id, t.name));
                if lines.len() >= 40 {
                    break;
                }
            }
        }
        let names: Vec<String> = settings
            .connections
            .servers
            .iter()
            .filter(|c| {
                status
                    .get(&c.id)
                    .is_some_and(|s| s.state == McpState::Ready)
            })
            .map(|c| format!("\"{}\" ({})", c.name, c.id))
            .collect();
        (!lines.is_empty()).then(|| {
            format!(
                "Connected services the user set up: {}. When the user mentions one of them, use its tools — \
                 not the built-in Notes, Reminders or Calendar. Call a tool with:\n\
                 use \"service/tool\" {{\"argument\": \"value\"}}\n\
                 Tools:\n{}",
                names.join(", "),
                lines.join("\n")
            )
        })
    }
}

/// " Arguments: title (string, required), body (string)" from a JSON Schema.
fn arguments_hint(schema: &serde_json::Value) -> String {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return String::new();
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|r| r.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let parts: Vec<String> = props
        .iter()
        .take(12)
        .map(|(name, p)| {
            let kind = p.get("type").and_then(|t| t.as_str()).unwrap_or("any");
            if required.contains(&name.as_str()) {
                format!("{name} ({kind}, required)")
            } else {
                format!("{name} ({kind})")
            }
        })
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!(". Arguments: {}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_ipc::types::connections::{McpHttpConfig, McpStdioConfig};

    fn server(transport: McpTransport, local_only: bool) -> McpServerConfig {
        McpServerConfig {
            id: "x".into(),
            name: "X".into(),
            enabled: true,
            transport,
            local_only,
            tools: vec![],
        }
    }

    #[test]
    fn local_only_allows_only_servers_the_user_marked_local() {
        let stdio = |local| {
            server(
                McpTransport::Stdio(McpStdioConfig {
                    command: "npx".into(),
                    args: vec![],
                    env: vec![],
                }),
                local,
            )
        };
        let http = |url: &str, local| {
            server(
                McpTransport::Http(McpHttpConfig {
                    url: url.into(),
                    keychain_account: None,
                }),
                local,
            )
        };
        assert!(allowed(&stdio(false), PrivacyMode::CloudAllowed));
        assert!(!allowed(&stdio(false), PrivacyMode::LocalOnly));
        assert!(allowed(&stdio(true), PrivacyMode::LocalOnly));
        assert!(!allowed(
            &http("https://mcp.notion.com/mcp", true),
            PrivacyMode::LocalOnly
        ));
        assert!(allowed(
            &http("http://127.0.0.1:9000/mcp", true),
            PrivacyMode::LocalOnly
        ));
    }

    #[test]
    fn describes_arguments_compactly() {
        let s = serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "number"}}, "required": ["query"]});
        assert_eq!(
            arguments_hint(&s),
            ". Arguments: limit (number), query (string, required)"
        );
    }
}
