#![allow(clippy::expect_used)] // test harness

//! Agent runs end to end with a fake host and a scripted model.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, IpcError, types::models::Locality};
use wisp_providers::mock::MockProvider;

const ID: &str = "0199c2a8-0000-7000-8000-0000000000a9";

#[derive(Default)]
struct FakeHost {
    performed: Mutex<Vec<Value>>,
}

#[async_trait]
impl wisp_providers::HostBridge for FakeHost {
    async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        Ok(match method {
            "host.screen.observe" => json!({
                "id": ID, "kind": "window",
                "display": {"id": 1, "name": "Built-in", "frame": {"x":0,"y":0,"width":1512,"height":982}, "scale": 2.0},
                "selection": {"x":100,"y":100,"width":600,"height":400},
                "crop": {"x":100,"y":100,"width":600,"height":400},
                "cursor": {"x":300,"y":200},
                "image": {"url": format!("wisp://focus/{ID}.png"), "widthPx": 1200, "heightPx": 800},
                "application": {"name": "TextEdit", "pid": 7},
                "accessibility": {"nodes": [
                    {"role": "AXButton", "title": "Save", "frame": {"x":560,"y":440,"width":80,"height":28},
                     "enabled": true, "focused": false, "selected": false, "actions": ["AXPress"], "depth": 3}
                ], "summary": "elements:\n- button \"Save\"\n"},
                "createdAtMs": 1
            }),
            "host.input.perform" => {
                self.performed.lock().expect("lock").push(params);
                json!({"verification": {"status": "verified", "detail": "the window changed"}})
            }
            _ => json!({}),
        })
    }
}

async fn setup(reply: &str) -> (tempfile::TempDir, Core, Arc<FakeHost>) {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    core.ai().set_overrides(vec![Arc::new(MockProvider {
        id: "mock-local".into(),
        locality: Locality::Local,
        vision: false,
        reply: reply.into(),
        calls: Default::default(),
        requests: Default::default(),
    }) as Arc<dyn wisp_ai::AiProvider>]);
    let host = Arc::new(FakeHost::default());
    core.attach_host(host.clone());
    core.dispatch(
        "settings.update",
        json!({"update": {"section": "computer", "value": {"enabled": true}}}),
    )
    .await
    .expect("enable");
    (dir, core, host)
}

/// Answer every approval request with `decision`.
fn approve_all(core: &Core, decision: &'static str) {
    let c = core.clone();
    let mut rx = core.events().subscribe();
    tokio::spawn(async move {
        while let Ok(env) = rx.recv().await {
            if let Envelope::Event { event, payload, .. } = env
                && event == "tool.approvalRequested"
            {
                let _ = c
                    .dispatch(
                        "tool.respond",
                        json!({"id": payload["id"], "decision": decision}),
                    )
                    .await;
            }
        }
    });
}

async fn wait_run(
    rx: &mut tokio::sync::broadcast::Receiver<Envelope>,
    until: impl Fn(&Value) -> bool,
) -> Value {
    loop {
        let env = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .expect("timely")
            .expect("event");
        if let Envelope::Event { event, payload, .. } = env
            && event == "agent.runUpdated"
            && until(&payload)
        {
            return payload;
        }
    }
}

#[tokio::test]
async fn plans_acts_through_the_pipeline_and_finishes() {
    let (_d, core, host) = setup(
        "1. Click Save\n2. Check it saved|||THOUGHT: Save is visible.\nSTEP: 1\nACTION: click \"Save\"|||STEP: 2\nACTION: done \"I saved the document.\"",
    )
    .await;
    approve_all(&core, "allowOnce");
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch("agent.run", json!({"goal": "Save this document"}))
        .await
        .expect("run");
    assert_eq!(r["status"], "planning");
    assert_eq!(r["agentName"], "Wisp");

    let done = wait_run(&mut rx, |r| r["status"] == "completed").await;
    assert_eq!(done["note"], "I saved the document.");
    assert_eq!(done["plan"].as_array().expect("plan").len(), 2);
    assert!(
        done["plan"]
            .as_array()
            .expect("plan")
            .iter()
            .all(|p| p["status"] == "done")
    );
    let titles: Vec<&str> = done["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| s["title"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        titles,
        ["Looking at the screen", "Making a plan", "Click “Save”"]
    );
    assert_eq!(done["steps"][2]["status"], "succeeded");
    assert_eq!(done["steps"][2]["detail"], "the window changed");
    assert_eq!(done["toolCallsUsed"], 1);
    let performed = host.performed.lock().expect("lock").clone();
    assert_eq!(performed.len(), 1);
    assert_eq!(
        performed[0]["call"],
        json!({"tool": "click", "args": {"x": 600.0, "y": 454.0, "label": "Save"}})
    );
    // Bound to the approval the user gave.
    assert!(performed[0]["approvalId"].is_string());

    // History persists.
    let runs = core
        .dispatch("agent.runs", json!({"limit": 5}))
        .await
        .expect("runs");
    assert_eq!(runs["runs"][0]["status"], "completed");
    assert_eq!(runs["runs"][0]["steps"].as_array().expect("steps").len(), 3);
}

#[tokio::test]
async fn repeated_actions_pause_as_stuck_and_a_denial_ends_the_run() {
    let (_d, core, host) = setup("1. Tab around|||ACTION: press tab").await;
    // Keys can't be "always" allowed; "for this session" is the most.
    approve_all(&core, "allowSession");
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch("agent.run", json!({"goal": "Find the field"}))
        .await
        .expect("run");
    let stuck = wait_run(&mut rx, |r| r["status"] == "stuck").await;
    assert_eq!(
        stuck["note"],
        "The assistant may be stuck. It has repeated the same action several times."
    );
    assert_eq!(
        host.performed.lock().expect("lock").len(),
        2,
        "the third repeat isn't performed"
    );
    core.dispatch("agent.control", json!({"runId": r["id"], "action": "stop"}))
        .await
        .expect("stop");
    let stopped = wait_run(&mut rx, |r| r["status"] == "cancelled").await;
    assert_eq!(stopped["note"], "Stopped.");

    let (_d2, core, host) = setup("1. Save|||ACTION: click \"Save\"").await;
    approve_all(&core, "deny");
    let mut rx = core.events().subscribe();
    core.dispatch("agent.run", json!({"goal": "Save"}))
        .await
        .expect("run");
    let ended = wait_run(&mut rx, |r| r["status"] == "cancelled").await;
    assert!(
        ended["note"]
            .as_str()
            .expect("note")
            .contains("wasn't allowed")
    );
    assert!(host.performed.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn agents_are_validated_and_limited_to_their_tools() {
    let (_d, core, host) = setup("1. Save|||ACTION: click \"Save\"").await;
    for bad in [
        json!({"name": ""}),
        json!({"name": "X", "tools": ["shell"]}),
        json!({"name": "X", "limits": {"maxSteps": 500, "maxMinutes": 10, "maxToolCalls": 10, "maxTokens": 5000}}),
    ] {
        assert!(core.dispatch("agent.save", bad).await.is_err());
    }
    let agent = core
        .dispatch("agent.save", json!({"name": "Reader", "avatar": "📖", "tools": ["readScreen"], "instructions": "Only read."}))
        .await
        .expect("save");
    assert_eq!(agent["avatar"], "📖");
    let list = core
        .dispatch("agent.list", Value::Null)
        .await
        .expect("list");
    assert_eq!(list["agents"][0]["name"], "Reader");

    let mut rx = core.events().subscribe();
    core.dispatch("agent.run", json!({"agentId": agent["id"], "goal": "Save"}))
        .await
        .expect("run");
    let ended = wait_run(&mut rx, |r| r["status"] == "stuck").await;
    assert!(
        ended["note"]
            .as_str()
            .expect("note")
            .contains("permission to use “click”")
    );
    assert!(host.performed.lock().expect("lock").is_empty());
}
