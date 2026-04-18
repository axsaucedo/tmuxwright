//! Terminal-mode locators.
//!
//! Two flavors, both producing [`Match`]es that carry an (x, y) origin
//! and the bounding region, so higher layers (e.g. the engine's click
//! resolver) can compute a click target without knowing the locator
//! kind.
//!
//! - [`TextLocator`]: substring search per row with optional
//!   case-insensitivity, an `nth` filter, and a `whole_row` mode that
//!   returns every occurrence in a row instead of stopping at the
//!   first.
//! - [`RegionLocator`]: an absolute (x, y, w, h) rectangle.
//!
//! Text search runs on the row-text produced by [`Grid::row_text`], so
//! trailing blanks never match and snapshots line up with `to_text`.

use crate::grid::Grid;

/// Rectangular region on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// Successful locator resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub region: Region,
}

impl Match {
    /// Center cell of the region — the natural click target.
    #[must_use]
    pub fn center(self) -> (u16, u16) {
        (
            self.region.x + self.region.width / 2,
            self.region.y + self.region.height / 2,
        )
    }
}

/// Find text on the grid.
#[derive(Debug, Clone)]
pub struct TextLocator {
    needle: String,
    case_insensitive: bool,
    nth: usize,
}

impl TextLocator {
    #[must_use]
    pub fn new(needle: impl Into<String>) -> Self {
        Self {
            needle: needle.into(),
            case_insensitive: false,
            nth: 0,
        }
    }

    #[must_use]
    pub fn case_insensitive(mut self) -> Self {
        self.case_insensitive = true;
        self
    }

    /// Zero-based index into the all-matches sequence (row-major).
    #[must_use]
    pub fn nth(mut self, n: usize) -> Self {
        self.nth = n;
        self
    }

    /// Return every match, row-major (top-to-bottom, then left-to-right).
    #[must_use]
    pub fn all(&self, grid: &Grid) -> Vec<Match> {
        if self.needle.is_empty() {
            return Vec::new();
        }
        let (needle, cmp_transform) = if self.case_insensitive {
            (self.needle.to_ascii_lowercase(), true)
        } else {
            (self.needle.clone(), false)
        };
        let nlen = u16::try_from(needle.chars().count()).unwrap_or(u16::MAX);
        let mut out = Vec::new();
        for y in 0..grid.height() {
            let row = grid.row_text(y);
            let hay = if cmp_transform {
                row.to_ascii_lowercase()
            } else {
                row.clone()
            };
            // Walk match positions; use a byte-offset search then map
            // back to char offset so the x coord is a real column.
            let mut from = 0usize;
            while let Some(byte_pos) = hay[from..].find(&needle) {
                let abs_byte = from + byte_pos;
                let char_pos = hay[..abs_byte].chars().count();
                let x = u16::try_from(char_pos).unwrap_or(u16::MAX);
                out.push(Match {
                    region: Region {
                        x,
                        y,
                        width: nlen,
                        height: 1,
                    },
                });
                // Advance by at least one byte so overlapping matches
                // still progress.
                from = abs_byte + needle.len().max(1);
            }
        }
        out
    }

    /// Resolve to the nth match (default 0).
    #[must_use]
    pub fn first(&self, grid: &Grid) -> Option<Match> {
        self.all(grid).into_iter().nth(self.nth)
    }
}

/// Absolute rectangular region locator.
#[derive(Debug, Clone, Copy)]
pub struct RegionLocator {
    pub region: Region,
}

impl RegionLocator {
    #[must_use]
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            region: Region {
                x,
                y,
                width,
                height,
            },
        }
    }

    /// Clamp the region to the grid bounds and return the match, or
    /// None if the region is entirely off-grid.
    #[must_use]
    pub fn resolve(self, grid: &Grid) -> Option<Match> {
        if self.region.x >= grid.width() || self.region.y >= grid.height() {
            return None;
        }
        let w = self.region.width.min(grid.width() - self.region.x);
        let h = self.region.height.min(grid.height() - self.region.y);
        if w == 0 || h == 0 {
            return None;
        }
        Some(Match {
            region: Region {
                x: self.region.x,
                y: self.region.y,
                width: w,
                height: h,
            },
        })
    }

    /// Extract the text inside the region, one row per line.
    #[must_use]
    pub fn text(self, grid: &Grid) -> String {
        let Some(m) = self.resolve(grid) else {
            return String::new();
        };
        let mut rows: Vec<String> = Vec::with_capacity(usize::from(m.region.height));
        for dy in 0..m.region.height {
            let mut row = String::with_capacity(usize::from(m.region.width));
            for dx in 0..m.region.width {
                let cell = grid
                    .cell(m.region.x + dx, m.region.y + dy)
                    .expect("clamped above");
                row.push(cell.ch);
            }
            rows.push(row);
        }
        rows.join("\n")
    }
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
    fn text_locator_finds_single_match() {
        let g = parse(20, 3, b"hello world\r\ngoodbye");
        let m = TextLocator::new("world").first(&g).expect("match");
        assert_eq!(
            m.region,
            Region {
                x: 6,
                y: 0,
                width: 5,
                height: 1
            }
        );
    }

    #[test]
    fn text_locator_case_insensitive() {
        let g = parse(20, 1, b"HELLO");
        assert!(TextLocator::new("hello").first(&g).is_none());
        let m = TextLocator::new("hello")
            .case_insensitive()
            .first(&g)
            .expect("match");
        assert_eq!(m.region.x, 0);
    }

    #[test]
    fn text_locator_nth_picks_later_match() {
        let g = parse(30, 2, b"ab ab ab\r\nab");
        let all = TextLocator::new("ab").all(&g);
        assert_eq!(all.len(), 4);
        let m = TextLocator::new("ab").nth(2).first(&g).expect("nth=2");
        assert_eq!(
            m.region,
            Region {
                x: 6,
                y: 0,
                width: 2,
                height: 1
            }
        );
    }

    #[test]
    fn text_locator_empty_needle_returns_nothing() {
        let g = parse(5, 1, b"abc");
        assert!(TextLocator::new("").first(&g).is_none());
    }

    #[test]
    fn match_center_is_middle_cell() {
        let m = Match {
            region: Region {
                x: 6,
                y: 0,
                width: 5,
                height: 1,
            },
        };
        assert_eq!(m.center(), (8, 0));
    }

    #[test]
    fn region_locator_clamps_to_grid() {
        let g = parse(10, 2, b"abcdefghij\r\nklmno");
        let r = RegionLocator::new(8, 0, 10, 3).resolve(&g).expect("some");
        // width clamped to 10-8=2, height clamped to 2-0=2
        assert_eq!(
            r.region,
            Region {
                x: 8,
                y: 0,
                width: 2,
                height: 2
            }
        );
    }

    #[test]
    fn region_locator_off_grid_returns_none() {
        let g = parse(5, 1, b"abc");
        assert!(RegionLocator::new(10, 0, 1, 1).resolve(&g).is_none());
        assert!(RegionLocator::new(0, 5, 1, 1).resolve(&g).is_none());
    }

    #[test]
    fn region_locator_text_extracts_multi_line() {
        let g = parse(10, 3, b"abcdefghij\r\nklmnopqrst\r\nuvwxyz");
        let t = RegionLocator::new(2, 0, 4, 2).text(&g);
        assert_eq!(t, "cdef\nmnop");
    }
}
