#![allow(clippy::expect_used)] // test harness

//! Skills (spec §64, stop gate 14): load, influence answers, disable, edit + versions, import,
//! survive restart, folders found on disk.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::{Envelope, types::models::Locality};
use wisp_providers::mock::MockProvider;

fn core(dir: &std::path::Path) -> Core {
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
    core
}

/// Ask and return (system prompt the model saw, context.skills).
async fn ask(core: &Core, text: &str) -> (String, Value) {
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
                payload["content"].as_str().unwrap_or("").to_owned(),
                payload["context"]["skills"].clone(),
            );
        }
    }
}

#[tokio::test]
async fn skills_load_affect_answers_toggle_version_and_survive_restart() {
    let dir = tempfile::tempdir().expect("tmp");
    let c = core(dir.path());
    let list = c.dispatch("skill.list", Value::Null).await.expect("list");
    let sample = list["skills"][0].clone();
    assert_eq!(sample["name"], "Mac keyboard shortcuts");
    assert_eq!(
        (sample["source"].clone(), sample["enabled"].clone()),
        (json!("builtin"), json!(true))
    );
    assert!(
        dir.path()
            .join("skills/mac-keyboard-shortcuts/SKILL.md")
            .is_file(),
        "mirrored to disk"
    );

    // Relevant → its instructions reach the model; irrelevant → they don't.
    let (system, used) = ask(&c, "Is there a keyboard shortcut to copy?").await;
    assert!(
        system.contains("## Mac keyboard shortcuts") && system.contains("Copy ⌘C"),
        "{system}"
    );
    assert_eq!(used, json!(["Mac keyboard shortcuts"]));
    let (system, _) = ask(&c, "What's the capital of France?").await;
    assert!(!system.contains("Mac keyboard shortcuts"));

    // Disabled → never used.
    c.dispatch(
        "skill.setEnabled",
        json!({"id": sample["id"], "enabled": false}),
    )
    .await
    .expect("disable");
    let (system, _) = ask(&c, "keyboard shortcut please").await;
    assert!(!system.contains("Mac keyboard shortcuts"));

    // Edit → new version; restore → back as another version.
    let edited = wisp_agents::skills::SAMPLE_SKILL.replace("Copy ⌘C", "Copy is ⌘C (edited)");
    let s = c
        .dispatch("skill.save", json!({"id": sample["id"], "content": edited}))
        .await
        .expect("edit");
    assert_eq!(s["summary"]["version"], 2);
    assert!(
        std::fs::read_to_string(dir.path().join("skills/mac-keyboard-shortcuts/SKILL.md"))
            .expect("file")
            .contains("(edited)")
    );
    let r = c
        .dispatch("skill.restore", json!({"id": sample["id"], "version": 1}))
        .await
        .expect("restore");
    assert_eq!(r["summary"]["version"], 3);
    assert!(!r["content"].as_str().expect("content").contains("(edited)"));
    assert_eq!(
        c.dispatch("skill.versions", json!({"id": sample["id"]}))
            .await
            .expect("v")["versions"]
            .as_array()
            .expect("arr")
            .len(),
        3
    );

    // Imports start disabled; invalid files are rejected with a reason.
    let imported = c
        .dispatch("skill.import", json!({"content": "---\nname: Docker helper\ntriggers: [docker]\n---\nUse docker compose."}))
        .await
        .expect("import");
    assert_eq!(
        (
            imported["summary"]["source"].clone(),
            imported["summary"]["enabled"].clone()
        ),
        (json!("imported"), json!(false))
    );
    let err = c
        .dispatch("skill.import", json!({"content": "not a skill"}))
        .await
        .expect_err("invalid");
    assert!(err.message.contains("front matter"), "{}", err.message);
    let export = c
        .dispatch("skill.export", json!({"id": imported["summary"]["id"]}))
        .await
        .expect("export");
    assert_eq!(export["filename"], "docker-helper.SKILL.md");

    // A folder dropped on disk is found at the next start (disabled); nothing is duplicated.
    std::fs::create_dir_all(dir.path().join("skills/git")).expect("dir");
    std::fs::write(
        dir.path().join("skills/git/SKILL.md"),
        "---\nname: Git\n---\nCommit often.",
    )
    .expect("write");
    drop(c);
    let c = core(dir.path());
    let names: Vec<(String, bool, u64)> =
        c.dispatch("skill.list", Value::Null).await.expect("list")["skills"]
            .as_array()
            .expect("arr")
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().unwrap_or("").to_owned(),
                    s["enabled"].as_bool().unwrap_or(true),
                    s["version"].as_u64().unwrap_or(0),
                )
            })
            .collect();
    assert_eq!(
        names,
        vec![
            ("Docker helper".into(), false, 1),
            ("Git".into(), false, 1),
            ("Mac keyboard shortcuts".into(), false, 3)
        ]
    );
}
