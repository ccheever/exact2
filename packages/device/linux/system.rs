//! What the Linux readings ask the system: rtnetlink for the route a global
//! destination takes, and D-Bus (UPower, power-profiles-daemon, the Power
//! Profile Monitor and Network Monitor portals) over the bus sockets. The
//! package takes no crate of its own (the desktop builder writes no Cargo
//! manifest), so the few libc calls are declared here.

use super::readers::{self, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

#[allow(non_camel_case_types)]
type c_int = i32;

extern "C" {
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn bind(fd: c_int, addr: *const u8, len: u32) -> c_int;
    fn send(fd: c_int, buf: *const u8, len: usize, flags: c_int) -> isize;
    fn recv(fd: c_int, buf: *mut u8, len: usize, flags: c_int) -> isize;
    fn close(fd: c_int) -> c_int;
    fn setsockopt(fd: c_int, level: c_int, name: c_int, value: *const u8, len: u32) -> c_int;
    fn if_indextoname(index: u32, name: *mut u8) -> *mut u8;
    fn getuid() -> u32;
}

const AF_NETLINK: c_int = 16;
const SOCK_RAW: c_int = 3;
const SOCK_CLOEXEC: c_int = 0o2000000;
const NETLINK_ROUTE: c_int = 0;
const SOL_SOCKET: c_int = 1;
const SO_RCVTIMEO: c_int = 20;

/// The route, link and policy-rule groups (D5): `RTNLGRP_LINK`,
/// `RTNLGRP_IPV4_ROUTE`, `RTNLGRP_IPV4_RULE`, `RTNLGRP_IPV6_ROUTE`,
/// `RTNLGRP_IPV6_RULE`, as the legacy bit mask (group n is bit n - 1).
pub const ROUTE_GROUPS: u32 = 1 | (1 << 6) | (1 << 7) | (1 << 10) | (1 << 18);

/// An rtnetlink socket, closed when dropped.
pub struct Netlink(c_int);

impl Drop for Netlink {
    fn drop(&mut self) {
        unsafe { close(self.0) };
    }
}

impl Netlink {
    /// A socket bound to `groups` (0: none), whose reads give up after `timeout_s`.
    pub fn open(groups: u32, timeout_s: i64) -> Option<Netlink> {
        let fd = unsafe { socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE) };
        if fd < 0 {
            return None;
        }
        let socket = Netlink(fd);
        // sockaddr_nl: family, pad, pid (0: the kernel assigns), groups.
        let mut addr = [0u8; 12];
        addr[..2].copy_from_slice(&(AF_NETLINK as u16).to_ne_bytes());
        addr[8..].copy_from_slice(&groups.to_ne_bytes());
        let timeval: [i64; 2] = [timeout_s, 0];
        let ok = unsafe {
            bind(fd, addr.as_ptr(), 12) == 0
                && setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, timeval.as_ptr().cast(), 16) == 0
        };
        ok.then_some(socket)
    }

    /// One datagram, or None on a timeout or an error.
    pub fn read(&self, buf: &mut [u8]) -> Option<usize> {
        let n = unsafe { recv(self.0, buf.as_mut_ptr(), buf.len(), 0) };
        (n > 0).then_some(n as usize)
    }

    /// The name of the interface the kernel would send `dst` out of.
    fn route(&self, dst: &[u8], seq: u32) -> Option<String> {
        let request = readers::route_request(dst, seq);
        if unsafe { send(self.0, request.as_ptr(), request.len(), 0) } < 0 {
            return None;
        }
        let mut buf = vec![0u8; 8192];
        let n = self.read(&mut buf)?;
        let index = readers::route_oif(&buf[..n])?;
        let mut name = [0u8; 16];
        if unsafe { if_indextoname(index, name.as_mut_ptr()) }.is_null() {
            return None;
        }
        let end = name.iter().position(|&b| b == 0).unwrap_or(16);
        Some(String::from_utf8_lossy(&name[..end]).into_owned())
    }
}

/// The network's `type` (D2's Linux row): the link of the route a global
/// destination takes, IPv4's when both families have one; `none` with no
/// route; `unknown` when rtnetlink cannot be asked.
pub fn network_type() -> &'static str {
    let Some(socket) = Netlink::open(0, 2) else {
        return "unknown";
    };
    let v4 = [8, 8, 8, 8];
    let v6 = [0x20, 0x01, 0x48, 0x60, 0x48, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0x88];
    match socket.route(&v4, 1).or_else(|| socket.route(&v6, 2)) {
        Some(name) => readers::link_type(Path::new("/sys/class/net"), &name),
        None => "none",
    }
}

/// A connection to a message bus, authenticated and named.
pub struct Bus {
    stream: UnixStream,
    serial: u32,
    /// Signals that arrived while a call waited for its reply.
    queued: std::collections::VecDeque<readers::Message>,
}

impl Bus {
    /// The system bus: UPower and power-profiles-daemon.
    pub fn system() -> Option<Bus> {
        let address = std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
            .unwrap_or_else(|_| "unix:path=/var/run/dbus/system_bus_socket".into());
        Bus::connect(&address)
    }

    /// The session bus: the desktop portals.
    pub fn session() -> Option<Bus> {
        Bus::connect(&std::env::var("DBUS_SESSION_BUS_ADDRESS").ok()?)
    }

    fn connect(address: &str) -> Option<Bus> {
        // The first address a bus names; only `unix:` transports.
        let first = address.split(';').next()?.strip_prefix("unix:")?;
        let key = |k: &str| {
            first
                .split(',')
                .find_map(|kv| kv.strip_prefix(k).and_then(|v| v.strip_prefix('=')))
        };
        let stream = if let Some(path) = key("path") {
            UnixStream::connect(path).ok()?
        } else {
            let name = key("abstract")?;
            use std::os::linux::net::SocketAddrExt;
            let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).ok()?;
            UnixStream::connect_addr(&addr).ok()?
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .ok()?;
        let mut bus = Bus {
            stream,
            serial: 0,
            queued: Default::default(),
        };
        // SASL EXTERNAL with the process's uid, as hex of its decimal digits.
        let uid: String = unsafe { getuid() }
            .to_string()
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect();
        bus.stream
            .write_all(format!("\0AUTH EXTERNAL {uid}\r\n").as_bytes())
            .ok()?;
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        while !line.ends_with(b"\r\n") {
            bus.stream.read_exact(&mut byte).ok()?;
            line.push(byte[0]);
        }
        if !line.starts_with(b"OK ") {
            return None;
        }
        bus.stream.write_all(b"BEGIN\r\n").ok()?;
        bus.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "Hello",
            &[],
        )?;
        Some(bus)
    }

    /// A clone of the socket, which another thread shuts down to end a wait.
    pub fn closer(&self) -> Option<UnixStream> {
        self.stream.try_clone().ok()
    }

    /// Wait for signals with no limit: a watcher's blocking read.
    pub fn wait_forever(&mut self) {
        let _ = self.stream.set_read_timeout(None);
    }

    fn message(&mut self) -> Option<readers::Message> {
        let mut head = [0u8; 16];
        self.stream.read_exact(&mut head).ok()?;
        let len = readers::message_len(&head)?;
        let mut buf = head.to_vec();
        buf.resize(len, 0);
        self.stream.read_exact(&mut buf[16..]).ok()?;
        readers::parse(&buf)
    }

    /// Call `member` with string `args`; its reply's values, or None for an
    /// error reply or no answer.
    pub fn call(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        args: &[&str],
    ) -> Option<Vec<Value>> {
        self.serial += 1;
        let serial = self.serial;
        let call = readers::method_call(serial, destination, path, interface, member, args);
        self.stream.write_all(&call).ok()?;
        loop {
            let m = self.message()?;
            match m.kind {
                readers::SIGNAL => self.queued.push_back(m),
                readers::METHOD_RETURN if m.reply_serial == Some(serial) => {
                    return readers::body_values(&m)
                }
                readers::ERROR if m.reply_serial == Some(serial) => return None,
                _ => {}
            }
        }
    }

    /// A property's value (`org.freedesktop.DBus.Properties.Get`).
    pub fn get(&mut self, destination: &str, path: &str, interface: &str, name: &str) -> Option<Value> {
        let values = self.call(
            destination,
            path,
            "org.freedesktop.DBus.Properties",
            "Get",
            &[interface, name],
        )?;
        values.into_iter().next()
    }

    /// Hear the signals `rule` matches (`AddMatch`).
    pub fn subscribe(&mut self, rule: &str) -> bool {
        self.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "AddMatch",
            &[rule],
        )
        .is_some()
    }

    /// The next signal, or None when the connection ends.
    pub fn signal(&mut self) -> Option<readers::Message> {
        if let Some(m) = self.queued.pop_front() {
            return Some(m);
        }
        loop {
            let m = self.message()?;
            if m.kind == readers::SIGNAL {
                return Some(m);
            }
        }
    }
}

pub const UPOWER: &str = "org.freedesktop.UPower";
pub const DISPLAY_DEVICE: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
pub const PORTAL: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
pub const POWER_PORTAL: &str = "org.freedesktop.portal.PowerProfileMonitor";
pub const NETWORK_PORTAL: &str = "org.freedesktop.portal.NetworkMonitor";
/// power-profiles-daemon's bus names and paths: from 0.20, and before it.
pub const PROFILES: [(&str, &str); 2] = [
    ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles"),
    ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
];

/// The display device's battery from UPower, or None without UPower.
pub fn upower(bus: &mut Bus) -> Option<readers::Battery> {
    let device = "org.freedesktop.UPower.Device";
    let present = matches!(bus.get(UPOWER, DISPLAY_DEVICE, device, "IsPresent")?, Value::Bool(true));
    let percentage = match bus.get(UPOWER, DISPLAY_DEVICE, device, "Percentage") {
        Some(Value::F64(p)) => p,
        _ => 100.0,
    };
    let state = match bus.get(UPOWER, DISPLAY_DEVICE, device, "State") {
        Some(Value::U32(s)) => s,
        _ => 0,
    };
    Some(readers::upower_battery(present, percentage, state))
}

/// The battery: UPower's display device, else the system battery in sysfs,
/// else the specification's no-battery values.
pub fn battery() -> readers::Battery {
    Bus::system()
        .and_then(|mut bus| upower(&mut bus))
        .or_else(|| readers::sysfs_battery(Path::new("/sys/class/power_supply")))
        .unwrap_or(readers::NO_BATTERY)
}

/// Where low power is read (D3's Linux row), and its value now.
pub enum LowPower {
    Portal(bool),
    Profiles(usize, bool),
    Neither,
}

/// The portal's `power-saver-enabled`, else power-profiles-daemon's
/// `ActiveProfile` is `power-saver`.
pub fn low_power() -> LowPower {
    if let Some(Value::Bool(on)) =
        Bus::session().and_then(|mut bus| bus.get(PORTAL, PORTAL_PATH, POWER_PORTAL, "power-saver-enabled"))
    {
        return LowPower::Portal(on);
    }
    if let Some(mut bus) = Bus::system() {
        for (i, (name, path)) in PROFILES.iter().enumerate() {
            if let Some(Value::Str(profile)) = bus.get(name, path, name, "ActiveProfile") {
                return LowPower::Profiles(i, profile == "power-saver");
            }
        }
    }
    LowPower::Neither
}

/// The Network Monitor portal's `GetMetered` and `GetConnectivity` (D9's
/// `linux` words), or None without the portal.
pub fn network_words() -> Option<(bool, &'static str)> {
    let mut bus = Bus::session()?;
    let metered = match bus.call(PORTAL, PORTAL_PATH, NETWORK_PORTAL, "GetMetered", &[])?.first() {
        Some(Value::Bool(m)) => *m,
        _ => return None,
    };
    let connectivity = match bus.call(PORTAL, PORTAL_PATH, NETWORK_PORTAL, "GetConnectivity", &[])?.first() {
        Some(Value::U32(1)) => "local",
        Some(Value::U32(2)) => "limited",
        Some(Value::U32(3)) => "captivePortal",
        _ => "full",
    };
    Some((metered, connectivity))
}
