//! Synchronous JSON-RPC client over framed byte streams.
//!
//! Designed for the stdio transport that adapters use: give it a
//! reader (stdout of the adapter) and a writer (stdin of the
//! adapter), and it correlates responses to requests by id.
//!
//! Single-threaded by construction — the adapter RPC is strictly
//! request/response from the engine's side, so a read-after-write
//! loop is sufficient. If we ever want server-initiated
//! notifications, we'll add a dedicated reader thread in a later
//! iteration.

use std::io::{BufRead, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::framing::{read_message, write_message, FrameError};
use crate::schema::{
    is_compatible, method, ActionParams, ActionResult, FocusResult, HandshakeParams,
    HandshakeResult, LocateParams, LocateResult, SemanticSnapshotResult, StateValueParams,
    StateValueResult, PROTOCOL_VERSION,
};
use crate::{Id, Request, Response, ResponseBody, RpcError};

#[derive(Debug)]
pub enum ClientError {
    Frame(FrameError),
    Serde(serde_json::Error),
    Rpc(RpcError),
    IdMismatch { sent: Id, got: Id },
    UnexpectedEof,
    ProtocolMismatch { got: String, want: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(e) => write!(f, "framing: {e}"),
            Self::Serde(e) => write!(f, "serde: {e}"),
            Self::Rpc(e) => write!(f, "rpc error {}: {}", e.code, e.message),
            Self::IdMismatch { sent, got } => {
                write!(f, "id mismatch: sent {sent:?} got {got:?}")
            }
            Self::UnexpectedEof => write!(f, "adapter closed before responding"),
            Self::ProtocolMismatch { got, want } => {
                write!(f, "adapter protocol {got:?} but engine wants {want:?}")
            }
        }
    }
}

impl std::error::Error for ClientError {}

impl From<FrameError> for ClientError {
    fn from(e: FrameError) -> Self {
        Self::Frame(e)
    }
}
impl From<serde_json::Error> for ClientError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serde(e)
    }
}

pub struct Client<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    next_id: i64,
}

impl<R: BufRead, W: Write> Client<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_id: 1,
        }
    }

    fn next_id(&mut self) -> Id {
        let i = self.next_id;
        self.next_id += 1;
        Id::Num(i)
    }

    /// Issue one request and wait for its matching response.
    ///
    /// # Errors
    /// Returns [`ClientError`] on framing, serde, mismatched id, RPC error, or EOF.
    pub fn call<P: Serialize, T: DeserializeOwned>(
        &mut self,
        method_name: &str,
        params: &P,
    ) -> Result<T, ClientError> {
        let id = self.next_id();
        let req = Request::new(id.clone(), method_name, Some(serde_json::to_value(params)?));
        let body = serde_json::to_string(&req)?;
        write_message(&mut self.writer, &body).map_err(FrameError::Io)?;

        let raw = read_message(&mut self.reader)?.ok_or(ClientError::UnexpectedEof)?;
        let resp: Response = serde_json::from_str(&raw)?;
        if resp.id != id {
            return Err(ClientError::IdMismatch {
                sent: id,
                got: resp.id,
            });
        }
        match resp.body {
            ResponseBody::Ok { result } => Ok(serde_json::from_value(result)?),
            ResponseBody::Err { error } => Err(ClientError::Rpc(error)),
        }
    }

    /// Issue a call with no params (`null`).
    ///
    /// # Errors
    /// See [`Client::call`].
    pub fn call_no_params<T: DeserializeOwned>(
        &mut self,
        method_name: &str,
    ) -> Result<T, ClientError> {
        self.call(method_name, &Value::Null)
    }

    // ---- Typed shortcuts for each schema method ---------------------

    /// Performs the handshake and verifies the protocol version.
    ///
    /// # Errors
    /// See [`Client::call`]; additionally returns `ProtocolMismatch`
    /// if the adapter reports a different protocol.
    pub fn handshake(
        &mut self,
        client: &str,
        client_version: &str,
    ) -> Result<HandshakeResult, ClientError> {
        let params = HandshakeParams {
            client: client.into(),
            client_version: client_version.into(),
            protocol: PROTOCOL_VERSION.into(),
        };
        let r: HandshakeResult = self.call(method::HANDSHAKE, &params)?;
        if !is_compatible(&r.protocol) {
            return Err(ClientError::ProtocolMismatch {
                got: r.protocol.clone(),
                want: PROTOCOL_VERSION.into(),
            });
        }
        Ok(r)
    }

    /// # Errors
    /// See [`Client::call`].
    pub fn locate(&mut self, params: &LocateParams) -> Result<LocateResult, ClientError> {
        self.call(method::LOCATE, params)
    }

    /// # Errors
    /// See [`Client::call`].
    pub fn action(&mut self, params: &ActionParams) -> Result<ActionResult, ClientError> {
        self.call(method::ACTION_DISPATCH, params)
    }

    /// # Errors
    /// See [`Client::call`].
    pub fn snapshot_semantic(&mut self) -> Result<SemanticSnapshotResult, ClientError> {
        self.call_no_params(method::SNAPSHOT_SEMANTIC)
    }

    /// # Errors
    /// See [`Client::call`].
    pub fn focus(&mut self) -> Result<FocusResult, ClientError> {
        self.call_no_params(method::STATE_FOCUS)
    }

    /// # Errors
    /// See [`Client::call`].
    pub fn value(&mut self, params: &StateValueParams) -> Result<StateValueResult, ClientError> {
        self.call(method::STATE_VALUE, params)
    }

    /// # Errors
    /// See [`Client::call`].
    pub fn shutdown(&mut self) -> Result<Value, ClientError> {
        self.call_no_params(method::SHUTDOWN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Remove the unused `serve_one` helper by not defining it — tests below
    // exercise it via the real `server::serve_one` path.

    #[test]
    fn handshake_happy_path() {
        // Preload the reader with a canned response for id=1.
        let hr = HandshakeResult {
            name: "mock".into(),
            version: "0.0.1".into(),
            protocol: PROTOCOL_VERSION.into(),
            capabilities: vec![],
        };
        let mut out = Vec::new();
        write_message(
            &mut out,
            &serde_json::to_string(&Response::ok(
                Id::Num(1),
                serde_json::to_value(&hr).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        let mut rd = std::io::Cursor::new(out);
        let mut wr = Vec::new();
        let mut c = Client::new(&mut rd, &mut wr);
        let r = c.handshake("tmw", "0").unwrap();
        assert_eq!(r.name, "mock");
        // Verify the client actually wrote a handshake request.
        let mut wrc = std::io::Cursor::new(wr);
        let raw = read_message(&mut wrc).unwrap().unwrap();
        let req: Request = serde_json::from_str(&raw).unwrap();
        assert_eq!(req.method, method::HANDSHAKE);
        assert_eq!(req.id, Id::Num(1));
    }

    #[test]
    fn rpc_error_surfaces_as_client_error() {
        // Preload a response with id 1 carrying an RpcError.
        let mut out = Vec::new();
        let resp = Response::err(
            Id::Num(1),
            RpcError::new(RpcError::METHOD_NOT_FOUND, "nope"),
        );
        write_message(&mut out, &serde_json::to_string(&resp).unwrap()).unwrap();
        let mut rd = std::io::Cursor::new(out);
        let mut wr = Vec::new();
        let mut c = Client::new(&mut rd, &mut wr);
        let e = c.call_no_params::<Value>("tmw.whatever").unwrap_err();
        match e {
            ClientError::Rpc(err) => {
                assert_eq!(err.code, RpcError::METHOD_NOT_FOUND);
            }
            other => panic!("wrong variant: {other}"),
        }
    }

    #[test]
    fn id_mismatch_is_an_error() {
        let mut out = Vec::new();
        let resp = Response::ok(Id::Num(999), Value::Null);
        write_message(&mut out, &serde_json::to_string(&resp).unwrap()).unwrap();
        let mut rd = std::io::Cursor::new(out);
        let mut wr = Vec::new();
        let mut c = Client::new(&mut rd, &mut wr);
        let e = c.call_no_params::<Value>("tmw.x").unwrap_err();
        matches!(e, ClientError::IdMismatch { .. })
            .then_some(())
            .expect("id mismatch");
    }

    #[test]
    fn protocol_mismatch_rejected_by_handshake() {
        let mut out = Vec::new();
        let resp = Response::ok(
            Id::Num(1),
            serde_json::to_value(HandshakeResult {
                name: "mock".into(),
                version: "0".into(),
                protocol: "99".into(),
                capabilities: vec![],
            })
            .unwrap(),
        );
        write_message(&mut out, &serde_json::to_string(&resp).unwrap()).unwrap();
        let mut rd = std::io::Cursor::new(out);
        let mut wr = Vec::new();
        let mut c = Client::new(&mut rd, &mut wr);
        let e = c.handshake("tmw", "0").unwrap_err();
        matches!(e, ClientError::ProtocolMismatch { .. })
            .then_some(())
            .expect("proto mismatch");
    }

    #[test]
    fn eof_before_response_is_an_error() {
        let mut rd = std::io::Cursor::new(Vec::<u8>::new());
        let mut wr = Vec::new();
        let mut c = Client::new(&mut rd, &mut wr);
        let e = c.call_no_params::<Value>("tmw.x").unwrap_err();
        matches!(e, ClientError::UnexpectedEof)
            .then_some(())
            .expect("eof");
    }
}
