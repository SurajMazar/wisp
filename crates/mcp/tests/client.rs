#![allow(clippy::expect_used)] // test harness

//! The MCP client against a real (tiny) MCP server, over stdio and Streamable HTTP. Needs
//! `node`; skipped without it.

use std::process::Stdio;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use wisp_mcp::{McpError, McpSession};

fn fixture() -> String {
    format!(
        "{}/tests/fixtures/test-server.mjs",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn node() -> Option<String> {
    [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ]
    .iter()
    .find(|p| std::path::Path::new(p).exists())
    .map(|p| (*p).to_owned())
}

#[tokio::test]
async fn stdio_lists_tools_across_pages_and_calls_them() {
    let Some(node) = node() else {
        eprintln!("skipped: node not installed");
        return;
    };
    let s = McpSession::connect_stdio(&node, &[fixture()], &[("EXTRA".into(), "1".into())])
        .await
        .expect("connect");
    assert_eq!(s.server_name.as_deref(), Some("wisp-test-server"));
    let names: Vec<&str> = s.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["echo", "add_note", "wipe", "fail"], "both pages");
    let echo = &s.tools[0];
    assert!(echo.read_only && !echo.destructive);
    assert_eq!(echo.title.as_deref(), Some("Echo"));
    assert!(s.tools[2].destructive, "wipe says it's destructive");

    let r = s
        .call_tool("echo", json!({"text": "hi"}))
        .await
        .expect("call");
    assert_eq!(r.text, "echo: hi");
    assert!(!r.is_error);
    let r = s.call_tool("fail", json!({})).await.expect("call");
    assert!(r.is_error, "tool-level failure is reported, not thrown");
    assert!(matches!(
        s.call_tool("nope", json!({})).await,
        Err(McpError::Server(m)) if m.contains("Unknown tool")
    ));
    s.close().await;
}

#[tokio::test]
async fn a_missing_program_is_a_clear_error() {
    let r = McpSession::connect_stdio("/nonexistent/wisp-mcp-server", &[], &[]).await;
    assert!(matches!(r, Err(McpError::Spawn(_))));
}

#[tokio::test]
async fn http_keeps_the_session_reads_sse_and_sends_the_token() {
    let Some(node) = node() else {
        eprintln!("skipped: node not installed");
        return;
    };
    let mut server = tokio::process::Command::new(&node)
        .args([
            fixture(),
            "--http".into(),
            "0".into(),
            "--token".into(),
            "s3cret".into(),
        ])
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("server");
    let mut lines = BufReader::new(server.stdout.take().expect("stdout")).lines();
    let port = lines
        .next_line()
        .await
        .expect("line")
        .and_then(|l| l.strip_prefix("PORT ").map(str::to_owned))
        .expect("port");
    let url = format!("http://127.0.0.1:{port}/mcp");

    assert!(matches!(
        McpSession::connect_http(&url, None).await,
        Err(McpError::Unauthorized(401))
    ));
    let s = McpSession::connect_http(&url, Some("s3cret".into()))
        .await
        .expect("connect");
    assert_eq!(s.tools.len(), 4);
    let r = s
        .call_tool("add_note", json!({"title": "Plan", "body": "abc"}))
        .await
        .expect("call over SSE");
    assert_eq!(r.text, "Saved “Plan” (3 characters)");
}

#[test]
fn results_are_capped_and_non_text_parts_named() {
    let long = "x".repeat(wisp_mcp::MAX_RESULT_CHARS + 50);
    let r = wisp_mcp::outcome(&json!({"content": [
        {"type": "text", "text": long},
        {"type": "image", "data": "...", "mimeType": "image/png"}
    ]}));
    assert!(r.text.ends_with("more characters)"));
    assert!(r.text.chars().count() < wisp_mcp::MAX_RESULT_CHARS + 100);
}
