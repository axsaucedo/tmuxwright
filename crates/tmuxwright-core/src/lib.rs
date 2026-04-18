//! Tmuxwright engine core.
//!
//! This crate owns the runtime model that makes Tmuxwright feel like a
//! modern E2E framework rather than a pile of scripts: action primitives,
//! wait/quiescence policies, screen snapshots and stable hashes, trace
//! recording, locator resolver dispatch, and adapter capability
//! negotiation.
//!
//! Implementation lands incrementally per `plan.md` workstream D.

#![allow(clippy::result_large_err)]

pub mod action;
pub mod capability;
pub mod driver;
pub mod error;
pub mod resolver;
pub mod snapshot;
pub mod wait;

pub use action::{Action, ChordKey, Key, Modifiers, MouseButton, Point};
pub use capability::{Capability, FallbackPolicy, Handshake, Negotiated, Route};
pub use driver::{Driver, DriverError};
pub use error::{EngineError, EngineResult, Preservation};
pub use resolver::{resolve, NullSemanticBackend, Resolved, Selector, SemanticBackend, Via};
pub use snapshot::Snapshot;
pub use wait::{PollPolicy, WaitCondition, WaitOutcome};
