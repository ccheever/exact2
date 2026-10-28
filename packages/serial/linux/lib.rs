//! @exact/serial on the Linux host (LLP 1133.037 D7–D9, D21): `/dev/tty*` with termios, the ports enumerated from
//! sysfs (`/sys/class/tty/*/device`, with the USB device's `idVendor`, `idProduct` and `product` above it), each
//! connection read by a thread of its own into the bytes not yet delivered.
//!
//! - Opened `O_NOCTTY | O_NONBLOCK`, then `TIOCEXCL`, so another process's open fails `EBUSY` (D5); termios raw at
//!   the call's line settings.
//! - Reading loses nothing (D8): the thread stops reading while the undelivered bytes reach the retained bound, so
//!   the rest waits in the kernel's and the device's buffers; each `serialInput` answer takes every byte since the
//!   last. End of file or `EIO` is the port gone (`NetworkError`, `done`).
//! - A node the process cannot open (`EACCES`) reads `granted` false (D17).
//! - Under the agent's driver it also offers the ports `EXACT_DEVICE_FIXTURE` names (a pseudo-terminal a drive made,
//!   D19 item 3); the Linux host runs headless on a Mac too, where the same POSIX calls drive the pty.
//!
//! The package names only `exact_linux` and `std`, so the C library's termios and ioctl calls are declared here,
//! with their structures and constants for each OS the host runs on.

use exact_linux::package::{json, Context, Json, Notifier, Package, Reply};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[cfg(target_os = "linux")]
mod sys {
    pub type Flag = u32;
    pub type Speed = u32;
    pub type Nfds = u64;
    #[repr(C)]
    pub struct Termios {
        pub iflag: Flag,
        pub oflag: Flag,
        pub cflag: Flag,
        pub lflag: Flag,
        pub line: u8,
        pub cc: [u8; 32],
        pub ispeed: Speed,
        pub ospeed: Speed,
    }
    pub const CSIZE: Flag = 0o60;
    pub const CS7: Flag = 0o40;
    pub const CS8: Flag = 0o60;
    pub const CSTOPB: Flag = 0o100;
    pub const CREAD: Flag = 0o200;
    pub const PARENB: Flag = 0o400;
    pub const PARODD: Flag = 0o1000;
    pub const CLOCAL: Flag = 0o4000;
    pub const CRTSCTS: Flag = 0o20000000000;
    pub const O_NOCTTY: i32 = 0o400;
    pub const O_NONBLOCK: i32 = 0o4000;
    pub const TIOCEXCL: u64 = 0x540C;
    pub const TIOCMGET: u64 = 0x5415;
    pub const TIOCMBIS: u64 = 0x5416;
    pub const TIOCMBIC: u64 = 0x5417;
    pub const TIOCSBRK: u64 = 0x5427;
    pub const TIOCCBRK: u64 = 0x5428;
}

#[cfg(target_os = "macos")]
mod sys {
    pub type Flag = u64;
    pub type Speed = u64;
    pub type Nfds = u32;
    #[repr(C)]
    pub struct Termios {
        pub iflag: Flag,
        pub oflag: Flag,
        pub cflag: Flag,
        pub lflag: Flag,
        pub cc: [u8; 20],
        pub ispeed: Speed,
        pub ospeed: Speed,
    }
    pub const CSIZE: Flag = 0x300;
    pub const CS7: Flag = 0x200;
    pub const CS8: Flag = 0x300;
    pub const CSTOPB: Flag = 0x400;
    pub const CREAD: Flag = 0x800;
    pub const PARENB: Flag = 0x1000;
    pub const PARODD: Flag = 0x2000;
    pub const CLOCAL: Flag = 0x8000;
    pub const CRTSCTS: Flag = 0x30000;
    pub const O_NOCTTY: i32 = 0x20000;
    pub const O_NONBLOCK: i32 = 0x4;
    pub const TIOCEXCL: u64 = 0x2000_740d;
    pub const TIOCMGET: u64 = 0x4004_746a;
    pub const TIOCMBIS: u64 = 0x8004_746c;
    pub const TIOCMBIC: u64 = 0x8004_746b;
    pub const TIOCSBRK: u64 = 0x2000_747b;
    pub const TIOCCBRK: u64 = 0x2000_747a;
}

use sys::*;

const TIOCM_DTR: i32 = 0x002;
const TIOCM_RTS: i32 = 0x004;
const TIOCM_CTS: i32 = 0x020;
const TIOCM_CD: i32 = 0x040;
const TIOCM_RI: i32 = 0x080;
const TIOCM_DSR: i32 = 0x100;
const MOST: usize = 16 * 1024 * 1024;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

extern "C" {
    fn tcgetattr(fd: i32, t: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, when: i32, t: *const Termios) -> i32;
    fn cfmakeraw(t: *mut Termios);
    fn cfsetspeed(t: *mut Termios, speed: Speed) -> i32;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn poll(fds: *mut PollFd, n: Nfds, timeout: i32) -> i32;
}

/// One port the platform or the drive offers.
#[derive(Clone)]
struct Port {
    id: String,
    name: String,
    path: String,
    vendor: Option<u64>,
    product: Option<u64>,
}

/// What a connection's reader thread and the calls share.
#[derive(Default)]
struct Stream {
    pending: Vec<u8>,
    errors: Vec<Json>,
    position: usize,
    done: bool,
    ended: bool,
}

struct Connection {
    port: Port,
    file: Arc<File>,
    options: Json,
    open: bool,
    stream: Arc<(Mutex<Stream>, Condvar)>,
    stop: Arc<AtomicBool>,
}

pub struct Serial {
    context: Context,
    connections: BTreeMap<String, Connection>,
    next: u64,
    remembered: Vec<String>,
}

fn read_text(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

/// The serial ports sysfs lists: each tty with a device behind it, named by its USB product where it has one.
fn sysfs_ports() -> Vec<Port> {
    let Ok(entries) = std::fs::read_dir("/sys/class/tty") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let tty = entry.file_name().to_string_lossy().into_owned();
        let device = entry.path().join("device");
        let Ok(real) = std::fs::canonicalize(&device) else {
            continue;
        };
        // The USB device is the nearest ancestor with an `idVendor`.
        let usb = real.ancestors().find(|a| a.join("idVendor").exists());
        let hex = |name: &str| usb.and_then(|u| read_text(&u.join(name).to_string_lossy())).and_then(|v| u64::from_str_radix(&v, 16).ok());
        let product = usb.and_then(|u| read_text(&u.join("product").to_string_lossy()));
        // A legacy 8250 port with no device answering is listed by the kernel too; it has no driver behind it.
        if usb.is_none() && tty.starts_with("ttyS") && !real.to_string_lossy().contains("serial8250") {
            continue;
        }
        let path = format!("/dev/{tty}");
        out.push(Port { id: path.clone(), name: product.unwrap_or_else(|| tty.clone()), path, vendor: hex("idVendor"), product: hex("idProduct") });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The ports a drive injects (`EXACT_DEVICE_FIXTURE`, `{"serial": [{"id", "name", "path"}]}`), read only in an app
/// the agent's driver launched (`EXACT_AGENT=1`). The Rust host's development and release builds share one profile
/// here, so unlike Apple's the path is compiled into both.
fn injected() -> Vec<Port> {
    if std::env::var_os("EXACT_AGENT").is_none_or(|v| v != "1") {
        return Vec::new();
    }
    let Some(file) = std::env::var_os("EXACT_DEVICE_FIXTURE") else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let json: Json = text.parse().unwrap_or(Json::Null);
    (json["serial"].as_array().into_iter().flatten())
        .filter_map(|p| {
            let path = p["path"].as_str()?.to_owned();
            std::path::Path::new(&path).exists().then(|| Port {
                id: p["id"].as_str().unwrap_or(&path).to_owned(),
                name: p["name"].as_str().unwrap_or(&path).to_owned(),
                path,
                vendor: p["usbVendorId"].as_u64(),
                product: p["usbProductId"].as_u64(),
            })
        })
        .collect()
}

fn ports() -> Vec<Port> {
    let mut all = sysfs_ports();
    all.extend(injected());
    all
}

/// Whether this process may open the node: `EACCES` reads `granted` false, and the recipe names the udev rule.
fn granted(path: &str) -> bool {
    match OpenOptions::new().read(true).write(true).custom_flags(O_NOCTTY | O_NONBLOCK).open(path) {
        Err(e) => e.kind() != ErrorKind::PermissionDenied,
        Ok(_) => true,
    }
}

fn record(p: &Port, connected: bool) -> Json {
    // An option the port lacks is null, not left out: data code passes the record on to Contract as it is.
    json!({ "id": p.id, "name": p.name, "connected": connected, "granted": granted(&p.path), "usbVendorId": p.vendor, "usbProductId": p.product })
}

fn matches(p: &Port, filters: &Json) -> bool {
    let Some(filters) = filters.as_array().filter(|f| !f.is_empty()) else {
        return true;
    };
    filters.iter().any(|f| {
        f["usbVendorId"].as_u64().is_none_or(|v| Some(v) == p.vendor) && f["usbProductId"].as_u64().is_none_or(|v| Some(v) == p.product)
    })
}

/// termios raw at the call's line settings.
fn configure(fd: i32, options: &Json) -> Result<(), String> {
    // SAFETY: a zeroed termios is a valid out-parameter for `tcgetattr`, which fills it.
    let mut t: Termios = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is an open descriptor this connection owns; `t` outlives the calls.
    unsafe {
        if tcgetattr(fd, &mut t) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        cfmakeraw(&mut t);
    }
    t.cflag &= !(CSIZE | CSTOPB | PARENB | PARODD | CRTSCTS);
    t.cflag |= CREAD | CLOCAL | if options["dataBits"] == 7 { CS7 } else { CS8 };
    if options["stopBits"] == 2 {
        t.cflag |= CSTOPB;
    }
    match options["parity"].as_str() {
        Some("even") => t.cflag |= PARENB,
        Some("odd") => t.cflag |= PARENB | PARODD,
        _ => {}
    }
    if options["flowControl"] == "hardware" {
        t.cflag |= CRTSCTS;
    }
    let baud = options["baudRate"].as_f64().unwrap_or(9600.0) as Speed;
    // SAFETY: as above.
    unsafe {
        if cfsetspeed(&mut t, baud) != 0 || tcsetattr(fd, 0, &t) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

/// The connection's reader: reads while the undelivered bytes are under `bound`, waiting on the descriptor.
fn read_loop(file: Arc<File>, stream: Arc<(Mutex<Stream>, Condvar)>, stop: Arc<AtomicBool>, bound: usize, notifier: Notifier) {
    let mut buf = vec![0u8; 4096];
    let (lock, room) = &*stream;
    while !stop.load(Ordering::Relaxed) {
        {
            let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
            // Backpressure (D8): nothing more is read until a delivery makes room.
            while s.pending.len() >= bound && !stop.load(Ordering::Relaxed) {
                s = room.wait_timeout(s, std::time::Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
            }
        }
        let mut p = PollFd { fd: file.as_raw_fd(), events: 1, revents: 0 };
        // SAFETY: one valid pollfd for the call's duration.
        if unsafe { poll(&mut p, 1, 100) } <= 0 {
            continue;
        }
        let room_left = {
            let s = lock.lock().unwrap_or_else(|e| e.into_inner());
            bound.saturating_sub(s.pending.len()).clamp(1, buf.len())
        };
        match (&*file).read(&mut buf[..room_left]) {
            Ok(0) => {
                finish(&stream, "NetworkError");
                break;
            }
            Ok(n) => lock.lock().unwrap_or_else(|e| e.into_inner()).pending.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => continue,
            // EIO or ENXIO: the port went away (D9).
            Err(_) => {
                finish(&stream, "NetworkError");
                break;
            }
        }
        notifier.changed("serial.serialInput");
    }
    notifier.changed("serial.serialInput");
}

fn finish(stream: &Arc<(Mutex<Stream>, Condvar)>, code: &str) {
    let mut s = stream.0.lock().unwrap_or_else(|e| e.into_inner());
    let at = s.position + s.pending.len();
    s.errors.push(json!({ "at": at, "code": code }));
    s.done = true;
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift) as usize & 63] as char } else { '=' });
        }
    }
    out
}

fn unbase64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for b in text.trim_end_matches('=').bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl Serial {
    fn open(&mut self, args: &Json) -> Json {
        let id = args["port"].as_str().unwrap_or_default();
        let Some(port) = ports().into_iter().find(|p| p.id == id) else {
            return json!({ "error": "NotFoundError" });
        };
        if self.connections.values().any(|c| c.open && c.port.path == port.path) {
            return json!({ "error": "InvalidStateError", "message": "already open" });
        }
        if !(args["baudRate"].as_f64().unwrap_or(0.0) > 0.0) {
            return json!({ "error": "TypeError", "message": "baudRate" });
        }
        let mut options = json!({ "dataBits": 8, "stopBits": 1, "parity": "none", "bufferSize": 255, "flowControl": "none" });
        for (k, v) in args.as_object().into_iter().flatten() {
            if k != "port" && !v.is_null() {
                options[k] = v.clone();
            }
        }
        let file = match OpenOptions::new().read(true).write(true).custom_flags(O_NOCTTY | O_NONBLOCK).open(&port.path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => return json!({ "error": "NotAllowedError", "message": e.to_string() }),
            Err(e) if e.raw_os_error() == Some(16) => return json!({ "error": "InvalidStateError", "message": e.to_string() }),
            Err(e) => return json!({ "error": "NetworkError", "message": e.to_string() }),
        };
        let fd = file.as_raw_fd();
        // SAFETY: an ioctl with no argument on a descriptor this call owns.
        unsafe { ioctl(fd, TIOCEXCL) };
        if let Err(e) = configure(fd, &options) {
            return json!({ "error": "NetworkError", "message": e });
        }
        let baud = options["baudRate"].as_f64().unwrap_or(9600.0) as usize;
        let bound = (baud / 10 * 2).max(options["bufferSize"].as_u64().unwrap_or(255) as usize).min(MOST);
        self.next += 1;
        let handle = format!("serial-{}", self.next);
        let (file, stream, stop) = (Arc::new(file), Arc::new((Mutex::new(Stream::default()), Condvar::new())), Arc::new(AtomicBool::new(false)));
        let (f, s, st, notifier) = (file.clone(), stream.clone(), stop.clone(), self.context.notifier());
        std::thread::spawn(move || read_loop(f, s, st, bound, notifier));
        if !self.remembered.contains(&port.id) {
            self.remembered.push(port.id.clone());
        }
        let answer = json!({ "connection": handle, "port": record(&port, true), "options": options });
        self.connections.insert(handle, Connection { port, file, options, open: true, stream, stop });
        self.context.changed("serial.getPorts");
        answer
    }

    fn input(&mut self, args: &Json) -> Json {
        let Some(c) = self.connections.get(args["connection"].as_str().unwrap_or_default()) else {
            return json!({ "error": "InvalidStateError", "message": "no such connection" });
        };
        let (lock, room) = &*c.stream;
        let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
        if s.ended {
            return json!({ "error": "InvalidStateError", "message": "the stream has ended" });
        }
        let bytes = std::mem::take(&mut s.pending);
        s.position += bytes.len();
        let errors = std::mem::take(&mut s.errors);
        let done = s.done || !c.open;
        s.ended = done;
        room.notify_all();
        json!({ "data": base64(&bytes), "errors": errors, "done": done, "position": s.position })
    }

    fn connection(&self, args: &Json) -> Result<&Connection, Json> {
        match self.connections.get(args["connection"].as_str().unwrap_or_default()) {
            Some(c) if c.open => Ok(c),
            _ => Err(json!({ "error": "InvalidStateError" })),
        }
    }

    fn close(&mut self, handle: &str) -> Json {
        let Some(c) = self.connections.get_mut(handle).filter(|c| c.open) else {
            return json!({ "error": "InvalidStateError" });
        };
        c.open = false;
        c.stop.store(true, Ordering::Relaxed);
        c.stream.1.notify_all();
        self.context.changed("serial.serialConnection");
        self.context.changed("serial.serialInput");
        Json::Null
    }
}

fn os_error() -> Json {
    json!({ "error": "NetworkError", "message": std::io::Error::last_os_error().to_string() })
}

impl Package for Serial {
    fn new(context: Context) -> Self {
        Serial { context, connections: BTreeMap::new(), next: 0, remembered: Vec::new() }
    }

    fn later(&mut self, name: &str, args: &Json, _preferred: Option<&Json>, reply: Reply) {
        let answer = match name {
            "serialStatus" => json!({ "available": true, "attachedList": true, "state": "granted" }),
            "attachedPorts" => json!(ports().iter().filter(|p| matches(p, &args["filters"])).map(|p| record(p, true)).collect::<Vec<_>>()),
            "getPorts" => {
                let here = ports();
                json!(self.remembered.iter().map(|id| match here.iter().find(|p| &p.id == id) {
                    Some(p) => record(p, true),
                    None => json!({ "id": id, "name": id, "connected": false, "granted": true, "usbVendorId": null, "usbProductId": null }),
                }).collect::<Vec<_>>())
            }
            // The painted presenter draws no chooser yet (D2): the app lists `attachedPorts` itself.
            "requestPort" => json!({ "error": "NotSupportedError" }),
            "open" => self.open(args),
            "serialConnection" => match self.connections.get(args["connection"].as_str().unwrap_or_default()) {
                Some(c) => {
                    let gone = c.stream.0.lock().map(|s| s.done).unwrap_or(true);
                    json!({ "open": c.open && !gone, "connected": !gone, "options": c.options })
                }
                None => json!({ "open": false, "connected": false }),
            },
            "serialInput" => self.input(args),
            "write" => match (self.connection(args), unbase64(args["data"].as_str().unwrap_or_default())) {
                (Err(e), _) => e,
                (_, None) => json!({ "error": "TypeError" }),
                (Ok(c), Some(bytes)) => {
                    let mut sent = 0;
                    let mut failed = None;
                    while sent < bytes.len() {
                        match (&*c.file).write(&bytes[sent..]) {
                            Ok(n) => sent += n,
                            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => std::thread::sleep(std::time::Duration::from_millis(1)),
                            Err(e) => {
                                failed = Some(e.to_string());
                                break;
                            }
                        }
                    }
                    match failed {
                        Some(m) => json!({ "error": "NetworkError", "message": m }),
                        None => Json::Null,
                    }
                }
            },
            "setSignals" => match self.connection(args) {
                Err(e) => e,
                Ok(c) => {
                    let fd = c.file.as_raw_fd();
                    let mut ok = true;
                    for (key, bit) in [("dataTerminalReady", TIOCM_DTR), ("requestToSend", TIOCM_RTS)] {
                        if let Some(on) = args[key].as_bool() {
                            let bits: i32 = bit;
                            // SAFETY: TIOCMBIS and TIOCMBIC read one int through the pointer.
                            ok &= unsafe { ioctl(fd, if on { TIOCMBIS } else { TIOCMBIC }, &bits as *const i32) } == 0;
                        }
                    }
                    if let Some(on) = args["break"].as_bool() {
                        // SAFETY: no argument.
                        ok &= unsafe { ioctl(fd, if on { TIOCSBRK } else { TIOCCBRK }) } == 0;
                    }
                    // A pseudo-terminal carries no modem lines (ENOTTY).
                    if ok { Json::Null } else { os_error() }
                }
            },
            "getSignals" => match self.connection(args) {
                Err(e) => e,
                Ok(c) => {
                    let mut bits: i32 = 0;
                    // SAFETY: TIOCMGET writes one int through the pointer.
                    if unsafe { ioctl(c.file.as_raw_fd(), TIOCMGET, &mut bits as *mut i32) } != 0 {
                        os_error()
                    } else {
                        json!({ "dataCarrierDetect": bits & TIOCM_CD != 0, "clearToSend": bits & TIOCM_CTS != 0, "ringIndicator": bits & TIOCM_RI != 0, "dataSetReady": bits & TIOCM_DSR != 0 })
                    }
                }
            },
            "close" => self.close(args["connection"].as_str().unwrap_or_default()),
            "forget" => {
                let id = args["port"].as_str().unwrap_or_default().to_owned();
                self.remembered.retain(|r| *r != id);
                let handles: Vec<String> = self.connections.iter().filter(|(_, c)| c.port.id == id).map(|(h, _)| h.clone()).collect();
                for h in handles {
                    self.close(&h);
                }
                self.context.changed("serial.getPorts");
                Json::Null
            }
            _ => json!({ "error": "NotSupportedError" }),
        };
        reply.send(if answer.is_null() { json!({}) } else { answer });
    }

    fn destroy(&mut self) {
        let handles: Vec<String> = self.connections.keys().cloned().collect();
        for h in handles {
            self.close(&h);
        }
    }
}
