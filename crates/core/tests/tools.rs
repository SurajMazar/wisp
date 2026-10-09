#![allow(clippy::expect_used)] // test harness

//! The tool pipeline end to end with a fake host: policy, approvals, grants, execution, audit.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, IpcError};

#[derive(Default)]
struct FakeHost {
    calls: Mutex<Vec<String>>,
}

#[async_trait]
impl wisp_providers::HostBridge for FakeHost {
    async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        self.calls.lock().expect("lock").push(format!(
            "{method} {}",
            params["tool"].as_str().unwrap_or("")
        ));
        Ok(match method {
            "host.ax.snapshot" => json!({
                "nodes": [{"role": "AXButton", "title": "Save", "frame": {"x": 10, "y": 10, "width": 80, "height": 24},
                           "enabled": true, "focused": false, "selected": false, "actions": ["AXPress"], "depth": 2}],
                "truncated": false, "elapsedMs": 3
            }),
            _ => json!({"verification": {"status": "verified", "detail": "The window changed"}}),
        })
    }
}

fn setup() -> (tempfile::TempDir, Core, Arc<FakeHost>) {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    let host = Arc::new(FakeHost::default());
    core.attach_host(host.clone());
    (dir, core, host)
}

fn click(label: &str) -> Value {
    json!({ "call": { "tool": "click", "args": { "x": 50, "y": 22, "label": label } } })
}

/// Run a tool in the background and answer its approval request with `answer`.
async fn run_answering(core: &Core, params: Value, answer: Option<&str>) -> (Value, Option<Value>) {
    let mut rx = core.events().subscribe();
    let c = core.clone();
    let task = tokio::spawn(async move { c.dispatch("tool.run", params).await.expect("run") });
    let mut request = None;
    if let Some(answer) = answer {
        loop {
            let env = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
                .await
                .expect("timely")
                .expect("event");
            if let Envelope::Event { event, payload, .. } = env
                && event == "tool.approvalRequested"
            {
                core.dispatch(
                    "tool.respond",
                    json!({ "id": payload["id"], "decision": answer }),
                )
                .await
                .expect("respond");
                request = Some(payload);
                break;
            }
        }
    }
    (task.await.expect("join"), request)
}

#[tokio::test]
async fn computer_control_is_off_until_enabled_but_reading_is_allowed() {
    let (_d, core, host) = setup();
    let (r, _) = run_answering(&core, click("Save"), None).await;
    assert_eq!(r["record"]["status"], "denied");
    assert_eq!(r["record"]["decision"], "policyDeny");
    assert!(
        r["record"]["error"]
            .as_str()
            .expect("why")
            .contains("Computer control is off")
    );

    let (r, _) = run_answering(
        &core,
        json!({"call": {"tool": "findElement", "args": {"label": "save"}}}),
        None,
    )
    .await;
    assert_eq!(r["record"]["status"], "succeeded");
    assert!(
        r["output"]
            .as_str()
            .expect("out")
            .contains("Button “save” at x=50 y=22")
    );
    // Only looks: the click's target was inspected, nothing was performed.
    let calls = host.calls.lock().expect("lock").clone();
    assert!(
        calls.iter().all(|c| c.starts_with("host.ax.snapshot")),
        "{calls:?}"
    );
}

#[tokio::test]
async fn approvals_grants_and_risk_escalation() {
    let (_d, core, host) = setup();
    core.dispatch(
        "settings.update",
        json!({"update": {"section": "computer", "value": {"enabled": true}}}),
    )
    .await
    .expect("enable");

    // First click asks; "allow for session" runs it and remembers.
    let (r, req) = run_answering(&core, click("Save"), Some("allowSession")).await;
    let req = req.expect("asked");
    assert_eq!(req["title"], "Click “Save”");
    assert_eq!(req["risk"], "low");
    assert_eq!(
        req["choices"],
        json!(["allowOnce", "allowSession", "alwaysAllow", "deny"])
    );
    assert_eq!(r["record"]["status"], "succeeded");
    assert_eq!(r["record"]["verification"]["status"], "verified");

    // Second click: no question.
    let (r, _) = run_answering(&core, click("Save"), None).await;
    assert_eq!(
        (
            r["record"]["status"].clone(),
            r["record"]["decision"].clone()
        ),
        (json!("succeeded"), json!("allowSession"))
    );

    // A destructive target asks again despite the session grant, and only offers "once".
    let (r, req) = run_answering(&core, click("Delete"), Some("alwaysAllow")).await;
    let req = req.expect("asked");
    assert_eq!(req["risk"], "high");
    assert_eq!(req["choices"], json!(["allowOnce", "deny"]));
    // "Always" wasn't offered, so it counts as a denial and nothing ran.
    assert_eq!(r["record"]["status"], "denied");
    let performed = host
        .calls
        .lock()
        .expect("lock")
        .iter()
        .filter(|c| c.starts_with("host.input.perform"))
        .count();
    assert_eq!(performed, 2);

    // Always-allow persists as a grant and can be revoked.
    let (_, _) = run_answering(
        &core,
        json!({"call": {"tool": "scroll", "args": {"x": 1, "y": 1, "dy": 3}}}),
        Some("alwaysAllow"),
    )
    .await;
    let grants = core
        .dispatch("tool.grants", Value::Null)
        .await
        .expect("grants");
    assert_eq!(grants["grants"][0]["tool"], "scroll");
    assert_eq!(grants["session"], json!(["click"]));
    let after = core
        .dispatch("tool.revoke", json!({"id": grants["grants"][0]["id"]}))
        .await
        .expect("revoke");
    assert!(after["grants"].as_array().expect("arr").is_empty());

    // Everything is audited, newest first, without typed text.
    let log = core
        .dispatch("tool.activity", json!({"limit": 10}))
        .await
        .expect("log");
    let items = log["items"].as_array().expect("items");
    assert_eq!(items.len(), 4);
    assert_eq!(items[1]["title"], "Click “Delete”");
    assert_eq!(items[1]["status"], "denied");
}

#[tokio::test]
async fn invalid_calls_never_reach_the_policy() {
    let (_d, core, host) = setup();
    let err = core
        .dispatch(
            "tool.run",
            json!({"call": {"tool": "openUrl", "args": {"url": "file:///etc/passwd"}}}),
        )
        .await
        .expect_err("invalid");
    assert!(
        err.details.as_deref().unwrap_or("").contains("http"),
        "{err:?}"
    );
    assert!(host.calls.lock().expect("lock").is_empty());
}

/// What's under the point decides the risk, not the label the model claims.
#[tokio::test]
async fn risk_is_judged_on_the_element_actually_under_the_click() {
    struct DeleteHost;
    #[async_trait]
    impl wisp_providers::HostBridge for DeleteHost {
        async fn call(&self, method: &str, _params: Value) -> Result<Value, IpcError> {
            Ok(match method {
                "host.ax.snapshot" => json!({
                    "application": {"name": "Settings", "pid": 42},
                    "underPointer": {"role": "AXButton", "title": "Delete Account", "frame": {"x": 0, "y": 0, "width": 9, "height": 9},
                                     "enabled": true, "focused": false, "selected": false, "actions": ["AXPress"], "depth": 1},
                    "nodes": [], "truncated": false, "elapsedMs": 1
                }),
                _ => json!({"verification": {"status": "verified", "detail": "ok"}}),
            })
        }
    }
    let (_d, core, _) = setup();
    core.attach_host(Arc::new(DeleteHost));
    core.dispatch(
        "settings.update",
        json!({"update": {"section": "computer", "value": {"enabled": true}}}),
    )
    .await
    .expect("enable");
    // The model says "Account"; the button there is "Delete Account" → critical, asks, shows the real label.
    let (r, req) = run_answering(&core, click("Account"), Some("allowOnce")).await;
    let req = req.expect("asked");
    assert_eq!(req["title"], "Click “Delete Account”");
    assert_eq!(req["risk"], "critical");
    assert_eq!(req["detail"], "In Settings");
    assert_eq!(r["record"]["status"], "succeeded");
}
