//! Driver trait — the engine's abstract interface to whatever is
//! actually running the terminal.
//!
//! A production driver wraps a [`tmuxwright_tmux::Session`] and real
//! framework adapters; tests can hand the engine a mock that records
//! calls and replays canned snapshots. This separation is what keeps
//! the engine unit-testable without tmux or adapters present.

use crate::action::Action;
use crate::snapshot::Snapshot;

/// Engine-level error kind. Fuller taxonomy lives in `error.rs` once
/// D5 lands; for now this is the surface drivers may return.
pub type DriverError = Box<dyn std::error::Error + Send + Sync>;

/// Backend-agnostic interface the engine drives.
pub trait Driver {
    /// Dispatch an action. Implementations may block briefly (e.g.,
    /// sending keys through tmux) but must not include waits — that
    /// layer belongs to the engine's wait loop.
    ///
    /// # Errors
    /// Returns a `DriverError` when the underlying backend (tmux,
    /// adapter RPC, mock) rejects or fails the action.
    fn dispatch(&mut self, action: &Action) -> Result<(), DriverError>;

    /// Capture the current visible terminal state.
    ///
    /// # Errors
    /// Returns a `DriverError` when the backend cannot produce a
    /// snapshot (e.g., pane died, capture-pane failed, adapter offline).
    fn snapshot(&mut self) -> Result<Snapshot, DriverError>;
}
