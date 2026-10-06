//! The cell grid: what the painter draws into and the writer diffs.
//!
//! @ref LLP 1101 D7 — each cell holds a grapheme (or the continuation of a
//! wide one), its colours and attributes. The writer emits only sequences it
//! constructs itself; app text reaches it already stripped of controls.

use std::fmt::Write as _;

/// An opaque colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// How a cell's text is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// Foreground; `None` is the terminal's own.
    pub fg: Option<Rgb>,
    /// Background; `None` is the terminal's own.
    pub bg: Option<Rgb>,
    /// SGR 1.
    pub bold: bool,
    /// SGR 2.
    pub faint: bool,
    /// SGR 3.
    pub italic: bool,
    /// SGR 4.
    pub underline: bool,
    /// SGR 9.
    pub strike: bool,
    /// SGR 7: the UA focus indicator (LLP 1101 D6).
    pub reverse: bool,
    /// A hyperlink, by its index in the grid's links (0: none): written as
    /// OSC 8, which a terminal opens on Cmd- or Ctrl-click with no mouse
    /// reporting, so selection and scroll stay the terminal's.
    pub link: u16,
}

/// One cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The cluster; empty when this cell continues a wide one to its left.
    pub text: String,
    /// Its style.
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            text: " ".into(),
            style: Style::default(),
        }
    }
}

/// A rectangle of cells, `x..x+w` × `y..y+h`, possibly empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    /// Left column.
    pub x: i32,
    /// Top row.
    pub y: i32,
    /// Columns.
    pub w: i32,
    /// Rows.
    pub h: i32,
}

impl CellRect {
    /// The overlap of two rectangles.
    pub fn intersect(self, o: CellRect) -> CellRect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = (self.x + self.w).min(o.x + o.w);
        let b = (self.y + self.h).min(o.y + o.h);
        CellRect {
            x,
            y,
            w: (r - x).max(0),
            h: (b - y).max(0),
        }
    }

    /// Whether the cell is inside.
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// The screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// Columns.
    pub cols: usize,
    /// Rows.
    pub rows: usize,
    cells: Vec<Cell>,
    links: Vec<String>,
    /// Where the terminal's cursor goes (a focused field's caret), if shown.
    pub cursor: Option<(usize, usize)>,
}

impl Grid {
    /// A blank grid.
    pub fn new(cols: usize, rows: usize) -> Grid {
        Grid {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            links: Vec::new(),
            cursor: None,
        }
    }

    /// The whole grid as a rectangle.
    pub fn bounds(&self) -> CellRect {
        CellRect {
            x: 0,
            y: 0,
            w: self.cols as i32,
            h: self.rows as i32,
        }
    }

    /// The cell at a position.
    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * self.cols + x]
    }

    fn cell_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        if x < 0 || y < 0 || x as usize >= self.cols || y as usize >= self.rows {
            return None;
        }
        Some(&mut self.cells[y as usize * self.cols + x as usize])
    }

    /// Paint a background over a rectangle, within a clip.
    pub fn fill(&mut self, rect: CellRect, clip: CellRect, bg: Rgb) {
        let r = rect.intersect(clip);
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                if let Some(c) = self.cell_mut(x, y) {
                    c.text = " ".into();
                    c.style = Style {
                        bg: Some(bg),
                        ..Style::default()
                    };
                }
            }
        }
    }

    /// Draw a cluster taking `cols` columns, keeping the background beneath
    /// when the style has none.
    pub fn put(&mut self, x: i32, y: i32, text: &str, cols: usize, style: Style, clip: CellRect) {
        if !(0..cols as i32).all(|i| clip.contains(x + i, y)) {
            return;
        }
        let Some(cell) = self.cell_mut(x, y) else {
            return;
        };
        let bg = style.bg.or(cell.style.bg);
        cell.text = text.to_string();
        cell.style = Style { bg, ..style };
        for i in 1..cols as i32 {
            if let Some(c) = self.cell_mut(x + i, y) {
                c.text = String::new();
                c.style = Style { bg, ..style };
            }
        }
    }

    /// Whether a cell shows the same as `other`'s at the same place: text,
    /// style and link target — links compared by URL, since each grid
    /// numbers its links itself (LLP 1101.002 §0).
    fn same(&self, x: usize, y: usize, other: &Grid) -> bool {
        let (a, b) = (self.cell(x, y), other.cell(x, y));
        a.text == b.text
            && Style { link: 0, ..a.style } == Style { link: 0, ..b.style }
            && self.url(a.style.link) == other.url(b.style.link)
    }

    fn url(&self, link: u16) -> Option<&str> {
        self.links
            .get((link as usize).wrapping_sub(1))
            .map(String::as_str)
    }

    /// The link index for `url`, interned.
    pub fn link(&mut self, url: &str) -> u16 {
        if let Some(i) = self.links.iter().position(|l| l == url) {
            return i as u16 + 1;
        }
        self.links.push(url.to_string());
        self.links.len() as u16
    }

    /// Switch the open hyperlink from `from` to `to` (OSC 8; 0 closes).
    fn link_to(&self, out: &mut String, from: u16, to: u16) {
        if from == to {
            return;
        }
        match self.links.get((to as usize).wrapping_sub(1)) {
            Some(url) => {
                let _ = write!(out, "\x1b]8;;{url}\x1b\\");
            }
            None => out.push_str("\x1b]8;;\x1b\\"),
        }
    }

    /// Fade everything painted so far: the backdrop under an open dialog.
    pub fn dim(&mut self) {
        for c in &mut self.cells {
            c.style.faint = true;
            c.style.reverse = false;
        }
        self.cursor = None;
    }

    /// Set an attribute over a rectangle: reverse video for focus.
    pub fn reverse(&mut self, rect: CellRect, clip: CellRect) {
        let r = rect.intersect(clip);
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                if let Some(c) = self.cell_mut(x, y) {
                    c.style.reverse = true;
                }
            }
        }
    }

    /// The grid as plain text, each line's trailing blanks trimmed.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for y in 0..self.rows {
            let mut line = String::new();
            for x in 0..self.cols {
                line.push_str(&self.cell(x, y).text);
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    /// One row as SGR text for the normal screen, its trailing blank cells
    /// dropped — so a selection in the terminal copies no trailing spaces.
    pub fn row_sgr(&self, y: usize) -> String {
        let mut end = self.cols;
        while end > 0 {
            let c = self.cell(end - 1, y);
            if c.text == " " && c.style.bg.is_none() && !c.style.reverse && !c.style.underline {
                end -= 1;
            } else {
                break;
            }
        }
        let mut out = String::new();
        let mut current = None;
        for x in 0..end {
            let cell = self.cell(x, y);
            if current != Some(cell.style) {
                self.link_to(&mut out, current.map_or(0, |c| c.link), cell.style.link);
                sgr(&mut out, cell.style);
                current = Some(cell.style);
            }
            out.push_str(&cell.text);
        }
        if let Some(c) = current {
            self.link_to(&mut out, c.link, 0);
            out.push_str("\x1b[0m");
        }
        out
    }

    /// The grid as text with SGR attributes, for `screenshot out.ans`.
    pub fn ansi(&self) -> String {
        let mut out = String::new();
        for y in 0..self.rows {
            let mut current = None;
            for x in 0..self.cols {
                let cell = self.cell(x, y);
                if current != Some(cell.style) {
                    self.link_to(&mut out, current.map_or(0, |c| c.link), cell.style.link);
                    sgr(&mut out, cell.style);
                    current = Some(cell.style);
                }
                out.push_str(&cell.text);
            }
            self.link_to(&mut out, current.map_or(0, |c| c.link), 0);
            out.push_str("\x1b[0m\n");
        }
        out
    }

    /// The changed runs of cells from `before` to this grid (the same size),
    /// for a region of the normal screen whose top-left the cursor is at:
    /// moves are relative (down by rows, to a column), never absolute. Ends
    /// with the cursor at the start of the returned row.
    pub fn diff_relative(&self, before: &Grid, out: &mut String) -> usize {
        let mut at_row = 0;
        let mut current: Option<Style> = None;
        for y in 0..self.rows {
            let mut x = 0;
            while x < self.cols {
                if self.same(x, y, before) {
                    x += 1;
                    continue;
                }
                let mut start = x;
                while start > 0 && self.cell(start, y).text.is_empty() {
                    start -= 1;
                }
                if y > at_row {
                    let _ = write!(out, "\x1b[{}B", y - at_row);
                    at_row = y;
                }
                let _ = write!(out, "\x1b[{}G", start + 1);
                let mut end = start;
                while end < self.cols {
                    let differs = !self.same(end, y, before);
                    if end > x && !differs && !self.cell(end, y).text.is_empty() {
                        break;
                    }
                    let cell = self.cell(end, y);
                    if current != Some(cell.style) {
                        self.link_to(out, current.map_or(0, |c| c.link), cell.style.link);
                        sgr(out, cell.style);
                        current = Some(cell.style);
                    }
                    out.push_str(&cell.text);
                    end += 1;
                }
                x = end;
            }
        }
        self.link_to(out, current.map_or(0, |c| c.link), 0);
        out.push_str("\x1b[0m\r");
        at_row
    }

    /// The bytes that turn `before` into this grid: every changed run of
    /// cells, positioned and styled, inside one synchronized update. A
    /// different size repaints everything.
    pub fn diff(&self, before: Option<&Grid>) -> String {
        let before = before.filter(|b| b.cols == self.cols && b.rows == self.rows);
        let mut out = String::from("\x1b[?2026h\x1b[?25l");
        if before.is_none() {
            out.push_str("\x1b[0m\x1b[2J");
        }
        let mut current: Option<Style> = None;
        for y in 0..self.rows {
            let mut x = 0;
            while x < self.cols {
                let changed = before.is_none_or(|b| !self.same(x, y, b));
                if !changed {
                    x += 1;
                    continue;
                }
                // A continuation is drawn by its lead: start there.
                let mut start = x;
                while start > 0 && self.cell(start, y).text.is_empty() {
                    start -= 1;
                }
                let _ = write!(out, "\x1b[{};{}H", y + 1, start + 1);
                let mut at = start;
                while at < self.cols {
                    let differs = before.is_none_or(|b| !self.same(at, y, b));
                    if at > x && !differs && !self.cell(at, y).text.is_empty() {
                        break;
                    }
                    let cell = self.cell(at, y);
                    if current != Some(cell.style) {
                        self.link_to(&mut out, current.map_or(0, |c| c.link), cell.style.link);
                        sgr(&mut out, cell.style);
                        current = Some(cell.style);
                    }
                    out.push_str(&cell.text);
                    at += 1;
                }
                x = at;
            }
        }
        self.link_to(&mut out, current.map_or(0, |c| c.link), 0);
        out.push_str("\x1b[0m");
        if let Some((cx, cy)) = self.cursor {
            let _ = write!(out, "\x1b[{};{}H\x1b[5 q\x1b[?25h", cy + 1, cx + 1);
        }
        out.push_str("\x1b[?2026l");
        out
    }
}

fn sgr(out: &mut String, s: Style) {
    out.push_str("\x1b[0");
    for (on, code) in [
        (s.bold, ";1"),
        (s.faint, ";2"),
        (s.italic, ";3"),
        (s.underline, ";4"),
        (s.reverse, ";7"),
        (s.strike, ";9"),
    ] {
        if on {
            out.push_str(code);
        }
    }
    if let Some(Rgb(r, g, b)) = s.fg {
        let _ = write!(out, ";38;2;{r};{g};{b}");
    }
    if let Some(Rgb(r, g, b)) = s.bg {
        let _ = write!(out, ";48;2;{r};{g};{b}");
    }
    out.push('m');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_that_changes_target_on_the_same_text_is_redrawn() {
        let all = Grid::new(3, 1).bounds();
        let mut before = Grid::new(3, 1);
        let one = before.link("https://a.example");
        before.put(
            0,
            0,
            "x",
            1,
            Style {
                link: one,
                ..Style::default()
            },
            all,
        );
        let mut after = Grid::new(3, 1);
        let two = after.link("https://b.example");
        after.put(
            0,
            0,
            "x",
            1,
            Style {
                link: two,
                ..Style::default()
            },
            all,
        );
        assert_eq!(one, two, "each grid numbers its links from 1");
        let mut out = String::new();
        after.diff_relative(&before, &mut out);
        assert!(out.contains("b.example"), "{out:?}");
    }

    #[test]
    fn a_wide_cluster_takes_two_cells_and_the_diff_redraws_from_its_lead() {
        let mut g = Grid::new(4, 1);
        let all = g.bounds();
        g.put(0, 0, "日", 2, Style::default(), all);
        assert_eq!(g.text(), "日\n");
        let before = g.clone();
        g.put(2, 0, "x", 1, Style::default(), all);
        let bytes = g.diff(Some(&before));
        assert!(bytes.contains("\x1b[1;3H"), "{bytes:?}");
        assert!(!bytes.contains('日'), "unchanged cells are not rewritten");
    }
}
