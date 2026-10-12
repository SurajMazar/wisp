//! A small Model Context Protocol client (protocol revision 2025-06-18): connect to a server,
//! list its tools, call them. Two transports:
//!
//! * **stdio** — a local program the user configured, speaking newline-delimited JSON-RPC.
//! * **Streamable HTTP** — a remote server: JSON-RPC over POST, replies as JSON or an SSE stream,
//!   with the `Mcp-Session-Id` the server assigns.
//!
//! Everything a server returns is data for the user's request, never instructions; callers fence
//! it before showing it to a model. Every request is time-limited and every reply size-limited.

use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex as AsyncMutex, oneshot},
};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// Starting a server can include a first-time package download (`npx …`).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Longest line / reply accepted from a server.
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
const MAX_TOOLS: usize = 200;
/// Characters of a tool result kept for the model.
pub const MAX_RESULT_CHARS: usize = 8000;

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("couldn't start the server: {0}")]
    Spawn(String),
    #[error("couldn't reach the server: {0}")]
    Network(String),
    #[error("the server didn't answer in time")]
    Timeout,
    #[error("the server closed the connection")]
    Closed,
    #[error("the server replied with something that isn't MCP: {0}")]
    Protocol(String),
    #[error("{0}")]
    Server(String),
    #[error("the server needs you to sign in (HTTP {0}); add a token for it")]
    Unauthorized(u16),
}

/// A tool a server offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub input_schema: Value,
    /// The server says it only reads.
    pub read_only: bool,
    /// The server says it may destroy or overwrite things.
    pub destructive: bool,
}

/// What a tool call produced, as text.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutcome {
    pub text: String,
    /// The tool reported a failure.
    pub is_error: bool,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, McpError>>>>>;

struct StdioConn {
    stdin: Arc<AsyncMutex<ChildStdin>>,
    child: AsyncMutex<Child>,
    pending: Pending,
}

struct HttpConn {
    client: reqwest::Client,
    url: String,
    bearer: Option<String>,
    session: Mutex<Option<String>>,
}

enum Transport {
    Stdio(StdioConn),
    Http(HttpConn),
}

/// A live connection to one server.
pub struct McpSession {
    transport: Transport,
    next_id: AtomicU64,
    pub server_name: Option<String>,
    pub tools: Vec<McpTool>,
}

impl McpSession {
    /// Start a local server: `command args…` with `env` added to a usable PATH.
    pub async fn connect_stdio(
        command: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<Self, McpError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .env("PATH", search_path())
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Spawn(format!("{command}: {e}")))?;
        let stdin = Arc::new(AsyncMutex::new(child.stdin.take().ok_or(McpError::Closed)?));
        let stdout = child.stdout.take().ok_or(McpError::Closed)?;
        if let Some(stderr) = child.stderr.take() {
            // Servers log to stderr; keep it out of the protocol, in our log at debug level.
            let name = command.to_owned();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    tracing::debug!(server = %name, "{}", l.chars().take(300).collect::<String>());
                }
            });
        }
        let pending: Pending = Arc::default();
        // Replies to server-initiated requests (ping) go back through stdin.
        let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let writer = stdin.clone();
        tokio::spawn(async move {
            while let Some(reply) = reply_rx.recv().await {
                if write_line(&writer, &reply).await.is_err() {
                    break;
                }
            }
        });
        // Reader: route responses to their waiters.
        let reader_pending = pending.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.len() > MAX_MESSAGE || line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(&line) {
                    Ok(msg) => route(&msg, &reader_pending, &reply_tx),
                    Err(_) => tracing::debug!("non-JSON line from MCP server ignored"),
                }
            }
            // Closed: fail everyone still waiting.
            if let Ok(mut p) = reader_pending.lock() {
                for (_, tx) in p.drain() {
                    let _ = tx.send(Err(McpError::Closed));
                }
            }
        });
        let mut session = Self {
            transport: Transport::Stdio(StdioConn {
                stdin,
                child: AsyncMutex::new(child),
                pending,
            }),
            next_id: AtomicU64::new(1),
            server_name: None,
            tools: vec![],
        };
        if let Err(e) = session.initialize_and_list().await {
            session.close().await;
            return Err(e);
        }
        Ok(session)
    }

    /// Connect to a remote server over Streamable HTTP.
    pub async fn connect_http(url: &str, bearer: Option<String>) -> Result<Self, McpError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| McpError::Network(e.to_string()))?;
        let mut session = Self {
            transport: Transport::Http(HttpConn {
                client,
                url: url.to_owned(),
                bearer,
                session: Mutex::new(None),
            }),
            next_id: AtomicU64::new(1),
            server_name: None,
            tools: vec![],
        };
        session.initialize_and_list().await?;
        Ok(session)
    }

    async fn initialize_and_list(&mut self) -> Result<(), McpError> {
        let init = tokio::time::timeout(
            CONNECT_TIMEOUT,
            self.request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "Wisp", "version": env!("CARGO_PKG_VERSION")},
                }),
            ),
        )
        .await
        .map_err(|_| McpError::Timeout)??;
        self.server_name = init
            .pointer("/serverInfo/name")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.notify("notifications/initialized", json!({})).await?;
        let mut cursor: Option<String> = None;
        let mut tools = vec![];
        for _ in 0..10 {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let page = tokio::time::timeout(CALL_TIMEOUT, self.request("tools/list", params))
                .await
                .map_err(|_| McpError::Timeout)??;
            for t in page
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(tool) = parse_tool(t) {
                    tools.push(tool);
                }
            }
            cursor = page
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() || tools.len() >= MAX_TOOLS {
                break;
            }
        }
        tools.truncate(MAX_TOOLS);
        self.tools = tools;
        Ok(())
    }

    /// Call a tool. A tool-level failure comes back as `is_error`, not as `Err`.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpError> {
        let r = tokio::time::timeout(
            CALL_TIMEOUT,
            self.request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            ),
        )
        .await
        .map_err(|_| McpError::Timeout)??;
        Ok(outcome(&r))
    }

    /// Stop a local server.
    pub async fn close(&self) {
        if let Transport::Stdio(c) = &self.transport {
            let _ = c.child.lock().await.kill().await;
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), McpError> {
        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        match &self.transport {
            Transport::Stdio(c) => write_line(&c.stdin, &msg).await,
            Transport::Http(h) => h.post(&msg).await.map(|_| ()),
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        match &self.transport {
            Transport::Stdio(c) => {
                let (tx, rx) = oneshot::channel();
                if let Ok(mut p) = c.pending.lock() {
                    p.insert(id, tx);
                }
                write_line(&c.stdin, &msg).await?;
                let r = rx.await.map_err(|_| McpError::Closed)?;
                if r.is_err()
                    && let Ok(mut p) = c.pending.lock()
                {
                    p.remove(&id);
                }
                r
            }
            Transport::Http(h) => {
                let reply = h
                    .post(&msg)
                    .await?
                    .ok_or(McpError::Protocol("no reply to a request".into()))?;
                result_of(&reply)
            }
        }
    }
}

async fn write_line(stdin: &AsyncMutex<ChildStdin>, msg: &Value) -> Result<(), McpError> {
    let mut line = serde_json::to_vec(msg).map_err(|e| McpError::Protocol(e.to_string()))?;
    line.push(b'\n');
    let mut w = stdin.lock().await;
    w.write_all(&line).await.map_err(|_| McpError::Closed)?;
    w.flush().await.map_err(|_| McpError::Closed)
}

/// A response goes to its waiter; a server request gets a reply queued (ping → {}, anything else
/// → method not found); notifications are ignored.
fn route(msg: &Value, pending: &Pending, replies: &tokio::sync::mpsc::UnboundedSender<Value>) {
    let id = msg.get("id");
    if msg.get("method").is_some() {
        if let Some(id) = id {
            let reply = if msg["method"] == "ping" {
                json!({ "jsonrpc": "2.0", "id": id, "result": {} })
            } else {
                json!({ "jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "not supported by Wisp"} })
            };
            let _ = replies.send(reply);
        }
        return;
    }
    let Some(id) = id.and_then(Value::as_u64) else {
        return;
    };
    if let Some(tx) = pending.lock().ok().and_then(|mut p| p.remove(&id)) {
        let _ = tx.send(result_of(msg));
    }
}

fn result_of(msg: &Value) -> Result<Value, McpError> {
    if let Some(e) = msg.get("error") {
        return Err(McpError::Server(
            e.get("message")
                .and_then(Value::as_str)
                .unwrap_or("the server reported an error")
                .chars()
                .take(500)
                .collect(),
        ));
    }
    msg.get("result")
        .cloned()
        .ok_or_else(|| McpError::Protocol("reply has neither result nor error".into()))
}

impl HttpConn {
    /// POST one JSON-RPC message; returns the JSON-RPC reply for requests (from a JSON body or
    /// the SSE stream), None for notifications.
    async fn post(&self, msg: &Value) -> Result<Option<Value>, McpError> {
        let mut req = self
            .client
            .post(&self.url)
            .timeout(CALL_TIMEOUT)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", PROTOCOL_VERSION)
            .json(msg);
        if let Some(s) = self.session.lock().ok().and_then(|s| s.clone()) {
            req = req.header("Mcp-Session-Id", s);
        }
        if let Some(t) = &self.bearer {
            req = req.bearer_auth(t);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| McpError::Network(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(McpError::Unauthorized(status.as_u16()));
        }
        if !status.is_success() {
            return Err(McpError::Network(format!("HTTP {status}")));
        }
        if let Some(s) = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            && let Ok(mut slot) = self.session.lock()
        {
            *slot = Some(s.to_owned());
        }
        if msg.get("id").is_none() {
            return Ok(None);
        }
        let kind = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if kind.starts_with("text/event-stream") {
            // Read events until the reply to this request arrives.
            let mut stream = resp.bytes_stream();
            let mut buf = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| McpError::Network(e.to_string()))?;
                buf.push_str(&String::from_utf8_lossy(&chunk));
                if buf.len() > MAX_MESSAGE {
                    return Err(McpError::Protocol("reply too large".into()));
                }
                while let Some(end) = buf.find("\n\n") {
                    let event: String = buf.drain(..end + 2).collect();
                    let data: String = event
                        .lines()
                        .filter_map(|l| l.strip_prefix("data:"))
                        .map(str::trim_start)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Ok(v) = serde_json::from_str::<Value>(&data)
                        && v.get("id") == msg.get("id")
                        && v.get("method").is_none()
                    {
                        return Ok(Some(v));
                    }
                }
            }
            Err(McpError::Closed)
        } else {
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| McpError::Network(e.to_string()))?;
            if bytes.len() > MAX_MESSAGE {
                return Err(McpError::Protocol("reply too large".into()));
            }
            serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| McpError::Protocol(e.to_string()))
        }
    }
}

fn parse_tool(t: &Value) -> Option<McpTool> {
    let name = t.get("name")?.as_str()?.to_owned();
    if name.is_empty() || name.len() > 128 {
        return None;
    }
    let ann = t.get("annotations");
    let hint = |k: &str| ann.and_then(|a| a.get(k)).and_then(Value::as_bool);
    let text = |v: Option<&Value>, max: usize| {
        v.and_then(Value::as_str)
            .map(|s| s.chars().take(max).collect::<String>())
    };
    Some(McpTool {
        title: text(t.get("title"), 120).or_else(|| text(ann.and_then(|a| a.get("title")), 120)),
        description: text(t.get("description"), 1000),
        input_schema: t
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({"type": "object"})),
        read_only: hint("readOnlyHint") == Some(true),
        // MCP's default for a non-read-only tool is "may be destructive"; only an explicit false
        // clears it.
        destructive: hint("readOnlyHint") != Some(true) && hint("destructiveHint") == Some(true),
        name,
    })
}

/// Text of a `tools/call` result (text parts verbatim, other parts named), capped.
pub fn outcome(r: &Value) -> ToolOutcome {
    let mut parts: Vec<String> = vec![];
    for c in r
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match c.get("type").and_then(Value::as_str) {
            Some("text") => parts.push(c["text"].as_str().unwrap_or_default().to_owned()),
            Some("resource_link") | Some("resource") => parts.push(format!(
                "[resource: {}]",
                c.pointer("/uri")
                    .or_else(|| c.pointer("/resource/uri"))
                    .and_then(Value::as_str)
                    .unwrap_or("?")
            )),
            Some(other) => parts.push(format!("[{other}]")),
            None => {}
        }
    }
    if parts.is_empty()
        && let Some(s) = r.get("structuredContent")
    {
        parts.push(s.to_string());
    }
    let text = parts.join("\n");
    let n = text.chars().count();
    let text = if n > MAX_RESULT_CHARS {
        format!(
            "{}\n… ({} more characters)",
            text.chars().take(MAX_RESULT_CHARS).collect::<String>(),
            n - MAX_RESULT_CHARS
        )
    } else {
        text
    };
    ToolOutcome {
        text,
        is_error: r.get("isError").and_then(Value::as_bool) == Some(true),
    }
}

/// Apps opened from the Finder get a minimal PATH; servers usually need Homebrew's (node, npx,
/// uvx, python).
fn search_path() -> String {
    let mut dirs: Vec<String> = ["/opt/homebrew/bin", "/usr/local/bin"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(format!("{home}/.local/bin"));
        dirs.push(format!("{home}/.cargo/bin"));
    }
    dirs.push(std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()));
    dirs.join(":")
}
