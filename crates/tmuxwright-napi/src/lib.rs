//! napi-rs bindings for Tmuxwright.
//!
//! Exposes the engine handle, async actions, trace streams, and error
//! types to the Node side (`packages/tmuxwright`). Compiled as a
//! `cdylib` and loaded by the SDK via napi-rs-generated glue.
//!
//! Implementation lands incrementally per `plan.md` workstream F1.
