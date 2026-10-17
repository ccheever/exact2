//! Unix raw mode, terminal signals and the interactive event loop.

use super::{decode, flush_escape, record, Decoder, Inline, Input, ESC_WAIT_MS};
use crate::host::{After, Host, Mode};
use crate::image::Protocol;
use exact_runner::DataSource;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::termios::{tcgetattr, tcgetwinsize, tcsetattr, OptionalActions, Termios};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vte::Parser;

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
                "term::unix::restoration_tests::restore_resets_stdout_flags_for_abort_hook",
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
}
