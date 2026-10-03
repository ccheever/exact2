//! A VNC server for the display: the frames the app puts on the screen,
//! served over RFB (raw encoding, no authentication), with the client's
//! pointer and keyboard fed back as input — eyes and hands on a box whose
//! KVM is unplugged, from any VNC client (`EXACT_VNC=1`, port 5900).
//!
//! @ref LLP 1015 §6
//!
//! Protocol 3.3, 3.7, or 3.8 as the client speaks it — macOS Screen Sharing
//! answers 3.3 and then insists on VNC authentication, so a 3.3 client gets
//! a challenge that any password answers; 3.7/3.8 clients get `None`. Either
//! way nobody is authenticated. `Raw` encoding only (every client must take
//! it), the client's 32-bit true-colour pixel format honoured. One
//! reader and one writer thread per client; frames reach the writer through
//! a shared slot, input reaches the display loop through a queue and a
//! socketpair it polls. Nothing here runs unless asked for, and nothing
//! encrypts: a development tool on a private network.

use crate::input::{InputEvent, Keyboard, LINE};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use tiny_skia::Pixmap;

/// A client's pixel format: 32-bit true colour with these shifts.
#[derive(Debug, Clone, Copy)]
struct PixelFormat {
    bpp: u8,
    big_endian: bool,
    max: [u16; 3],
    shift: [u8; 3],
}

impl PixelFormat {
    const OURS: PixelFormat = PixelFormat {
        bpp: 32,
        big_endian: false,
        max: [255, 255, 255],
        shift: [16, 8, 0],
    };

    fn encode(&self, out: &mut Vec<u8>) {
        out.push(self.bpp);
        out.push(24);
        out.push(u8::from(self.big_endian));
        out.push(1);
        for m in self.max {
            out.extend_from_slice(&m.to_be_bytes());
        }
        out.extend_from_slice(&self.shift);
        out.extend_from_slice(&[0, 0, 0]);
    }

    fn parse(b: &[u8; 16]) -> Option<PixelFormat> {
        let true_colour = b[3] != 0;
        if !true_colour || b[0] != 32 {
            return None;
        }
        let shift = [b[10], b[11], b[12]];
        // A channel shifted past the pixel is a client this server does not
        // speak, not a shift to overflow on.
        if shift.iter().any(|s| *s > 24) {
            return None;
        }
        Some(PixelFormat {
            bpp: 32,
            big_endian: b[2] != 0,
            max: [
                u16::from_be_bytes([b[4], b[5]]),
                u16::from_be_bytes([b[6], b[7]]),
                u16::from_be_bytes([b[8], b[9]]),
            ],
            shift,
        })
    }

    fn pixel(&self, r: u8, g: u8, b: u8) -> [u8; 4] {
        let scale = |c: u8, max: u16| c as u64 * max as u64 / 255;
        let v = ((scale(r, self.max[0]) << self.shift[0])
            | (scale(g, self.max[1]) << self.shift[1])
            | (scale(b, self.max[2]) << self.shift[2])) as u32;
        if self.big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    }
}

struct Shared {
    frame: Mutex<(Option<Arc<Pixmap>>, u64)>,
    changed: Condvar,
    events: Mutex<Vec<InputEvent>>,
    wake: Mutex<UnixStream>,
    width: u32,
    height: u32,
}

/// The server: frames in, input out.
pub struct Vnc {
    shared: Arc<Shared>,
    wake_rx: UnixStream,
    published: AtomicU64,
}

impl Vnc {
    /// Listen on `addr` (`"1"` is `0.0.0.0:5900`) for a screen of this
    /// size in pixels.
    pub fn start(addr: &str, width: u32, height: u32) -> io::Result<Vnc> {
        let addr = if addr == "1" { "0.0.0.0:5900" } else { addr };
        let listener = TcpListener::bind(addr)?;
        let (wake_tx, wake_rx) = UnixStream::pair()?;
        wake_rx.set_nonblocking(true)?;
        let shared = Arc::new(Shared {
            frame: Mutex::new((None, 0)),
            changed: Condvar::new(),
            events: Mutex::new(Vec::new()),
            wake: Mutex::new(wake_tx),
            width,
            height,
        });
        let accept = shared.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let shared = accept.clone();
                thread::spawn(move || {
                    if let Err(e) = serve(stream, shared) {
                        if e.kind() != io::ErrorKind::UnexpectedEof {
                            eprintln!("vnc: client: {e}");
                        }
                    }
                });
            }
        });
        Ok(Vnc {
            shared,
            wake_rx,
            published: AtomicU64::new(0),
        })
    }

    /// The descriptor to poll: readable when input arrived.
    pub fn fd(&self) -> RawFd {
        self.wake_rx.as_raw_fd()
    }

    /// A new frame for every client.
    pub fn publish(&self, frame: Arc<Pixmap>) {
        let seq = self.published.fetch_add(1, Ordering::Relaxed) + 1;
        let mut slot = self.shared.frame.lock().unwrap_or_else(|e| e.into_inner());
        *slot = (Some(frame), seq);
        self.shared.changed.notify_all();
    }

    /// Everything the clients did since the last call.
    pub fn take_events(&mut self) -> Vec<InputEvent> {
        let mut drain = [0u8; 64];
        while let Ok(n) = self.wake_rx.read(&mut drain) {
            if n == 0 {
                break;
            }
        }
        std::mem::take(&mut *self.shared.events.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Read and discard `n` bytes: a message this server ignores, however long
/// the client says it is — never an allocation of the client's size.
fn drain(s: &mut TcpStream, mut n: u64) -> io::Result<()> {
    let mut buf = [0u8; 4096];
    while n > 0 {
        let take = n.min(buf.len() as u64) as usize;
        s.read_exact(&mut buf[..take])?;
        n -= take as u64;
    }
    Ok(())
}

fn read_exact<const N: usize>(s: &mut TcpStream) -> io::Result<[u8; N]> {
    let mut b = [0u8; N];
    s.read_exact(&mut b)?;
    Ok(b)
}

fn serve(mut stream: TcpStream, shared: Arc<Shared>) -> io::Result<()> {
    stream.set_nodelay(true)?;
    // Handshake: version, security (none), init. The security exchange
    // depends on the version the client answers with: 3.3 takes one u32
    // type from the server and nothing else (macOS Screen Sharing speaks
    // 3.3); 3.7 a list and a choice; 3.8 the list, the choice, a result.
    stream.write_all(b"RFB 003.008\n")?;
    let version = read_exact::<12>(&mut stream)?;
    let minor = std::str::from_utf8(&version[8..11])
        .ok()
        .and_then(|m| m.parse::<u32>().ok())
        .unwrap_or(8);
    match minor {
        0..=3 => {
            // Screen Sharing will not proceed past `None` in 3.3; it wants
            // VNC authentication. A challenge it answers with any password:
            // this server authenticates nobody, and says so.
            stream.write_all(&2u32.to_be_bytes())?;
            let challenge: [u8; 16] = std::array::from_fn(|i| {
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                (t >> (i * 5)) as u8 ^ (i as u8).wrapping_mul(37)
            });
            stream.write_all(&challenge)?;
            let _response = read_exact::<16>(&mut stream)?;
            stream.write_all(&0u32.to_be_bytes())?;
        }
        4..=7 => {
            stream.write_all(&[1, 1])?;
            let _chosen = read_exact::<1>(&mut stream)?;
        }
        _ => {
            stream.write_all(&[1, 1])?;
            let _chosen = read_exact::<1>(&mut stream)?;
            stream.write_all(&0u32.to_be_bytes())?;
        }
    }
    let _shared_flag = read_exact::<1>(&mut stream)?;
    let mut init = Vec::new();
    init.extend_from_slice(&(shared.width as u16).to_be_bytes());
    init.extend_from_slice(&(shared.height as u16).to_be_bytes());
    PixelFormat::OURS.encode(&mut init);
    let name = b"Exact";
    init.extend_from_slice(&(name.len() as u32).to_be_bytes());
    init.extend_from_slice(name);
    stream.write_all(&init)?;

    // The writer: sends a frame whenever one is requested and a new one
    // exists (an incremental request), or at once (a full one).
    let format = Arc::new(Mutex::new(PixelFormat::OURS));
    let wanted = Arc::new((Mutex::new((false, false)), Condvar::new())); // (requested, full)
                                                                         // The client is gone: set under each lock the writer waits on, so a
                                                                         // wake-up is never lost between its check and its wait.
    let closed = Arc::new(AtomicBool::new(false));
    let writer = {
        let mut out = stream.try_clone()?;
        let shared = shared.clone();
        let format = format.clone();
        let wanted = wanted.clone();
        let closed = closed.clone();
        thread::spawn(move || -> io::Result<()> {
            let mut sent = 0u64;
            loop {
                let full = {
                    let mut w = wanted.0.lock().unwrap_or_else(|e| e.into_inner());
                    while !w.0 && !closed.load(Ordering::Relaxed) {
                        w = wanted.1.wait(w).unwrap_or_else(|e| e.into_inner());
                    }
                    if closed.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    w.0 = false;
                    std::mem::replace(&mut w.1, false)
                };
                let frame = {
                    let mut slot = shared.frame.lock().unwrap_or_else(|e| e.into_inner());
                    while slot.0.is_none() || (!full && slot.1 <= sent) {
                        if closed.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        slot = shared.changed.wait(slot).unwrap_or_else(|e| e.into_inner());
                    }
                    sent = slot.1;
                    slot.0.clone().expect("checked")
                };
                let f = *format.lock().unwrap_or_else(|e| e.into_inner());
                send_frame(&mut out, &frame, f, shared.width, shared.height)?;
            }
        })
    };

    let mut keyboard = Keyboard::default();
    let mut held = std::collections::BTreeSet::new();
    // The reader: the client's messages.
    let mut buttons = 0u8;
    let result = (|| -> io::Result<()> {
        loop {
            let kind = read_exact::<1>(&mut stream)?[0];
            match kind {
                0 => {
                    let _pad = read_exact::<3>(&mut stream)?;
                    let pf = read_exact::<16>(&mut stream)?;
                    if let Some(f) = PixelFormat::parse(&pf) {
                        *format.lock().unwrap_or_else(|e| e.into_inner()) = f;
                    } else {
                        eprintln!("vnc: a pixel format this server does not speak; sending 32-bit");
                    }
                }
                1 => {
                    // FixColourMapEntries: nothing here is colour-mapped.
                    let _pad = read_exact::<1>(&mut stream)?;
                    let _first = read_exact::<2>(&mut stream)?;
                    let n = u16::from_be_bytes(read_exact::<2>(&mut stream)?);
                    drain(&mut stream, n as u64 * 6)?;
                }
                2 => {
                    // SetEncodings: `Raw` is the one encoding, whatever is asked.
                    let _pad = read_exact::<1>(&mut stream)?;
                    let n = u16::from_be_bytes(read_exact::<2>(&mut stream)?);
                    drain(&mut stream, n as u64 * 4)?;
                }
                3 => {
                    let b = read_exact::<9>(&mut stream)?;
                    let incremental = b[0] != 0;
                    let mut w = wanted.0.lock().unwrap_or_else(|e| e.into_inner());
                    w.0 = true;
                    w.1 |= !incremental;
                    wanted.1.notify_one();
                }
                4 => {
                    let b = read_exact::<7>(&mut stream)?;
                    let down = b[0] != 0;
                    let key = u32::from_be_bytes([b[3], b[4], b[5], b[6]]);
                    if let Some((code, shift)) = keysym(key) {
                        let value = if down {
                            if held.insert(code) {
                                1
                            } else {
                                2
                            }
                        } else {
                            held.remove(&code);
                            0
                        };
                        if let Some(event) = keyboard.event(code, value, shift) {
                            push(&shared, event);
                        }
                    }
                }
                5 => {
                    let b = read_exact::<5>(&mut stream)?;
                    let mask = b[0];
                    let x = u16::from_be_bytes([b[1], b[2]]) as f32;
                    let y = u16::from_be_bytes([b[3], b[4]]) as f32;
                    push(
                        &shared,
                        InputEvent::Absolute(
                            Some(x / shared.width.max(1) as f32),
                            Some(y / shared.height.max(1) as f32),
                        ),
                    );
                    let pressed = mask & !buttons;
                    let released = buttons & !mask;
                    if pressed & 1 != 0 {
                        push(&shared, InputEvent::Button(true));
                    }
                    if released & 1 != 0 {
                        push(&shared, InputEvent::Button(false));
                    }
                    // RFB's middle (2) and right (4) are the web's 4 and 2.
                    for (rfb, bit) in [(2, 4), (4, 2)] {
                        if (pressed | released) & rfb != 0 {
                            push(&shared, InputEvent::Aux(bit, pressed & rfb != 0));
                        }
                    }
                    if pressed & 8 != 0 {
                        push(&shared, InputEvent::Wheel(0.0, -LINE));
                    }
                    if pressed & 16 != 0 {
                        push(&shared, InputEvent::Wheel(0.0, LINE));
                    }
                    buttons = mask;
                }
                6 => {
                    // ClientCutText: a length from the wire, drained, never
                    // allocated.
                    let _pad = read_exact::<3>(&mut stream)?;
                    let n = u32::from_be_bytes(read_exact::<4>(&mut stream)?);
                    drain(&mut stream, n as u64)?;
                }
                other => {
                    return Err(io::Error::other(format!("unknown client message {other}")));
                }
            }
        }
    })();
    if buttons & 1 != 0 {
        push(&shared, InputEvent::Cancel);
    }
    for code in held {
        if let Some(event) = keyboard.event(code, 0, None) {
            push(&shared, event);
        }
    }
    // Tell the writer, wherever it waits, and wait for it: no thread and no
    // socket outlive the client.
    {
        let _w = wanted.0.lock().unwrap_or_else(|e| e.into_inner());
        let _f = shared.frame.lock().unwrap_or_else(|e| e.into_inner());
        closed.store(true, Ordering::Relaxed);
        wanted.1.notify_all();
        shared.changed.notify_all();
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = writer.join();
    result
}

fn push(shared: &Shared, event: InputEvent) {
    shared
        .events
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(event);
    let _ = shared
        .wake
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .write_all(&[1]);
}

/// One `FramebufferUpdate` with one raw rectangle: the whole screen.
fn send_frame(
    out: &mut TcpStream,
    frame: &Pixmap,
    format: PixelFormat,
    width: u32,
    height: u32,
) -> io::Result<()> {
    let (w, h) = (width as usize, height as usize);
    let mut msg = Vec::with_capacity(16 + w * h * 4);
    msg.extend_from_slice(&[0, 0]);
    msg.extend_from_slice(&1u16.to_be_bytes());
    msg.extend_from_slice(&0u16.to_be_bytes());
    msg.extend_from_slice(&0u16.to_be_bytes());
    msg.extend_from_slice(&(w as u16).to_be_bytes());
    msg.extend_from_slice(&(h as u16).to_be_bytes());
    msg.extend_from_slice(&0i32.to_be_bytes());
    let fw = frame.width() as usize;
    let fh = frame.height() as usize;
    let data = frame.data();
    for y in 0..h {
        for x in 0..w {
            if x < fw && y < fh {
                let i = (y * fw + x) * 4;
                msg.extend_from_slice(&format.pixel(data[i], data[i + 1], data[i + 2]));
            } else {
                msg.extend_from_slice(&format.pixel(0, 0, 0));
            }
        }
    }
    out.write_all(&msg)
}

/// VNC has logical keysyms; use the same US map as the physical keyboard.
fn keysym(sym: u32) -> Option<(u16, Option<bool>)> {
    if (0x20..=0x7e).contains(&sym) {
        for code in 1..=57 {
            for shift in [false, true] {
                if crate::input::key(code, shift)
                    .is_some_and(|(_, key)| key.len() == 1 && u32::from(key.as_bytes()[0]) == sym)
                {
                    return Some((code, Some(shift)));
                }
            }
        }
        return None;
    }
    Some((
        match sym {
            0xff08 => 14,
            0xff09 => 15,
            0xff0d => 28,
            0xff8d => 96,
            0xff1b => 1,
            0xff50 => 102,
            0xff51 => 105,
            0xff52 => 103,
            0xff53 => 106,
            0xff54 => 108,
            0xff55 => 104,
            0xff56 => 109,
            0xff57 => 107,
            0xff63 => 110,
            0xffff => 111,
            0xffe1 => 42,
            0xffe2 => 54,
            0xffe3 => 29,
            0xffe4 => 97,
            0xffe9 => 56,
            0xffea => 100,
            0xffeb => 125,
            0xffec => 126,
            _ => return None,
        },
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_keysyms_use_physical_map_without_losing_case_or_releases() {
        let mut keyboard = Keyboard::default();
        for (sym, code, key) in [
            (b'W' as u32, "KeyW", "W"),
            (b'w' as u32, "KeyW", "w"),
            (b'!' as u32, "Digit1", "!"),
            (0xff8d, "NumpadEnter", "Enter"),
            (0xff51, "ArrowLeft", "ArrowLeft"),
            (0xff09, "Tab", "Tab"),
        ] {
            let (physical, shift) = keysym(sym).unwrap();
            for (value, down, repeat) in [(1, true, false), (2, true, true), (0, false, false)] {
                let Some(InputEvent::Key {
                    code: actual,
                    shift,
                    down: pressed,
                    repeat: repeated,
                }) = keyboard.event(physical, value, shift)
                else {
                    panic!("key event");
                };
                assert_eq!(crate::input::key(actual, shift), Some((code, key)));
                assert_eq!((pressed, repeated), (down, repeat));
            }
        }
        assert_eq!(keysym(0xffffffff), None);
    }
    #[test]
    fn wire_keys_repeat_release_and_disconnect_release_held_key() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (wake, _rx) = UnixStream::pair().unwrap();
        let shared = Arc::new(Shared {
            frame: Mutex::new((None, 0)),
            changed: Condvar::new(),
            events: Mutex::new(Vec::new()),
            wake: Mutex::new(wake),
            width: 1,
            height: 1,
        });
        let server = shared.clone();
        let task = thread::spawn(move || serve(listener.accept().unwrap().0, server));
        let mut client = TcpStream::connect(addr).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        assert_eq!(&read_exact::<12>(&mut client).unwrap(), b"RFB 003.008\n");
        client.write_all(b"RFB 003.008\n").unwrap();
        assert_eq!(read_exact::<2>(&mut client).unwrap(), [1, 1]);
        client.write_all(&[1]).unwrap();
        assert_eq!(read_exact::<4>(&mut client).unwrap(), [0; 4]);
        client.write_all(&[1]).unwrap();
        let init = read_exact::<24>(&mut client).unwrap();
        drain(
            &mut client,
            u32::from_be_bytes(init[20..24].try_into().unwrap()).into(),
        )
        .unwrap();
        for (down, key) in [(true, b'w'), (true, b'w'), (false, b'w'), (true, b'd')] {
            client
                .write_all(&[4, u8::from(down), 0, 0, 0, 0, 0, key])
                .unwrap();
        }
        client.shutdown(std::net::Shutdown::Both).unwrap();
        assert_eq!(
            task.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert_eq!(
            *shared.events.lock().unwrap(),
            vec![
                InputEvent::Key {
                    code: 17,
                    shift: false,
                    down: true,
                    repeat: false
                },
                InputEvent::Key {
                    code: 17,
                    shift: false,
                    down: true,
                    repeat: true
                },
                InputEvent::Key {
                    code: 17,
                    shift: false,
                    down: false,
                    repeat: false
                },
                InputEvent::Key {
                    code: 32,
                    shift: false,
                    down: true,
                    repeat: false
                },
                InputEvent::Key {
                    code: 32,
                    shift: false,
                    down: false,
                    repeat: false
                },
            ]
        );
    }
}

#[cfg(test)]
mod contact_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn disconnect_while_down_queues_cancel_and_never_a_successful_release() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (wake, _read) = UnixStream::pair().unwrap();
        let shared = Arc::new(Shared {
            frame: Mutex::new((None, 0)),
            changed: Condvar::new(),
            events: Mutex::new(Vec::new()),
            wake: Mutex::new(wake),
            width: 400,
            height: 500,
        });
        let server = shared.clone();
        let (finished, receive) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let result = serve(stream, server);
            finished.send(result.map_err(|e| e.kind())).unwrap();
        });
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!(&read_exact::<12>(&mut client).unwrap(), b"RFB 003.008\n");
        client.write_all(b"RFB 003.008\n").unwrap();
        assert_eq!(read_exact::<2>(&mut client).unwrap(), [1, 1]);
        client.write_all(&[1]).unwrap();
        read_exact::<4>(&mut client).unwrap();
        client.write_all(&[1]).unwrap();
        let init = read_exact::<24>(&mut client).unwrap();
        drain(
            &mut client,
            u32::from_be_bytes(init[20..24].try_into().unwrap()) as u64,
        )
        .unwrap();
        client.write_all(&[5, 1, 0, 20, 0, 40]).unwrap();
        client.shutdown(std::net::Shutdown::Both).unwrap();
        drop(client);
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(3)).unwrap(),
            Err(io::ErrorKind::UnexpectedEof)
        );
        worker.join().unwrap();
        let events = shared.events.lock().unwrap();
        assert!(events.contains(&InputEvent::Button(true)));
        assert_eq!(events.last(), Some(&InputEvent::Cancel));
        assert!(!events.contains(&InputEvent::Button(false)));
    }
}
