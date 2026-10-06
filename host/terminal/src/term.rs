//! The terminal itself: raw mode, the bytes in, the bytes out, and always
//! putting it back.
//!
//! @ref LLP 1101 D6 (input), D7 (restoration), D9 (vte parses input; the
//! output is constructed here; rustix for termios and the window size)

use crate::host::{After, Host, Key};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::termios::{tcgetattr, tcgetwinsize, tcsetattr, OptionalActions, Termios};
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;
use vte::{Params, Parser, Perform};

/// The terminal's settings before raw mode, for every way out.
static SAVED: Mutex<Option<Termios>> = Mutex::new(None);

/// Alternate screen, SGR mouse with drags, bracketed paste, kitty keyboard
/// disambiguation (LLP 1101 D6).
const ENTER: &str = "\x1b[?1049h\x1b[?25l\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?2004h\x1b[>1u";
/// Each mode undone, in reverse.
const LEAVE: &str =
    "\x1b[<u\x1b[?2004l\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[0m\x1b[0 q\x1b[?25h\x1b[?1049l";

fn restore() {
    let mut out = std::io::stdout();
    let _ = out.write_all(LEAVE.as_bytes());
    let _ = out.flush();
    if let Some(saved) = SAVED.lock().ok().and_then(|mut s| s.take()) {
        let _ = tcsetattr(rustix::stdio::stdin(), OptionalActions::Now, &saved);
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
enum Input {
    Key(Key),
    Click(i32, i32),
    Wheel(i32, i32, i32),
}

#[derive(Default)]
struct Decoder {
    out: Vec<Input>,
    pasting: bool,
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

impl Perform for Decoder {
    fn print(&mut self, c: char) {
        self.key(Key::Char(c));
    }

    fn execute(&mut self, byte: u8) {
        if self.pasting {
            // A pasted line break is text, never a submit.
            if byte == b'\n' || byte == b'\r' {
                self.key(Key::Char(' '));
            }
            return;
        }
        if let Some(k) = ctrl_of(byte) {
            self.key(k);
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let p: Vec<u16> = params
            .iter()
            .map(|p| p.first().copied().unwrap_or(0))
            .collect();
        let first = p.first().copied().unwrap_or(0);
        let mods = p.get(1).copied().unwrap_or(1).saturating_sub(1);
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
            (_, 'A') => self.key(Key::Named("ArrowUp")),
            (_, 'B') => self.key(Key::Named("ArrowDown")),
            (_, 'C') => self.key(Key::Named("ArrowRight")),
            (_, 'D') => self.key(Key::Named("ArrowLeft")),
            (_, 'H') => self.key(Key::Named("Home")),
            (_, 'F') => self.key(Key::Named("End")),
            (_, 'Z') => self.key(Key::BackTab),
            (_, '~') => match first {
                1 | 7 => self.key(Key::Named("Home")),
                4 | 8 => self.key(Key::Named("End")),
                3 => self.key(Key::Named("Delete")),
                5 => self.key(Key::Named("PageUp")),
                6 => self.key(Key::Named("PageDown")),
                200 => self.pasting = true,
                201 => self.pasting = false,
                _ => {}
            },
            // The kitty keyboard protocol: a code point and its modifiers.
            (_, 'u') => {
                let ctrl = mods & 4 != 0;
                match first {
                    27 => self.key(Key::Named("Escape")),
                    13 => self.key(Key::Named("Enter")),
                    9 if mods & 1 != 0 => self.key(Key::BackTab),
                    9 => self.key(Key::Named("Tab")),
                    127 => self.key(Key::Named("Backspace")),
                    c => match char::from_u32(c as u32) {
                        Some(c) if ctrl => self.key(Key::Ctrl(c.to_ascii_lowercase())),
                        Some(c) => self.key(Key::Char(c)),
                        None => {}
                    },
                }
            }
            _ => {}
        }
    }
}

/// Decode a read: DEL and a lone ESC by hand (a terminal parser ignores the
/// one and waits on the other), everything else through vte.
fn decode(parser: &mut Parser, bytes: &[u8]) -> Vec<Input> {
    let mut d = Decoder::default();
    let mut start = 0;
    for (i, b) in bytes.iter().enumerate() {
        if *b == 0x7f {
            parser.advance(&mut d, &bytes[start..i]);
            d.key(Key::Named("Backspace"));
            start = i + 1;
        }
    }
    let rest = &bytes[start..];
    if rest == [0x1b] {
        d.key(Key::Named("Escape"));
    } else {
        parser.advance(&mut d, rest);
    }
    d.out
}

/// Run the host in this terminal until the app or the user quits.
pub fn run(host: &mut Host) -> std::io::Result<()> {
    let stdin = rustix::stdio::stdin();
    let saved = tcgetattr(stdin)?;
    let mut raw = saved.clone();
    raw.make_raw();
    *SAVED.lock().expect("saved") = Some(saved);
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

fn run_raw(host: &mut Host) -> std::io::Result<()> {
    let mut out = std::io::stdout();
    out.write_all(ENTER.as_bytes())?;
    let start = Instant::now();
    let mut parser = Parser::new();
    let mut shown: Option<crate::grid::Grid> = None;
    let mut buf = [0u8; 4096];
    loop {
        if let Some((cols, rows)) = size() {
            host.resize(cols, rows);
        }
        host.tick(start.elapsed().as_secs_f64() * 1000.0);
        let grid = &host.frame().grid;
        if shown.as_ref() != Some(grid) {
            out.write_all(grid.diff(shown.as_ref()).as_bytes())?;
            out.flush()?;
            shown = Some(grid.clone());
        }
        let stdin = rustix::stdio::stdin();
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        let timeout = Timespec {
            tv_sec: 0,
            tv_nsec: 50_000_000,
        };
        if poll(&mut fds, Some(&timeout))? == 0 {
            continue;
        }
        let n = rustix::io::read(stdin, &mut buf)?;
        if n == 0 {
            return Ok(());
        }
        for input in decode(&mut parser, &buf[..n]) {
            match input {
                Input::Key(k) => {
                    if host.key(k) == After::Quit {
                        return Ok(());
                    }
                }
                Input::Click(x, y) => host.click(x, y),
                Input::Wheel(x, y, rows) => host.wheel(x, y, rows),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_mouse_and_paste_decode() {
        let mut p = Parser::new();
        assert_eq!(
            decode(&mut p, b"a\r"),
            vec![Input::Key(Key::Char('a')), Input::Key(Key::Named("Enter"))]
        );
        assert_eq!(
            decode(&mut p, b"\x1b[A\x7f"),
            vec![
                Input::Key(Key::Named("ArrowUp")),
                Input::Key(Key::Named("Backspace"))
            ]
        );
        assert_eq!(decode(&mut p, b"\x1b[<0;5;3M"), vec![Input::Click(4, 2)]);
        assert_eq!(
            decode(&mut p, b"\x1b[<65;1;1M"),
            vec![Input::Wheel(0, 0, 3)]
        );
        assert_eq!(
            decode(&mut p, b"\x1b[99;5u"),
            vec![Input::Key(Key::Ctrl('c'))]
        );
        assert_eq!(
            decode(&mut p, b"\x1b"),
            vec![Input::Key(Key::Named("Escape"))]
        );
        assert_eq!(
            decode(&mut p, b"\x1b[200~x\ny\x1b[201~"),
            vec![
                Input::Key(Key::Char('x')),
                Input::Key(Key::Char(' ')),
                Input::Key(Key::Char('y'))
            ]
        );
    }
}
