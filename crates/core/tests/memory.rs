#![allow(clippy::expect_used)] // test harness

//! Memory (stop gate 15): user states a preference → memory created → later conversation →
//! relevant memory retrieved; inspection, deletion, settings.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, IpcError, types::models::Locality};
use wisp_providers::mock::MockProvider;

/// A host whose embeddings are bags of words (deterministic stand-in for Apple's).
struct EmbedHost;

#[async_trait]
impl wisp_providers::HostBridge for EmbedHost {
    async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        if method != "host.embed" {
            return Ok(json!({}));
        }
        let vocab = [
            "coffee", "roast", "tea", "dentist", "friday", "name", "editor",
        ];
        let vectors: Vec<Vec<f32>> = params["texts"]
            .as_array()
            .expect("texts")
            .iter()
            .map(|t| {
                let t = t.as_str().unwrap_or("").to_lowercase();
                let mut v: Vec<f32> = vocab
                    .iter()
                    .map(|w| if t.contains(w) { 1.0 } else { 0.0 })
                    .collect();
                v.push(0.1);
                v
            })
            .collect();
        Ok(json!({"model": "test-bow", "dims": 8, "vectors": vectors}))
    }
}

fn core(dir: &std::path::Path, host: bool) -> Core {
    let paths = DataPaths::new(dir);
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    core.ai().set_overrides(vec![Arc::new(MockProvider {
        id: "mock-local".into(),
        locality: Locality::Local,
        vision: false,
        reply: "<echo>".into(),
        calls: Default::default(),
        requests: Default::default(),
    }) as Arc<dyn wisp_ai::AiProvider>]);
    if host {
        core.attach_host(Arc::new(EmbedHost));
    }
    core
}

async fn ask(core: &Core, text: &str) -> (Value, String, Value) {
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch("conversation.send", json!({ "text": text }))
        .await
        .expect("send");
    let id = r["assistantMessage"]["id"].clone();
    loop {
        let env = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("timely")
            .expect("event");
        if let Envelope::Event { event, payload, .. } = env
            && event == "conversation.messageUpdated"
            && payload["id"] == id
            && payload["status"] != "streaming"
        {
            return (
                r,
                payload["content"].as_str().unwrap_or("").to_owned(),
                payload["context"]["memories"].clone(),
            );
        }
    }
}

#[tokio::test]
async fn stated_preferences_are_saved_visibly_and_recalled_later() {
    for with_embeddings in [false, true] {
        let dir = tempfile::tempdir().expect("tmp");
        let c = core(dir.path(), with_embeddings);

        // The user states a preference → saved, and the answer says so.
        let (r, _, _) = ask(&c, "I prefer dark roast coffee.").await;
        assert_eq!(
            r["assistantMessage"]["savedMemories"][0]["content"],
            "The user prefers dark roast coffee."
        );
        let list = c.dispatch("memory.list", json!({})).await.expect("list");
        assert_eq!(list["items"][0]["source"], "extracted");
        assert_eq!(list["items"][0]["category"], "preference");

        // A later, related conversation retrieves it; an unrelated one doesn't.
        let (_, system, used) = ask(&c, "Which coffee should I buy?").await;
        assert!(
            system.contains("- The user prefers dark roast coffee."),
            "{with_embeddings}: {system}"
        );
        assert_eq!(used, json!(["The user prefers dark roast coffee."]));
        let (_, system, _) = ask(&c, "When is my dentist appointment?").await;
        assert!(!system.contains("dark roast"), "{with_embeddings}");

        // Forget on request.
        ask(&c, "Forget that I prefer dark roast coffee").await;
        assert!(
            c.dispatch("memory.list", json!({})).await.expect("list")["items"]
                .as_array()
                .expect("arr")
                .is_empty()
        );
    }
}

#[tokio::test]
async fn memory_is_inspectable_editable_and_respects_settings() {
    let dir = tempfile::tempdir().expect("tmp");
    let c = core(dir.path(), false);
    let m = c
        .dispatch("memory.save", json!({"content": "Reports go in ~/Reports", "category": "fact", "level": "persistent"}))
        .await
        .expect("add");
    assert_eq!(m["source"], "user");
    let edited = c
        .dispatch("memory.save", json!({"id": m["id"], "content": "Reports go in ~/Documents/Reports", "category": "fact", "level": "temporary"}))
        .await
        .expect("edit");
    assert!(
        edited["expiresAtMs"].as_i64().is_some(),
        "temporary memories expire"
    );
    assert!(
        c.dispatch(
            "memory.save",
            json!({"content": "", "category": "fact", "level": "persistent"})
        )
        .await
        .is_err()
    );
    // Filter, then delete.
    assert_eq!(
        c.dispatch("memory.list", json!({"query": "Reports"}))
            .await
            .expect("q")["items"]
            .as_array()
            .expect("a")
            .len(),
        1
    );
    c.dispatch("memory.delete", json!({"id": m["id"]}))
        .await
        .expect("delete");

    // Learning off → nothing saved; memory off → nothing recalled.
    c.dispatch("settings.update", json!({"update": {"section": "memory", "value": {"enabled": true, "learnFromChats": false}}}))
        .await
        .expect("settings");
    let (r, _, _) = ask(&c, "My name is Suraj").await;
    assert!(r["assistantMessage"].get("savedMemories").is_none());
    c.dispatch("memory.save", json!({"content": "The user's name is Suraj.", "category": "userProfile", "level": "persistent"}))
        .await
        .expect("add");
    let (_, system, _) = ask(&c, "What's my name?").await;
    assert!(system.contains("Suraj"));
    c.dispatch("settings.update", json!({"update": {"section": "memory", "value": {"enabled": false, "learnFromChats": false}}}))
        .await
        .expect("settings");
    let (_, system, _) = ask(&c, "What's my name?").await;
    assert!(!system.contains("Suraj"));

    let cleared = c
        .dispatch("memory.clear", Value::Null)
        .await
        .expect("clear");
    assert_eq!(cleared["deleted"], 1);
}
