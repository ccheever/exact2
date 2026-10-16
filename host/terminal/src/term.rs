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
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vte::{Params, Parser, Perform};

/// The terminal's settings before raw mode, for every way out.
static SAVED: Mutex<Option<(Termios, Mode)>> = Mutex::new(None);
static SIGNAL: AtomicI32 = AtomicI32::new(0);
static SIGNAL_FD: AtomicI32 = AtomicI32::new(-1);
static OUTPUT_FLAGS: AtomicI32 = AtomicI32::new(-1);

struct Signals {
    reader: UnixStream,
    _writer: UnixStream,
    previous: Vec<(i32, libc::sigaction)>,
}

#[allow(unsafe_code)]
extern "C" fn received(signal: libc::c_int) {
    if SIGNAL
        .compare_exchange(0, signal, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let fd = SIGNAL_FD.load(Ordering::SeqCst);
        if fd >= 0 {
            let byte = signal as u8;
            unsafe { libc::write(fd, &byte as *const u8 as *const libc::c_void, 1) };
        }
    }
}

impl Signals {
    #[allow(unsafe_code)]
    fn install() -> std::io::Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        writer.set_nonblocking(true)?;
        let mut guard = Self {
            reader,
            _writer: writer,
            previous: Vec::new(),
        };
        SIGNAL.store(0, Ordering::SeqCst);
        SIGNAL_FD.store(guard._writer.as_raw_fd(), Ordering::SeqCst);
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = received as *const () as libc::sighandler_t;
            unsafe { libc::sigemptyset(&mut action.sa_mask) };
            let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
            guard.previous.push((signal, previous));
        }
        Ok(guard)
    }
}

#[allow(unsafe_code)]
impl Drop for Signals {
    fn drop(&mut self) {
        SIGNAL_FD.store(-1, Ordering::SeqCst);
        for (signal, previous) in self.previous.drain(..).rev() {
            unsafe { libc::sigaction(signal, &previous, std::ptr::null_mut()) };
        }
    }
}

struct OutputFlags(i32);

impl OutputFlags {
    #[allow(unsafe_code)]
    fn nonblocking() -> std::io::Result<Self> {
        let fd = std::io::stdout().as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        OUTPUT_FLAGS.store(flags, Ordering::SeqCst);
        Ok(Self(flags))
    }
}

#[allow(unsafe_code)]
impl Drop for OutputFlags {
    fn drop(&mut self) {
        unsafe { libc::fcntl(std::io::stdout().as_raw_fd(), libc::F_SETFL, self.0) };
        OUTPUT_FLAGS.store(-1, Ordering::SeqCst);
    }
}

fn write_output(mut bytes: &[u8], signal_rx: &UnixStream) -> std::io::Result<()> {
    let stdout = rustix::stdio::stdout();
    while !bytes.is_empty() {
        if SIGNAL.load(Ordering::SeqCst) != 0 {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        match rustix::io::write(stdout, bytes) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(rustix::io::Errno::INTR) => continue,
            Err(rustix::io::Errno::AGAIN) => {
                let mut fds = [
                    PollFd::new(&stdout, PollFlags::OUT),
                    PollFd::new(signal_rx, PollFlags::IN),
                ];
                match poll(&mut fds, None) {
                    Ok(_) | Err(rustix::io::Errno::INTR) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[allow(unsafe_code)]
fn terminate_from_signal(signal: i32) -> ! {
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
        libc::_exit(128 + signal);
    }
}

/// A resize wakes the same socket as data arrivals. Its signal action owns
/// a separate descriptor and does only an async-signal-safe, nonblocking write.
struct ResizeWake(signal_hook::SigId);

impl ResizeWake {
    fn new(wake: UnixStream) -> std::io::Result<Self> {
        signal_hook::low_level::pipe::register(signal_hook::consts::SIGWINCH, wake).map(Self)
    }
}

impl Drop for ResizeWake {
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.0);
    }
}

/// Alternate screen, SGR mouse with drags, bracketed paste, kitty keyboard
/// disambiguation (LLP 1101 D6).
const ENTER_FULL: &str = "\x1b[?1049h\x1b[?25l\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?2004h\x1b[>1u";
/// The normal screen: no mouse capture, so selection stays the terminal's.
const ENTER_INLINE: &str = "\x1b[?25l\x1b[?2004h\x1b[>1u";
/// Each mode undone, in reverse.
const LEAVE_FULL: &str =
    "\x1b]8;;\x1b\\\x1b[?2026l\x1b[<u\x1b[?2004l\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[0m\x1b[0 q\x1b[?25h\x1b[?1049l";
const LEAVE_INLINE: &str =
    "\x1b]8;;\x1b\\\x1b[?2026l\x1b[<u\x1b[?2004l\x1b[?1006l\x1b[?1000l\x1b[0m\x1b[0 q\x1b[?25h";

#[allow(unsafe_code)]
fn restore_output_flags() {
    let flags = OUTPUT_FLAGS.swap(-1, Ordering::SeqCst);
    if flags >= 0 {
        unsafe { libc::fcntl(std::io::stdout().as_raw_fd(), libc::F_SETFL, flags) };
    }
}

fn leave_output(mut bytes: &[u8]) -> std::io::Result<()> {
    let stdout = rustix::stdio::stdout();
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    while !bytes.is_empty() {
        match rustix::io::write(stdout, bytes) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(rustix::io::Errno::INTR) => continue,
            Err(rustix::io::Errno::AGAIN) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(std::io::ErrorKind::TimedOut.into());
                }
                let timeout = Timespec {
                    tv_sec: remaining.as_secs() as i64,
                    tv_nsec: remaining.subsec_nanos() as i64,
                };
                let mut fds = [PollFd::new(&stdout, PollFlags::OUT)];
                match poll(&mut fds, Some(&timeout)) {
                    Ok(_) | Err(rustix::io::Errno::INTR) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[allow(unsafe_code)]
fn write_leave_blocking(bytes: &[u8]) -> std::io::Result<()> {
    let fd = std::io::stdout().as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Flush before termios and the signal handlers go back: the leave bytes
    // hold no newline, so stdout's line buffer would otherwise keep them until
    // process exit, where a signal can drop them.
    let mut out = std::io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

fn restore(signal: bool) -> std::io::Result<()> {
    let saved = SAVED.lock().ok().and_then(|mut s| s.take());
    let result = if let Some((termios, mode)) = saved {
        let leave = if mode == Mode::Fullscreen {
            LEAVE_FULL
        } else {
            LEAVE_INLINE
        };
        if signal {
            // A stopped PTY cannot accept the leave bytes: recover the shell's
            // input first, then give output a bounded chance to drain.
            let _ = tcsetattr(rustix::stdio::stdin(), OptionalActions::Now, &termios);
            leave_output(leave.as_bytes())
        } else {
            let result = write_leave_blocking(leave.as_bytes());
            let _ = tcsetattr(rustix::stdio::stdin(), OptionalActions::Now, &termios);
            result
        }
    } else {
        Ok(())
    };
    restore_output_flags();
    result
}

#[cfg(test)]
mod restoration_tests {
    use super::*;

    #[test]
    fn leave_sequences_close_frame_modes() {
        for leave in [LEAVE_INLINE, LEAVE_FULL] {
            assert!(leave.contains("\x1b]8;;\x1b\\"));
            assert!(leave.contains("\x1b[?2026l"));
        }
    }

    #[allow(unsafe_code)]
    #[test]
    fn restore_resets_stdout_flags_for_abort_hook() {
        if std::env::var_os("EXACT_TERMINAL_FLAG_CHILD").is_some() {
            let fd = std::io::stdout().as_raw_fd();
            let before = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            assert!(before >= 0);
            let flags = OutputFlags::nonblocking().unwrap();
            assert_ne!(
                unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_NONBLOCK,
                0
            );
            restore(true).unwrap();
            assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFL) }, before);
            std::mem::forget(flags);
            return;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "term::restoration_tests::restore_resets_stdout_flags_for_abort_hook",
                "--nocapture",
            ])
            .env("EXACT_TERMINAL_FLAG_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success(), "{status}");
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
    /// Which log `through` belongs to: one more for each new log, so an
    /// app can tell a new transcript's `"1"` from the old one's.
    logs: u64,
    /// The (log, through) last published to the app.
    published: (u64, String),
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
            logs: 0,
            published: (0, String::new()),
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

    /// The record to publish once this frame's bytes are out, when what is
    /// printed changed since the last one (LLP 1101.002 §0 P5).
    pub(crate) fn printed(
        &mut self,
        mode: Mode,
        colors: crate::color::ColorDepth,
    ) -> Option<String> {
        let now = (self.logs, self.through.clone());
        if now == self.published {
            return None;
        }
        self.published = now;
        Some(record(
            mode,
            self.protocol,
            colors,
            self.logs,
            &self.through,
        ))
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
            self.through.clear();
            self.logs += 1;
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
        // Back to the live region's top-left. After a resize this is still
        // `cursor_row` rows up: every region row ends in a hard line break,
        // which a reflowing terminal never rejoins, so a wider window keeps
        // the rows; a narrower one may wrap them into more, and moving up
        // too few leaves a stale fragment above rather than erasing printed
        // rows (the safer error; LLP 1101.002 code review).
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
            // Scrollback can arrive in a single large burst. Rasterize it
            // in bounded strips instead of allocating a cell for every
            // column of the entire transcript before writing any of it.
            for top in (from..to).step_by(128) {
                let painted = host.render_settled(top, (to - top).min(128));
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
        // Where the region is on the screen: asked on every frame while
        // clicks matter, so the answer after a resize re-anchors them, and
        // never otherwise.
        if self.mouse {
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
                    // An image may begin in an earlier strip. Its last
                    // row emits the protocol image over the rows already
                    // printed, exactly once.
                    if r.y + r.h - 1 == y as i32 && r.h > 0 {
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

/// The terminal's record for the app, `exactSurface("terminal")`: `log`
/// counts the logs seen (a new one is a new transcript), and `printed` is
/// the `id` of that log's last child written to scrollback.
pub(crate) fn record(
    mode: Mode,
    images: Protocol,
    colors: crate::color::ColorDepth,
    log: u64,
    printed: &str,
) -> String {
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
    let colors = colors as u8;
    format!(
        "{{\"mode\":\"{mode}\",\"images\":\"{images}\",\"colors\":{colors},\"log\":{log},\"printed\":\"{printed}\"}}"
    )
}

/// Hand the URLs the app asked to open (`openURL`, a clicked link) to the
/// system's opener, detached: the terminal stays the app's.
fn open_outbound<D: DataSource>(host: &mut Host<D>) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    for url in host.outbound.drain(..) {
        let _ = std::process::Command::new(opener)
            .arg(&url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// Run the host in this terminal until the app or the user quits.
pub fn run<D: DataSource>(host: &mut Host<D>) -> std::io::Result<()> {
    host.set_colors(host.colors.unwrap_or_else(crate::color::ColorDepth::detect));
    let stdin = rustix::stdio::stdin();
    let saved = tcgetattr(stdin)?;
    let signals = Signals::install()?;
    let output_flags = OutputFlags::nonblocking()?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore(true);
        hook(info);
    }));
    let mut raw = saved.clone();
    raw.make_raw();
    // Keep output post-processing: "\n" still returns the carriage, so a
    // panic message is readable.
    *SAVED.lock().expect("saved") = Some((saved, host.mode));
    tcsetattr(stdin, OptionalActions::Now, &raw)?;
    let result = run_raw(host, &signals.reader);
    let restored = restore(SIGNAL.load(Ordering::SeqCst) != 0);
    drop(output_flags);
    drop(signals);
    let signal = SIGNAL.swap(0, Ordering::SeqCst);
    if signal != 0 {
        terminate_from_signal(signal);
    }
    result.and(restored)
}

fn run_raw<D: DataSource>(host: &mut Host<D>, signal_rx: &UnixStream) -> std::io::Result<()> {
    let mode = host.mode;
    write_output(
        if mode == Mode::Fullscreen {
            ENTER_FULL
        } else {
            ENTER_INLINE
        }
        .as_bytes(),
        signal_rx,
    )?;
    // The data module's threads wake the loop through a socket.
    let (mut wake_rx, wake_tx) = UnixStream::pair()?;
    wake_rx.set_nonblocking(true)?;
    wake_tx.set_nonblocking(true)?;
    let _resize_wake = ResizeWake::new(wake_tx.try_clone()?)?;
    let wake_tx = Arc::new(Mutex::new(wake_tx));
    host.listen(Arc::new(move || {
        if let Ok(mut w) = wake_tx.lock() {
            let _ = w.write(&[1]);
        }
    }));
    host.announced();
    let start = Instant::now();
    host.live_from(start);
    let now = || start.elapsed().as_secs_f64() * 1000.0;
    let mut parser = Parser::new();
    let mut decoder = Decoder::default();
    let mut shown: Option<crate::grid::Grid> = None;
    let mut inline = Inline::new(Protocol::detect());
    let mut buf = [0u8; 8192];
    host.publish(&record(mode, inline.protocol, host.colors(), 0, ""));
    loop {
        if SIGNAL.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        if let Some((cols, rows)) = size() {
            host.resize(cols, rows);
        }
        host.tick(now());
        if SIGNAL.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
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
            write_output(out.as_bytes(), signal_rx)?;
            return Ok(());
        }
        if !out.is_empty() {
            write_output(out.as_bytes(), signal_rx)?;
            host.frames += 1;
        }
        // Tell the app what is in scrollback now that the bytes are out,
        // so it can let go (LLP 1101.001 P1; 1101.002 §0 P5).
        if mode == Mode::Inline {
            if let Some(r) = inline.printed(mode, host.colors()) {
                host.publish(&r);
            }
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
            PollFd::new(signal_rx, PollFlags::IN),
        ];
        let ready = match poll(&mut fds, Some(&timeout)) {
            Ok(ready) => ready,
            Err(rustix::io::Errno::INTR) => continue,
            Err(e) => return Err(e.into()),
        };
        if !fds[2].revents().is_empty() || SIGNAL.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        if ready == 0 {
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
        let n = match rustix::io::read(stdin, &mut buf) {
            Ok(n) => n,
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        };
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
                        write_output(out.as_bytes(), signal_rx)?;
                        return Ok(());
                    }
                }
                Input::Click(x, y) => host.click(x, y),
                Input::CursorAt(row, _) => inline.answered(host, row),
                Input::Wheel(x, y, rows) => host.wheel(x, y, rows),
                Input::Paste(text) => host.paste(&text),
            }
        }
        open_outbound(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_registration_wakes_and_releases_only_its_socket_writer() {
        let (mut rx, mut tx) = UnixStream::pair().unwrap();
        rx.set_nonblocking(true).unwrap();
        let registration = ResizeWake::new(tx.try_clone().unwrap()).unwrap();
        let mut bytes = [0; 8];
        signal_hook::low_level::raise(signal_hook::consts::SIGWINCH).unwrap();
        assert_eq!(rx.read(&mut bytes).unwrap(), 1);
        drop(registration);
        signal_hook::low_level::raise(signal_hook::consts::SIGWINCH).unwrap();
        assert_eq!(
            rx.read(&mut bytes).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        // Unregistering a resize must not close the data module's writer.
        tx.write_all(b"data").unwrap();
        assert_eq!(rx.read(&mut bytes).unwrap(), 4);
        assert_eq!(&bytes[..4], b"data");
    }

    #[test]
    fn protocol_images_cross_paint_strips_without_being_dropped_or_repeated() {
        let path =
            std::env::temp_dir().join(format!("terminal-strip-image-{}.png", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(file, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[20, 40, 60])
            .unwrap();
        let source = format!(
            "component App\n  view\n    column\n      text \"before\"\n      image \"{}\" width=\"2ch\" height=\"260lh\"\n      text \"after\"\n",
            path.display()
        );
        let plan = contract::compile_terminal(&source).unwrap();
        let mut host = Host::boot(plan, (), Mode::Inline, 20, 12).unwrap();
        for protocol in [Protocol::Kitty, Protocol::Iterm] {
            let writer = Inline::new(protocol);
            let mut whole = String::new();
            writer.print_rows(
                &host.render_settled(0, host.document_rows()),
                &host.images,
                &mut whole,
            );
            let mut strips = String::new();
            for top in (0..host.document_rows()).step_by(128) {
                writer.print_rows(
                    &host.render_settled(top, (host.document_rows() - top).min(128)),
                    &host.images,
                    &mut strips,
                );
            }
            assert_eq!(strips, whole);
            let command = match protocol {
                Protocol::Kitty => "a=T",
                _ => "1337;File=",
            };
            assert_eq!(strips.matches(command).count(), 1);
        }
        host.set_colors(crate::color::ColorDepth::Monochrome);
        for protocol in [Protocol::Blocks, Protocol::Kitty, Protocol::Iterm] {
            let writer = Inline::new(protocol);
            let painted = host.render_settled(0, host.document_rows());
            assert!(painted.images.is_empty());
            // The fixture is only two columns wide, so its alternate text
            // is clipped to the same image box.
            assert!(painted.grid.text().contains("[i"));
            let mut out = String::new();
            writer.print_rows(&painted, &host.images, &mut out);
            assert!(!out.contains("a=T") && !out.contains("1337;File="));
            assert!(!out.contains("38;") && !out.contains("48;"));
        }
        std::fs::remove_file(path).unwrap();
    }

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
