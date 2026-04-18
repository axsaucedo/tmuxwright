//! Capture the visible screen, scrollback, and pane geometry.
//!
//! Three tmux primitives back this module:
//!
//! - `capture-pane -p -J` for visible text (plain).
//! - `capture-pane -p -J -e -S -` for visible + scrollback **with**
//!   ANSI escapes preserved so downstream ANSI parsing (workstream C)
//!   can reconstruct attributes.
//! - `display-message -p` with tmux format variables for numeric
//!   metadata (cursor, size) in a single round-trip.

use crate::session::{Session, SessionError};

/// Plain text snapshot of the visible pane. Newlines preserved, escape
/// sequences stripped by tmux (no `-e`).
pub fn capture_visible_plain(session: &Session) -> Result<String, SessionError> {
    let target = session.primary_pane_target();
    let out = session.tmux_cmd(&["capture-pane", "-t", &target, "-p", "-J"])?;
    Ok(strip_trailing_newline(
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

/// Visible pane **plus** scrollback, with ANSI escape sequences
/// preserved (`-e`). `-S -` starts from the earliest history line.
/// Intended to feed the terminal parser in a later workstream.
pub fn capture_with_scrollback_ansi(session: &Session) -> Result<String, SessionError> {
    let target = session.primary_pane_target();
    let out = session.tmux_cmd(&["capture-pane", "-t", &target, "-p", "-J", "-e", "-S", "-"])?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Pane geometry + cursor position captured in one `display-message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneGeometry {
    pub width: u16,
    pub height: u16,
    pub cursor_x: u16,
    pub cursor_y: u16,
}

/// Query the pane's current width/height and cursor cell.
pub fn pane_geometry(session: &Session) -> Result<PaneGeometry, SessionError> {
    let target = session.primary_pane_target();
    // tmux expands #{pane_width} etc; we delimit with ';' to parse back.
    let fmt = "#{pane_width};#{pane_height};#{cursor_x};#{cursor_y}";
    let out = session.tmux_cmd(&["display-message", "-p", "-t", &target, fmt])?;
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    parse_geometry(&raw).ok_or_else(|| SessionError::TmuxFailed {
        op: "display-message",
        status: Some(0),
        stderr: format!("could not parse geometry: {raw:?}"),
    })
}

fn parse_geometry(s: &str) -> Option<PaneGeometry> {
    let parts: Vec<&str> = s.split(';').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(PaneGeometry {
        width: parts[0].parse().ok()?,
        height: parts[1].parse().ok()?,
        cursor_x: parts[2].parse().ok()?,
        cursor_y: parts[3].parse().ok()?,
    })
}

fn strip_trailing_newline(mut s: String) -> String {
    while s.ends_with('\n') {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_geometry_happy_path() {
        assert_eq!(
            parse_geometry("80;24;10;3"),
            Some(PaneGeometry {
                width: 80,
                height: 24,
                cursor_x: 10,
                cursor_y: 3,
            })
        );
    }

    #[test]
    fn parse_geometry_rejects_bad_input() {
        assert!(parse_geometry("").is_none());
        assert!(parse_geometry("80;24;10").is_none());
        assert!(parse_geometry("80;24;10;nope").is_none());
    }

    #[test]
    fn strip_trailing_newline_removes_one_or_many() {
        assert_eq!(strip_trailing_newline("abc".into()), "abc");
        assert_eq!(strip_trailing_newline("abc\n".into()), "abc");
        assert_eq!(strip_trailing_newline("abc\n\n\n".into()), "abc");
    }
}

#[cfg(test)]
mod integ {
    use super::*;
    use crate::detect::detect;
    use crate::input::{send_keys, type_text, Key};
    use crate::session::{Session, SessionOptions};
    use std::thread::sleep;
    use std::time::Duration;

    fn try_new_session() -> Option<Session> {
        let tmux = detect().ok()?;
        let opts = SessionOptions {
            width: 80,
            height: 24,
            command: vec!["bash".into(), "--noprofile".into(), "--norc".into()],
        };
        Session::create(tmux, &opts).ok()
    }

    #[test]
    fn visible_plain_returns_pane_contents() {
        let Some(s) = try_new_session() else { return };
        sleep(Duration::from_millis(150));
        type_text(&s, "echo hello_visible\n").unwrap();
        sleep(Duration::from_millis(250));
        let v = capture_visible_plain(&s).expect("capture");
        assert!(v.contains("hello_visible"), "got: {v:?}");
    }

    #[test]
    fn scrollback_capture_includes_history_and_ansi() {
        let Some(s) = try_new_session() else { return };
        sleep(Duration::from_millis(150));
        type_text(&s, "for i in $(seq 1 40); do echo row_$i; done\n").unwrap();
        sleep(Duration::from_millis(400));
        let full = capture_with_scrollback_ansi(&s).expect("scrollback");
        assert!(full.contains("row_1"), "row_1 missing in scrollback");
        assert!(full.contains("row_40"), "row_40 missing in scrollback");
    }

    #[test]
    fn geometry_reports_size_and_cursor_advance() {
        let Some(s) = try_new_session() else { return };
        sleep(Duration::from_millis(150));
        let g0 = pane_geometry(&s).expect("geometry");
        assert_eq!(g0.width, 80);
        assert!(g0.height >= 20 && g0.height <= 24);
        let before_x = g0.cursor_x;
        send_keys(&s, &[Key::new("a"), Key::new("b"), Key::new("c")]).unwrap();
        sleep(Duration::from_millis(150));
        let g1 = pane_geometry(&s).expect("geometry2");
        assert_eq!(g1.cursor_x, before_x + 3);
    }
}
