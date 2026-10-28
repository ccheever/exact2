//! @exact/hid on the Windows host (LLP 1133.037 D10–D12, D14, D21): Win32's `hid.dll`. Windows enumerates each
//! top-level collection as a device interface of its own (Configuration Manager's
//! `CM_Get_Device_Interface_ListW` over the HID class's interface GUID), so a collection on WebHID's blocklist,
//! which the system's keyboard and mouse mappers hold anyway, is never opened, and a pad's is never offered
//! (D11). A collection is opened once per process and leased to each connection (D5); a thread per open reads its
//! input reports (`ReadFile`) into a log each reader takes a batch from (D12). Output reports are
//! `HidD_SetOutputReport`, feature reports `HidD_SetFeature` and `HidD_GetFeature`, each the length the
//! collection's capabilities name.
//!
//! WinRT's `HidDevice` needs a package capability the unpackaged host cannot declare (D14), so this is Win32's,
//! declared here: the package names only `exact_linux` and `std`. Not built yet: the plug notifications
//! (`CM_Register_Notification`); the lists are read again at each ask. `requestDevice` (the `DevicePicker`) fails
//! `NotSupportedError`: the app lists `attachedDevices` itself.

use exact_linux::package::{json, Context, Json, Notifier, Package, Reply};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

type Handle = *mut c_void;
const INVALID: Handle = -1isize as Handle;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid(u32, u16, u16, [u8; 8]);

#[repr(C)]
struct Attributes {
    size: u32,
    vendor: u16,
    product: u16,
    version: u16,
}

#[cfg_attr(windows, link(name = "kernel32"))]
extern "system" {
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut c_void, disposition: u32, flags: u32, template: Handle) -> Handle;
    fn ReadFile(file: Handle, buffer: *mut u8, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
    fn CloseHandle(h: Handle) -> i32;
    fn CancelIoEx(h: Handle, overlapped: *mut c_void) -> i32;
}
#[cfg_attr(windows, link(name = "hid"))]
extern "system" {
    fn HidD_GetHidGuid(guid: *mut Guid);
    fn HidD_GetAttributes(h: Handle, a: *mut Attributes) -> u8;
    fn HidD_GetProductString(h: Handle, buffer: *mut u16, len: u32) -> u8;
    fn HidD_GetPreparsedData(h: Handle, data: *mut *mut c_void) -> u8;
    fn HidD_FreePreparsedData(data: *mut c_void) -> u8;
    fn HidP_GetCaps(data: *mut c_void, caps: *mut [u16; 32]) -> i32;
    fn HidD_SetFeature(h: Handle, buffer: *mut u8, len: u32) -> u8;
    fn HidD_GetFeature(h: Handle, buffer: *mut u8, len: u32) -> u8;
    fn HidD_SetOutputReport(h: Handle, buffer: *mut u8, len: u32) -> u8;
}
#[cfg_attr(windows, link(name = "cfgmgr32"))]
extern "system" {
    fn CM_Get_Device_Interface_List_SizeW(len: *mut u32, guid: *const Guid, device: *const u16, flags: u32) -> u32;
    fn CM_Get_Device_Interface_ListW(guid: *const Guid, device: *const u16, buffer: *mut u16, len: u32, flags: u32) -> u32;
}

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const SHARE_READ_WRITE: u32 = 3;
const OPEN_EXISTING: u32 = 3;
const PRESENT: u32 = 0;
const KEPT: usize = 1024;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// One collection's capabilities: its usage page and usage, and the lengths of its reports.
#[derive(Clone, Copy, Default)]
struct Caps {
    usage: u16,
    page: u16,
    input: u16,
    output: u16,
    feature: u16,
}

#[derive(Clone)]
struct Device {
    id: String,
    path: String,
    vendor: u16,
    product: u16,
    name: String,
    caps: Caps,
}

/// Open a collection's interface for reading and writing (`access` 0 to read its attributes alone).
fn open_path(path: &str, access: u32) -> Option<Handle> {
    let name = wide(path);
    // SAFETY: a NUL-terminated wide path; the other arguments are the documented defaults.
    let h = unsafe { CreateFileW(name.as_ptr(), access, SHARE_READ_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
    (h != INVALID).then_some(h)
}

fn describe(path: &str) -> Option<Device> {
    let h = open_path(path, 0)?;
    let mut a = Attributes { size: std::mem::size_of::<Attributes>() as u32, vendor: 0, product: 0, version: 0 };
    let (mut name, mut prepared, mut caps) = ([0u16; 128], std::ptr::null_mut(), [0u16; 32]);
    // SAFETY: `h` is open; each out-parameter is sized as the call documents.
    let ok = unsafe {
        HidD_GetAttributes(h, &mut a) != 0 && {
            HidD_GetProductString(h, name.as_mut_ptr(), (name.len() * 2) as u32);
            HidD_GetPreparsedData(h, &mut prepared) != 0
        }
    };
    if ok {
        // SAFETY: `prepared` came from HidD_GetPreparsedData and is freed once.
        unsafe {
            HidP_GetCaps(prepared, &mut caps);
            HidD_FreePreparsedData(prepared);
        }
    }
    // SAFETY: closed once.
    unsafe { CloseHandle(h) };
    if !ok {
        return None;
    }
    let caps = Caps { usage: caps[0], page: caps[1], input: caps[2], output: caps[3], feature: caps[4] };
    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    Some(Device { id: path.to_lowercase(), path: path.to_owned(), vendor: a.vendor, product: a.product, name: String::from_utf16_lossy(&name[..end]), caps })
}

/// WebHID's blocklist by collection, and the pads `@exact/gamepad` owns (D6, D11).
fn offered(c: &Caps) -> bool {
    let blocked = c.page == 0xF1D0 || (c.page == 1 && matches!(c.usage, 0x01 | 0x02 | 0x06 | 0x07 | 0x80..=0x8F));
    let pad = c.page == 1 && matches!(c.usage, 0x04 | 0x05 | 0x08);
    !blocked && !pad
}

fn devices() -> Vec<Device> {
    let mut guid = Guid(0, 0, 0, [0; 8]);
    let mut len = 0u32;
    // SAFETY: the list's length, then the list, NUL-separated and double-NUL-terminated.
    let paths = unsafe {
        HidD_GetHidGuid(&mut guid);
        if CM_Get_Device_Interface_List_SizeW(&mut len, &guid, std::ptr::null(), PRESENT) != 0 || len == 0 {
            return Vec::new();
        }
        let mut buffer = vec![0u16; len as usize];
        if CM_Get_Device_Interface_ListW(&guid, std::ptr::null(), buffer.as_mut_ptr(), len, PRESENT) != 0 {
            return Vec::new();
        }
        buffer
    };
    let mut out: Vec<Device> = (paths.split(|c| *c == 0))
        .filter(|p| !p.is_empty())
        .filter_map(|p| describe(&String::from_utf16_lossy(p)))
        .filter(|d| offered(&d.caps))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn record(d: &Device, connected: bool) -> Json {
    let reports = |len: u16| if len > 0 { json!([{ "reportId": 0 }]) } else { json!([]) };
    json!({
        "id": d.id, "vendorId": d.vendor, "productId": d.product, "productName": d.name, "connected": connected, "granted": true,
        "collections": [{ "usagePage": d.caps.page, "usage": d.caps.usage, "type": 1, "inputReports": reports(d.caps.input), "outputReports": reports(d.caps.output), "featureReports": reports(d.caps.feature) }],
    })
}

fn matches(d: &Device, filters: &Json) -> bool {
    (filters.as_array().into_iter().flatten()).any(|f| {
        f["vendorId"].as_u64().is_none_or(|v| v == d.vendor as u64)
            && f["productId"].as_u64().is_none_or(|v| v == d.product as u64)
            && f["usagePage"].as_u64().is_none_or(|p| p == d.caps.page as u64)
            && f["usage"].as_u64().is_none_or(|u| u == d.caps.usage as u64)
    })
}

#[derive(Default)]
struct Log {
    reports: Vec<(u64, u8, Vec<u8>, f64)>,
    sequence: u64,
    gone: bool,
}

/// The open collection's handle, shared with its reader thread.
struct Shared(Handle);
// SAFETY: a Win32 file handle may be used from any thread; the reader only reads, and the handle is closed after
// the reader is told to stop and its read cancelled.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

struct Open {
    handle: Arc<Shared>,
    log: Arc<Mutex<Log>>,
    stop: Arc<AtomicBool>,
    leases: usize,
}

struct Connection {
    device: Device,
    open: bool,
    readers: BTreeMap<String, u64>,
}

pub struct Hid {
    context: Context,
    opens: BTreeMap<String, Open>,
    connections: BTreeMap<String, Connection>,
    next: u64,
    remembered: Vec<String>,
}

fn read_loop(handle: Arc<Shared>, log: Arc<Mutex<Log>>, stop: Arc<AtomicBool>, len: usize, notifier: Notifier, epoch: std::time::Instant) {
    let mut buf = vec![0u8; len.max(1)];
    while !stop.load(Ordering::Relaxed) {
        let mut read = 0u32;
        // SAFETY: a synchronous read into `buf`, which outlives it; `close` cancels it.
        let ok = unsafe { ReadFile(handle.0, buf.as_mut_ptr(), buf.len() as u32, &mut read, std::ptr::null_mut()) } != 0;
        if !ok || read == 0 {
            if !stop.load(Ordering::Relaxed) {
                log.lock().unwrap_or_else(|e| e.into_inner()).gone = true;
                notifier.changed("hid.inputReports");
                notifier.changed("hid.hidConnection");
            }
            break;
        }
        // Windows leads every report with its id, 0 for a collection that uses none.
        let mut l = log.lock().unwrap_or_else(|e| e.into_inner());
        l.sequence += 1;
        let seq = l.sequence;
        l.reports.push((seq, buf[0], buf[1..read as usize].to_vec(), epoch.elapsed().as_secs_f64() * 1000.0));
        if l.reports.len() > KEPT {
            l.reports.remove(0);
        }
        drop(l);
        notifier.changed("hid.inputReports");
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift) as usize & 63] as char } else { '=' });
        }
    }
    out
}

fn unbase64(text: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0);
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

impl Hid {
    fn open(&mut self, id: &str) -> Json {
        let Some(device) = devices().into_iter().find(|d| d.id == id) else {
            return json!({ "error": "NotFoundError" });
        };
        if !self.opens.contains_key(id) {
            let Some(h) = open_path(&device.path, GENERIC_READ | GENERIC_WRITE) else {
                return json!({ "error": "NetworkError", "message": std::io::Error::last_os_error().to_string() });
            };
            let (handle, log, stop) = (Arc::new(Shared(h)), Arc::new(Mutex::new(Log::default())), Arc::new(AtomicBool::new(false)));
            let (hh, l, s, n, len) = (handle.clone(), log.clone(), stop.clone(), self.context.notifier(), device.caps.input as usize);
            let epoch = std::time::Instant::now();
            if len > 0 {
                std::thread::spawn(move || read_loop(hh, l, s, len, n, epoch));
            }
            self.opens.insert(id.to_owned(), Open { handle, log, stop, leases: 0 });
        }
        self.opens.get_mut(id).expect("just opened").leases += 1;
        self.next += 1;
        let connection = format!("hid-{}", self.next);
        if !self.remembered.iter().any(|r| r == id) {
            self.remembered.push(id.to_owned());
        }
        let answer = json!({ "connection": connection, "device": record(&device, true) });
        self.connections.insert(connection, Connection { device, open: true, readers: BTreeMap::new() });
        self.context.changed("hid.getDevices");
        answer
    }

    fn reports(&mut self, args: &Json) -> Json {
        let Some(c) = self.connections.get_mut(args["connection"].as_str().unwrap_or_default()) else {
            return json!({ "error": "InvalidStateError" });
        };
        let Some(o) = self.opens.get(&c.device.id) else {
            return json!({ "reports": [], "dropped": 0, "interrupted": true });
        };
        let log = o.log.lock().unwrap_or_else(|e| e.into_inner());
        let wanted = args["reportId"].as_u64().map(|r| r as u8);
        let history = args["history"].as_u64().unwrap_or(16).min(1024) as usize;
        let reader = format!("{wanted:?} {history}");
        let of: Vec<&(u64, u8, Vec<u8>, f64)> = log.reports.iter().filter(|r| wanted.is_none_or(|w| w == r.1)).collect();
        let mut fresh: Vec<&(u64, u8, Vec<u8>, f64)> = match c.readers.get(&reader) {
            Some(cursor) => of.into_iter().filter(|r| r.0 > *cursor).collect(),
            None => of.into_iter().last().into_iter().collect(),
        };
        let dropped = fresh.len().saturating_sub(history);
        fresh.drain(..dropped);
        c.readers.insert(reader, log.sequence);
        json!({ "reports": fresh.iter().map(|r| json!({ "reportId": r.1, "data": base64(&r.2), "at": r.3 })).collect::<Vec<_>>(), "dropped": dropped, "interrupted": log.gone })
    }

    /// A report the call names, at the length the collection's capabilities give its kind: the id first.
    fn report(&self, args: &Json, kind: char) -> Json {
        let Some(c) = self.connections.get(args["connection"].as_str().unwrap_or_default()).filter(|c| c.open) else {
            return json!({ "error": "InvalidStateError" });
        };
        let Some(o) = self.opens.get(&c.device.id) else {
            return json!({ "error": "NetworkError" });
        };
        let len = if kind == 'o' { c.device.caps.output } else { c.device.caps.feature } as usize;
        if len == 0 {
            return json!({ "error": "NotAllowedError", "message": "the collection has no such report" });
        }
        let mut buf = vec![0u8; len];
        buf[0] = args["reportId"].as_u64().unwrap_or(0) as u8;
        if kind == 'g' {
            // SAFETY: a buffer of the collection's feature report length, the id in its first byte.
            if unsafe { HidD_GetFeature(o.handle.0, buf.as_mut_ptr(), len as u32) } == 0 {
                return json!({ "error": "NotAllowedError", "message": std::io::Error::last_os_error().to_string() });
            }
            return json!({ "data": base64(&buf[1..]) });
        }
        let Some(data) = args["data"].as_str().and_then(unbase64) else {
            return json!({ "error": "TypeError" });
        };
        let n = data.len().min(len - 1);
        buf[1..1 + n].copy_from_slice(&data[..n]);
        // SAFETY: as above.
        let ok = unsafe { if kind == 'o' { HidD_SetOutputReport(o.handle.0, buf.as_mut_ptr(), len as u32) } else { HidD_SetFeature(o.handle.0, buf.as_mut_ptr(), len as u32) } } != 0;
        if ok { Json::Null } else { json!({ "error": "NetworkError", "message": std::io::Error::last_os_error().to_string() }) }
    }

    fn release(&mut self, connection: &str) -> Json {
        let Some(c) = self.connections.get_mut(connection).filter(|c| c.open) else {
            return json!({ "error": "InvalidStateError" });
        };
        c.open = false;
        let id = c.device.id.clone();
        if let Some(o) = self.opens.get_mut(&id) {
            o.leases -= 1;
            if o.leases == 0 {
                o.stop.store(true, Ordering::Relaxed);
                // SAFETY: the reader's read is cancelled before the handle closes, once.
                unsafe {
                    CancelIoEx(o.handle.0, std::ptr::null_mut());
                    CloseHandle(o.handle.0);
                }
                self.opens.remove(&id);
            }
        }
        self.context.changed("hid.hidConnection");
        Json::Null
    }
}

impl Package for Hid {
    fn new(context: Context) -> Self {
        Hid { context, opens: BTreeMap::new(), connections: BTreeMap::new(), next: 0, remembered: Vec::new() }
    }

    fn later(&mut self, name: &str, args: &Json, _preferred: Option<&Json>, reply: Reply) {
        let answer = match name {
            "hidStatus" => json!({ "available": true, "attachedList": true, "state": "granted" }),
            "attachedDevices" => json!(devices().iter().filter(|d| matches(d, &args["filters"])).map(|d| record(d, true)).collect::<Vec<_>>()),
            "getDevices" => {
                let here = devices();
                json!(self.remembered.iter().map(|id| match here.iter().find(|d| &d.id == id) {
                    Some(d) => record(d, true),
                    None => json!({ "id": id, "vendorId": 0, "productId": 0, "productName": "", "collections": [], "connected": false, "granted": true }),
                }).collect::<Vec<_>>())
            }
            "requestDevice" => json!({ "error": "NotSupportedError" }),
            "open" => self.open(args["device"].as_str().unwrap_or_default()),
            "hidConnection" => match self.connections.get(args["connection"].as_str().unwrap_or_default()) {
                Some(c) => {
                    let gone = self.opens.get(&c.device.id).is_none_or(|o| o.log.lock().map(|l| l.gone).unwrap_or(true));
                    json!({ "open": c.open && !gone, "connected": !gone })
                }
                None => json!({ "open": false, "connected": false }),
            },
            "inputReports" => self.reports(args),
            "sendReport" => self.report(args, 'o'),
            "sendFeatureReport" => self.report(args, 'f'),
            "receiveFeatureReport" => self.report(args, 'g'),
            "close" => self.release(args["connection"].as_str().unwrap_or_default()),
            "forget" => {
                let id = args["device"].as_str().unwrap_or_default().to_owned();
                self.remembered.retain(|r| *r != id);
                let open: Vec<String> = self.connections.iter().filter(|(_, c)| c.device.id == id && c.open).map(|(h, _)| h.clone()).collect();
                for h in open {
                    self.release(&h);
                }
                self.context.changed("hid.getDevices");
                Json::Null
            }
            _ => json!({ "error": "NotSupportedError" }),
        };
        reply.send(if answer.is_null() { json!({}) } else { answer });
    }

    fn destroy(&mut self) {
        let open: Vec<String> = self.connections.iter().filter(|(_, c)| c.open).map(|(h, _)| h.clone()).collect();
        for h in open {
            self.release(&h);
        }
    }
}
