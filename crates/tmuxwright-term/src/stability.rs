//! Quiescence detection.
//!
//! A [`Stability`] tracker takes a stream of [`ScreenHash`] samples
//! and a clock; it reports "stable" once the *same* hash has been
//! seen for at least `quiet_for` of wall-clock time without change,
//! and "timeout" once `deadline_from_start` elapses without settling.
//!
//! The detector is clock-agnostic — tests drive it with a controllable
//! [`Clock`] impl; runtime callers use [`MonotonicClock`]. This is the
//! same pattern Tmuxwright uses everywhere polling is involved, so
//! wait loops stay deterministic in tests.

use std::time::{Duration, Instant};

use crate::hash::ScreenHash;

/// Clock abstraction so tests can move time forward without sleeping.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// Real monotonic clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct MonotonicClock;

impl Clock for MonotonicClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// What the detector reports after each sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Still waiting for the hash to hold still for `quiet_for`.
    Changing,
    /// Hash has been constant for at least `quiet_for`.
    Stable,
    /// Total elapsed >= `timeout`; caller should surface the timeout.
    Timeout,
}

/// Configuration for the detector.
#[derive(Debug, Clone, Copy)]
pub struct StabilityConfig {
    pub quiet_for: Duration,
    pub timeout: Duration,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            quiet_for: Duration::from_millis(150),
            timeout: Duration::from_secs(5),
        }
    }
}

/// Stateful quiescence detector. Feed it one sample per poll via
/// [`Stability::observe`].
#[derive(Debug)]
pub struct Stability<C: Clock> {
    clock: C,
    cfg: StabilityConfig,
    started_at: Instant,
    last_hash: Option<ScreenHash>,
    last_change_at: Instant,
}

impl<C: Clock> Stability<C> {
    pub fn new(clock: C, cfg: StabilityConfig) -> Self {
        let now = clock.now();
        Self {
            clock,
            cfg,
            started_at: now,
            last_hash: None,
            last_change_at: now,
        }
    }

    /// Record a new sample and return the current status.
    pub fn observe(&mut self, hash: ScreenHash) -> Status {
        let now = self.clock.now();
        match self.last_hash {
            Some(prev) if prev == hash => { /* unchanged */ }
            _ => {
                self.last_hash = Some(hash);
                self.last_change_at = now;
            }
        }
        let quiet = now.saturating_duration_since(self.last_change_at);
        let total = now.saturating_duration_since(self.started_at);
        if quiet >= self.cfg.quiet_for {
            Status::Stable
        } else if total >= self.cfg.timeout {
            Status::Timeout
        } else {
            Status::Changing
        }
    }

    /// Expose the most recent observed hash, if any.
    #[must_use]
    pub fn current(&self) -> Option<ScreenHash> {
        self.last_hash
    }
}
