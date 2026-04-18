//! Trace recorder.
//!
//! Each meaningful engine step (action dispatch, wait, assertion,
//! resolver hit, error) appends a [`TraceEntry`] to a [`Recorder`].
//! The recorder can emit JSON-lines and persist artifacts (per-step
//! raw captures) to a directory for debugging a failing run.
//!
//! The entry schema is deliberately open-ended — serialized as JSON
//! with a `kind` tag — so the TS SDK and future agents can consume it
//! without importing Rust types.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::error::EngineError;
use crate::resolver::{Selector, Via};
use crate::snapshot::Snapshot;

/// One step in the trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TraceEntry {
    Action {
        step: u64,
        action: String,
        before_hash: String,
        after_hash: String,
        /// Artifact filename (relative to the trace dir) of the raw
        /// post-action capture. `None` if the recorder was not
        /// persisting artifacts.
        after_artifact: Option<String>,
    },
    Wait {
        step: u64,
        condition: String,
        outcome: String,
        elapsed_ms: u64,
        final_hash: String,
    },
    Assert {
        step: u64,
        description: String,
        ok: bool,
        hash: String,
    },
    Resolve {
        step: u64,
        selector: String,
        via: String,
        region: RegionRecord,
    },
    Error {
        step: u64,
        error_kind: String,
        message: String,
        preservation: Option<PreservationRecord>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionRecord {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservationRecord {
    pub socket: String,
    pub session: String,
    pub reconnect_cmd: String,
}

/// In-memory recorder with optional on-disk artifact persistence.
#[derive(Debug)]
pub struct Recorder {
    entries: Vec<TraceEntry>,
    next_step: u64,
    artifact_dir: Option<PathBuf>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_step: 0,
            artifact_dir: None,
        }
    }

    /// Enable on-disk artifact persistence. The directory is created
    /// on first write.
    #[must_use]
    pub fn with_artifact_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.artifact_dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    fn bump(&mut self) -> u64 {
        let s = self.next_step;
        self.next_step += 1;
        s
    }

    /// Record an action step. Snapshots are optional — callers may
    /// skip `before` when it's identical to the previous `after`.
    ///
    /// # Errors
    /// Returns an I/O error when artifact persistence was requested
    /// and writing the raw capture failed.
    pub fn record_action(
        &mut self,
        action: &Action,
        before: Option<&Snapshot>,
        after: &Snapshot,
    ) -> std::io::Result<()> {
        let step = self.bump();
        let after_artifact = self.persist_artifact(step, "after", &after.raw)?;
        self.entries.push(TraceEntry::Action {
            step,
            action: format!("{action:?}"),
            before_hash: before.map_or_else(String::new, |b| b.hash.hex()),
            after_hash: after.hash.hex(),
            after_artifact,
        });
        Ok(())
    }

    pub fn record_wait(
        &mut self,
        condition: &str,
        outcome: &str,
        elapsed: std::time::Duration,
        final_hash: &str,
    ) {
        let step = self.bump();
        self.entries.push(TraceEntry::Wait {
            step,
            condition: condition.to_owned(),
            outcome: outcome.to_owned(),
            elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            final_hash: final_hash.to_owned(),
        });
    }

    pub fn record_assert(&mut self, description: &str, ok: bool, snap: &Snapshot) {
        let step = self.bump();
        self.entries.push(TraceEntry::Assert {
            step,
            description: description.to_owned(),
            ok,
            hash: snap.hash.hex(),
        });
    }

    pub fn record_resolve(&mut self, selector: &Selector, via: Via, region: RegionRecord) {
        let step = self.bump();
        let via_s = match via {
            Via::Adapter => "adapter",
            Via::Terminal => "terminal",
        };
        self.entries.push(TraceEntry::Resolve {
            step,
            selector: format!("{}={selector:?}", selector.tag()),
            via: via_s.to_owned(),
            region,
        });
    }

    pub fn record_error(&mut self, err: &EngineError) {
        let step = self.bump();
        let preservation = err.preservation().map(|p| PreservationRecord {
            socket: p.socket.clone(),
            session: p.session.clone(),
            reconnect_cmd: p.reconnect_cmd.clone(),
        });
        self.entries.push(TraceEntry::Error {
            step,
            error_kind: err.kind().to_owned(),
            message: err.to_string(),
            preservation,
        });
    }

    /// Serialize all entries as newline-delimited JSON.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            let line = serde_json::to_string(e).expect("trace entry is always serializable");
            out.push_str(&line);
            out.push('\n');
        }
        out
    }

    /// Write `trace.jsonl` into the `artifact_dir` (if configured).
    ///
    /// # Errors
    /// Returns an I/O error when the directory cannot be created or
    /// trace.jsonl cannot be written.
    pub fn persist_trace(&self) -> std::io::Result<Option<PathBuf>> {
        let Some(dir) = self.artifact_dir.as_ref() else {
            return Ok(None);
        };
        fs::create_dir_all(dir)?;
        let path = dir.join("trace.jsonl");
        let mut f = fs::File::create(&path)?;
        f.write_all(self.to_jsonl().as_bytes())?;
        Ok(Some(path))
    }

    fn persist_artifact(
        &self,
        step: u64,
        label: &str,
        body: &str,
    ) -> std::io::Result<Option<String>> {
        let Some(dir) = self.artifact_dir.as_ref() else {
            return Ok(None);
        };
        fs::create_dir_all(dir)?;
        let name = format!("step-{step:04}-{label}.txt");
        let path = Path::new(dir).join(&name);
        fs::write(path, body)?;
        Ok(Some(name))
    }
}
