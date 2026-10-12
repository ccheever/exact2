//! The cell grid: what the painter draws into and the writer diffs.
//!
//! @ref LLP 1101 D7 — each cell holds a grapheme (or the continuation of a
//! wide one), its colours and attributes. The writer emits only sequences it
//! constructs itself; app text reaches it already stripped of controls.

use crate::color::ColorDepth;
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
    pub text: smol_str::SmolStr,
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
    colors: ColorDepth,
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
            colors: ColorDepth::TrueColor,
            cursor: None,
        }
    }

    /// Reduce paint to the output palette before diffing, so invisible colour
    /// changes cause no cell writes in monochrome mode.
    pub fn limit_colors(&mut self, colors: ColorDepth) {
        self.colors = colors;
        if colors != ColorDepth::TrueColor {
            for cell in &mut self.cells {
                cell.style.fg = cell.style.fg.and_then(|rgb| colors.reduce(rgb));
                cell.style.bg = cell.style.bg.and_then(|rgb| colors.reduce(rgb));
            }
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
        cell.text = text.into();
        cell.style = Style { bg, ..style };
        for i in 1..cols as i32 {
            if let Some(c) = self.cell_mut(x + i, y) {
                c.text = smol_str::SmolStr::default();
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
                sgr_colors(&mut out, current, cell.style, self.colors);
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
                    sgr_colors(&mut out, current, cell.style, self.colors);
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
                        sgr_colors(out, current, cell.style, self.colors);
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
                        sgr_colors(&mut out, current, cell.style, self.colors);
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

// SGR 22 clears both bold and faint; restore either one that remains on.
// Other attributes and the two colours can change independently, without
// resetting and resending the whole style for each differently coloured cell.
#[cfg(test)]
fn sgr(out: &mut String, before: Option<Style>, s: Style) {
    sgr_colors(out, before, s, ColorDepth::TrueColor);
}

fn sgr_colors(out: &mut String, before: Option<Style>, s: Style, colors: ColorDepth) {
    let start = out.len();
    out.push_str("\x1b[");
    let mut old = before.unwrap_or_default();
    if before.is_none() {
        out.push_str("0;");
    }
    if (old.bold && !s.bold) || (old.faint && !s.faint) {
        out.push_str("22;");
        old.bold = false;
        old.faint = false;
    }
    if s.bold && !old.bold {
        out.push_str("1;");
    }
    if s.faint && !old.faint {
        out.push_str("2;");
    }
    for (was, on, enable, disable) in [
        (old.italic, s.italic, "3;", "23;"),
        (old.underline, s.underline, "4;", "24;"),
        (old.reverse, s.reverse, "7;", "27;"),
        (old.strike, s.strike, "9;", "29;"),
    ] {
        if was != on {
            out.push_str(if on { enable } else { disable });
        }
    }
    if old.fg != s.fg {
        if let Some(rgb) = s.fg {
            colors.write(out, rgb, false);
        } else {
            out.push_str("39;");
        }
    }
    if old.bg != s.bg {
        if let Some(rgb) = s.bg {
            colors.write(out, rgb, true);
        } else {
            out.push_str("49;");
        }
    }
    if out.len() == start + 2 {
        // A link can change while the SGR attributes stay the same.
        out.truncate(start);
    } else {
        out.pop();
        out.push('m');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_reduction_preserves_text_and_suppresses_invisible_color_changes() {
        for (colors, expected) in [
            (ColorDepth::Monochrome, vt100::Color::Default),
            (ColorDepth::Ansi16, vt100::Color::Idx(9)),
            (ColorDepth::Ansi256, vt100::Color::Idx(196)),
            (ColorDepth::TrueColor, vt100::Color::Rgb(255, 0, 0)),
        ] {
            let mut grid = Grid::new(4, 1);
            grid.put(
                0,
                0,
                "界",
                2,
                Style {
                    fg: Some(Rgb(255, 0, 0)),
                    bg: Some(Rgb(255, 0, 0)),
                    bold: true,
                    underline: true,
                    reverse: true,
                    ..Style::default()
                },
                grid.bounds(),
            );
            grid.limit_colors(colors);
            for bytes in [grid.ansi(), grid.row_sgr(0), grid.diff(None)] {
                // `ansi` terminates its row with a newline; leave room so
                // the row under test does not scroll off the emulator.
                let mut parser = vt100::Parser::new(2, 4, 0);
                parser.process(bytes.as_bytes());
                let cell = parser.screen().cell(0, 0).unwrap();
                assert_eq!(cell.contents(), "界");
                assert_eq!(cell.fgcolor(), expected);
                assert_eq!(cell.bgcolor(), expected);
                assert!(cell.bold() && cell.underline() && cell.inverse());
            }
        }
        let mut a = Grid::new(1, 1);
        let mut b = Grid::new(1, 1);
        for (grid, rgb) in [(&mut a, Rgb(10, 20, 30)), (&mut b, Rgb(200, 100, 0))] {
            grid.put(
                0,
                0,
                "x",
                1,
                Style {
                    fg: Some(rgb),
                    bg: Some(rgb),
                    ..Style::default()
                },
                grid.bounds(),
            );
            grid.limit_colors(ColorDepth::Monochrome);
        }
        assert_eq!(a, b);
        let mut out = String::new();
        b.diff_relative(&a, &mut out);
        assert!(
            !out.contains('x'),
            "an invisible colour change rewrote the cell"
        );
    }

    #[test]
    fn sgr_transitions_preserve_other_attributes_and_restore_default_colours() {
        let styles: Vec<_> = (0..64)
            .flat_map(|bits| {
                (0..3).map(move |colour| Style {
                    bold: bits & 1 != 0,
                    faint: bits & 2 != 0,
                    italic: bits & 4 != 0,
                    underline: bits & 8 != 0,
                    reverse: bits & 16 != 0,
                    strike: bits & 32 != 0,
                    fg: [None, Some(Rgb(1, 2, 3)), Some(Rgb(234, 56, 78))][colour],
                    bg: [Some(Rgb(40, 50, 60)), None, Some(Rgb(210, 220, 230))][colour],
                    ..Style::default()
                })
            })
            .collect();
        for &before in &styles {
            for &after in &styles {
                let mut out = String::new();
                sgr(&mut out, None, before);
                out.push('a');
                sgr(&mut out, Some(before), after);
                out.push('b');
                let mut vt = vt100::Parser::new(1, 3, 0);
                vt.process(out.as_bytes());
                let c = vt.screen().cell(0, 1).unwrap();
                let colour = |v| match v {
                    Some(Rgb(r, g, b)) => vt100::Color::Rgb(r, g, b),
                    None => vt100::Color::Default,
                };
                assert_eq!(c.contents(), "b");
                assert_eq!(c.fgcolor(), colour(after.fg), "{before:?} -> {after:?}");
                assert_eq!(c.bgcolor(), colour(after.bg));
                assert_eq!(c.bold(), after.bold);
                assert_eq!(c.italic(), after.italic);
                assert_eq!(c.underline(), after.underline);
                assert_eq!(c.inverse(), after.reverse);
            }
        }
    }

    #[test]
    fn sgr_intensity_reset_restores_the_remaining_attribute() {
        // vt100 does not expose faint or strike, so check their SGR codes.
        let both = Style {
            bold: true,
            faint: true,
            ..Style::default()
        };
        let mut out = String::new();
        sgr(
            &mut out,
            Some(both),
            Style {
                faint: true,
                ..Style::default()
            },
        );
        assert_eq!(out, "\x1b[22;2m");
        out.clear();
        sgr(
            &mut out,
            Some(both),
            Style {
                bold: true,
                ..Style::default()
            },
        );
        assert_eq!(out, "\x1b[22;1m");
        out.clear();
        sgr(
            &mut out,
            Some(Style {
                strike: true,
                ..Style::default()
            }),
            Style::default(),
        );
        assert_eq!(out, "\x1b[29m");
        out.clear();
        sgr(&mut out, Some(both), Style { link: 1, ..both });
        assert!(out.is_empty(), "OSC links do not change SGR attributes");
    }

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
