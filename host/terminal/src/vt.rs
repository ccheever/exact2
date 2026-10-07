//! The headless screen (LLP 1101.002 §0 P1): the bytes the interactive loop
//! would write go into a terminal emulator (vt100) instead, and the agent
//! reads that emulator's screen. What `print` and `screenshot` show is what
//! a person would see, scrollback and all, not the document the host laid
//! out; a writer bug shows up as a wrong screen.

use crate::grid::Grid;
use crate::host::{Host, Mode};
use crate::image::Protocol;
use crate::term::{record, Inline};
use exact_runner::DataSource;

/// The emulator keeps every row that scrolls off, so its count of them
/// only grows and each new one is found by that count.
const SCROLLBACK: usize = usize::MAX;

/// A terminal the host writes into, as the interactive loop writes into a
/// real one.
pub struct Vt {
    parser: vt100::Parser,
    /// Rows that have scrolled off the top, oldest first.
    scrolled: Vec<String>,
    /// How many of the emulator's scrollback rows `scrolled` holds.
    seen: usize,
    mode: Mode,
    inline: Inline,
    /// Full screen: the grid last written.
    shown: Option<Grid>,
}

impl Vt {
    /// A blank terminal the host's size, in the host's mode.
    pub fn new<D: DataSource>(host: &mut Host<D>) -> Vt {
        let (cols, rows) = host.size();
        let mode = host.mode;
        host.publish(&record(mode, Protocol::Blocks, 0, ""));
        Vt {
            parser: vt100::Parser::new(rows as u16, cols as u16, SCROLLBACK),
            scrolled: Vec::new(),
            seen: 0,
            mode,
            inline: Inline::new(Protocol::Blocks),
            shown: None,
        }
    }

    /// Write the host's next frame, if anything changed, and answer the
    /// writer's position queries from the emulator's cursor, as a terminal
    /// does. The bytes written, for a test that counts them.
    pub fn render<D: DataSource>(&mut self, host: &mut Host<D>) -> String {
        let (cols, rows) = host.size();
        if self.parser.screen().size() != (rows as u16, cols as u16) {
            self.parser.set_size(rows as u16, cols as u16);
        }
        let mut out = String::new();
        match self.mode {
            Mode::Fullscreen => {
                let grid = &host.frame().grid;
                if self.shown.as_ref() != Some(grid) {
                    out = grid.diff(self.shown.as_ref());
                    self.shown = Some(grid.clone());
                }
            }
            Mode::Inline => self.inline.frame(host, &mut out),
        }
        if out.is_empty() {
            return out;
        }
        let mut pieces = out.split("\x1b[6n").peekable();
        while let Some(piece) = pieces.next() {
            self.feed(piece);
            if pieces.peek().is_some() {
                let row = self.parser.screen().cursor_position().0 as usize + 1;
                self.inline.answered(host, row);
            }
        }
        host.frames += 1;
        // The app learns what is in scrollback once the bytes are out.
        if self.mode == Mode::Inline {
            if let Some(r) = self.inline.printed(self.mode) {
                host.publish(&r);
            }
        }
        out
    }

    /// Position queries the writer asked that went unanswered: a writer
    /// that waits on one is waiting forever.
    pub fn unanswered(&self) -> usize {
        self.inline.unanswered()
    }

    /// Feed the emulator a line at a time, collecting after each: a row can
    /// be read back only while it is within a screen's height of the
    /// bottom of the scrollback (vt100 shows scrollback by offset, and an
    /// offset past the screen's height is out of its range).
    fn feed(&mut self, bytes: &str) {
        for piece in bytes.as_bytes().split_inclusive(|b| *b == b'\n') {
            self.parser.process(piece);
            self.collect();
        }
    }

    /// Copy rows that scrolled off since the last look into `scrolled`.
    fn collect(&mut self) {
        self.parser.set_scrollback(usize::MAX);
        let len = self.parser.screen().scrollback();
        let (rows, cols) = self.parser.screen().size();
        let new = len.saturating_sub(self.seen);
        if new > 0 {
            // One line scrolls at most a few rows; more than a screen at
            // once (a scroll-up sequence) leaves rows no one can read back.
            let readable = new.min(rows as usize);
            if new > readable {
                self.scrolled
                    .push(format!("[{} rows scrolled past unread]", new - readable));
            }
            self.parser.set_scrollback(readable);
            self.scrolled
                .extend(self.parser.screen().rows(0, cols).take(readable));
        }
        self.parser.set_scrollback(0);
        self.seen = len;
    }

    /// Rows that have scrolled off so far: `text_since` reads from here.
    pub fn scrolled(&self) -> usize {
        self.scrolled.len()
    }

    /// The rows that scrolled off since `from`, then the screen.
    pub fn text_since(&self, from: usize) -> String {
        let (_, cols) = self.parser.screen().size();
        let mut out = String::new();
        let screen = self.parser.screen().rows(0, cols);
        for l in self.scrolled[from.min(self.scrolled.len())..]
            .iter()
            .cloned()
            .chain(screen)
        {
            out.push_str(l.trim_end());
            out.push('\n');
        }
        out
    }

    /// The screen as text, one line per row; `all` puts the scrollback
    /// above it.
    pub fn text(&self, all: bool) -> String {
        self.text_since(if all { 0 } else { self.scrolled.len() })
    }

    /// The visible screen with its colours, as SGR. Full screen, that is
    /// the grid the host wrote; inline, the emulator's screen, which keeps
    /// colours, bold, italic, underline and inverse but not faint, strike
    /// or links (vt100 has no cell for them).
    pub fn ansi(&self) -> String {
        match &self.shown {
            Some(grid) => grid.ansi(),
            None => {
                String::from_utf8_lossy(&self.parser.screen().contents_formatted()).into_owned()
            }
        }
    }
}
