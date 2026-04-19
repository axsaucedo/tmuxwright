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
    /// Initial pane id (form `%N`). tmux pane ids are server-global and
    /// independent of the user's `base-index` / `pane-base-index`, so
    /// this is the only safe way to target the pane without re-querying.
    pane_id: String,
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
            "-P",
            "-F",
            "#{pane_id}",
        ]);
        if !opts.command.is_empty() {
            // When users supply a command, run it as the pane's initial
            // process. tmux accepts trailing args as the command argv.
            cmd.args(&opts.command);
        }
        let out = run_output(&mut cmd, "new-session")?;
        let pane_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !pane_id.starts_with('%') {
            return Err(SessionError::TmuxFailed {
                op: "new-session",
                status: out.status.code(),
                stderr: format!("unexpected pane id: {pane_id:?}"),
            });
        }

        Ok(Self {
            tmux,
            socket,
            name,
            pane_id,
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

    /// Primary pane target for `-t` in send-keys, capture-pane, etc.
    /// Uses the tmux pane id (`%N`) captured at session creation, which
    /// is server-global and immune to user `base-index` settings.
    #[must_use]
    pub fn primary_pane_target(&self) -> String {
        self.pane_id.clone()
    }

    /// Raw pane id (e.g. `%17`).
    #[must_use]
    pub fn pane_id(&self) -> &str {
        &self.pane_id
    }

    /// Preserve the tmux session on Drop instead of killing it. Intended
    /// to be called from failure-handling paths so developers can attach
    /// via `reconnect_command()` and inspect the failing state.
    pub fn preserve(&mut self) {
        self.kill_on_drop = false;
    }

    /// Whether the tmux server backing this session is still running.
    /// Runs `has-session -t <name>` on the session's isolated socket;
    /// any non-zero exit is treated as "not alive" rather than an
    /// error so callers can use this as a boolean probe.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        let mut cmd = Command::new(self.tmux.path());
        cmd.args(["-L", &self.socket, "has-session", "-t", &self.name]);
        matches!(cmd.output(), Ok(o) if o.status.success())
    }

    /// Metadata a higher layer can pack into a failure report so a
    /// developer can reconnect to the preserved session.
    #[must_use]
    pub fn reconnect_hint(&self) -> ReconnectHint {
        ReconnectHint {
            command: self.reconnect_command(),
            socket: self.socket.clone(),
            session: self.name.clone(),
            pane_id: self.pane_id.clone(),
        }
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

    /// Resize the session's single window to `width` x `height` cells.
    /// Uses `resize-window` (tmux ≥ 2.9) which works on detached
    /// sessions; the pane inside the window inherits the new size.
    pub fn resize(&self, width: u16, height: u16) -> Result<(), SessionError> {
        if width < 2 || height < 2 {
            return Err(SessionError::TmuxFailed {
                op: "resize-window",
                status: None,
                stderr: format!("refusing to resize below 2x2 (got {width}x{height})"),
            });
        }
        let w = width.to_string();
        let h = height.to_string();
        let target = format!("{}:", self.name);
        self.tmux_cmd(&["resize-window", "-t", &target, "-x", &w, "-y", &h])
            .map(|_| ())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.kill_on_drop {
            let _ = self.kill();
        }
    }
}

/// Structured metadata describing how to reconnect to a preserved
/// tmux session after a failure. Plain data so higher layers can
/// serialize it into trace/error output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectHint {
    pub command: String,
    pub socket: String,
    pub session: String,
    pub pane_id: String,
}

fn random_suffix(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect::<String>()
        .to_ascii_lowercase()
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

#[cfg(test)]
mod b5_tests {
    use super::*;
    use crate::detect::detect;
    use std::process::Command;

    fn try_new() -> Option<Session> {
        let tmux = detect().ok()?;
        Session::create(tmux, &SessionOptions::default()).ok()
    }

    #[test]
    fn is_alive_tracks_server_lifecycle() {
        let Some(s) = try_new() else { return };
        assert!(s.is_alive(), "just-created session must be alive");
        let socket = s.socket().to_string();
        let name = s.name().to_string();
        let path = s.tmux_path();
        drop(s);
        let post = Command::new(&path)
            .args(["-L", &socket, "has-session", "-t", &name])
            .output()
            .is_ok_and(|o| o.status.success());
        assert!(!post, "Drop(kill_on_drop=true) must kill the server");
    }

    #[test]
    fn reconnect_hint_exposes_all_fields() {
        let Some(s) = try_new() else { return };
        let h = s.reconnect_hint();
        assert!(h.command.contains(" -L "));
        assert!(h.command.contains("attach"));
        assert!(h.command.contains(&h.socket));
        assert!(h.command.contains(&h.session));
        assert!(h.pane_id.starts_with('%'));
        assert_eq!(h.socket, s.socket());
        assert_eq!(h.session, s.name());
    }

    #[test]
    fn preserve_keeps_server_alive_past_drop() {
        let Some(mut s) = try_new() else { return };
        s.preserve();
        let path = s.tmux_path();
        let socket = s.socket().to_string();
        let name = s.name().to_string();
        drop(s);
        let alive = Command::new(&path)
            .args(["-L", &socket, "has-session", "-t", &name])
            .output()
            .is_ok_and(|o| o.status.success());
        assert!(alive, "preserve() must keep server running");
        // cleanup so we don't leak a server between test runs
        let _ = Command::new(&path)
            .args(["-L", &socket, "kill-server"])
            .output();
    }
}

#[cfg(test)]
mod b6_tests {
    use super::*;
    use crate::capture::pane_geometry;
    use crate::detect::detect;

    fn try_new() -> Option<Session> {
        let tmux = detect().ok()?;
        Session::create(tmux, &SessionOptions::default()).ok()
    }

    #[test]
    fn resize_grows_and_shrinks_pane() {
        let Some(s) = try_new() else { return };
        s.resize(120, 40).expect("grow");
        let g1 = pane_geometry(&s).expect("geom");
        assert_eq!(g1.width, 120);
        // status line consumes 1 row
        assert_eq!(g1.height, 39);

        s.resize(60, 20).expect("shrink");
        let g2 = pane_geometry(&s).expect("geom");
        assert_eq!(g2.width, 60);
        assert_eq!(g2.height, 19);
    }

    #[test]
    fn resize_refuses_degenerate_size() {
        let Some(s) = try_new() else { return };
        let err = s.resize(1, 1).unwrap_err();
        match err {
            SessionError::TmuxFailed { op, .. } => assert_eq!(op, "resize-window"),
            SessionError::Io { .. } => panic!("expected TmuxFailed, got IO"),
        }
    }
}
