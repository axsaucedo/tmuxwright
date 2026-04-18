//! Engine error taxonomy.
//!
//! Every externally visible failure funnels through [`EngineError`] so
//! the top-level TS SDK can render a single, consistent failure shape
//! with the all-important [`Preservation`] metadata: the tmux session
//! id, the reconnect command, and a human-readable hint so a developer
//! can drop into the failing pane and look around.

use std::fmt;
use std::time::Duration;

use crate::action::Action;
use crate::wait::WaitCondition;

/// Information for reconnecting to the tmux session that produced the
/// failure. Populated by the engine once tmux integration is wired up;
/// until then D2/D3 tests can construct it directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preservation {
    /// The tmux socket name (passed as `tmux -L`). Empty if unknown.
    pub socket: String,
    /// The tmux session name.
    pub session: String,
    /// Fully-formed reconnect command the developer can copy-paste.
    pub reconnect_cmd: String,
    /// Free-form hint, e.g. "pane preserved; run the command below".
    pub hint: String,
}

impl Preservation {
    /// Convenience: build one from the canonical pieces. The
    /// `reconnect_cmd` is generated from `socket` and `session`.
    #[must_use]
    pub fn new(socket: impl Into<String>, session: impl Into<String>) -> Self {
        let socket = socket.into();
        let session = session.into();
        let reconnect_cmd = format!("tmux -L {socket} attach -t {session}");
        let hint = format!("tmux pane preserved; run: {reconnect_cmd}");
        Self {
            socket,
            session,
            reconnect_cmd,
            hint,
        }
    }
}

/// Engine-level error. Variants carry enough context for the SDK to
/// render a useful failure and for tests to assert on structured
/// fields rather than on prose.
#[derive(Debug)]
pub enum EngineError {
    /// A wait primitive exceeded its timeout.
    WaitTimeout {
        condition: WaitCondition,
        waited: Duration,
        preservation: Option<Preservation>,
    },
    /// An assertion failed (e.g., expected text not present).
    AssertFailed {
        description: String,
        preservation: Option<Preservation>,
    },
    /// A locator found zero or more-than-expected matches.
    LocatorMiss {
        selector: String,
        found: usize,
        preservation: Option<Preservation>,
    },
    /// Dispatching an action failed at the driver layer.
    Dispatch {
        action: Action,
        source: Box<dyn std::error::Error + Send + Sync>,
        preservation: Option<Preservation>,
    },
    /// Capturing a snapshot failed.
    Snapshot {
        source: Box<dyn std::error::Error + Send + Sync>,
        preservation: Option<Preservation>,
    },
    /// An adapter handshake or RPC call failed (D3/E wiring).
    Adapter {
        message: String,
        preservation: Option<Preservation>,
    },
    /// tmux itself (or an equivalent backend) reported a failure that
    /// does not fit into the other buckets.
    Backend {
        message: String,
        preservation: Option<Preservation>,
    },
}

impl EngineError {
    /// Borrow the preservation payload, if any.
    #[must_use]
    pub fn preservation(&self) -> Option<&Preservation> {
        match self {
            Self::WaitTimeout { preservation, .. }
            | Self::AssertFailed { preservation, .. }
            | Self::LocatorMiss { preservation, .. }
            | Self::Dispatch { preservation, .. }
            | Self::Snapshot { preservation, .. }
            | Self::Adapter { preservation, .. }
            | Self::Backend { preservation, .. } => preservation.as_ref(),
        }
    }

    /// Attach (or replace) the preservation payload. Returns `self`
    /// for chaining from `Err(e.with_preservation(p))`.
    #[must_use]
    pub fn with_preservation(mut self, p: Preservation) -> Self {
        match &mut self {
            Self::WaitTimeout { preservation, .. }
            | Self::AssertFailed { preservation, .. }
            | Self::LocatorMiss { preservation, .. }
            | Self::Dispatch { preservation, .. }
            | Self::Snapshot { preservation, .. }
            | Self::Adapter { preservation, .. }
            | Self::Backend { preservation, .. } => *preservation = Some(p),
        }
        self
    }

    /// Short, stable, machine-readable kind tag. Used by the TS SDK
    /// to switch on error type without string-matching Display.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::WaitTimeout { .. } => "wait_timeout",
            Self::AssertFailed { .. } => "assert_failed",
            Self::LocatorMiss { .. } => "locator_miss",
            Self::Dispatch { .. } => "dispatch",
            Self::Snapshot { .. } => "snapshot",
            Self::Adapter { .. } => "adapter",
            Self::Backend { .. } => "backend",
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WaitTimeout {
                condition, waited, ..
            } => write!(
                f,
                "wait timed out after {waited:?} for condition {condition:?}"
            ),
            Self::AssertFailed { description, .. } => {
                write!(f, "assertion failed: {description}")
            }
            Self::LocatorMiss {
                selector, found, ..
            } => write!(f, "locator {selector:?} expected a match, found {found}"),
            Self::Dispatch { action, source, .. } => {
                write!(f, "dispatch of {action:?} failed: {source}")
            }
            Self::Snapshot { source, .. } => write!(f, "snapshot failed: {source}"),
            Self::Adapter { message, .. } => write!(f, "adapter error: {message}"),
            Self::Backend { message, .. } => write!(f, "backend error: {message}"),
        }?;
        if let Some(p) = self.preservation() {
            write!(f, " ({})", p.hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dispatch { source, .. } | Self::Snapshot { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Engine-level result alias.
pub type EngineResult<T> = Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preservation_new_builds_reconnect_cmd_and_hint() {
        let p = Preservation::new("tmuxwright-abc", "run-42");
        assert_eq!(p.socket, "tmuxwright-abc");
        assert_eq!(p.session, "run-42");
        assert_eq!(p.reconnect_cmd, "tmux -L tmuxwright-abc attach -t run-42");
        assert!(p.hint.contains("run:"));
        assert!(p.hint.contains(&p.reconnect_cmd));
    }

    #[test]
    fn kind_tags_are_stable() {
        let cases: Vec<(EngineError, &str)> = vec![
            (
                EngineError::WaitTimeout {
                    condition: WaitCondition::Stable {
                        quiet_for: Duration::from_millis(10),
                    },
                    waited: Duration::from_millis(20),
                    preservation: None,
                },
                "wait_timeout",
            ),
            (
                EngineError::AssertFailed {
                    description: "x".into(),
                    preservation: None,
                },
                "assert_failed",
            ),
            (
                EngineError::LocatorMiss {
                    selector: "text=ok".into(),
                    found: 0,
                    preservation: None,
                },
                "locator_miss",
            ),
            (
                EngineError::Adapter {
                    message: "offline".into(),
                    preservation: None,
                },
                "adapter",
            ),
            (
                EngineError::Backend {
                    message: "tmux gone".into(),
                    preservation: None,
                },
                "backend",
            ),
        ];
        for (e, expected) in cases {
            assert_eq!(e.kind(), expected);
        }
    }

    #[test]
    fn preservation_can_be_attached_after_construction() {
        let e = EngineError::AssertFailed {
            description: "expected 'hi'".into(),
            preservation: None,
        };
        assert!(e.preservation().is_none());
        let e = e.with_preservation(Preservation::new("sock", "sess"));
        assert_eq!(e.preservation().unwrap().session, "sess");
    }

    #[test]
    fn display_includes_preservation_hint() {
        let e = EngineError::AssertFailed {
            description: "no match".into(),
            preservation: Some(Preservation::new("s1", "run")),
        };
        let s = format!("{e}");
        assert!(s.contains("assertion failed: no match"));
        assert!(s.contains("tmux -L s1 attach -t run"));
    }

    #[test]
    fn dispatch_error_exposes_source() {
        let source: Box<dyn std::error::Error + Send + Sync> = "boom".into();
        let e = EngineError::Dispatch {
            action: Action::Press(crate::action::Key::Enter),
            source,
            preservation: None,
        };
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn non_source_errors_return_none_for_source() {
        let e = EngineError::AssertFailed {
            description: "x".into(),
            preservation: None,
        };
        assert!(std::error::Error::source(&e).is_none());
    }
}
