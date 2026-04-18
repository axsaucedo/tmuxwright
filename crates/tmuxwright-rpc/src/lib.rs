//! JSON-RPC 2.0 wire types for Tmuxwright adapter transport.
//!
//! This crate owns the on-the-wire schema that lives between the
//! engine and a framework adapter (Textual / Bubble Tea / Ratatui,
//! workstream H). Transports — stdio framing, Unix-domain sockets —
//! come next; this module is pure data so it can be unit-tested and
//! reused from both sides of the protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod client;
pub mod framing;
pub mod schema;
pub mod server;

/// Request or response id. Matches the JSON-RPC 2.0 `id` field, which
/// may be a string or number (null/omitted is reserved for
/// notifications — we model those separately so `Request` always has
/// an id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Num(i64),
    Str(String),
}

/// JSON-RPC 2.0 call expecting a response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: JsonRpcV,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub params: Option<Value>,
    pub id: Id,
}

/// JSON-RPC 2.0 notification (no id, no response).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: JsonRpcV,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response. Exactly one of `result`/`error` is set per
/// the spec; we model it as an enum so the type system enforces that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: JsonRpcV,
    #[serde(flatten)]
    pub body: ResponseBody,
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseBody {
    Ok { result: Value },
    Err { error: RpcError },
}

/// Spec-defined error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<Value>,
}

impl RpcError {
    /// -32700 Parse error
    pub const PARSE_ERROR: i32 = -32700;
    /// -32600 Invalid request
    pub const INVALID_REQUEST: i32 = -32600;
    /// -32601 Method not found
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// -32602 Invalid params
    pub const INVALID_PARAMS: i32 = -32602;
    /// -32603 Internal error
    pub const INTERNAL_ERROR: i32 = -32603;

    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Serializer helper that pins the "jsonrpc" field to "2.0".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonRpcV;

impl Serialize for JsonRpcV {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpcV {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == "2.0" {
            Ok(JsonRpcV)
        } else {
            Err(serde::de::Error::custom("jsonrpc must be \"2.0\""))
        }
    }
}

impl Request {
    #[must_use]
    pub fn new(id: Id, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JsonRpcV,
            method: method.into(),
            params,
            id,
        }
    }
}

impl Notification {
    #[must_use]
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JsonRpcV,
            method: method.into(),
            params,
        }
    }
}

impl Response {
    #[must_use]
    pub fn ok(id: Id, result: Value) -> Self {
        Self {
            jsonrpc: JsonRpcV,
            body: ResponseBody::Ok { result },
            id,
        }
    }

    #[must_use]
    pub fn err(id: Id, error: RpcError) -> Self {
        Self {
            jsonrpc: JsonRpcV,
            body: ResponseBody::Err { error },
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let r = Request::new(
            Id::Num(7),
            "handshake",
            Some(serde_json::json!({"client":"tmuxwright"})),
        );
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains(r#""jsonrpc":"2.0""#));
        assert!(j.contains(r#""method":"handshake""#));
        assert!(j.contains(r#""id":7"#));
        let back: Request = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn response_ok_and_err_have_correct_shape() {
        let ok = Response::ok(Id::Str("a".into()), serde_json::json!(42));
        let j = serde_json::to_string(&ok).unwrap();
        assert!(j.contains(r#""result":42"#));
        assert!(!j.contains("error"));

        let e = Response::err(
            Id::Num(1),
            RpcError::new(RpcError::METHOD_NOT_FOUND, "no such method"),
        );
        let j = serde_json::to_string(&e).unwrap();
        assert!(j.contains(r#""code":-32601"#));
        assert!(j.contains(r#""message":"no such method""#));
        assert!(!j.contains("result"));
    }

    #[test]
    fn response_roundtrip_both_variants() {
        let ok = Response::ok(Id::Num(3), serde_json::json!({"x":1}));
        let back: Response = serde_json::from_str(&serde_json::to_string(&ok).unwrap()).unwrap();
        assert_eq!(back, ok);

        let e = Response::err(Id::Num(4), RpcError::new(-1, "bad"));
        let back: Response = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn notification_has_no_id() {
        let n = Notification::new("progress", Some(serde_json::json!({"p":50})));
        let j = serde_json::to_string(&n).unwrap();
        assert!(!j.contains(r#""id""#));
    }

    #[test]
    fn rejects_wrong_jsonrpc_version() {
        let bad = r#"{"jsonrpc":"1.0","method":"x","id":1}"#;
        let r: Result<Request, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn rpc_error_with_data_includes_data() {
        let e = RpcError::new(-32000, "x").with_data(serde_json::json!({"why":"because"}));
        let j = serde_json::to_string(&e).unwrap();
        assert!(j.contains(r#""data":{"why":"because"}"#));
    }

    #[test]
    fn rpc_error_without_data_omits_data_field() {
        let e = RpcError::new(-32000, "x");
        let j = serde_json::to_string(&e).unwrap();
        assert!(!j.contains("data"));
    }

    #[test]
    fn id_supports_string_and_number_forms() {
        let r: Request =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"m","id":"abc"}"#).unwrap();
        assert_eq!(r.id, Id::Str("abc".into()));
        let r: Request = serde_json::from_str(r#"{"jsonrpc":"2.0","method":"m","id":5}"#).unwrap();
        assert_eq!(r.id, Id::Num(5));
    }
}
