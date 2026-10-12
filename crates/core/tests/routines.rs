#![allow(clippy::expect_used)] // test harness

//! Routines (stop gate 17): an every-5-minutes routine triggers, its run is recorded, the next
//! run is scheduled, failures retry and notify.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, IpcError, types::models::Locality};
use wisp_providers::mock::MockProvider;

#[derive(Default)]
struct NotifyHost {
    notes: Mutex<Vec<Value>>,
}

#[async_trait]
impl wisp_providers::HostBridge for NotifyHost {
    async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        if method == "host.notify" {
            self.notes.lock().expect("lock").push(params);
        }
        Ok(json!({}))
    }
}

fn setup(reply: &str, locality: Locality) -> (tempfile::TempDir, Core, Arc<NotifyHost>) {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    core.ai().set_overrides(vec![Arc::new(MockProvider {
        id: "mock".into(),
        locality,
        vision: false,
        reply: reply.into(),
        calls: Default::default(),
        requests: Default::default(),
    }) as Arc<dyn wisp_ai::AiProvider>]);
    let host = Arc::new(NotifyHost::default());
    core.attach_host(host.clone());
    core.set_retry_unit_ms(20);
    (dir, core, host)
}

async fn finished(rx: &mut tokio::sync::broadcast::Receiver<Envelope>, attempt: u64) -> Value {
    loop {
        let env = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .expect("timely")
            .expect("event");
        if let Envelope::Event { event, payload, .. } = env
            && event == "routine.runUpdated"
            && payload["attempt"] == attempt
            && payload["status"] != "running"
        {
            return payload;
        }
    }
}

#[tokio::test]
async fn every_five_minutes_triggers_records_and_reschedules() {
    let (_d, core, host) = setup("All quiet today.", Locality::Local);
    let r = core
        .dispatch(
            "routine.save",
            json!({"name": "Check-in", "schedule": "*/5 * * * *", "timezone": "Asia/Kathmandu", "prompt": "Anything new?"}),
        )
        .await
        .expect("save");
    assert_eq!(r["scheduleText"], "Every 5 minutes");
    let next = r["nextRunAtMs"].as_i64().expect("next");
    let now = wisp_common::time::now_ms();
    assert!(next > now && next <= now + 5 * 60_000);

    // Not due yet → nothing; at its time → it triggers once.
    assert_eq!(core.tick(next - 1).await, 0);
    let mut rx = core.events().subscribe();
    assert_eq!(core.tick(next).await, 1);
    let run = finished(&mut rx, 1).await;
    assert_eq!(run["status"], "succeeded");
    assert_eq!(run["scheduledForMs"], next);

    // Recorded in history, the answer lives in a "Routine: …" conversation, and the user was notified.
    let runs = core
        .dispatch("routine.runs", json!({"routineId": r["id"], "limit": 5}))
        .await
        .expect("runs");
    assert_eq!(runs["runs"].as_array().expect("runs").len(), 1);
    let conv = core
        .dispatch("conversation.get", json!({"id": run["conversationId"]}))
        .await
        .expect("conv");
    assert_eq!(conv["title"], "Routine: Check-in");
    assert_eq!(conv["messages"][1]["content"], "All quiet today.");
    assert_eq!(
        host.notes.lock().expect("lock")[0],
        json!({"title": "Check-in", "body": "All quiet today."})
    );

    // The next run moved forward by about five minutes, and a late tick doesn't fire twice.
    let after = core
        .dispatch("routine.list", Value::Null)
        .await
        .expect("list")["routines"][0]
        .clone();
    assert_eq!(after["lastStatus"], "succeeded");
    assert!(after["nextRunAtMs"].as_i64().expect("next") > next);
    assert_eq!(core.tick(next).await, 0);
}

#[tokio::test]
async fn failures_retry_then_notify_and_bad_schedules_are_explained() {
    // Local Only with only a cloud model: every attempt fails.
    let (_d, core, host) = setup("x", Locality::Cloud);
    let err = core
        .dispatch(
            "routine.save",
            json!({"name": "X", "schedule": "every day", "timezone": "UTC", "prompt": "p"}),
        )
        .await
        .expect_err("bad schedule");
    assert!(
        err.message.contains("schedule doesn't work"),
        "{}",
        err.message
    );
    assert!(
        core.dispatch(
            "routine.save",
            json!({"name": "X", "schedule": "0 9 * * *", "timezone": "Moon/Base", "prompt": "p"})
        )
        .await
        .is_err()
    );

    let r = core
        .dispatch(
            "routine.save",
            json!({"name": "Digest", "schedule": "0 9 * * *", "timezone": "UTC", "prompt": "Summarize",
                   "retry": {"maxAttempts": 2, "backoffMinutes": 1}, "notify": {"onSuccess": false, "onFailure": true}}),
        )
        .await
        .expect("save");
    let mut rx = core.events().subscribe();
    let first = core
        .dispatch("routine.runNow", json!({"id": r["id"]}))
        .await
        .expect("run now");
    assert_eq!(first["attempt"], 1);
    assert_eq!(finished(&mut rx, 1).await["status"], "failed");
    let second = finished(&mut rx, 2).await;
    assert_eq!(second["status"], "failed");
    assert!(
        second["error"]
            .as_str()
            .expect("why")
            .contains("Local Only")
    );
    let notes = host.notes.lock().expect("lock").clone();
    assert_eq!(notes.len(), 1, "one notification after the last attempt");
    assert!(
        notes[0]["body"]
            .as_str()
            .expect("body")
            .starts_with("Didn't work")
    );
}
