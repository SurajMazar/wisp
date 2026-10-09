#![allow(clippy::expect_used, clippy::print_stdout)] // measurement harness

//! Performance measurements (Phase 19). Run with:
//!   cargo test --release -p wisp-core --test perf -- --ignored --nocapture
//! Prints a table and writes target/perf.json.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;
use wisp_ipc::IpcError;

/// Embeddings like Apple's: 512 dims, deterministic per text, costing ~1 ms per text.
struct FakeEmbedHost;

#[async_trait]
impl wisp_providers::HostBridge for FakeEmbedHost {
    async fn call(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        if method != "host.embed" {
            return Ok(json!({}));
        }
        let texts = params["texts"].as_array().cloned().unwrap_or_default();
        tokio::time::sleep(Duration::from_millis(texts.len() as u64)).await;
        let vectors: Vec<Vec<f32>> = texts
            .iter()
            .map(|t| {
                let s = t.as_str().unwrap_or("");
                let mut v = vec![0f32; 512];
                for (i, b) in s.bytes().enumerate() {
                    v[(i * 31 + b as usize) % 512] += 1.0;
                }
                v
            })
            .collect();
        Ok(json!({"model": "perf", "dims": 512, "vectors": vectors}))
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "measurement; run explicitly"]
async fn measure() {
    let mut results = serde_json::Map::new();
    let mut record = |name: &str, ms: f64| {
        println!("{name:<48} {ms:>9.2} ms");
        results.insert(name.to_owned(), json!(ms));
    };

    // Engine startup on a fresh data folder (migrations, seeding).
    let dir = tempfile::tempdir().expect("tmp");
    let t = Instant::now();
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let db = Database::open(&paths.database_file()).expect("db");
    let core = Core::new(db, &paths, EventBus::new()).expect("core");
    record(
        "engine startup (fresh, migrations + seed)",
        t.elapsed().as_secs_f64() * 1e3,
    );
    core.attach_host(Arc::new(FakeEmbedHost));

    // IPC dispatch round trip (decode → handle → encode).
    let mut samples = vec![];
    for _ in 0..2000 {
        let t = Instant::now();
        core.dispatch("app.info", Value::Null).await.expect("info");
        samples.push(t.elapsed().as_secs_f64() * 1e3);
    }
    record("dispatch app.info (median of 2000)", median(samples));

    // Settings read (done several times per question).
    let mut samples = vec![];
    for _ in 0..500 {
        let t = Instant::now();
        core.dispatch("settings.get", Value::Null)
            .await
            .expect("settings");
        samples.push(t.elapsed().as_secs_f64() * 1e3);
    }
    record("settings.get (median)", median(samples));

    // 2 000 memories, then recall.
    for i in 0..500 {
        core.dispatch("memory.save", json!({"content": format!("Fact number {i}: the project codename is item{i}."), "category": "fact", "level": "persistent"}))
            .await
            .expect("memory");
    }
    let mut samples = vec![];
    for i in 0..20 {
        let t = Instant::now();
        core.dispatch("memory.list", json!({"query": format!("item{i}")}))
            .await
            .expect("list");
        samples.push(t.elapsed().as_secs_f64() * 1e3);
    }
    record(
        "memory.list filtered, 500 memories (median)",
        median(samples),
    );

    // Files: 40 documents × ~50 passages.
    let doc: String = (0..2500)
        .map(|i| {
            format!("Line {i}: notes about budgets, plans, vacation policy and equipment rules.\n")
        })
        .collect();
    let t = Instant::now();
    for i in 0..40 {
        let text = format!("{doc}\nUnique marker zebra{i} lives here.\n");
        core.dispatch(
            "file.importData",
            json!({"name": format!("doc{i}.md"), "dataBase64": base64_encode(text.as_bytes())}),
        )
        .await
        .expect("import");
    }
    // Wait for indexing to finish.
    loop {
        let list = core.dispatch("file.list", Value::Null).await.expect("list");
        if list["items"]
            .as_array()
            .expect("items")
            .iter()
            .all(|f| f["status"] == "indexed")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    record(
        "import + index 40 docs (~180 KB each)",
        t.elapsed().as_secs_f64() * 1e3,
    );
    let chunks: u64 = core.dispatch("file.list", Value::Null).await.expect("list")["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|f| f["chunks"].as_u64().unwrap_or(0))
        .sum();
    println!("  ({chunks} passages indexed)");
    let mut samples = vec![];
    for i in 0..10 {
        let t = Instant::now();
        let r = core
            .dispatch(
                "file.search",
                json!({"query": format!("where is zebra{i}"), "limit": 5}),
            )
            .await
            .expect("search");
        samples.push(t.elapsed().as_secs_f64() * 1e3);
        if i == 0 {
            println!(
                "  top hit: {} | {}",
                r["hits"][0]["citation"], r["hits"][0]["score"]
            );
        }
    }
    record(
        &format!("file.search over {chunks} passages (median)"),
        median(samples),
    );

    // Conversations: list with 1 000 conversations.
    let db2 = Database::open(&paths.database_file()).expect("db");
    for i in 0..1000 {
        db2.create_conversation(&format!("Conversation {i}"))
            .expect("conv");
    }
    let mut samples = vec![];
    for _ in 0..20 {
        let t = Instant::now();
        core.dispatch("conversation.list", json!({"limit": 200}))
            .await
            .expect("list");
        samples.push(t.elapsed().as_secs_f64() * 1e3);
    }
    record(
        "conversation.list (1000 conversations, median)",
        median(samples),
    );

    // Scheduling math.
    let s =
        wisp_agents::schedule::Schedule::parse("0 9 * * 1-5", "Asia/Kathmandu").expect("schedule");
    let t = Instant::now();
    let mut at = 0i64;
    for _ in 0..1000 {
        at = s.next_after(at).expect("next");
    }
    record("schedule.next_after ×1000", t.elapsed().as_secs_f64() * 1e3);

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/perf.json");
    std::fs::write(
        out,
        serde_json::to_string_pretty(&Value::Object(results)).expect("json"),
    )
    .expect("write");
}

fn base64_encode(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}
