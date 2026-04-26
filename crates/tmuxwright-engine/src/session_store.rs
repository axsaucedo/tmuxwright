//! Engine-owned tmux session registry.

use std::collections::HashMap;

use tmuxwright_rpc::RpcError;
use tmuxwright_tmux::session::Session;

use crate::errors::invalid_params;

#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: HashMap<String, Session>,
    next_id: u64,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn insert(&mut self, session: Session) -> String {
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        self.sessions.insert(id.clone(), session);
        id
    }

    pub fn get(&self, id: &str) -> Result<&Session, RpcError> {
        self.sessions
            .get(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))
    }

    pub fn get_mut(&mut self, id: &str) -> Result<&mut Session, RpcError> {
        self.sessions
            .get_mut(id)
            .ok_or_else(|| invalid_params(format!("unknown session_id: {id}")))
    }

    pub fn remove(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id)
    }
}
