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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{
        ActionKind, ActionParams, ActionResult, HandshakeResult, LocateParams, LocateResult,
        NodeRef, RegionWire, SelectorWire, PROTOCOL_VERSION,
    };
    use crate::{Id, Request};

    #[derive(Default)]
    struct Mock {
        stop: bool,
    }

    impl Handler for Mock {
        fn handshake(&mut self, _p: Value) -> Result<Value, RpcError> {
            Ok(serde_json::to_value(HandshakeResult {
                name: "mock".into(),
                version: "0.0.1".into(),
                protocol: PROTOCOL_VERSION.into(),
                capabilities: vec![crate::schema::Capability::KeyInput],
            })
            .unwrap())
        }
        fn locate(&mut self, _p: Value) -> Result<Value, RpcError> {
            Ok(serde_json::to_value(LocateResult {
                nodes: vec![NodeRef {
                    node_id: "n1".into(),
                    region: Some(RegionWire {
                        x: 0,
                        y: 0,
                        w: 3,
                        h: 1,
                    }),
                    role: None,
                    name: None,
                }],
            })
            .unwrap())
        }
        fn action(&mut self, _p: Value) -> Result<Value, RpcError> {
            Ok(serde_json::to_value(ActionResult { applied: true }).unwrap())
        }
        fn shutdown(&mut self, _p: Value) -> Result<Value, RpcError> {
            self.stop = true;
            Ok(Value::Null)
        }
        fn should_stop(&self) -> bool {
            self.stop
        }
    }

    fn roundtrip(method_name: &str, params: Value) -> Response {
        let mut m = Mock::default();
        let req = Request::new(Id::Num(1), method_name, Some(params));
        let mut input = Vec::new();
        write_message(&mut input, &serde_json::to_string(&req).unwrap()).unwrap();
        let mut rd = std::io::Cursor::new(input);
        let mut wr = Vec::new();
        assert!(serve_one(&mut m, &mut rd, &mut wr).unwrap());
        let raw = read_message(&mut std::io::Cursor::new(wr))
            .unwrap()
            .unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn handshake_dispatches_to_hook() {
        let resp = roundtrip(method::HANDSHAKE, Value::Null);
        if let crate::ResponseBody::Ok { result } = resp.body {
            let h: HandshakeResult = serde_json::from_value(result).unwrap();
            assert_eq!(h.name, "mock");
        } else {
            panic!("expected ok");
        }
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let resp = roundtrip("tmw.bogus", Value::Null);
        if let crate::ResponseBody::Err { error } = resp.body {
            assert_eq!(error.code, RpcError::METHOD_NOT_FOUND);
        } else {
            panic!("expected err");
        }
    }

    #[test]
    fn locate_and_action_roundtrip_end_to_end_via_client() {
        let mut mock = Mock::default();

        // Build the request stream by serializing two requests into a buffer.
        let mut req_buf = Vec::new();
        let lp = LocateParams {
            selector: SelectorWire::Text {
                value: "foo".into(),
                nth: None,
            },
        };
        let locate_req = Request::new(
            Id::Num(1),
            method::LOCATE,
            Some(serde_json::to_value(&lp).unwrap()),
        );
        write_message(&mut req_buf, &serde_json::to_string(&locate_req).unwrap()).unwrap();
        let ap = ActionParams {
            node_id: "n1".into(),
            action: ActionKind::Click,
        };
        let action_req = Request::new(
            Id::Num(2),
            method::ACTION_DISPATCH,
            Some(serde_json::to_value(&ap).unwrap()),
        );
        write_message(&mut req_buf, &serde_json::to_string(&action_req).unwrap()).unwrap();

        // Server consumes both requests, writes both responses.
        let mut rd = std::io::Cursor::new(req_buf);
        let mut resp_buf = Vec::new();
        assert!(serve_one(&mut mock, &mut rd, &mut resp_buf).unwrap());
        assert!(serve_one(&mut mock, &mut rd, &mut resp_buf).unwrap());

        // Decode responses.
        let mut rd = std::io::Cursor::new(resp_buf);
        let raw1 = read_message(&mut rd).unwrap().unwrap();
        let raw2 = read_message(&mut rd).unwrap().unwrap();
        let r1: Response = serde_json::from_str(&raw1).unwrap();
        let r2: Response = serde_json::from_str(&raw2).unwrap();
        if let crate::ResponseBody::Ok { result } = r1.body {
            let lr: LocateResult = serde_json::from_value(result).unwrap();
            assert_eq!(lr.nodes[0].node_id, "n1");
        } else {
            panic!("locate failed");
        }
        if let crate::ResponseBody::Ok { result } = r2.body {
            let ar: ActionResult = serde_json::from_value(result).unwrap();
            assert!(ar.applied);
        } else {
            panic!("action failed");
        }
    }

    #[test]
    fn serve_stops_when_handler_requests_stop() {
        let mut mock = Mock::default();
        let shutdown_req = Request::new(Id::Num(1), method::SHUTDOWN, None);
        let mut input = Vec::new();
        write_message(&mut input, &serde_json::to_string(&shutdown_req).unwrap()).unwrap();
        // Add a second request that should never be served.
        let extra = Request::new(Id::Num(2), method::LOCATE, Some(Value::Null));
        write_message(&mut input, &serde_json::to_string(&extra).unwrap()).unwrap();
        let mut rd = std::io::Cursor::new(input);
        let mut wr = Vec::new();
        serve(&mut mock, &mut rd, &mut wr).unwrap();
        // Only one response in wr.
        let mut out = std::io::Cursor::new(wr);
        assert!(read_message(&mut out).unwrap().is_some());
        assert!(read_message(&mut out).unwrap().is_none());
    }

    #[test]
    fn malformed_request_gets_parse_error_response() {
        let mut mock = Mock::default();
        let mut input = Vec::new();
        write_message(&mut input, "not json").unwrap();
        let mut rd = std::io::Cursor::new(input);
        let mut wr = Vec::new();
        assert!(serve_one(&mut mock, &mut rd, &mut wr).unwrap());
        let raw = read_message(&mut std::io::Cursor::new(wr))
            .unwrap()
            .unwrap();
        let resp: Response = serde_json::from_str(&raw).unwrap();
        if let crate::ResponseBody::Err { error } = resp.body {
            assert_eq!(error.code, RpcError::PARSE_ERROR);
        } else {
            panic!("expected err");
        }
    }
}
