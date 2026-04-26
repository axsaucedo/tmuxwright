//! Engine JSON-RPC method names and wire payloads.

use serde::{Deserialize, Serialize};

pub const ENGINE_PROTOCOL: &str = "1";

pub mod method {
    pub const HANDSHAKE: &str = "engine.handshake";
    pub const LAUNCH: &str = "engine.launch";
    pub const SEND_KEYS: &str = "engine.send_keys";
    pub const TYPE: &str = "engine.type";
    pub const SNAPSHOT: &str = "engine.snapshot";
    pub const WAIT_STABLE: &str = "engine.wait_stable";
    pub const WAIT_TEXT: &str = "engine.wait_text";
    pub const WAIT_HASH: &str = "engine.wait_hash";
    pub const ASSERT_TEXT: &str = "engine.assert_text";
    pub const PRESERVE: &str = "engine.preserve";
    pub const TRACE: &str = "engine.trace";
    pub const CLOSE: &str = "engine.close";
    pub const SHUTDOWN: &str = "engine.shutdown";
}

#[derive(Debug, Deserialize)]
pub struct LaunchParams {
    pub command: Vec<String>,
    #[serde(default)]
    pub width: Option<u16>,
    #[serde(default)]
    pub height: Option<u16>,
    #[serde(default)]
    pub trace_dir: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LaunchResult {
    pub session_id: String,
    pub socket: String,
    pub pane_id: String,
    pub reconnect: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SessionIdParams {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SendKeysParams {
    pub session_id: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TypeParams {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotParams {
    pub session_id: String,
    #[serde(default)]
    pub with_scrollback: bool,
}

#[derive(Debug, Serialize)]
pub struct SnapshotResult {
    pub text: String,
    pub hash: String,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Deserialize)]
pub struct WaitStableParams {
    pub session_id: String,
    #[serde(default = "default_quiet")]
    pub quiet_ms: u64,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_quiet() -> u64 {
    250
}

fn default_timeout() -> u64 {
    5_000
}

#[derive(Debug, Serialize)]
pub struct WaitStableResult {
    pub status: &'static str,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct WaitTextParams {
    pub session_id: String,
    pub contains: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct WaitTextResult {
    pub status: &'static str,
    pub matched: bool,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<[u16; 4]>,
}

#[derive(Debug, Deserialize)]
pub struct WaitHashParams {
    pub session_id: String,
    pub hash: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct WaitHashResult {
    pub status: &'static str,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct AssertTextParams {
    pub session_id: String,
    pub contains: String,
}

#[derive(Debug, Serialize)]
pub struct AssertTextResult {
    pub matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<[u16; 4]>,
}

#[derive(Debug, Serialize)]
pub struct PreserveResult {
    pub reconnect: String,
}

#[derive(Debug, Serialize)]
pub struct TraceResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_path: Option<String>,
}
