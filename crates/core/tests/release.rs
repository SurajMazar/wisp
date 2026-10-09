#![allow(clippy::expect_used)] // test harness

//! Backups and export (Phase 21).

use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;

#[tokio::test]
async fn backs_up_keeps_seven_and_exports_everything() {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let c = Core::new(
        Database::open(&paths.database_file()).expect("db"),
        &paths,
        EventBus::new(),
    )
    .expect("core");
    c.dispatch("memory.save", json!({"content": "The user prefers tea.", "category": "preference", "level": "persistent"}))
        .await
        .expect("memory");

    for _ in 0..9 {
        c.dispatch("backup.create", Value::Null)
            .await
            .expect("backup");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let list = c.dispatch("backup.list", Value::Null).await.expect("list");
    assert_eq!(list["backups"].as_array().expect("arr").len(), 7);
    // A backup is a working database with the data in it.
    let newest = dir
        .path()
        .join("backups")
        .join(list["backups"][0]["name"].as_str().expect("name"));
    let copy = Database::open(&newest).expect("open backup");
    assert_eq!(
        copy.list_memories(None, None).expect("mem")[0].content,
        "The user prefers tea."
    );

    let e = c
        .dispatch("data.export", Value::Null)
        .await
        .expect("export");
    assert_eq!(
        (e["memories"].clone(), e["skills"].clone()),
        (json!(1), json!(1))
    );
    let doc: Value =
        serde_json::from_slice(&std::fs::read(e["path"].as_str().expect("path")).expect("read"))
            .expect("json");
    assert_eq!(doc["format"], "wisp-export-1");
    assert_eq!(doc["skills"][0]["name"], "Mac keyboard shortcuts");
}
