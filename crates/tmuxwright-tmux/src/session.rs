//! Tmux session/window/pane manager.
//!
//! Each Tmuxwright test gets its own **isolated tmux server** by passing
//! `-L <socket-name>` to every tmux invocation. This prevents tests from
//! interfering with each other (or with the developer's interactive tmux)
//! and makes per-test cleanup a one-liner: kill the socket.
//!
//! A `Session` is the test-facing handle. It owns the socket, a single
//! session name, and at least one window with one pane. Higher layers
//! build input/capture/preservation on top of this.

use std::path::PathBuf;
use std::process::{Command, Output};

use rand::{distributions::Alphanumeric, Rng};
use thiserror::Error;

use crate::detect::Tmux;

/// Errors produced when talking to tmux through the session manager.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("tmux {op} failed (exit {status:?}): {stderr}")]
    TmuxFailed {
        op: &'static str,
        status: Option<i32>,
        stderr: String,
    },
    #[error("io error invoking tmux {op}: {source}")]
    Io {
        op: &'static str,
        #[source]
        source: std::io::Error,
    },
}

/// Options used to create a session.
#[derive(Debug, Clone)]
pub struct SessionOptions {
    /// Width in columns for the initial window.
    pub width: u16,
    /// Height in rows for the initial window.
    pub height: u16,
    /// Command line to run inside the initial pane. If empty, tmux
    /// launches the user's default shell (rarely what tests want).
    pub command: Vec<String>,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            width: 120,
            height: 40,
            command: Vec::new(),
        }
    }
}

/// A running Tmuxwright-managed tmux session on an isolated socket.
#[derive(Debug)]
pub struct Session {
    tmux: Tmux,
    socket: String,
    name: String,
    /// Whether the session should be killed on Drop. Preserve-on-failure
    /// flips this to false so developers can reconnect.
    kill_on_drop: bool,
}

impl Session {
    /// Spawn a fresh tmux server on an isolated socket and create a
    /// single detached session with one window/pane.
    pub fn create(tmux: Tmux, opts: &SessionOptions) -> Result<Self, SessionError> {
        let socket = format!("tmw-{}", random_suffix(10));
        let name = format!("tmw-{}", random_suffix(6));

        let mut cmd = Command::new(tmux.path());
        cmd.args([
            "-L",
            &socket,
            "new-session",
            "-d",
            "-s",
            &name,
            "-x",
            &opts.width.to_string(),
            "-y",
            &opts.height.to_string(),
        ]);
        if !opts.command.is_empty() {
            // When users supply a command, run it as the pane's initial
            // process. tmux accepts trailing args as the command argv.
            cmd.args(&opts.command);
        }
        run(&mut cmd, "new-session")?;

        Ok(Self {
            tmux,
            socket,
            name,
            kill_on_drop: true,
        })
    }

    /// Socket name (passed to every `-L` invocation).
    #[must_use]
    pub fn socket(&self) -> &str {
        &self.socket
    }

    /// Session name (the `-t <name>` target).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Path to the tmux binary backing this session.
    #[must_use]
    pub fn tmux_path(&self) -> PathBuf {
        self.tmux.path().to_path_buf()
    }

    /// Command a developer can run in another terminal to attach to the
    /// preserved session after a failure.
    #[must_use]
    pub fn reconnect_command(&self) -> String {
        format!(
            "{} -L {} attach -t {}",
            self.tmux.path().display(),
            self.socket,
            self.name,
        )
    }

    /// Primary pane target of the form `session:window.pane` suitable
    /// for `-t` in send-keys, capture-pane, etc. For now we always use
    /// window 0 pane 0 — multi-pane support lands later in workstream B.
    #[must_use]
    pub fn primary_pane_target(&self) -> String {
        format!("{}:0.0", self.name)
    }

    /// Preserve the tmux session on Drop instead of killing it. Intended
    /// to be called from failure-handling paths so developers can attach
    /// via `reconnect_command()` and inspect the failing state.
    pub fn preserve(&mut self) {
        self.kill_on_drop = false;
    }

    /// Run `tmux -L <socket> <args...>` against this session's server.
    pub fn tmux_cmd(&self, args: &[&str]) -> Result<Output, SessionError> {
        let mut cmd = Command::new(self.tmux.path());
        cmd.arg("-L").arg(&self.socket).args(args);
        run_output(&mut cmd, "tmux_cmd")
    }

    /// Explicitly kill the backing tmux server. Idempotent.
    pub fn kill(&mut self) -> Result<(), SessionError> {
        let mut cmd = Command::new(self.tmux.path());
        cmd.args(["-L", &self.socket, "kill-server"]);
        match cmd.output() {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(SessionError::Io {
                op: "kill-server",
                source: err,
            }),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.kill_on_drop {
            let _ = self.kill();
        }
    }
}

fn random_suffix(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn run(cmd: &mut Command, op: &'static str) -> Result<(), SessionError> {
    run_output(cmd, op).map(|_| ())
}

fn run_output(cmd: &mut Command, op: &'static str) -> Result<Output, SessionError> {
    let output = cmd
        .output()
        .map_err(|source| SessionError::Io { op, source })?;
    if !output.status.success() {
        return Err(SessionError::TmuxFailed {
            op,
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect;

    fn tmux_or_skip() -> Option<Tmux> {
        if let Ok(t) = detect() {
            Some(t)
        } else {
            eprintln!("skipping: tmux not detected on PATH");
            None
        }
    }

    #[test]
    fn random_suffix_is_lowercase_alphanumeric_and_expected_length() {
        for _ in 0..50 {
            let s = random_suffix(12);
            assert_eq!(s.len(), 12);
            assert!(s
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        }
    }

    #[test]
    fn default_session_options_have_sane_shape() {
        let o = SessionOptions::default();
        assert!(o.width >= 80);
        assert!(o.height >= 24);
        assert!(o.command.is_empty());
    }

    #[test]
    fn create_and_drop_cleans_up_socket() {
        let Some(tmux) = tmux_or_skip() else { return };
        let opts = SessionOptions {
            width: 80,
            height: 24,
            command: vec!["cat".into()],
        };
        let tmux_path = tmux.path().to_path_buf();
        let (socket, session_name) = {
            let session = Session::create(tmux, &opts).expect("create session");
            let socket = session.socket().to_string();
            let name = session.name().to_string();

            // has-session should succeed while the session is alive.
            let out = session
                .tmux_cmd(&["has-session", "-t", &name])
                .expect("has-session");
            assert!(out.status.success());

            (socket, name)
        };

        // After Drop, the server should be gone; has-session against the
        // old socket/session must not succeed.
        let out = Command::new(&tmux_path)
            .args(["-L", &socket, "has-session", "-t", &session_name])
            .output()
            .expect("has-session post-drop");
        assert!(
            !out.status.success(),
            "session {session_name} on socket {socket} still alive after Drop",
        );
    }

    #[test]
    fn preserve_prevents_drop_cleanup() {
        let Some(tmux) = tmux_or_skip() else { return };
        let opts = SessionOptions {
            width: 80,
            height: 24,
            command: vec!["cat".into()],
        };
        let tmux_path = tmux.path().to_path_buf();
        let (socket, session_name) = {
            let mut session = Session::create(tmux, &opts).expect("create session");
            session.preserve();
            (session.socket().to_string(), session.name().to_string())
        };

        // After Drop with preserve(), has-session should still succeed.
        let out = Command::new(&tmux_path)
            .args(["-L", &socket, "has-session", "-t", &session_name])
            .output()
            .expect("has-session post-drop");
        assert!(
            out.status.success(),
            "preserve() did not keep session {session_name} alive",
        );

        // Clean up manually so we don't leak a server.
        let _ = Command::new(&tmux_path)
            .args(["-L", &socket, "kill-server"])
            .output();
    }

    #[test]
    fn reconnect_command_contains_all_required_parts() {
        let Some(tmux) = tmux_or_skip() else { return };
        let opts = SessionOptions {
            width: 80,
            height: 24,
            command: vec!["cat".into()],
        };
        let session = Session::create(tmux, &opts).expect("create session");
        let cmd = session.reconnect_command();
        assert!(cmd.contains("-L "));
        assert!(cmd.contains(session.socket()));
        assert!(cmd.contains("attach"));
        assert!(cmd.contains(session.name()));
    }
}
