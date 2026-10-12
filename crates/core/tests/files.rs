#![allow(clippy::expect_used)] // test harness

//! Files & semantic search (stop gate 16): import a document, ask a question answered in it —
//! the right document and passage are found, the answer is grounded and cited, Local Only stays
//! local.

use std::sync::{Arc, atomic::Ordering};

use base64::Engine;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, types::models::Locality};
use wisp_providers::mock::MockProvider;

fn core(dir: &std::path::Path, provider: Arc<MockProvider>) -> Core {
    let paths = DataPaths::new(dir);
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    core.ai()
        .set_overrides(vec![provider as Arc<dyn wisp_ai::AiProvider>]);
    core
}

fn mock(locality: Locality, reply: &str) -> Arc<MockProvider> {
    Arc::new(MockProvider {
        id: if locality == Locality::Local {
            "mock-local".into()
        } else {
            "mock-cloud".into()
        },
        locality,
        vision: false,
        reply: reply.into(),
        calls: Default::default(),
        requests: Default::default(),
    })
}

const HANDBOOK: &str = "# Acme handbook\n\n## Office\nThe office is open 9 to 6 on weekdays.\nBring your badge.\n\n## Time off\nEmployees get 25 vacation days per year.\nRequests go to the HR portal at least two weeks ahead.\n\n## Equipment\nLaptops are replaced every three years.\n";
const RECIPES: &str =
    "Pancakes: mix flour, eggs and milk. Fry in butter until golden.\nServe with maple syrup.\n";

async fn import(c: &Core, name: &str, text: &str) -> Value {
    let mut rx = c.events().subscribe();
    let f = c
        .dispatch("file.importData", json!({"name": name, "dataBase64": base64::engine::general_purpose::STANDARD.encode(text)}))
        .await
        .expect("import");
    if f["status"] == "indexed" {
        return f;
    }
    loop {
        let env = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("timely")
            .expect("event");
        if let Envelope::Event { event, payload, .. } = env
            && event == "file.updated"
            && payload["id"] == f["id"]
            && (payload["status"] == "indexed" || payload["status"] == "failed")
        {
            return payload;
        }
    }
}

async fn ask(c: &Core, text: &str) -> Value {
    let mut rx = c.events().subscribe();
    let r = c
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
            return payload;
        }
    }
}

#[tokio::test]
async fn finds_the_right_document_grounds_and_cites_the_answer() {
    let dir = tempfile::tempdir().expect("tmp");
    let c = core(dir.path(), mock(Locality::Local, "<echo>"));
    let handbook = import(&c, "../../Acme handbook.md", HANDBOOK).await;
    assert_eq!(handbook["status"], "indexed");
    assert_eq!(handbook["name"], "Acme handbook.md", "name sanitized");
    import(&c, "recipes.txt", RECIPES).await;

    // Correct document and passage found.
    let r = c
        .dispatch(
            "file.search",
            json!({"query": "How many vacation days do I get?", "limit": 3}),
        )
        .await
        .expect("search");
    assert_eq!(r["hits"][0]["name"], "Acme handbook.md");
    assert!(
        r["hits"][0]["snippet"]
            .as_str()
            .expect("snippet")
            .contains("25 vacation days")
    );
    assert!(
        r["hits"][0]["citation"]
            .as_str()
            .expect("cite")
            .starts_with("Acme handbook.md, lines")
    );

    // The answer is grounded in it, with a citation attached.
    let a = ask(&c, "How many vacation days do I get?").await;
    let system = a["content"].as_str().expect("content");
    assert!(
        system.contains("<documents>")
            && system.contains("Employees get 25 vacation days per year."),
        "{system}"
    );
    assert!(
        !system.contains("Pancakes"),
        "unrelated document not included"
    );
    assert_eq!(a["citations"][0]["n"], 1);
    assert_eq!(a["citations"][0]["fileId"], handbook["id"]);
    assert!(
        a["context"]["files"][0]
            .as_str()
            .expect("label")
            .starts_with("Acme handbook.md")
    );
    // Unrelated questions get no documents.
    assert!(ask(&c, "Tell me a joke").await.get("citations").is_none());

    // Same content isn't stored twice; unsupported types are refused; delete removes it everywhere.
    let again = import(&c, "copy.md", HANDBOOK).await;
    assert_eq!(again["id"], handbook["id"]);
    let err = c
        .dispatch(
            "file.importData",
            json!({"name": "setup.exe", "dataBase64": "AAAA"}),
        )
        .await
        .expect_err("refused");
    assert!(err.message.contains("can't read"));
    c.dispatch("file.delete", json!({"id": handbook["id"]}))
        .await
        .expect("delete");
    let r = c
        .dispatch("file.search", json!({"query": "vacation days", "limit": 3}))
        .await
        .expect("search");
    assert!(
        r["hits"]
            .as_array()
            .expect("hits")
            .iter()
            .all(|h| h["name"] != "Acme handbook.md")
    );
    let left: Vec<_> = std::fs::read_dir(dir.path().join("files"))
        .expect("dir")
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(left.len(), 1, "only recipes.txt remains on disk: {left:?}");
}

#[tokio::test]
async fn local_only_never_sends_documents_to_the_cloud() {
    let dir = tempfile::tempdir().expect("tmp");
    let cloud = mock(Locality::Cloud, "leaked");
    let c = core(dir.path(), cloud.clone());
    import(&c, "handbook.md", HANDBOOK).await;
    // Local Only is the default: with only a cloud model, nothing is sent.
    let a = ask(&c, "How many vacation days do I get?").await;
    assert_eq!(a["status"], "error");
    assert_eq!(
        cloud.calls.load(Ordering::SeqCst),
        0,
        "the cloud model was never called"
    );
}
