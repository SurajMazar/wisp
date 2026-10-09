#![allow(clippy::expect_used)] // test harness

//! Conversation engine against deterministic mock providers: streaming, persistence,
//! cancellation, privacy enforcement.

use std::{sync::Arc, time::Duration};

use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, types::models::Locality};
use wisp_providers::mock::MockProvider;

fn core(dir: &std::path::Path, providers: Vec<MockProvider>) -> Core {
    let paths = DataPaths::new(dir);
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    core.ai().set_overrides(
        providers
            .into_iter()
            .map(|p| Arc::new(p) as Arc<dyn wisp_ai::AiProvider>)
            .collect(),
    );
    core
}

fn local(reply: &str) -> MockProvider {
    MockProvider {
        id: "mock-local".into(),
        locality: Locality::Local,
        vision: true,
        reply: reply.into(),
    }
}

/// Wait for the assistant message to leave `streaming`; returns (final message, deltas seen).
async fn wait_done(
    rx: &mut tokio::sync::broadcast::Receiver<Envelope>,
    id: &str,
) -> (Value, String) {
    let mut deltas = String::new();
    loop {
        let env = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("timely")
            .expect("event");
        if let Envelope::Event { event, payload, .. } = env {
            if event == "conversation.delta" && payload["messageId"] == id {
                deltas.push_str(payload["delta"].as_str().expect("delta"));
            }
            if event == "conversation.messageUpdated"
                && payload["id"] == id
                && payload["status"] != "streaming"
            {
                return (payload, deltas);
            }
        }
    }
}

#[tokio::test]
async fn streams_persists_and_continues_a_conversation() {
    let dir = tempfile::tempdir().expect("tmp");
    let core = core(dir.path(), vec![local("Two plus two is four.")]);
    let mut rx = core.events().subscribe();

    let r = core
        .dispatch("conversation.send", json!({ "text": "What is 2 + 2?" }))
        .await
        .expect("send");
    let conv = r["conversationId"].as_str().expect("id").to_owned();
    let aid = r["assistantMessage"]["id"]
        .as_str()
        .expect("aid")
        .to_owned();
    assert_eq!(r["assistantMessage"]["status"], "streaming");

    let (done, deltas) = wait_done(&mut rx, &aid).await;
    assert_eq!(done["status"], "complete");
    assert_eq!(done["content"], "Two plus two is four.");
    assert_eq!(deltas.trim(), "Two plus two is four.");
    assert_eq!(done["locality"], "local");
    assert_eq!(done["modelName"], "mock");

    // Continue the same conversation, then confirm everything persisted in order.
    let r2 = core
        .dispatch(
            "conversation.send",
            json!({ "conversationId": conv, "text": "And 3 + 3?" }),
        )
        .await
        .expect("send");
    wait_done(&mut rx, r2["assistantMessage"]["id"].as_str().expect("aid")).await;
    let got = core
        .dispatch("conversation.get", json!({ "id": conv }))
        .await
        .expect("get");
    let msgs = got["messages"].as_array().expect("messages");
    assert_eq!(msgs.len(), 4);
    assert_eq!(msgs[0]["content"], "What is 2 + 2?");
    assert_eq!(msgs[2]["content"], "And 3 + 3?");
    assert_eq!(got["title"], "What is 2 + 2?");

    let list = core
        .dispatch("conversation.list", json!({ "limit": 10 }))
        .await
        .expect("list");
    assert_eq!(list["items"][0]["messageCount"], 4);
}

#[tokio::test]
async fn cancelling_keeps_partial_text_and_marks_cancelled() {
    let dir = tempfile::tempdir().expect("tmp");
    let long = "word ".repeat(400);
    let core = core(dir.path(), vec![local(&long)]);
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch(
            "conversation.send",
            json!({ "text": "Tell me a long story" }),
        )
        .await
        .expect("send");
    let aid = r["assistantMessage"]["id"]
        .as_str()
        .expect("aid")
        .to_owned();
    tokio::time::sleep(Duration::from_millis(80)).await;
    core.dispatch("conversation.cancel", json!({ "id": aid }))
        .await
        .expect("cancel");
    let (done, _) = wait_done(&mut rx, &aid).await;
    assert_eq!(done["status"], "cancelled");
    let len = done["content"].as_str().expect("content").len();
    assert!(
        len > 0 && len < long.trim().len(),
        "partial text kept ({len})"
    );
}

#[tokio::test]
async fn local_only_refuses_cloud_and_says_why() {
    let dir = tempfile::tempdir().expect("tmp");
    let cloud = MockProvider {
        id: "mock-cloud".into(),
        locality: Locality::Cloud,
        vision: true,
        reply: "leaked".into(),
    };
    let core = core(dir.path(), vec![cloud]);
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch(
            "conversation.send",
            json!({ "text": "hello", "allowCloud": true }),
        )
        .await
        .expect("send");
    let (done, deltas) =
        wait_done(&mut rx, r["assistantMessage"]["id"].as_str().expect("aid")).await;
    assert_eq!(done["status"], "error");
    assert!(
        done["error"]
            .as_str()
            .expect("error")
            .contains("Local Only")
    );
    assert!(
        deltas.is_empty(),
        "nothing may be generated by a cloud model in Local Only"
    );
}

#[tokio::test]
async fn validates_input() {
    let dir = tempfile::tempdir().expect("tmp");
    let core = core(dir.path(), vec![local("x")]);
    assert!(
        core.dispatch("conversation.send", json!({ "text": "   " }))
            .await
            .is_err()
    );
    assert!(
        core.dispatch("conversation.send", json!({ "text": "x".repeat(9000) }))
            .await
            .is_err()
    );
    assert!(
        core.dispatch(
            "conversation.send",
            json!({ "text": "hi", "conversationId": "../../x" })
        )
        .await
        .is_err()
    );
    assert!(
        core.dispatch(
            "conversation.get",
            json!({ "id": "0199c2a8-0000-7000-8000-000000000001" })
        )
        .await
        .is_err()
    );
}

/// A provider that fails before producing any text.
struct Broken;

#[async_trait::async_trait]
impl wisp_ai::AiProvider for Broken {
    fn id(&self) -> &str {
        "broken"
    }
    fn kind(&self) -> wisp_ipc::types::models::ProviderKind {
        wisp_ipc::types::models::ProviderKind::Apple
    }
    fn locality(&self) -> Locality {
        Locality::Local
    }
    async fn info(&self) -> wisp_ipc::types::models::ProviderInfo {
        wisp_ipc::types::models::ProviderInfo {
            id: "broken".into(),
            kind: self.kind(),
            name: "Broken".into(),
            locality: Locality::Local,
            state: wisp_ipc::types::models::ProviderState::Available,
            detail: None,
            version: None,
        }
    }
    async fn models(&self) -> Result<Vec<wisp_ipc::types::models::ModelInfo>, wisp_ai::AiError> {
        Ok(vec![wisp_ipc::types::models::ModelInfo {
            provider_id: "broken".into(),
            id: "system".into(),
            name: "broken".into(),
            locality: Locality::Local,
            capabilities: vec![wisp_ipc::types::models::ModelCapability::Text],
            size_bytes: None,
            context_length: Some(4096),
            parameter_size: None,
            family: None,
        }])
    }
    async fn generate(
        &self,
        _: wisp_ai::GenerateRequest,
        _: tokio_util::sync::CancellationToken,
    ) -> Result<wisp_ai::ChunkStream, wisp_ai::AiError> {
        Ok(Box::pin(futures::stream::once(async {
            Err(wisp_ai::AiError::unavailable(
                "busy",
                "ModelManagerError 1013",
            ))
        })))
    }
}

#[tokio::test]
async fn falls_back_to_another_local_model_when_the_first_fails() {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    // Apple-kind providers win routing for short text, so Broken is tried first.
    core.ai()
        .set_overrides(vec![Arc::new(Broken), Arc::new(local("Fallback answer."))]);
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch("conversation.send", json!({ "text": "hi" }))
        .await
        .expect("send");
    let (done, _) = wait_done(&mut rx, r["assistantMessage"]["id"].as_str().expect("aid")).await;
    assert_eq!(done["status"], "complete", "{done}");
    assert_eq!(done["content"], "Fallback answer.");
}

#[tokio::test]
async fn regenerate_replaces_the_answer() {
    let dir = tempfile::tempdir().expect("tmp");
    let core = core(dir.path(), vec![local("First answer.")]);
    let mut rx = core.events().subscribe();
    let r = core
        .dispatch("conversation.send", json!({ "text": "Question?" }))
        .await
        .expect("send");
    let conv = r["conversationId"].as_str().expect("id").to_owned();
    let first = r["assistantMessage"]["id"]
        .as_str()
        .expect("aid")
        .to_owned();
    wait_done(&mut rx, &first).await;
    let again = core
        .dispatch("conversation.regenerate", json!({ "messageId": first }))
        .await
        .expect("regenerate");
    let second = again["id"].as_str().expect("id").to_owned();
    assert_ne!(second, first);
    let (done, _) = wait_done(&mut rx, &second).await;
    assert_eq!(done["status"], "complete");
    let got = core
        .dispatch("conversation.get", json!({ "id": conv }))
        .await
        .expect("get");
    let msgs = got["messages"].as_array().expect("messages");
    assert_eq!(msgs.len(), 2, "old answer replaced, question kept");
    assert_eq!(msgs[1]["id"], second);
}
