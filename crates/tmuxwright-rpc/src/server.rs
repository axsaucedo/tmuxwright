//! Minimal synchronous JSON-RPC server loop.
//!
//! Adapter authors (workstream H) can use this to implement a working
//! server by supplying a `Handler` trait impl; the loop reads framed
//! requests from a `BufRead`, dispatches them, and writes responses
//! to a `Write`. It also gives the engine a reusable reference
//! implementation for its own contract tests.

use std::io::{BufRead, Write};

use serde_json::Value;

use crate::framing::{read_message, write_message, FrameError};
use crate::schema::method;
use crate::{Id, Request, Response, RpcError};

/// Server-side handler. Each method gets a typed `Value` params and
/// returns either a result `Value` or an `RpcError`.
pub trait Handler {
    /// Dispatch one request. The default implementation routes to
    /// per-method hooks; override individual hooks (or this method
    /// directly) to customize.
    ///
    /// # Errors
    /// Returns an `RpcError` on any failure the adapter wants to surface.
    fn handle(&mut self, method_name: &str, params: Value) -> Result<Value, RpcError> {
        match method_name {
            method::HANDSHAKE => self.handshake(params),
            method::SHUTDOWN => self.shutdown(params),
            method::LOCATE => self.locate(params),
            method::ACTION_DISPATCH => self.action(params),
            method::SNAPSHOT_SEMANTIC => self.snapshot_semantic(params),
            method::STATE_FOCUS => self.focus(params),
            method::STATE_VALUE => self.value(params),
            _ => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method: {method_name}"),
            )),
        }
    }

    /// # Errors
    /// Returns `RpcError` to surface handshake failures to the caller.
    fn handshake(&mut self, _params: Value) -> Result<Value, RpcError> {
        Err(not_implemented(method::HANDSHAKE))
    }
    /// # Errors
    /// Returns `RpcError` to surface shutdown failures to the caller.
    fn shutdown(&mut self, _params: Value) -> Result<Value, RpcError> {
        Ok(Value::Null)
    }
    /// # Errors
    /// Returns `RpcError` when locate cannot be served.
    fn locate(&mut self, _params: Value) -> Result<Value, RpcError> {
        Err(not_implemented(method::LOCATE))
    }
    /// # Errors
    /// Returns `RpcError` when action dispatch fails.
    fn action(&mut self, _params: Value) -> Result<Value, RpcError> {
        Err(not_implemented(method::ACTION_DISPATCH))
    }
    /// # Errors
    /// Returns `RpcError` when a semantic snapshot cannot be produced.
    fn snapshot_semantic(&mut self, _params: Value) -> Result<Value, RpcError> {
        Err(not_implemented(method::SNAPSHOT_SEMANTIC))
    }
    /// # Errors
    /// Returns `RpcError` when focus state is unavailable.
    fn focus(&mut self, _params: Value) -> Result<Value, RpcError> {
        Err(not_implemented(method::STATE_FOCUS))
    }
    /// # Errors
    /// Returns `RpcError` when the value query fails.
    fn value(&mut self, _params: Value) -> Result<Value, RpcError> {
        Err(not_implemented(method::STATE_VALUE))
    }

    /// Adapters set this to true after processing `tmw.shutdown` so
    /// the serve loop exits cleanly.
    fn should_stop(&self) -> bool {
        false
    }
}

fn not_implemented(m: &str) -> RpcError {
    RpcError::new(
        RpcError::METHOD_NOT_FOUND,
        format!("method not implemented by this adapter: {m}"),
    )
}

/// Run one request/response cycle. Returns `Ok(true)` if a message
/// was processed, `Ok(false)` on clean EOF.
///
/// # Errors
/// Propagates any I/O error from the underlying reader or writer.
pub fn serve_one<H: Handler, R: BufRead, W: Write>(
    handler: &mut H,
    r: &mut R,
    w: &mut W,
) -> Result<bool, FrameError> {
    let Some(raw) = read_message(r)? else {
        return Ok(false);
    };
    let body = match serde_json::from_str::<Request>(&raw) {
        Ok(req) => {
            let params = req.params.unwrap_or(Value::Null);
            let resp = match handler.handle(&req.method, params) {
                Ok(v) => Response::ok(req.id, v),
                Err(e) => Response::err(req.id, e),
            };
            serde_json::to_string(&resp).unwrap()
        }
        Err(e) => {
            let resp = Response::err(
                Id::Num(0),
                RpcError::new(RpcError::PARSE_ERROR, e.to_string()),
            );
            serde_json::to_string(&resp).unwrap()
        }
    };
    write_message(w, &body).map_err(FrameError::Io)?;
    Ok(true)
}

/// Read/dispatch/write until EOF or until `handler.should_stop()`.
///
/// # Errors
/// Propagates the first framing or I/O error encountered.
pub fn serve<H: Handler, R: BufRead, W: Write>(
    handler: &mut H,
    r: &mut R,
    w: &mut W,
) -> Result<(), FrameError> {
    while !handler.should_stop() {
        if !serve_one(handler, r, w)? {
            break;
        }
    }
    Ok(())
}
