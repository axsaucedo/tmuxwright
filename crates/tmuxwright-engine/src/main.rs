//! Tmuxwright engine daemon.
//!
//! One long-lived process per test run. Owns tmux sessions, terminal
//! snapshots, waits, and assertions, then exposes them to clients over
//! JSON-RPC 2.0 with LSP-style Content-Length framing.

mod assertions;
mod engine;
mod errors;
mod protocol;
mod session_store;
mod waits;

use std::io::{BufReader, Write};

use engine::Engine;
use tmuxwright_rpc::server::serve;

fn main() {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut engine = Engine::new();
    if let Err(e) = serve(&mut engine, &mut reader, &mut writer) {
        eprintln!("engine: fatal: {e}");
        std::process::exit(1);
    }
    let _ = writer.flush();
}
