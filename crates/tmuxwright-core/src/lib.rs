//! Tmuxwright engine core.
//!
//! This crate owns the runtime model that makes Tmuxwright feel like a
//! modern E2E framework rather than a pile of scripts: action primitives,
//! wait/quiescence policies, screen snapshots and stable hashes, trace
//! recording, locator resolver dispatch, and adapter capability
//! negotiation.
//!
//! Implementation lands incrementally per `plan.md` workstream D.
