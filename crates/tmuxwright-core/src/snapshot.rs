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
