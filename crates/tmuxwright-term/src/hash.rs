//! Stable screen hash.
//!
//! Produces a deterministic, cheap-to-compare digest of a [`Grid`] so
//! that assertions, quiescence detection (C3), and trace entries can
//! treat "the screen" as a single equality-comparable value.
//!
//! Canonical form (what we hash):
//!
//! ```text
//! width u16_le | height u16_le |
//! for each cell in row-major order:
//!   char_utf8 len u8 | char utf8 bytes |
//!   attrs byte (bit0 bold, bit1 underline, bit2 reverse) |
//!   fg u8 (0xff = Default, else index) |
//!   bg u8 (0xff = Default, else index)
//! ```
//!
//! SHA-256 is overkill cryptographically but gives us plenty of room
//! to compare by digest across processes and into logs.

use sha2::{Digest, Sha256};

use crate::grid::{Color, Grid};

/// 32-byte SHA-256 digest newtype. `Display` renders the lowercase
/// hex representation, which is what tests and traces embed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenHash(pub [u8; 32]);

impl ScreenHash {
    #[must_use]
    pub fn hex(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(64);
        for b in self.0 {
            write!(&mut out, "{b:02x}").expect("writing to String cannot fail");
        }
        out
    }

    /// Short prefix suitable for log lines.
    #[must_use]
    pub fn short(&self) -> String {
        self.hex().chars().take(12).collect()
    }
}

impl std::fmt::Display for ScreenHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.hex())
    }
}

fn color_byte(c: Color) -> u8 {
    match c {
        Color::Default => 0xff,
        Color::Indexed(i) => i,
    }
}

/// Compute the canonical hash of `grid`.
#[must_use]
pub fn hash_grid(grid: &Grid) -> ScreenHash {
    let mut h = Sha256::new();
    h.update(grid.width().to_le_bytes());
    h.update(grid.height().to_le_bytes());
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let cell = grid.cell(x, y).expect("bounds-checked");
            let mut buf = [0u8; 4];
            let utf8 = cell.ch.encode_utf8(&mut buf);
            let len = u8::try_from(utf8.len()).unwrap_or(0);
            h.update([len]);
            h.update(utf8.as_bytes());
            let mut flags: u8 = 0;
            if cell.attrs.bold {
                flags |= 0b0000_0001;
            }
            if cell.attrs.underline {
                flags |= 0b0000_0010;
            }
            if cell.attrs.reverse {
                flags |= 0b0000_0100;
            }
            h.update([flags, color_byte(cell.attrs.fg), color_byte(cell.attrs.bg)]);
        }
    }
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    ScreenHash(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Parser;

    fn parse(w: u16, h: u16, s: &[u8]) -> Grid {
        let mut p = Parser::new(w, h);
        p.feed(s);
        p.into_grid()
    }

    #[test]
    fn hash_is_deterministic() {
        let a = hash_grid(&parse(10, 3, b"hello\r\nworld"));
        let b = hash_grid(&parse(10, 3, b"hello\r\nworld"));
        assert_eq!(a, b);
    }

    #[test]
    fn hash_changes_on_text_change() {
        let a = hash_grid(&parse(10, 3, b"hello"));
        let b = hash_grid(&parse(10, 3, b"hellp"));
        assert_ne!(a, b);
    }

    #[test]
    fn hash_changes_on_size_change() {
        let a = hash_grid(&parse(10, 3, b"hi"));
        let b = hash_grid(&parse(11, 3, b"hi"));
        assert_ne!(a, b);
    }

    #[test]
    fn hash_changes_on_attr_change() {
        let a = hash_grid(&parse(5, 1, b"X"));
        let b = hash_grid(&parse(5, 1, b"\x1b[1mX"));
        assert_ne!(a, b, "bold should change the hash");
    }

    #[test]
    fn hash_stable_across_idempotent_redraws() {
        let a = hash_grid(&parse(10, 2, b"abc"));
        // Repaint the same content via absolute cursor + text.
        let b = hash_grid(&parse(10, 2, b"\x1b[1;1Habc"));
        assert_eq!(a, b);
    }

    #[test]
    fn hex_and_short_render_as_expected() {
        let h = hash_grid(&parse(5, 1, b"xy"));
        let full = h.hex();
        assert_eq!(full.len(), 64);
        assert_eq!(h.short(), full.chars().take(12).collect::<String>());
    }
}
