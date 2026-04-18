//! Grid model and ANSI/VT parser.
//!
//! A [`Grid`] is a fixed-size rectangular cell buffer with a cursor.
//! [`Parser`] feeds raw bytes through the `vte` crate and materializes
//! them into the grid, tracking the most common SGR attributes that
//! downstream assertions care about: bold, underline, reverse, and
//! 8-color foreground/background.
//!
//! This is intentionally a *subset* of a full terminal emulator — the
//! goal is not to render arbitrary TUIs pixel-perfectly, it is to give
//! Tmuxwright a deterministic, hashable view of the pane that tests
//! can query with text and region locators.

use vte::{Params, Perform};

/// 8-color palette index for SGR 30-37 / 40-47. `Default` is "caller
/// didn't set one", which is distinct from a zero color value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
}

/// Per-cell text attributes tracked by the grid. Kept tight so the
/// hash in C2 stays cheap.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attrs {
    pub bold: bool,
    pub underline: bool,
    pub reverse: bool,
    pub fg: Color,
    pub bg: Color,
}

/// A single grid cell. Space + default attrs is the canonical "empty".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub ch: char,
    pub attrs: Attrs,
}

impl Cell {
    #[must_use]
    pub fn blank() -> Self {
        Self {
            ch: ' ',
            attrs: Attrs::default(),
        }
    }
}

/// Fixed-size character grid with a cursor. Origin (0,0) is top-left.
#[derive(Debug, Clone)]
pub struct Grid {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    cursor_x: u16,
    cursor_y: u16,
    attrs: Attrs,
}

impl Grid {
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let len = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            cells: vec![Cell::blank(); len],
            cursor_x: 0,
            cursor_y: 0,
            attrs: Attrs::default(),
        }
    }

    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }
    #[must_use]
    pub fn cursor(&self) -> (u16, u16) {
        (self.cursor_x, self.cursor_y)
    }

    #[must_use]
    pub fn cell(&self, x: u16, y: u16) -> Option<&Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get(self.idx(x, y))
    }

    /// Render one row as a plain string with trailing blanks trimmed.
    #[must_use]
    pub fn row_text(&self, y: u16) -> String {
        if y >= self.height {
            return String::new();
        }
        let start = self.idx(0, y);
        let end = start + usize::from(self.width);
        let mut s: String = self.cells[start..end].iter().map(|c| c.ch).collect();
        let trimmed = s.trim_end_matches(' ').len();
        s.truncate(trimmed);
        s
    }

    /// Full grid as text, one row per line, trailing blanks trimmed.
    #[must_use]
    pub fn to_text(&self) -> String {
        (0..self.height)
            .map(|y| self.row_text(y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn idx(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.width) + usize::from(x)
    }

    fn put_char(&mut self, ch: char) {
        if self.cursor_y >= self.height {
            self.scroll_up();
            self.cursor_y = self.height - 1;
        }
        if self.cursor_x >= self.width {
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y.saturating_add(1);
            if self.cursor_y >= self.height {
                self.scroll_up();
                self.cursor_y = self.height - 1;
            }
        }
        let i = self.idx(self.cursor_x, self.cursor_y);
        self.cells[i] = Cell {
            ch,
            attrs: self.attrs,
        };
        self.cursor_x = self.cursor_x.saturating_add(1);
    }

    fn newline(&mut self) {
        self.cursor_y = self.cursor_y.saturating_add(1);
        if self.cursor_y >= self.height {
            self.scroll_up();
            self.cursor_y = self.height - 1;
        }
    }

    fn carriage_return(&mut self) {
        self.cursor_x = 0;
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
    }

    fn scroll_up(&mut self) {
        let w = usize::from(self.width);
        // Move rows [1..height) up by one.
        self.cells.copy_within(w.., 0);
        // Clear last row.
        let start = usize::from(self.height - 1) * w;
        for c in &mut self.cells[start..start + w] {
            *c = Cell::blank();
        }
    }

    fn erase_in_line(&mut self, mode: u16) {
        let y = self.cursor_y;
        if y >= self.height {
            return;
        }
        let (from, to) = match mode {
            0 => (self.cursor_x, self.width),          // cursor to EOL
            1 => (0, self.cursor_x.saturating_add(1)), // BOL to cursor
            2 => (0, self.width),                      // whole line
            _ => return,
        };
        let row_start = self.idx(0, y);
        for x in from..to {
            self.cells[row_start + usize::from(x)] = Cell::blank();
        }
    }

    fn erase_in_display(&mut self, mode: u16) {
        match mode {
            0 => {
                // Cursor to end: clear rest of current line then all below.
                self.erase_in_line(0);
                let y0 = self.cursor_y.saturating_add(1);
                for y in y0..self.height {
                    let start = self.idx(0, y);
                    for c in &mut self.cells[start..start + usize::from(self.width)] {
                        *c = Cell::blank();
                    }
                }
            }
            1 => {
                // Start to cursor: clear all above then BOL..cursor.
                for y in 0..self.cursor_y {
                    let start = self.idx(0, y);
                    for c in &mut self.cells[start..start + usize::from(self.width)] {
                        *c = Cell::blank();
                    }
                }
                self.erase_in_line(1);
            }
            2 | 3 => {
                for c in &mut self.cells {
                    *c = Cell::blank();
                }
            }
            _ => {}
        }
    }

    fn cursor_to(&mut self, x: u16, y: u16) {
        self.cursor_x = x.min(self.width.saturating_sub(1));
        self.cursor_y = y.min(self.height.saturating_sub(1));
    }

    fn apply_sgr(&mut self, params: &Params) {
        let mut had_any = false;
        for group in params {
            had_any = true;
            let code = group.first().copied().unwrap_or(0);
            match code {
                0 => self.attrs = Attrs::default(),
                1 => self.attrs.bold = true,
                22 => self.attrs.bold = false,
                4 => self.attrs.underline = true,
                24 => self.attrs.underline = false,
                7 => self.attrs.reverse = true,
                27 => self.attrs.reverse = false,
                30..=37 => self.attrs.fg = Color::Indexed(u8::try_from(code - 30).unwrap_or(0)),
                39 => self.attrs.fg = Color::Default,
                40..=47 => self.attrs.bg = Color::Indexed(u8::try_from(code - 40).unwrap_or(0)),
                49 => self.attrs.bg = Color::Default,
                90..=97 => {
                    self.attrs.fg = Color::Indexed(u8::try_from(code - 90 + 8).unwrap_or(0));
                }
                100..=107 => {
                    self.attrs.bg = Color::Indexed(u8::try_from(code - 100 + 8).unwrap_or(0));
                }
                _ => {}
            }
        }
        if !had_any {
            self.attrs = Attrs::default();
        }
    }
}

/// Stateful parser: feeds bytes through `vte::Parser` into a [`Grid`].
pub struct Parser {
    grid: Grid,
    vte: vte::Parser,
}

impl Parser {
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            grid: Grid::new(width, height),
            vte: vte::Parser::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut performer = Performer {
            grid: &mut self.grid,
        };
        for b in bytes {
            self.vte.advance(&mut performer, *b);
        }
    }

    #[must_use]
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    #[must_use]
    pub fn into_grid(self) -> Grid {
        self.grid
    }
}

struct Performer<'a> {
    grid: &'a mut Grid,
}

impl Perform for Performer<'_> {
    fn print(&mut self, c: char) {
        self.grid.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => self.grid.backspace(),
            b'\n' => self.grid.newline(),
            b'\r' => self.grid.carriage_return(),
            b'\t' => {
                // Advance to next multiple of 8.
                let next = (self.grid.cursor_x / 8 + 1) * 8;
                let target = next.min(self.grid.width.saturating_sub(1));
                self.grid.cursor_x = target;
            }
            // BEL (0x07) and other C0 controls we don't model fall through.
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _inter: &[u8], _ignored: bool, action: char) {
        let first = params
            .iter()
            .next()
            .and_then(|g| g.first().copied())
            .unwrap_or(0);
        let second = params
            .iter()
            .nth(1)
            .and_then(|g| g.first().copied())
            .unwrap_or(0);
        match action {
            'm' => self.grid.apply_sgr(params),
            'H' | 'f' => {
                // CUP: row;col, 1-based in the protocol.
                let row = first.saturating_sub(1);
                let col = second.saturating_sub(1);
                self.grid.cursor_to(col, row);
            }
            'A' => {
                let n = first.max(1);
                self.grid.cursor_y = self.grid.cursor_y.saturating_sub(n);
            }
            'B' => {
                let n = first.max(1);
                let max = self.grid.height.saturating_sub(1);
                self.grid.cursor_y = (self.grid.cursor_y + n).min(max);
            }
            'C' => {
                let n = first.max(1);
                let max = self.grid.width.saturating_sub(1);
                self.grid.cursor_x = (self.grid.cursor_x + n).min(max);
            }
            'D' => {
                let n = first.max(1);
                self.grid.cursor_x = self.grid.cursor_x.saturating_sub(n);
            }
            'G' => {
                let col = first.saturating_sub(1);
                let max = self.grid.width.saturating_sub(1);
                self.grid.cursor_x = col.min(max);
            }
            'J' => self.grid.erase_in_display(first),
            'K' => self.grid.erase_in_line(first),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
}
