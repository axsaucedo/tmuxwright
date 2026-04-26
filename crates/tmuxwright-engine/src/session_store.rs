//! Engine-owned tmux session registry.

use std::collections::HashMap;
use std::path::PathBuf;

use tmuxwright_core::trace::Recorder;
use tmuxwright_core::Action;
use tmuxwright_core::Snapshot;
use tmuxwright_rpc::RpcError;
use tmuxwright_tmux::session::Session;

use crate::errors::invalid_params;

#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: HashMap<String, ManagedSession>,
    next_id: u64,
}

#[derive(Debug)]
pub struct ManagedSession {
    session: Session,
    recorder: Option<Recorder>,
    trace_dir: Option<PathBuf>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn insert(&mut self, session: Session, trace_dir: Option<PathBuf>) -> String {
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        let recorder = trace_dir
            .clone()
            .map(|dir| Recorder::new().with_artifact_dir(dir));
        self.sessions.insert(
            id.clone(),
            ManagedSession {
                session,
                recorder,
                trace_dir,
            },
        );
        id
    }

    pub fn get(&self, id: &str) -> Result<&Session, RpcError> {
        self.sessions
            .get(id)
            .map(|managed| &managed.session)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))
    }

    pub fn get_mut(&mut self, id: &str) -> Result<&mut Session, RpcError> {
        self.sessions
            .get_mut(id)
            .map(|managed| &mut managed.session)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))
    }

    pub fn remove(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id).map(|managed| managed.session)
    }

    pub fn trace_dir(&self, id: &str) -> Result<Option<String>, RpcError> {
        Ok(self
            .sessions
            .get(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))?
            .trace_dir
            .as_ref()
            .map(|path| path.display().to_string()))
    }

    pub fn persist_trace(&self, id: &str) -> Result<Option<PathBuf>, RpcError> {
        let Some(recorder) = self
            .sessions
            .get(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))?
            .recorder
            .as_ref()
        else {
            return Ok(None);
        };
        recorder.persist_trace().map_err(crate::errors::internal)
    }

    pub fn record_action(
        &mut self,
        id: &str,
        action: &Action,
        before: Option<&Snapshot>,
        after: &Snapshot,
    ) -> Result<(), RpcError> {
        let Some(recorder) = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))?
            .recorder
            .as_mut()
        else {
            return Ok(());
        };
        recorder
            .record_action(action, before, after)
            .map_err(crate::errors::internal)?;
        recorder.persist_trace().map_err(crate::errors::internal)?;
        Ok(())
    }

    pub fn record_wait(
        &mut self,
        id: &str,
        condition: &str,
        outcome: &str,
        elapsed: std::time::Duration,
        final_hash: &str,
    ) -> Result<(), RpcError> {
        let Some(recorder) = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))?
            .recorder
            .as_mut()
        else {
            return Ok(());
        };
        recorder.record_wait(condition, outcome, elapsed, final_hash);
        recorder.persist_trace().map_err(crate::errors::internal)?;
        Ok(())
    }

    pub fn record_assert(
        &mut self,
        id: &str,
        description: &str,
        ok: bool,
        snap: &Snapshot,
    ) -> Result<(), RpcError> {
        let Some(recorder) = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))?
            .recorder
            .as_mut()
        else {
            return Ok(());
        };
        recorder.record_assert(description, ok, snap);
        recorder.persist_trace().map_err(crate::errors::internal)?;
        Ok(())
    }
}
