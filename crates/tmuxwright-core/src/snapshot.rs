//! Screen snapshot bundle — plain text, parsed grid, and stable hash.
//!
//! The engine's trace recorder (D2) stores one [`Snapshot`] per action,
//! so this type is the canonical "visible state" currency the rest of
//! the engine moves around.

use tmuxwright_term::{hash_grid, Grid, Parser, ScreenHash};

/// Bundle of terminal state at a single moment.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub grid: Grid,
    pub hash: ScreenHash,
    /// Original visible text (may contain ANSI control sequences if the
    /// caller asked for scrollback; plain otherwise).
    pub raw: String,
}

impl Snapshot {
    /// Parse a visible-plain capture (no ANSI) at the given geometry
    /// and return the snapshot.
    #[must_use]
    pub fn from_plain(width: u16, height: u16, text: &str) -> Self {
        let mut p = Parser::new(width, height);
        p.feed(text.as_bytes());
        let grid = p.into_grid();
        let hash = hash_grid(&grid);
        Self {
            grid,
            hash,
            raw: text.to_owned(),
        }
    }

    /// Parse ANSI-bearing capture (scrollback + escape sequences).
    #[must_use]
    pub fn from_ansi(width: u16, height: u16, bytes: &[u8]) -> Self {
        let mut p = Parser::new(width, height);
        p.feed(bytes);
        let grid = p.into_grid();
        let hash = hash_grid(&grid);
        Self {
            grid,
            hash,
            raw: String::from_utf8_lossy(bytes).into_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_plain_populates_grid_and_hash() {
        let s = Snapshot::from_plain(5, 2, "hi\r\nok");
        assert_eq!(s.grid.row_text(0).trim_end(), "hi");
        assert_eq!(s.grid.row_text(1).trim_end(), "ok");
        assert_ne!(s.hash, ScreenHash([0; 32]));
    }

    #[test]
    fn identical_input_produces_identical_hash() {
        let a = Snapshot::from_plain(10, 1, "same");
        let b = Snapshot::from_plain(10, 1, "same");
        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn different_input_produces_different_hash() {
        let a = Snapshot::from_plain(10, 1, "left");
        let b = Snapshot::from_plain(10, 1, "right");
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn from_ansi_strips_escape_into_grid() {
        let bytes = b"\x1b[31mred\x1b[0m";
        let s = Snapshot::from_ansi(10, 1, bytes);
        assert_eq!(s.grid.row_text(0).trim_end(), "red");
    }
}
