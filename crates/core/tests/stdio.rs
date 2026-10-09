//! Drives the real `wisp-core` binary over stdio — the same protocol the Swift host uses.
#![allow(clippy::expect_used)] // test harness: failures should panic with context

use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use serde_json::{Value, json};

struct Proc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Proc {
    fn spawn(data_dir: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_wisp-core"))
            .args(["--data-dir", data_dir.to_str().expect("utf8")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn wisp-core");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut p = Proc {
            child,
            stdin,
            stdout,
        };
        let ready = p.next_frame();
        assert_eq!(ready["kind"], "event");
        assert_eq!(ready["event"], "core.ready");
        p
    }

    fn send_raw(&mut self, line: &str) {
        self.stdin.write_all(line.as_bytes()).expect("write");
        self.stdin.write_all(b"\n").expect("write");
        self.stdin.flush().expect("flush");
    }

    fn next_frame(&mut self) -> Value {
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).expect("read");
        assert!(n > 0, "wisp-core closed stdout");
        serde_json::from_str(&line).expect("valid json frame")
    }

    /// Send a request and return its response, skipping interleaved events.
    fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        self.send_raw(
            &json!({"kind":"request","v":1,"id":id,"method":method,"params":params}).to_string(),
        );
        loop {
            let f = self.next_frame();
            if f["kind"] == "response" && f["id"] == id {
                return f;
            }
        }
    }

    fn shutdown(mut self) {
        drop(self.stdin);
        let status = self.child.wait().expect("wait");
        assert!(status.success(), "wisp-core exited with {status}");
    }
}

#[test]
fn settings_persist_across_restart() {
    let dir = tempfile::tempdir().expect("tempdir");

    let mut p = Proc::spawn(dir.path());
    let info = p.call("1", "app.info", Value::Null);
    assert_eq!(info["result"]["protocolVersion"], 1);
    assert!(info["result"]["schemaVersion"].as_u64().expect("schema") >= 7);

    let r = p.call(
        "2",
        "settings.update",
        json!({"update": {"section": "appearance", "value": {"theme": "dark", "accent": "purple"}}}),
    );
    assert_eq!(r["result"]["appearance"]["theme"], "dark", "{r}");
    p.shutdown();

    let mut p = Proc::spawn(dir.path());
    let r = p.call("1", "settings.get", json!({}));
    assert_eq!(r["result"]["appearance"]["theme"], "dark");
    assert_eq!(r["result"]["appearance"]["accent"], "purple");
    assert_eq!(r["result"]["privacy"]["mode"], "localOnly");
    p.shutdown();
}

#[test]
fn bad_input_is_rejected_without_crashing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut p = Proc::spawn(dir.path());

    p.send_raw("this is not json");
    p.send_raw(r#"{"kind":"request","v":2,"id":"x","method":"app.info"}"#);

    let r = p.call("a", "no.such.method", Value::Null);
    assert_eq!(r["error"]["code"], "methodNotFound");

    let r = p.call(
        "b",
        "settings.update",
        json!({"update": {"section": "appearance", "value": {"theme": "neon"}}}),
    );
    assert_eq!(r["error"]["code"], "invalidParams");

    let r = p.call(
        "c",
        "settings.update",
        json!({"update": {"section": "appearance", "value": {"customColor": "red"}}}),
    );
    assert_eq!(r["error"]["code"], "invalidParams");

    let r = p.call("d", "settings.get", json!({"unexpected": true}));
    assert_eq!(r["error"]["code"], "invalidParams");

    // Still healthy.
    let r = p.call("e", "app.info", Value::Null);
    assert!(r["result"].is_object());
    p.shutdown();
}

#[test]
fn settings_update_emits_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut p = Proc::spawn(dir.path());
    p.send_raw(
        &json!({"kind":"request","v":1,"id":"1","method":"settings.update",
                "params":{"update":{"section":"privacy","value":{"mode":"localPreferred"}}}})
        .to_string(),
    );
    let mut saw_event = false;
    let mut saw_response = false;
    while !(saw_event && saw_response) {
        let f = p.next_frame();
        if f["kind"] == "event" && f["event"] == "settings.changed" {
            assert_eq!(f["payload"]["privacy"]["mode"], "localPreferred");
            saw_event = true;
        }
        if f["kind"] == "response" {
            saw_response = true;
        }
    }
    p.shutdown();
}

#[test]
fn host_invocation_events_are_recorded_and_junk_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut p = Proc::spawn(dir.path());
    p.send_raw(
        &json!({"kind":"event","v":1,"event":"invocation.started",
                "payload":{"source":"keyboard","behavior":"hold","atMs":1}})
        .to_string(),
    );
    p.send_raw(&json!({"kind":"event","v":1,"event":"invocation.started","payload":{"source":"telepathy"}}).to_string());
    p.send_raw(&json!({"kind":"event","v":1,"event":"made.up","payload":{}}).to_string());
    // Events are processed in order before this request.
    let _ = p.call("1", "app.info", Value::Null);
    p.shutdown();

    let db =
        wisp_database::Database::open(&dir.path().join("database/wisp.sqlite3")).expect("open db");
    assert_eq!(db.count_events("invocation.started").expect("count"), 1);
    assert_eq!(db.count_events("made.up").expect("count"), 0);
}

#[test]
fn focus_regions_are_validated_kept_in_memory_and_logged_without_pixels() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut p = Proc::spawn(dir.path());
    let id = "0199c2a8-0000-7000-8000-000000000001";
    let region = json!({
        "id": id, "kind": "rectangle",
        "display": {"id": 1, "name": "Built-in", "frame": {"x":0,"y":0,"width":1512,"height":982}, "scale": 2.0},
        "selection": {"x":100,"y":100,"width":200,"height":100},
        "crop": {"x":76,"y":76,"width":248,"height":148},
        "cursor": {"x":300,"y":200},
        "image": {"url": format!("wisp://focus/{id}.png"), "widthPx": 496, "heightPx": 296},
        "createdAtMs": 1
    });
    p.send_raw(
        &json!({"kind":"event","v":1,"event":"focus.completed","payload": region}).to_string(),
    );
    let latest = p.call("1", "focus.latest", Value::Null);
    assert_eq!(latest["result"]["region"]["id"], id, "{latest}");

    // An inconsistent region (1x pixels on a 2x display) is rejected and doesn't replace it.
    let mut bad = region.clone();
    bad["id"] = json!("0199c2a8-0000-7000-8000-000000000002");
    bad["image"]["widthPx"] = json!(248);
    p.send_raw(&json!({"kind":"event","v":1,"event":"focus.completed","payload": bad}).to_string());
    let latest = p.call("2", "focus.latest", Value::Null);
    assert_eq!(latest["result"]["region"]["id"], id);

    p.send_raw(&json!({"kind":"event","v":1,"event":"focus.cleared","payload": {}}).to_string());
    let latest = p.call("3", "focus.latest", Value::Null);
    assert!(latest["result"]["region"].is_null());
    p.shutdown();

    let db =
        wisp_database::Database::open(&dir.path().join("database/wisp.sqlite3")).expect("open db");
    assert_eq!(db.count_events("focus.completed").expect("count"), 1);
}
