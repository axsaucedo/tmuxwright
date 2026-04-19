//! End-to-end test: drive the `tmuxwright-engine` binary as a subprocess
//! using the same framed JSON-RPC transport the TS SDK uses. Requires
//! tmux on PATH; skips with a log otherwise.

#![allow(clippy::needless_pass_by_value)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use tmuxwright_tmux::detect;

fn engine_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tmuxwright-engine"))
}

fn tmux_available() -> bool {
    detect().is_ok()
}

struct Driver {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Driver {
    fn spawn() -> Self {
        let mut child = Command::new(engine_binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn engine");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let body = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id,
        }))
        .unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
        self.read_response(id)
    }

    fn read_response(&mut self, expect_id: u64) -> Value {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).unwrap();
            if line == "\r\n" || line == "\n" {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case("Content-Length") {
                    content_length = Some(v.trim().parse().unwrap());
                }
            }
        }
        let mut buf = vec![0u8; content_length.expect("content-length")];
        self.stdout.read_exact(&mut buf).unwrap();
        let v: Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["id"].as_u64(), Some(expect_id), "id mismatch: {v}");
        v
    }

    fn shutdown(mut self) {
        let _ = self.call("engine.shutdown", json!({}));
        std::thread::sleep(Duration::from_millis(100));
        let _ = self.child.wait();
    }
}

#[test]
fn full_lifecycle_against_real_tmux() {
    if !tmux_available() {
        eprintln!("tmux not available; skipping");
        return;
    }

    let mut d = Driver::spawn();

    let hs = d.call("engine.handshake", json!({}));
    assert_eq!(hs["result"]["protocol"], "1");
    assert_eq!(hs["result"]["name"], "tmuxwright-engine");

    let launch = d.call(
        "engine.launch",
        json!({
            "command": ["bash", "-lc", "echo integration-ok; sleep 999"],
            "width": 80,
            "height": 24,
        }),
    );
    let sid = launch["result"]["session_id"].as_str().unwrap().to_string();
    assert!(launch["result"]["reconnect"]
        .as_str()
        .unwrap()
        .contains("attach"));

    std::thread::sleep(Duration::from_millis(300));

    let wait = d.call(
        "engine.wait_stable",
        json!({"session_id": sid, "quiet_ms": 200, "timeout_ms": 3000}),
    );
    assert_eq!(wait["result"]["status"], "stable");

    let snap = d.call("engine.snapshot", json!({"session_id": sid}));
    // tmux subtracts a row for its status bar, so height is typically
    // 1 less than what we requested at launch.
    assert!(snap["result"]["width"].as_u64().unwrap() == 80);
    assert!(snap["result"]["height"].as_u64().unwrap() >= 20);
    assert_eq!(snap["result"]["hash"].as_str().unwrap().len(), 64);
    assert!(snap["result"]["text"]
        .as_str()
        .unwrap()
        .contains("integration-ok"));

    let hit = d.call(
        "engine.assert_text",
        json!({"session_id": sid, "contains": "integration-ok"}),
    );
    assert_eq!(hit["result"]["matched"], true);
    let miss = d.call(
        "engine.assert_text",
        json!({"session_id": sid, "contains": "will-never-appear-xyz"}),
    );
    assert_eq!(miss["result"]["matched"], false);

    let preserve = d.call("engine.preserve", json!({"session_id": sid}));
    assert!(preserve["result"]["reconnect"]
        .as_str()
        .unwrap()
        .contains("attach"));

    d.call("engine.close", json!({"session_id": sid}));
    d.shutdown();
}

#[test]
fn unknown_method_returns_rpc_error() {
    let mut d = Driver::spawn();
    let resp = d.call("engine.does_not_exist", json!({}));
    assert!(resp["error"].is_object(), "expected error: {resp}");
    assert_eq!(resp["error"]["code"], -32601);
    d.shutdown();
}
