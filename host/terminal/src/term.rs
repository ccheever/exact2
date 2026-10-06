//! The terminal itself: raw mode, the bytes in, the bytes out, and always
//! putting it back.
//!
//! @ref LLP 1101 D6 (input), D7 (restoration), D9 (vte parses input; the
//! output is constructed here; rustix for termios and the window size),
//! D10 (full screen and inline)
//!
//! Inline, the app lives on the normal screen like a shell's output: each
//! settled transcript entry is printed once and becomes the terminal's own
//! scrollback — scrolled, selected and copied by the terminal, in tmux too —
//! and only the live tail below it (a streaming reply, a spinner, the input)
//! is redrawn. Full screen, the app owns the alternate screen and the mouse.

use crate::host::{After, Host, Key, Mode};
use crate::image::{protocol_bytes, Protocol};
use exact_runner::DataSource;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::termios::{tcgetattr, tcgetwinsize, tcsetattr, OptionalActions, Termios};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vte::{Params, Parser, Perform};

/// The terminal's settings before raw mode, for every way out.
static SAVED: Mutex<Option<(Termios, Mode)>> = Mutex::new(None);

/// Alternate screen, SGR mouse with drags, bracketed paste, kitty keyboard
/// disambiguation (LLP 1101 D6).
const ENTER_FULL: &str = "\x1b[?1049h\x1b[?25l\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?2004h\x1b[>1u";
/// The normal screen: no mouse capture, so selection stays the terminal's.
const ENTER_INLINE: &str = "\x1b[?25l\x1b[?2004h\x1b[>1u";
/// Each mode undone, in reverse.
const LEAVE_FULL: &str =
    "\x1b[<u\x1b[?2004l\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[0m\x1b[0 q\x1b[?25h\x1b[?1049l";
const LEAVE_INLINE: &str = "\x1b[<u\x1b[?2004l\x1b[?1006l\x1b[?1000l\x1b[0m\x1b[0 q\x1b[?25h";

fn restore() {
    let saved = SAVED.lock().ok().and_then(|mut s| s.take());
    let mut out = std::io::stdout();
    if let Some((termios, mode)) = saved {
        let leave = if mode == Mode::Fullscreen {
            LEAVE_FULL
        } else {
            LEAVE_INLINE
        };
        let _ = out.write_all(leave.as_bytes());
        let _ = out.flush();
        let _ = tcsetattr(rustix::stdio::stdin(), OptionalActions::Now, &termios);
    }
}

/// The terminal's size in cells, if it reports one.
pub fn size() -> Option<(usize, usize)> {
    let w = tcgetwinsize(rustix::stdio::stdout()).ok()?;
    (w.ws_col > 0 && w.ws_row > 0).then_some((w.ws_col as usize, w.ws_row as usize))
}

/// Whether stdout is a terminal.
pub fn is_terminal() -> bool {
    rustix::termios::isatty(rustix::stdio::stdout())
        && rustix::termios::isatty(rustix::stdio::stdin())
}

/// What the input decodes to.
#[derive(Debug, PartialEq)]
pub enum Input {
    /// A key.
    Key(Key),
    /// A mouse press at a cell.
    Click(i32, i32),
    /// A wheel at a cell, rows (negative is up).
    Wheel(i32, i32, i32),
    /// A bracketed paste, whole.
    Paste(String),
    /// The terminal's answer to a cursor position query: row, column (1-based).
    CursorAt(usize, usize),
}

/// The input decoder: its state (a paste in progress) outlives a read.
#[derive(Default)]
pub struct Decoder {
    out: Vec<Input>,
    paste: Option<String>,
    /// A lone ESC ended the last read.
    escape: bool,
}

impl Decoder {
    fn key(&mut self, k: Key) {
        self.out.push(Input::Key(k));
    }
}

fn ctrl_of(byte: u8) -> Option<Key> {
    match byte {
        0x0d | 0x0a => Some(Key::Named("Enter")),
        0x09 => Some(Key::Named("Tab")),
        0x08 => Some(Key::Named("Backspace")),
        0x01..=0x1a => Some(Key::Ctrl((b'a' + byte - 1) as char)),
        0x1c => Some(Key::Ctrl('\\')),
        _ => None,
    }
}

/// A named key with modifiers (xterm/kitty: 1 + shift 1, alt 2, ctrl 4).
fn with_mods(name: &'static str, mods: u16) -> Key {
    if mods == 0 {
        return Key::Named(name);
    }
    if mods == 1 && name == "Tab" {
        return Key::BackTab;
    }
    let mut chord = String::new();
    for (bit, word) in [(4, "Control+"), (2, "Alt+"), (1, "Shift+")] {
        if mods & bit != 0 {
            chord.push_str(word);
        }
    }
    chord.push_str(name);
    Key::Chord(chord)
}

impl Perform for Decoder {
    fn print(&mut self, c: char) {
        match &mut self.paste {
            Some(p) => p.push(c),
            None => self.key(Key::Char(c)),
        }
    }

    fn execute(&mut self, byte: u8) {
        if let Some(p) = &mut self.paste {
            match byte {
                b'\n' | b'\r' => p.push('\n'),
                b'\t' => p.push('\t'),
                _ => {}
            }
            return;
        }
        if let Some(k) = ctrl_of(byte) {
            self.key(k);
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        // ESC followed by a key: Alt+key in the legacy encoding.
        if intermediates.is_empty() && self.paste.is_none() && byte.is_ascii_graphic() {
            self.key(Key::Chord(format!("Alt+{}", byte as char)));
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let p: Vec<u16> = params
            .iter()
            .map(|p| p.first().copied().unwrap_or(0))
            .collect();
        let first = p.first().copied().unwrap_or(0);
        let mods = p.get(1).copied().unwrap_or(1).saturating_sub(1);
        if action == '~' && (first == 200 || first == 201) {
            if first == 200 {
                self.paste = Some(String::new());
            } else if let Some(text) = self.paste.take() {
                self.out.push(Input::Paste(text));
            }
            return;
        }
        if self.paste.is_some() {
            return;
        }
        match (intermediates, action) {
            (b"<", 'M') | (b"<", 'm') => {
                let (b, x, y) = (
                    first,
                    p.get(1).copied().unwrap_or(1) as i32 - 1,
                    p.get(2).copied().unwrap_or(1) as i32 - 1,
                );
                if b & 64 != 0 {
                    self.out
                        .push(Input::Wheel(x, y, if b & 1 == 0 { -3 } else { 3 }));
                } else if action == 'M' && b & 32 == 0 && b & 3 == 0 {
                    self.out.push(Input::Click(x, y));
                }
            }
            (_, 'A') => self.key(with_mods("ArrowUp", mods)),
            (_, 'B') => self.key(with_mods("ArrowDown", mods)),
            (_, 'C') => self.key(with_mods("ArrowRight", mods)),
            (_, 'D') => self.key(with_mods("ArrowLeft", mods)),
            (_, 'H') => self.key(with_mods("Home", mods)),
            (_, 'F') => self.key(with_mods("End", mods)),
            (_, 'Z') => self.key(Key::BackTab),
            (b"", 'R') if p.len() == 2 => {
                self.out.push(Input::CursorAt(p[0] as usize, p[1] as usize))
            }
            (_, '~') => match first {
                1 | 7 => self.key(with_mods("Home", mods)),
                4 | 8 => self.key(with_mods("End", mods)),
                3 => self.key(with_mods("Delete", mods)),
                5 => self.key(with_mods("PageUp", mods)),
                6 => self.key(with_mods("PageDown", mods)),
                _ => {}
            },
            // The kitty keyboard protocol: a code point and its modifiers.
            (_, 'u') => {
                let named = match first {
                    27 => Some("Escape"),
                    13 => Some("Enter"),
                    9 => Some("Tab"),
                    127 => Some("Backspace"),
                    _ => None,
                };
                if let Some(name) = named {
                    self.key(with_mods(name, mods));
                } else if let Some(c) = char::from_u32(first as u32) {
                    if mods & 4 != 0 && mods & !5 == 0 {
                        self.key(Key::Ctrl(c.to_ascii_lowercase()));
                    } else if mods & 2 != 0 {
                        self.key(Key::Chord(format!("Alt+{c}")));
                    } else {
                        self.key(Key::Char(c));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Decode a read: DEL by hand (a terminal parser ignores it), everything
/// else through vte. A read ending in a lone ESC is held: it is the Escape
/// key only if nothing follows within [`ESC_WAIT_MS`] (`flush_escape`), and
/// otherwise the start of a sequence split across reads.
pub fn decode(parser: &mut Parser, decoder: &mut Decoder, bytes: &[u8]) -> Vec<Input> {
    let held = std::mem::take(&mut decoder.escape);
    let joined;
    let bytes = if held {
        joined = [&[0x1b][..], bytes].concat();
        &joined[..]
    } else {
        bytes
    };
    let mut start = 0;
    for (i, b) in bytes.iter().enumerate() {
        if *b == 0x7f {
            parser.advance(decoder, &bytes[start..i]);
            if decoder.paste.is_none() {
                decoder.key(Key::Named("Backspace"));
            }
            start = i + 1;
        }
    }
    let rest = &bytes[start..];
    if rest.last() == Some(&0x1b) && decoder.paste.is_none() {
        parser.advance(decoder, &rest[..rest.len() - 1]);
        decoder.escape = true;
    } else {
        parser.advance(decoder, rest);
    }
    std::mem::take(&mut decoder.out)
}

/// How long a lone ESC waits for the rest of a sequence.
pub const ESC_WAIT_MS: f64 = 25.0;

/// The held ESC, now that nothing followed it: the Escape key.
pub fn flush_escape(decoder: &mut Decoder) -> Vec<Input> {
    if std::mem::take(&mut decoder.escape) {
        vec![Input::Key(Key::Named("Escape"))]
    } else {
        Vec::new()
    }
}

/// The inline writer's memory of what it already put on the screen.
pub(crate) struct Inline {
    /// Transcript children already printed into scrollback, by identity,
    /// and the `id` of the last one, which the app reads to retire them.
    printed: std::collections::HashSet<exact_kernel::NodeKey>,
    through: String,
    /// The log node the printed children belong to, and whether it has
    /// printed any.
    log: Option<exact_kernel::NodeKey>,
    printed_any: bool,
    /// The live region's height and the cursor's row inside it.
    live: usize,
    cursor_row: usize,
    /// What the last frame showed: generation, size.
    shown: Option<(u64, usize, usize)>,
    protocol: Protocol,
    /// The live region as last written.
    last: Option<crate::grid::Grid>,
    /// Mouse reporting is on (only while a dialog is open, LLP 1101.001),
    /// and the cursor row inside the region at each position query still
    /// unanswered: the answer places the presented painting on the screen.
    mouse: bool,
    queries: std::collections::VecDeque<usize>,
}

impl Inline {
    /// A writer that has written nothing.
    pub(crate) fn new(protocol: Protocol) -> Inline {
        Inline {
            printed: std::collections::HashSet::new(),
            through: String::new(),
            log: None,
            printed_any: false,
            live: 0,
            cursor_row: 0,
            shown: None,
            protocol,
            last: None,
            mouse: false,
            queries: std::collections::VecDeque::new(),
        }
    }

    /// The `id` of the last transcript child written to scrollback.
    pub(crate) fn through(&self) -> &str {
        &self.through
    }

    /// Position queries asked and not yet answered.
    pub(crate) fn unanswered(&self) -> usize {
        self.queries.len()
    }
}

impl Inline {
    pub(crate) fn frame<D: DataSource>(&mut self, host: &mut Host<D>, out: &mut String) {
        let (cols, rows) = host.size();
        if self.shown == Some((host.generation, cols, rows)) {
            return;
        }
        let resized = self.shown.is_some_and(|(_, c, r)| (c, r) != (cols, rows));
        self.shown = Some((host.generation, cols, rows));
        let total = host.document_rows();
        // A new log node is a new transcript: clear the screen and the
        // scrollback, as `/clear` does in a terminal's own shell.
        let log = host.log_key();
        // Printed children are retired as they go, so remember that this log
        // printed at all, not which children it still has.
        let cleared = self.log.is_some() && log != self.log && self.printed_any;
        if log != self.log {
            self.printed_any = false;
        }
        self.log = log;
        if cleared {
            if self.cursor_row > 0 {
                out.push_str(&format!("\x1b[{}A", self.cursor_row));
            }
            // The screen only: the terminal's scrollback holds the person's
            // own history too, which a new transcript has no right to erase.
            out.push_str("\x1b[H\x1b[2J");
            self.printed.clear();
            self.cursor_row = 0;
            self.last = None;
        }
        let children = host.transcript();
        // Printed children are known by identity (LLP 1101.001 P1): those the
        // app has since retired, or cleared, are forgotten.
        self.printed
            .retain(|k| children.iter().any(|c| c.key == *k));
        let settled = children
            .iter()
            .position(|c| c.busy)
            .unwrap_or(children.len());
        let lead = children
            .iter()
            .take_while(|c| self.printed.contains(&c.key))
            .count();
        let bottom_of = |n: usize| if n == 0 { 0 } else { children[n - 1].bottom };
        out.push_str("\x1b[?2026h\x1b[?25l");
        // Back to the live region's top-left.
        if self.cursor_row > 0 {
            out.push_str(&format!("\x1b[{}A", self.cursor_row));
        }
        out.push('\r');
        let mut fresh = resized || cleared;
        let mut through = lead;
        if settled > lead {
            fresh = true;
            out.push_str("\x1b[J");
            let (from, to) = (bottom_of(lead), bottom_of(settled));
            if to > from {
                let painted = host.render(from, to - from);
                self.print_rows(&painted, &host.images, out);
            }
            self.printed
                .extend(children[lead..settled].iter().map(|c| c.key));
            self.printed_any = true;
            through = settled;
            self.through = children[settled - 1].id.clone();
        }
        let top = bottom_of(through);
        let live_total = total.saturating_sub(top);
        // An open dialog goes below the live content, anchored to the
        // region's bottom; the region grows down into new rows to hold it,
        // never up over what is printed.
        let layer = host.layer_rows().unwrap_or(0);
        let show = (live_total + layer).min(rows.saturating_sub(1)).max(1);
        let content = show.saturating_sub(layer);
        let from = total.saturating_sub(content).max(top);
        let painted = host.render(from, show);
        // The same region as last time: write only the cells that changed
        // (a spinner's tick is a few bytes). Otherwise repaint it whole.
        let row = match self
            .last
            .as_ref()
            .filter(|last| !fresh && last.rows == show && last.cols == cols)
        {
            Some(last) => painted.grid.diff_relative(last, out),
            None => {
                out.push_str("\x1b[J");
                for y in 0..show {
                    out.push_str(&painted.grid.row_sgr(y));
                    if y + 1 < show {
                        out.push_str("\r\n");
                    }
                }
                out.push('\r');
                show - 1
            }
        };
        self.live = show;
        // The cursor from `row` to the caret, or to the region's last row.
        let (target_row, col) = match painted.grid.cursor {
            Some((cx, cy)) => (cy.min(show - 1), Some(cx)),
            None => (show - 1, None),
        };
        if target_row > row {
            out.push_str(&format!("\x1b[{}B", target_row - row));
        } else if row > target_row {
            out.push_str(&format!("\x1b[{}A", row - target_row));
        }
        if let Some(cx) = col {
            out.push_str(&format!("\x1b[{}G\x1b[5 q\x1b[?25h", cx + 1));
        }
        self.cursor_row = target_row;
        host.present(&painted, None);
        self.last = Some(painted.grid);
        // Clicks only while a dialog is open; scroll and selection are the
        // terminal's the rest of the time.
        let want = host.has_layer();
        if want != self.mouse {
            out.push_str(if want {
                "\x1b[?1000h\x1b[?1006h"
            } else {
                "\x1b[?1006l\x1b[?1000l"
            });
            self.mouse = want;
        }
        // Where the region is on the screen: asked while clicks matter and
        // after a resize, when the terminal has reflowed what is above.
        if self.mouse || resized {
            out.push_str("\x1b[6n");
            self.queries.push_back(self.cursor_row);
        }
        out.push_str("\x1b[?2026l");
    }

    /// A position answer: the region's top is the answered row less the
    /// cursor's row in the region when it was asked.
    pub(crate) fn answered<D: DataSource>(&mut self, host: &mut Host<D>, row: usize) {
        if let Some(in_region) = self.queries.pop_front() {
            host.anchor((row as i32 - 1 - in_region as i32).max(0));
        }
    }

    /// Print settled rows into the scrollback, an image protocol's picture
    /// over each image's cells once its last row is out.
    fn print_rows(
        &self,
        painted: &crate::paint::Painted,
        images: &crate::image::Images,
        out: &mut String,
    ) {
        let rows = painted.grid.rows;
        for y in 0..rows {
            out.push_str(&painted.grid.row_sgr(y));
            if self.protocol != Protocol::Blocks {
                for (id, r) in &painted.images {
                    if r.y + r.h - 1 == y as i32 && r.y >= 0 && r.h > 0 {
                        if let Some(image) = images.of(*id) {
                            let bytes =
                                protocol_bytes(self.protocol, image, r.w as usize, r.h as usize);
                            out.push_str("\x1b7");
                            if r.h > 1 {
                                out.push_str(&format!("\x1b[{}A", r.h - 1));
                            }
                            out.push_str(&format!("\x1b[{}G", r.x + 1));
                            out.push_str(&bytes);
                            out.push_str("\x1b8");
                        }
                    }
                }
            }
            out.push_str("\r\n");
        }
    }

    /// Leave the screen with the last frame in place and the cursor below it.
    fn finish(&self, out: &mut String) {
        let down = self.live.saturating_sub(1 + self.cursor_row);
        if down > 0 {
            out.push_str(&format!("\x1b[{down}B"));
        }
        out.push_str("\r\n");
    }
}

/// The terminal's record for the app, `exactSurface("terminal")`.
/// A JSON string's contents: quotes, backslashes and controls escaped, so
/// an id round-trips unchanged.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn record(mode: Mode, images: Protocol, printed: &str) -> String {
    let mode = if mode == Mode::Inline {
        "inline"
    } else {
        "fullscreen"
    };
    let images = match images {
        Protocol::Kitty => "kitty",
        Protocol::Iterm => "iterm",
        Protocol::Blocks => "blocks",
    };
    let printed = json_string(printed);
    format!(
        "{{\"mode\":\"{mode}\",\"images\":\"{images}\",\"colors\":24,\"printed\":\"{printed}\"}}"
    )
}

/// Run the host in this terminal until the app or the user quits.
pub fn run<D: DataSource>(host: &mut Host<D>) -> std::io::Result<()> {
    let stdin = rustix::stdio::stdin();
    let saved = tcgetattr(stdin)?;
    let mut raw = saved.clone();
    raw.make_raw();
    // Keep output post-processing: "\n" still returns the carriage, so a
    // panic message is readable.
    *SAVED.lock().expect("saved") = Some((saved, host.mode));
    tcsetattr(stdin, OptionalActions::Now, &raw)?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));
    let result = run_raw(host);
    restore();
    result
}

fn run_raw<D: DataSource>(host: &mut Host<D>) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    let mode = host.mode;
    stdout.write_all(
        if mode == Mode::Fullscreen {
            ENTER_FULL
        } else {
            ENTER_INLINE
        }
        .as_bytes(),
    )?;
    stdout.flush()?;
    // The data module's threads wake the loop through a socket.
    let (mut wake_rx, wake_tx) = UnixStream::pair()?;
    wake_rx.set_nonblocking(true)?;
    wake_tx.set_nonblocking(true)?;
    let wake_tx = Arc::new(Mutex::new(wake_tx));
    host.listen(Arc::new(move || {
        if let Ok(mut w) = wake_tx.lock() {
            let _ = w.write(&[1]);
        }
    }));
    host.announced();
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64() * 1000.0;
    let mut parser = Parser::new();
    let mut decoder = Decoder::default();
    let mut shown: Option<crate::grid::Grid> = None;
    let mut inline = Inline::new(Protocol::detect());
    let mut buf = [0u8; 8192];
    let mut published = String::new();
    host.publish(&record(mode, inline.protocol, ""));
    loop {
        if let Some((cols, rows)) = size() {
            host.resize(cols, rows);
        }
        host.tick(now());
        let mut out = String::new();
        match mode {
            Mode::Fullscreen => {
                let grid = &host.frame().grid;
                if shown.as_ref() != Some(grid) {
                    out = grid.diff(shown.as_ref());
                    shown = Some(grid.clone());
                }
            }
            Mode::Inline => inline.frame(host, &mut out),
        }
        if host.quitting() {
            if mode == Mode::Inline {
                inline.finish(&mut out);
            }
            stdout.write_all(out.as_bytes())?;
            stdout.flush()?;
            return Ok(());
        }
        if !out.is_empty() {
            stdout.write_all(out.as_bytes())?;
            stdout.flush()?;
            host.frames += 1;
        }
        // Tell the app what is in scrollback now that the bytes are out,
        // so it can let go (LLP 1101.001 P1; 1101.002 §0 P5).
        if mode == Mode::Inline && inline.through != published {
            published = inline.through.clone();
            host.publish(&record(mode, inline.protocol, &published));
        }
        // Sleep until input, a wake, the next timer, or a size check.
        let wait = host
            .timer_due_ms()
            .map(|due| (due - now()).clamp(0.0, 250.0))
            .unwrap_or(250.0);
        let wait = if decoder.escape {
            wait.min(ESC_WAIT_MS)
        } else {
            wait
        };
        let timeout = Timespec {
            tv_sec: 0,
            tv_nsec: (wait * 1_000_000.0) as i64,
        };
        let stdin = rustix::stdio::stdin();
        let mut fds = [
            PollFd::new(&stdin, PollFlags::IN),
            PollFd::new(&wake_rx, PollFlags::IN),
        ];
        if poll(&mut fds, Some(&timeout))? == 0 {
            for input in flush_escape(&mut decoder) {
                if let Input::Key(k) = input {
                    if host.key(k) == After::Quit {
                        return Ok(());
                    }
                }
            }
            continue;
        }
        let (input_ready, woken) = (!fds[0].revents().is_empty(), !fds[1].revents().is_empty());
        if woken {
            let mut sink = [0u8; 256];
            while matches!(wake_rx.read(&mut sink), Ok(n) if n > 0) {}
            host.announced();
        }
        if !input_ready {
            continue;
        }
        let n = rustix::io::read(stdin, &mut buf)?;
        if n == 0 {
            return Ok(());
        }
        // These keys were typed against what the terminal showed by now.
        host.read_at = host.frames;
        for input in decode(&mut parser, &mut decoder, &buf[..n]) {
            match input {
                Input::Key(k) => {
                    if host.key(k) == After::Quit {
                        let mut out = String::new();
                        if mode == Mode::Inline {
                            inline.frame(host, &mut out);
                            inline.finish(&mut out);
                        }
                        stdout.write_all(out.as_bytes())?;
                        return Ok(());
                    }
                }
                Input::Click(x, y) => host.click(x, y),
                Input::CursorAt(row, _) => inline.answered(host, row),
                Input::Wheel(x, y, rows) => host.wheel(x, y, rows),
                Input::Paste(text) => host.paste(&text),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(bytes: &[u8]) -> Vec<Input> {
        decode(&mut Parser::new(), &mut Decoder::default(), bytes)
    }

    #[test]
    fn keys_mouse_and_paste_decode() {
        assert_eq!(
            keys(b"a\r"),
            vec![Input::Key(Key::Char('a')), Input::Key(Key::Named("Enter"))]
        );
        assert_eq!(
            keys(b"\x1b[A\x7f"),
            vec![
                Input::Key(Key::Named("ArrowUp")),
                Input::Key(Key::Named("Backspace"))
            ]
        );
        assert_eq!(keys(b"\x1b[<0;5;3M"), vec![Input::Click(4, 2)]);
        assert_eq!(keys(b"\x1b[<65;1;1M"), vec![Input::Wheel(0, 0, 3)]);
        assert_eq!(keys(b"\x1b[99;5u"), vec![Input::Key(Key::Ctrl('c'))]);
        assert_eq!(
            keys(b"\x1b[13;2u"),
            vec![Input::Key(Key::Chord("Shift+Enter".into()))]
        );
        assert_eq!(
            keys(b"\x1b[1;5A"),
            vec![Input::Key(Key::Chord("Control+ArrowUp".into()))]
        );
        let (mut p, mut d) = (Parser::new(), Decoder::default());
        assert_eq!(decode(&mut p, &mut d, b"\x1b"), vec![]);
        assert_eq!(flush_escape(&mut d), vec![Input::Key(Key::Named("Escape"))]);
        // An arrow split across reads stays an arrow.
        assert_eq!(decode(&mut p, &mut d, b"\x1b"), vec![]);
        assert_eq!(
            decode(&mut p, &mut d, b"[A"),
            vec![Input::Key(Key::Named("ArrowUp"))]
        );
        assert_eq!(
            keys(b"\x1b[200~x\ny\x1b[201~"),
            vec![Input::Paste("x\ny".into())]
        );
    }

    #[test]
    fn a_paste_split_across_reads_stays_a_paste() {
        let (mut p, mut d) = (Parser::new(), Decoder::default());
        assert_eq!(decode(&mut p, &mut d, b"\x1b[200~line one\r"), vec![]);
        assert_eq!(
            decode(&mut p, &mut d, b"q\x7fline two\x1b[201~"),
            vec![Input::Paste("line one\nqline two".into())]
        );
    }
}
