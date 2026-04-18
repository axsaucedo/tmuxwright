//! Wait/poll policies.
//!
//! The engine's wait loops are deterministic: a [`PollPolicy`] drives
//! how often to sample and when to give up, and the quiescence logic
//! lives in [`tmuxwright_term::Stability`]. This keeps the same
//! fake-clock testing story from the term crate extending all the way
//! into the engine.

use std::time::Duration;

/// How to poll and when to give up on a wait.
#[derive(Debug, Clone, Copy)]
pub struct PollPolicy {
    /// Duration between samples.
    pub interval: Duration,
    /// Hard upper bound on total wait time.
    pub timeout: Duration,
}

impl Default for PollPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(25),
            timeout: Duration::from_secs(5),
        }
    }
}

/// What the engine is waiting for.
#[derive(Debug, Clone)]
pub enum WaitCondition {
    /// Screen hash is stable for `quiet_for`.
    Stable { quiet_for: Duration },
    /// A text locator resolves. The string is compared with
    /// [`tmuxwright_term::TextLocator::new`] plus case-insensitivity if
    /// `case_insensitive`.
    Text {
        needle: String,
        case_insensitive: bool,
    },
}

/// Outcome of a single wait invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitOutcome {
    Satisfied,
    TimedOut,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_policy_default_values() {
        let p = PollPolicy::default();
        assert_eq!(p.interval, Duration::from_millis(25));
        assert_eq!(p.timeout, Duration::from_secs(5));
    }

    #[test]
    fn wait_condition_variants_are_distinct() {
        let a = WaitCondition::Stable {
            quiet_for: Duration::from_millis(100),
        };
        let b = WaitCondition::Text {
            needle: "go".into(),
            case_insensitive: false,
        };
        let names = |c: &WaitCondition| match c {
            WaitCondition::Stable { .. } => "stable",
            WaitCondition::Text { .. } => "text",
        };
        assert_ne!(names(&a), names(&b));
    }

    #[test]
    fn outcome_equality() {
        assert_eq!(WaitOutcome::Satisfied, WaitOutcome::Satisfied);
        assert_ne!(WaitOutcome::Satisfied, WaitOutcome::TimedOut);
    }
}
