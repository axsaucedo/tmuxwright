//! Helpers for converting daemon errors to JSON-RPC errors.

use serde::de::DeserializeOwned;
use serde_json::Value;
use tmuxwright_rpc::RpcError;

pub fn invalid_params(msg: impl Into<String>) -> RpcError {
    RpcError::new(RpcError::INVALID_PARAMS, msg)
}

pub fn internal<E: std::error::Error>(e: E) -> RpcError {
    RpcError::new(RpcError::INTERNAL_ERROR, e.to_string())
}

pub fn internal_display<E: std::fmt::Display>(e: E) -> RpcError {
    RpcError::new(RpcError::INTERNAL_ERROR, e.to_string())
}

pub fn parse<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|e| invalid_params(e.to_string()))
}
