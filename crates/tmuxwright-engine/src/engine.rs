//! Engine JSON-RPC handler and high-level tmux operations.

#![allow(clippy::needless_pass_by_value)]

use serde_json::Value;
use tmuxwright_core::snapshot::Snapshot;
use tmuxwright_rpc::server::Handler;
use tmuxwright_rpc::RpcError;
use tmuxwright_tmux::capture::{
    capture_visible_plain, capture_with_scrollback_ansi, pane_geometry,
};
use tmuxwright_tmux::detect::detect;
use tmuxwright_tmux::input::{send_keys as tmux_send_keys, type_text, Key};
use tmuxwright_tmux::session::{Session, SessionOptions};

use crate::assertions;
use crate::errors::{internal, internal_display, invalid_params, parse};
use crate::protocol::{
    method, AssertTextParams, LaunchParams, LaunchResult, PreserveResult, SendKeysParams,
    SessionIdParams, SnapshotParams, SnapshotResult, TypeParams, WaitStableParams, ENGINE_PROTOCOL,
};
use crate::session_store::SessionStore;
use crate::waits;

#[derive(Debug)]
pub struct Engine {
    sessions: SessionStore,
    stop: bool,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            sessions: SessionStore::new(),
            stop: false,
        }
    }

    fn do_snapshot(&self, id: &str, with_scrollback: bool) -> Result<Snapshot, RpcError> {
        let s = self.sessions.get(id)?;
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

impl Handler for Engine {
    fn handle(&mut self, method_name: &str, params: Value) -> Result<Value, RpcError> {
        match method_name {
            method::HANDSHAKE => Ok(serde_json::json!({
                "name": "tmuxwright-engine",
                "version": env!("CARGO_PKG_VERSION"),
                "protocol": ENGINE_PROTOCOL,
            })),
            method::LAUNCH => self.launch(parse(params)?),
            method::SEND_KEYS => self.send_keys(parse(params)?),
            method::TYPE => self.type_text(parse(params)?),
            method::SNAPSHOT => self.snapshot(parse(params)?),
            method::WAIT_STABLE => self.wait_stable(parse(params)?),
            method::ASSERT_TEXT => self.assert_text(parse(params)?),
            method::PRESERVE => self.preserve(parse(params)?),
            method::CLOSE => Ok(self.close(parse(params)?)),
            method::SHUTDOWN => {
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
        let out = LaunchResult {
            session_id: String::new(),
            socket: session.socket().to_string(),
            pane_id: session.pane_id().to_string(),
            reconnect: session.reconnect_command(),
        };
        let id = self.sessions.insert(session);
        Ok(serde_json::to_value(LaunchResult {
            session_id: id,
            ..out
        })
        .unwrap())
    }

    fn send_keys(&mut self, p: SendKeysParams) -> Result<Value, RpcError> {
        let s = self.sessions.get_mut(&p.session_id)?;
        let keys: Vec<Key> = p.keys.into_iter().map(Key).collect();
        tmux_send_keys(s, &keys).map_err(internal)?;
        Ok(serde_json::json!({}))
    }

    fn type_text(&mut self, p: TypeParams) -> Result<Value, RpcError> {
        let s = self.sessions.get_mut(&p.session_id)?;
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
        let result = waits::wait_stable(p.timeout_ms, p.quiet_ms, || {
            self.do_snapshot(&p.session_id, false)
        })?;
        Ok(serde_json::to_value(result).unwrap())
    }

    fn assert_text(&mut self, p: AssertTextParams) -> Result<Value, RpcError> {
        let snap = self.do_snapshot(&p.session_id, false)?;
        Ok(serde_json::to_value(assertions::assert_text(&snap, &p.contains)).unwrap())
    }

    fn preserve(&mut self, p: SessionIdParams) -> Result<Value, RpcError> {
        let s = self.sessions.get_mut(&p.session_id)?;
        s.preserve();
        Ok(serde_json::to_value(PreserveResult {
            reconnect: s.reconnect_command(),
        })
        .unwrap())
    }

    fn close(&mut self, p: SessionIdParams) -> Value {
        if let Some(mut s) = self.sessions.remove(&p.session_id) {
            let _ = s.kill();
        }
        serde_json::json!({})
    }
}
