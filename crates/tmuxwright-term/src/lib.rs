//! Terminal model for Tmuxwright.
//!
//! Parses ANSI/VT streams (captured from tmux) into a grid with
//! attributes, produces a stable screen hash suitable for quiescence
//! detection and assertions, and implements text/region locators used
//! in terminal-mode resolution.
//!
//! Implementation lands incrementally per `plan.md` workstream C.

pub mod grid;
pub mod hash;

pub use grid::{Attrs, Cell, Color, Grid, Parser};
pub use hash::{hash_grid, ScreenHash};
