//! @exact/hid on the Linux host (LLP 1133.037 D10–D12, D21): `hidraw`. The devices come from sysfs
//! (`/sys/class/hidraw/*/device`: the bus, vendor and product in `uevent`, the top-level collections parsed from
//! `report_descriptor`); a node is opened once per process and leased to each connection (D5); a thread per open
//! reads its input reports into a log each reader takes a batch from (D12).
//!
//! - Not offered (D6, D11): a device holding a game controller's collection (`@exact/gamepad` owns pads) and one
//!   whose every collection is on WebHID's blocklist. A node is the whole interface, so a keyboard's own reports on
//!   a node that also holds an allowed collection are dropped before any batch, and a send of one refused: a
//!   report is placed in its collection by its report id.
//! - Output reports are `write(2)`, feature reports `HIDIOCSFEATURE` and `HIDIOCGFEATURE` (the kernel's
//!   hidraw.rst).
//! - The painted presenter draws no chooser (D2): `requestDevice` fails `NotSupportedError`.
//!
//! The package names only `exact_linux` and `std`, so the ioctls are declared here.

use exact_linux::package::{json, Context, Json, Notifier, Package, Reply};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

/// `_IOC(_IOC_WRITE | _IOC_READ, 'H', nr, len)`, the hidraw feature report ioctls.
fn hid_ioc(nr: u64, len: usize) -> u64 {
    (3 << 30) | ((len as u64) << 16) | ((b'H' as u64) << 8) | nr
}

const KEPT: usize = 1024;

/// One top-level collection, and the report ids its items name.
#[derive(Clone, Debug, Default)]
struct Collection {
    page: u32,
    usage: u32,
    reports: Vec<u8>,
}

/// The top-level application collections of a HID report descriptor (HID 1.11 §6.2.2), each with the report ids
/// declared inside it.
fn collections(descriptor: &[u8]) -> Vec<Collection> {
    let (mut out, mut depth, mut page, mut usage, mut at) = (Vec::<Collection>::new(), 0, 0u32, 0u32, 0);
    while at < descriptor.len() {
        let prefix = descriptor[at];
        if prefix == 0xFE {
            // A long item: its size is the next byte.
            at += 3 + *descriptor.get(at + 1).unwrap_or(&0) as usize;
            continue;
        }
        let size = [0, 1, 2, 4][(prefix & 3) as usize];
        let data = descriptor.get(at + 1..at + 1 + size).unwrap_or(&[]);
        let value = data.iter().rev().fold(0u32, |v, b| v << 8 | *b as u32);
        match prefix & 0xFC {
            0x04 => page = value,                                     // Usage Page
            0x08 => usage = if size == 4 { value & 0xFFFF } else { value }, // Usage
            0xA0 => {
                if depth == 0 {
                    out.push(Collection { page, usage, reports: Vec::new() });
                }
                depth += 1;
            }
            0xC0 => depth = (depth - 1).max(0),
            0x84 => {
                if let Some(c) = out.last_mut() {
                    c.reports.push(value as u8);
                }
            }
            _ => {}
        }
        at += 1 + size;
    }
    out
}

/// WebHID's blocklist (https://github.com/whatwg/hid/blob/main/blocklist.txt), by collection: FIDO, and Generic
/// Desktop's pointer, mouse, keyboard, keypad and system controls.
fn blocked(c: &Collection) -> bool {
    c.page == 0xF1D0 || (c.page == 1 && matches!(c.usage, 0x01 | 0x02 | 0x06 | 0x07 | 0x80..=0x8F))
}

fn pad(c: &Collection) -> bool {
    c.page == 1 && matches!(c.usage, 0x04 | 0x05 | 0x08)
}

#[derive(Clone)]
struct Device {
    id: String,
    path: String,
    vendor: u64,
    product: u64,
    name: String,
    collections: Vec<Collection>,
}

fn read_text(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

fn devices() -> Vec<Device> {
    let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let node = e.file_name().to_string_lossy().into_owned();
        let dev = e.path().join("device");
        let uevent = read_text(&dev.join("uevent")).unwrap_or_default();
        let field = |k: &str| uevent.lines().find_map(|l| l.strip_prefix(k)).unwrap_or_default().to_owned();
        let ids: Vec<u64> = field("HID_ID=").split(':').filter_map(|v| u64::from_str_radix(v, 16).ok()).collect();
        let descriptor = std::fs::read(dev.join("report_descriptor")).unwrap_or_default();
        let cs = collections(&descriptor);
        // Not offered (D11): a pad's device, or one whose every collection is blocked.
        if cs.iter().any(pad) || cs.iter().all(blocked) {
            continue;
        }
        let id = format!("{}:{:04x}:{:04x}@{}", ids.first().copied().unwrap_or(0), ids.get(1).copied().unwrap_or(0), ids.get(2).copied().unwrap_or(0), field("HID_PHYS="));
        out.push(Device { id, path: format!("/dev/{node}"), vendor: ids.get(1).copied().unwrap_or(0), product: ids.get(2).copied().unwrap_or(0), name: field("HID_NAME="), collections: cs });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn record(d: &Device, connected: bool) -> Json {
    let collections: Vec<Json> = (d.collections.iter())
        .map(|c| json!({ "usagePage": c.page, "usage": c.usage, "type": 1, "inputReports": c.reports.iter().map(|r| json!({ "reportId": r })).collect::<Vec<_>>(), "outputReports": null, "featureReports": null }))
        .collect();
    json!({ "id": d.id, "vendorId": d.vendor, "productId": d.product, "productName": d.name, "collections": collections, "connected": connected, "granted": OpenOptions::new().read(true).write(true).open(&d.path).map_or_else(|e| e.kind() != ErrorKind::PermissionDenied, |_| true) })
}

fn matches(d: &Device, filters: &Json) -> bool {
    (filters.as_array().into_iter().flatten()).any(|f| {
        f["vendorId"].as_u64().is_none_or(|v| v == d.vendor)
            && f["productId"].as_u64().is_none_or(|v| v == d.product)
            && f["usagePage"].as_u64().is_none_or(|p| d.collections.iter().any(|c| c.page as u64 == p && f["usage"].as_u64().is_none_or(|u| c.usage as u64 == u)))
    })
}

/// Whether report `id` belongs to a blocked collection: a device that uses no report ids has one structure, its
/// first collection's.
fn report_blocked(d: &Device, id: u8) -> bool {
    let of = (d.collections.iter()).find(|c| c.reports.contains(&id)).or(d.collections.first());
    of.is_some_and(blocked)
}

#[derive(Default)]
struct Log {
    reports: Vec<(u64, u8, Vec<u8>, f64)>,
    sequence: u64,
    gone: bool,
}

struct Open {
    file: Arc<File>,
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

fn read_loop(file: Arc<File>, log: Arc<Mutex<Log>>, stop: Arc<AtomicBool>, device: Device, notifier: Notifier, epoch: std::time::Instant) {
    let numbered = device.collections.iter().any(|c| !c.reports.is_empty());
    let mut buf = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        match (&*file).read(&mut buf) {
            Ok(0) | Err(_) => {
                log.lock().unwrap_or_else(|e| e.into_inner()).gone = true;
                notifier.changed("hid.inputReports");
                notifier.changed("hid.hidConnection");
                break;
            }
            Ok(n) => {
                let (id, data) = if numbered { (buf[0], buf[1..n].to_vec()) } else { (0, buf[..n].to_vec()) };
                if report_blocked(&device, id) {
                    continue;
                }
                let mut l = log.lock().unwrap_or_else(|e| e.into_inner());
                l.sequence += 1;
                let seq = l.sequence;
                l.reports.push((seq, id, data, epoch.elapsed().as_secs_f64() * 1000.0));
                if l.reports.len() > KEPT {
                    l.reports.remove(0);
                }
                drop(l);
                notifier.changed("hid.inputReports");
            }
        }
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
            let file = match OpenOptions::new().read(true).write(true).open(&device.path) {
                Ok(f) => Arc::new(f),
                Err(e) if e.kind() == ErrorKind::PermissionDenied => return json!({ "error": "NotAllowedError", "message": e.to_string() }),
                Err(e) => return json!({ "error": "NetworkError", "message": e.to_string() }),
            };
            let (log, stop) = (Arc::new(Mutex::new(Log::default())), Arc::new(AtomicBool::new(false)));
            let (f, l, s, d, n) = (file.clone(), log.clone(), stop.clone(), device.clone(), self.context.notifier());
            let epoch = std::time::Instant::now();
            std::thread::spawn(move || read_loop(f, l, s, d, n, epoch));
            self.opens.insert(id.to_owned(), Open { file, log, stop, leases: 0 });
        }
        self.opens.get_mut(id).expect("just opened").leases += 1;
        self.next += 1;
        let handle = format!("hid-{}", self.next);
        if !self.remembered.iter().any(|r| r == id) {
            self.remembered.push(id.to_owned());
        }
        let answer = json!({ "connection": handle, "device": record(&device, true) });
        self.connections.insert(handle, Connection { device, open: true, readers: BTreeMap::new() });
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

    fn send(&self, args: &Json, feature: bool) -> Json {
        let Some(c) = self.connections.get(args["connection"].as_str().unwrap_or_default()).filter(|c| c.open) else {
            return json!({ "error": "InvalidStateError" });
        };
        let Some(o) = self.opens.get(&c.device.id) else {
            return json!({ "error": "NetworkError" });
        };
        let id = args["reportId"].as_u64().unwrap_or(0) as u8;
        if report_blocked(&c.device, id) {
            return json!({ "error": "NotAllowedError", "message": "a blocked report" });
        }
        let Some(data) = args["data"].as_str().and_then(unbase64) else {
            return json!({ "error": "TypeError" });
        };
        // The report id leads the bytes; 0 for a device that uses none is the kernel's convention too.
        let mut bytes = vec![id];
        bytes.extend(data);
        let ok = if feature {
            // SAFETY: HIDIOCSFEATURE reads `bytes.len()` bytes from the buffer, which outlives the call.
            unsafe { ioctl(o.file.as_raw_fd(), hid_ioc(0x06, bytes.len()), bytes.as_ptr()) >= 0 }
        } else {
            (&*o.file).write_all(&bytes).is_ok()
        };
        if ok { Json::Null } else { json!({ "error": "NetworkError", "message": std::io::Error::last_os_error().to_string() }) }
    }

    fn release(&mut self, handle: &str) -> Json {
        let Some(c) = self.connections.get_mut(handle).filter(|c| c.open) else {
            return json!({ "error": "InvalidStateError" });
        };
        c.open = false;
        let id = c.device.id.clone();
        if let Some(o) = self.opens.get_mut(&id) {
            o.leases -= 1;
            if o.leases == 0 {
                o.stop.store(true, Ordering::Relaxed);
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
            "sendReport" => self.send(args, false),
            "sendFeatureReport" => self.send(args, true),
            "receiveFeatureReport" => match self.connections.get(args["connection"].as_str().unwrap_or_default()).filter(|c| c.open) {
                None => json!({ "error": "InvalidStateError" }),
                Some(c) => {
                    let id = args["reportId"].as_u64().unwrap_or(0) as u8;
                    match self.opens.get(&c.device.id) {
                        _ if report_blocked(&c.device, id) => json!({ "error": "NotAllowedError" }),
                        None => json!({ "error": "NetworkError" }),
                        Some(o) => {
                            let mut buf = vec![0u8; 4096];
                            buf[0] = id;
                            // SAFETY: HIDIOCGFEATURE writes at most `buf.len()` bytes into the buffer.
                            let n = unsafe { ioctl(o.file.as_raw_fd(), hid_ioc(0x07, buf.len()), buf.as_mut_ptr()) };
                            if n < 0 {
                                json!({ "error": "NotAllowedError", "message": std::io::Error::last_os_error().to_string() })
                            } else {
                                json!({ "data": base64(&buf[..n as usize]) })
                            }
                        }
                    }
                }
            },
            "close" => self.release(args["connection"].as_str().unwrap_or_default()),
            "forget" => {
                let id = args["device"].as_str().unwrap_or_default().to_owned();
                self.remembered.retain(|r| *r != id);
                let handles: Vec<String> = self.connections.iter().filter(|(_, c)| c.device.id == id && c.open).map(|(h, _)| h.clone()).collect();
                for h in handles {
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
        for o in self.opens.values() {
            o.stop.store(true, Ordering::Relaxed);
        }
    }
}

