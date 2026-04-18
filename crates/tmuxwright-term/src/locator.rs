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
