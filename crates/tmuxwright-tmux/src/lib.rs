//! Tmux control layer for Tmuxwright.
//!
//! Owns session/window/pane lifecycle, input injection (`send-keys`,
//! `load-buffer`/`paste-buffer`, mouse events), screen + scrollback
//! capture, and preservation-on-failure with a reconnect hint.
//!
//! Implementation lands incrementally per `plan.md` workstream B.

pub mod capture;
pub mod detect;
pub mod input;
pub mod session;

pub use capture::{
    capture_visible_plain, capture_with_scrollback_ansi, pane_geometry, PaneGeometry,
};
pub use detect::{
    detect, detect_at, parse_version_banner, DetectError, Tmux, Version, MIN_TMUX_VERSION,
};
pub use input::{encode_mouse_sgr, send_keys, send_mouse, type_text, Key, MouseButton, MouseEvent};
pub use session::{ReconnectHint, Session, SessionError, SessionOptions};
