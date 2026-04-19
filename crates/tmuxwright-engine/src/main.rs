//! Tmuxwright engine daemon.
//!
//! One long-lived process per test run. Owns tmux sessions, the core
//! engine's snapshot/stability/locator logic, and speaks JSON-RPC 2.0
//! to a client (the TypeScript SDK, or any other language that can
//! frame messages). Framing is LSP-style Content-Length, so the same
//! code that drives adapter processes drives this one.

#![allow(
    clippy::needless_pass_by_value,
    clippy::doc_markdown,
    clippy::unnecessary_wraps,
    clippy::too_many_lines
)]

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tmuxwright_core::snapshot::Snapshot;
use tmuxwright_rpc::server::{serve, Handler};
use tmuxwright_rpc::RpcError;
use tmuxwright_term::locator::TextLocator;
use tmuxwright_tmux::capture::{
    capture_visible_plain, capture_with_scrollback_ansi, pane_geometry,
};
use tmuxwright_tmux::detect::detect;
use tmuxwright_tmux::input::{send_keys as tmux_send_keys, type_text, Key};
use tmuxwright_tmux::session::{Session, SessionOptions};

const ENGINE_PROTOCOL: &str = "1";

#[derive(Debug, Deserialize)]
struct LaunchParams {
    command: Vec<String>,
    #[serde(default)]
    width: Option<u16>,
    #[serde(default)]
    height: Option<u16>,
}

#[derive(Debug, Serialize)]
struct LaunchResult {
    session_id: String,
    socket: String,
    pane_id: String,
    reconnect: String,
}

#[derive(Debug, Deserialize)]
struct SessionIdParams {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct SendKeysParams {
    session_id: String,
    keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TypeParams {
    session_id: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct SnapshotParams {
    session_id: String,
    #[serde(default)]
    with_scrollback: bool,
}

#[derive(Debug, Serialize)]
struct SnapshotResult {
    text: String,
    hash: String,
    width: u16,
    height: u16,
}

#[derive(Debug, Deserialize)]
struct WaitStableParams {
    session_id: String,
    #[serde(default = "default_quiet")]
    quiet_ms: u64,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_quiet() -> u64 {
    250
}
fn default_timeout() -> u64 {
    5_000
}

#[derive(Debug, Serialize)]
struct WaitStableResult {
    status: &'static str,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct AssertTextParams {
    session_id: String,
    contains: String,
}

#[derive(Debug, Serialize)]
struct AssertTextResult {
    matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<[u16; 4]>,
}

#[derive(Debug, Serialize)]
struct PreserveResult {
    reconnect: String,
}

struct Engine {
    sessions: HashMap<String, Session>,
    next_id: u64,
    stop: bool,
}

impl Engine {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
            stop: false,
        }
    }

    fn new_session_id(&mut self) -> String {
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        id
    }

    fn session_mut(&mut self, id: &str) -> Result<&mut Session, RpcError> {
        self.sessions
            .get_mut(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))
    }

    fn session(&self, id: &str) -> Result<&Session, RpcError> {
        self.sessions
            .get(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))
    }

    fn do_snapshot(&self, id: &str, with_scrollback: bool) -> Result<Snapshot, RpcError> {
        let s = self.session(id)?;
        let geom = pane_geometry(s).map_err(internal)?;
        if with_scrollback {
            let ansi = capture_with_scrollback_ansi(s).map_err(internal)?;
            Ok(Snapshot::from_ansi(
                geom.width,
                geom.height,
                ansi.as_bytes(),
            ))
        } else {
            let plain = capture_visible_plain(s).map_err(internal)?;
            Ok(Snapshot::from_plain(geom.width, geom.height, &plain))
        }
    }
}

fn invalid_params(msg: impl Into<String>) -> RpcError {
    RpcError::new(RpcError::INVALID_PARAMS, msg)
}
fn internal<E: std::error::Error>(e: E) -> RpcError {
    RpcError::new(RpcError::INTERNAL_ERROR, e.to_string())
}
fn internal_display<E: std::fmt::Display>(e: E) -> RpcError {
    RpcError::new(RpcError::INTERNAL_ERROR, e.to_string())
}

fn parse<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|e| invalid_params(e.to_string()))
}

impl Handler for Engine {
    fn handle(&mut self, method_name: &str, params: Value) -> Result<Value, RpcError> {
        match method_name {
            "engine.handshake" => Ok(serde_json::json!({
                "name": "tmuxwright-engine",
                "version": env!("CARGO_PKG_VERSION"),
                "protocol": ENGINE_PROTOCOL,
            })),
            "engine.launch" => self.launch(parse(params)?),
            "engine.send_keys" => self.send_keys(parse(params)?),
            "engine.type" => self.type_text(parse(params)?),
            "engine.snapshot" => self.snapshot(parse(params)?),
            "engine.wait_stable" => self.wait_stable(parse(params)?),
            "engine.assert_text" => self.assert_text(parse(params)?),
            "engine.preserve" => self.preserve(parse(params)?),
            "engine.close" => self.close(parse(params)?),
            "engine.shutdown" => {
                self.stop = true;
                Ok(Value::Null)
            }
            other => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method: {other}"),
            )),
        }
    }

    fn should_stop(&self) -> bool {
        self.stop
    }
}

impl Engine {
    fn launch(&mut self, p: LaunchParams) -> Result<Value, RpcError> {
        if p.command.is_empty() {
            return Err(invalid_params("command must be non-empty"));
        }
        let tmux = detect().map_err(internal_display)?;
        let opts = SessionOptions {
            width: p.width.unwrap_or(120),
            height: p.height.unwrap_or(40),
            command: p.command,
        };
        let session = Session::create(tmux, &opts).map_err(internal)?;
        let id = self.new_session_id();
        let out = LaunchResult {
            session_id: id.clone(),
            socket: session.socket().to_string(),
            pane_id: session.pane_id().to_string(),
            reconnect: session.reconnect_command(),
        };
        self.sessions.insert(id, session);
        Ok(serde_json::to_value(out).unwrap())
    }

    fn send_keys(&mut self, p: SendKeysParams) -> Result<Value, RpcError> {
        let s = self.session_mut(&p.session_id)?;
        let keys: Vec<Key> = p.keys.into_iter().map(Key).collect();
        tmux_send_keys(s, &keys).map_err(internal)?;
        Ok(serde_json::json!({}))
    }

    fn type_text(&mut self, p: TypeParams) -> Result<Value, RpcError> {
        let s = self.session_mut(&p.session_id)?;
        type_text(s, &p.text).map_err(internal)?;
        Ok(serde_json::json!({}))
    }

    fn snapshot(&mut self, p: SnapshotParams) -> Result<Value, RpcError> {
        let snap = self.do_snapshot(&p.session_id, p.with_scrollback)?;
        Ok(serde_json::to_value(SnapshotResult {
            text: snap.grid.to_text(),
            hash: snap.hash.hex(),
            width: snap.grid.width(),
            height: snap.grid.height(),
        })
        .unwrap())
    }

    fn wait_stable(&mut self, p: WaitStableParams) -> Result<Value, RpcError> {
        let deadline = Instant::now() + Duration::from_millis(p.timeout_ms);
        let quiet = Duration::from_millis(p.quiet_ms);
        let mut last_hash = self.do_snapshot(&p.session_id, false)?.hash;
        let mut unchanged_since = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let snap = self.do_snapshot(&p.session_id, false)?;
            if snap.hash != last_hash {
                last_hash = snap.hash;
                unchanged_since = Instant::now();
            }
            if unchanged_since.elapsed() >= quiet {
                return Ok(serde_json::to_value(WaitStableResult {
                    status: "stable",
                    hash: last_hash.hex(),
                })
                .unwrap());
            }
            if Instant::now() >= deadline {
                return Ok(serde_json::to_value(WaitStableResult {
                    status: "timeout",
                    hash: last_hash.hex(),
                })
                .unwrap());
            }
        }
    }

    fn assert_text(&mut self, p: AssertTextParams) -> Result<Value, RpcError> {
        let snap = self.do_snapshot(&p.session_id, false)?;
        let loc = TextLocator::new(&p.contains);
        if let Some(hit) = loc.first(&snap.grid) {
            let r = hit.region;
            Ok(serde_json::to_value(AssertTextResult {
                matched: true,
                region: Some([r.x, r.y, r.width, r.height]),
            })
            .unwrap())
        } else {
            Ok(serde_json::to_value(AssertTextResult {
                matched: false,
                region: None,
            })
            .unwrap())
        }
    }

    fn preserve(&mut self, p: SessionIdParams) -> Result<Value, RpcError> {
        let s = self.session_mut(&p.session_id)?;
        s.preserve();
        Ok(serde_json::to_value(PreserveResult {
            reconnect: s.reconnect_command(),
        })
        .unwrap())
    }

    fn close(&mut self, p: SessionIdParams) -> Result<Value, RpcError> {
        if let Some(mut s) = self.sessions.remove(&p.session_id) {
            let _ = s.kill();
        }
        Ok(serde_json::json!({}))
    }
}

fn main() {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut engine = Engine::new();
    if let Err(e) = serve(&mut engine, &mut reader, &mut writer) {
        eprintln!("engine: fatal: {e}");
        std::process::exit(1);
    }
    let _ = writer.flush();
}
