#![allow(clippy::expect_used)] // test harness

//! Teacher mode end to end: observe → step with a resolved target → watch for the user → done.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, IpcError, types::models::Locality};
use wisp_providers::mock::MockProvider;

#[derive(Default)]
struct FakeHost {
    snapshots: AtomicUsize,
    calls: Mutex<Vec<String>>,
    /// The window title changes after this many fingerprint reads (the user "did it").
    change_after: usize,
}

const ID: &str = "0199c2a8-0000-7000-8000-0000000000e1";

#[async_trait]
impl wisp_providers::HostBridge for FakeHost {
    async fn call(&self, method: &str, _params: Value) -> Result<Value, IpcError> {
        self.calls.lock().expect("lock").push(method.to_owned());
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
            "host.ax.snapshot" => {
                let n = self.snapshots.fetch_add(1, Ordering::SeqCst);
                let title = if self.change_after > 0 && n >= self.change_after {
                    "Saved"
                } else {
                    "Untitled"
                };
                json!({"application": {"name": "TextEdit", "pid": 7}, "window": {"id": 1, "title": title,
                       "frame": {"x":0,"y":0,"width":10,"height":10}}, "nodes": [], "truncated": false, "elapsedMs": 1})
            }
            _ => json!({}),
        })
    }
}

fn setup(reply: &str, change_after: usize) -> (tempfile::TempDir, Core, Arc<FakeHost>) {
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
    }) as Arc<dyn wisp_ai::AiProvider>]);
    let host = Arc::new(FakeHost {
        change_after,
        ..Default::default()
    });
    core.attach_host(host.clone());
    (dir, core, host)
}

async fn next_lesson(
    rx: &mut tokio::sync::broadcast::Receiver<Envelope>,
    until: impl Fn(&Value) -> bool,
) -> Value {
    loop {
        let env = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .expect("timely")
            .expect("event");
        if let Envelope::Event { event, payload, .. } = env
            && event == "teach.updated"
            && until(&payload)
        {
            return payload;
        }
    }
}

#[tokio::test]
async fn teaches_step_by_step_and_notices_when_the_user_did_it() {
    let (_d, core, host) = setup(
        "Click Save [[point: Save]] to save your work.|||DONE: Nice — it's saved!",
        2,
    );
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch(
            "conversation.send",
            json!({ "text": "Teach me how to save this document" }),
        )
        .await
        .expect("send");

    let waiting = next_lesson(&mut rx, |l| l["status"] == "waitingForUser").await;
    assert_eq!(waiting["goal"], "save this document");
    assert_eq!(waiting["watching"], true);
    assert_eq!(
        waiting["steps"][0]["instruction"],
        "Click Save to save your work."
    );
    assert_eq!(waiting["steps"][0]["target"]["label"], "Save");
    assert_eq!(waiting["steps"][0]["target"]["rect"]["x"], 560.0);

    let done = next_lesson(&mut rx, |l| l["status"] == "done").await;
    assert_eq!(done["steps"][0]["status"], "done");
    assert_eq!(done["steps"][0]["confirmed"], "Wisp saw the screen change");
    assert_eq!(done["note"], "Nice — it's saved!");

    let calls = host.calls.lock().expect("lock").clone();
    assert!(calls.iter().any(|c| c == "host.guidance.show"), "{calls:?}");
    assert!(
        !calls.iter().any(|c| c == "host.input.perform"),
        "a lesson never acts by itself"
    );

    // The lesson lives in the conversation: the goal, the step, the wrap-up.
    let conv = core
        .dispatch("conversation.get", json!({"id": r["conversationId"]}))
        .await
        .expect("get");
    let contents: Vec<&str> = conv["messages"]
        .as_array()
        .expect("msgs")
        .iter()
        .map(|m| m["content"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        contents,
        [
            "Teach me how to save this document",
            "Click Save to save your work.",
            "DONE: Nice — it's saved!"
        ]
    );
}

#[tokio::test]
async fn the_user_can_advance_and_stop() {
    // The screen never changes: the user says "I did it", then stops.
    let (_d, core, _host) = setup(
        "Open the File menu [[point: File]].|||Choose Save [[point: Save]].",
        0,
    );
    let mut rx = core.events().subscribe();
    core.dispatch(
        "conversation.send",
        json!({ "text": "Save it", "teach": true }),
    )
    .await
    .expect("send");
    let l = next_lesson(&mut rx, |l| l["status"] == "waitingForUser").await;
    core.dispatch(
        "teach.control",
        json!({"lessonId": l["id"], "action": "next"}),
    )
    .await
    .expect("next");
    let l = next_lesson(&mut rx, |l| {
        l["status"] == "waitingForUser" && l["steps"].as_array().is_some_and(|s| s.len() == 2)
    })
    .await;
    assert_eq!(l["steps"][0]["confirmed"], "You said you did it");
    core.dispatch(
        "teach.control",
        json!({"lessonId": l["id"], "action": "stop"}),
    )
    .await
    .expect("stop");
    let stopped = next_lesson(&mut rx, |l| l["status"] == "stopped").await;
    assert_eq!(stopped["steps"].as_array().expect("steps").len(), 2);
    let current = core
        .dispatch("teach.current", Value::Null)
        .await
        .expect("current");
    assert_eq!(current["lesson"]["status"], "stopped");
}
