//! Input injection for a Tmuxwright session.
//!
//! Three layers of input are supported, each mapping to how tmux
//! expects it:
//!
//! - **Named keys** (`Enter`, `Up`, `C-c`, …) go through `send-keys`
//!   directly — tmux translates symbolic names into the right bytes.
//! - **Literal text** is routed through `load-buffer` + `paste-buffer`.
//!   Using `send-keys` for literal text would mangle any character that
//!   tmux interprets as a key name (e.g. the word "Enter"), so paste is
//!   the correct primitive for typing credentials, URLs, or anything
//!   else that should land byte-for-byte.
//! - **Mouse events** are encoded as SGR escape sequences and sent as
//!   raw hex via `send-keys -H`, which matches what a terminal emulator
//!   would produce for a real click.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::session::{Session, SessionError};

/// A pressable key name understood by `tmux send-keys`. Free-form so
/// callers can pass tmux-native names (`Enter`, `BSpace`, `C-c`, …)
/// without this crate having to enumerate every one.
#[derive(Debug, Clone)]
pub struct Key(pub String);

impl Key {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Send one or more named keys to the session's primary pane.
pub fn send_keys(session: &Session, keys: &[Key]) -> Result<(), SessionError> {
    if keys.is_empty() {
        return Ok(());
    }
    let target = session.primary_pane_target();
    let mut args: Vec<String> = vec!["send-keys".into(), "-t".into(), target];
    for k in keys {
        args.push(k.0.clone());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    session.tmux_cmd(&refs).map(|_| ())
}

/// Type literal text into the primary pane by routing it through a
/// tmux paste buffer. This preserves every byte exactly — `send-keys`
/// on its own would interpret strings like "Enter" as the Enter key.
pub fn type_text(session: &Session, text: &str) -> Result<(), SessionError> {
    // 1. load-buffer -b <buf> - (read from stdin)
    let buffer_name = format!("tmw-{}-paste", session.name());
    let mut child = Command::new(session.tmux_path())
        .args([
            "-L",
            session.socket(),
            "load-buffer",
            "-b",
            &buffer_name,
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| SessionError::Io {
            op: "load-buffer",
            source,
        })?;
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(text.as_bytes())
        .map_err(|source| SessionError::Io {
            op: "load-buffer:write",
            source,
        })?;
    let status = child
        .wait_with_output()
        .map_err(|source| SessionError::Io {
            op: "load-buffer:wait",
            source,
        })?;
    if !status.status.success() {
        return Err(SessionError::TmuxFailed {
            op: "load-buffer",
            status: status.status.code(),
            stderr: String::from_utf8_lossy(&status.stderr).trim().to_string(),
        });
    }

    // 2. paste-buffer -b <buf> -t <target> -d  (-d deletes buffer after)
    let target = session.primary_pane_target();
    session
        .tmux_cmd(&[
            "paste-buffer",
            "-b",
            &buffer_name,
            "-t",
            &target,
            "-d",
            "-p",
        ])
        .map(|_| ())
}

/// Mouse button for [`send_mouse`].
#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

impl MouseButton {
    fn sgr_code(self) -> u16 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::WheelUp => 64,
            MouseButton::WheelDown => 65,
        }
    }
}

/// Mouse event kind for [`send_mouse`].
#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Press,
    Release,
}

/// Encode a mouse event as an SGR-mode sequence (`ESC[<Cb;Cx;Cy[Mm]`)
/// suitable for feeding into `send-keys -H`. `x` and `y` are 1-based
/// terminal cell coordinates. Exposed publicly so callers and tests
/// can inspect the exact bytes we will inject.
#[must_use]
pub fn encode_mouse_sgr(button: MouseButton, event: MouseEvent, x: u16, y: u16) -> String {
    let suffix = match event {
        MouseEvent::Press => 'M',
        MouseEvent::Release => 'm',
    };
    format!("\x1b[<{};{};{}{}", button.sgr_code(), x, y, suffix)
}

/// Send a mouse event to the primary pane. The bytes are computed via
/// [`encode_mouse_sgr`] and shipped to tmux as hex through
/// `send-keys -H`, which is the documented path for injecting raw
/// input sequences.
pub fn send_mouse(
    session: &Session,
    button: MouseButton,
    event: MouseEvent,
    x: u16,
    y: u16,
) -> Result<(), SessionError> {
    let bytes = encode_mouse_sgr(button, event, x, y);
    let target = session.primary_pane_target();
    let mut args: Vec<String> = vec!["send-keys".into(), "-t".into(), target, "-H".into()];
    for b in bytes.bytes() {
        args.push(format!("{b:02x}"));
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    session.tmux_cmd(&refs).map(|_| ())
}
