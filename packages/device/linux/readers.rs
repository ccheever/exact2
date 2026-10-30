//! The Linux readings as pure functions over what the system answered: the
//! power supplies in `/sys/class/power_supply`, a link's `/sys/class/net`
//! entry, rtnetlink's route reply, UPower's display device and the D-Bus
//! wire format these come over (LLP 1133.031 D1–D3). Nothing here opens a
//! socket, so `rustc --test` runs it over recorded answers
//! (`packages/device/test/linux.mjs`).

use std::path::Path;

/// A battery reading: `level` 0 to 1 in hundredths, `charging` false only
/// while discharging (D1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Battery {
    pub level: f64,
    pub charging: bool,
}

/// The specification's no-battery values (D1).
pub const NO_BATTERY: Battery = Battery {
    level: 1.0,
    charging: true,
};

/// A level in hundredths.
pub fn hundredths(level: f64) -> f64 {
    (level.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
}

/// The system battery under `root` (`/sys/class/power_supply`): a supply of
/// `type` `Battery` whose `scope` is not `Device` (a mouse's or a headset's
/// battery is). None when there is none.
pub fn sysfs_battery(root: &Path) -> Option<Battery> {
    let mut names: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for supply in names {
        if read(&supply.join("type")).as_deref() != Some("Battery")
            || read(&supply.join("scope")).as_deref() == Some("Device")
            || read(&supply.join("present")).as_deref() == Some("0")
        {
            continue;
        }
        let level = read(&supply.join("capacity"))
            .and_then(|c| c.parse::<f64>().ok())
            .map(|c| c / 100.0)
            .or_else(|| {
                let now = read(&supply.join("energy_now")).or_else(|| read(&supply.join("charge_now")))?;
                let full = read(&supply.join("energy_full")).or_else(|| read(&supply.join("charge_full")))?;
                let (now, full) = (now.parse::<f64>().ok()?, full.parse::<f64>().ok()?);
                (full > 0.0).then(|| now / full)
            })?;
        let charging = read(&supply.join("status")).as_deref() != Some("Discharging");
        return Some(Battery {
            level: hundredths(level),
            charging,
        });
    }
    None
}

/// UPower's display device: `IsPresent`, `Percentage` and `State` (D1's
/// Linux row). `Charging`, `FullyCharged`, `PendingCharge` and `Unknown`
/// are charging; `Discharging`, `Empty` and `PendingDischarge` are not.
pub fn upower_battery(present: bool, percentage: f64, state: u32) -> Battery {
    if !present {
        return NO_BATTERY;
    }
    Battery {
        level: hundredths(percentage / 100.0),
        charging: !matches!(state, 2 | 3 | 6),
    }
}

/// The `type` of the link `name` from its `/sys/class/net/<name>` entry
/// under `root` (D2's Linux row): a tunnel first, whose underlying link the
/// kernel does not name, then Wi-Fi, a WWAN modem, Ethernet, else other.
pub fn link_type(root: &Path, name: &str) -> &'static str {
    let dir = root.join(name);
    let uevent = read(&dir.join("uevent")).unwrap_or_default();
    let devtype = uevent
        .lines()
        .find_map(|l| l.strip_prefix("DEVTYPE="))
        .unwrap_or("");
    if dir.join("tun_flags").exists() || devtype == "wireguard" {
        return "other";
    }
    if dir.join("phy80211").exists() || devtype == "wlan" {
        return "wifi";
    }
    if devtype == "wwan" {
        return "cellular";
    }
    if read(&dir.join("type")).as_deref() == Some("1") {
        return "ethernet";
    }
    "other"
}

// rtnetlink (rtnetlink(7)): an RTM_GETROUTE request for one destination, as
// `ip route get` asks, and the output interface of its reply.

pub const RTM_NEWROUTE: u16 = 24;
pub const RTM_GETROUTE: u16 = 26;
const NLM_F_REQUEST: u16 = 1;
const NLMSG_ERROR: u16 = 2;
const RTA_DST: u16 = 1;
const RTA_OIF: u16 = 4;
const RTM_F_LOOKUP_TABLE: u32 = 0x1000;

/// An RTM_GETROUTE request for `dst` (4 or 16 bytes), with sequence `seq`.
pub fn route_request(dst: &[u8], seq: u32) -> Vec<u8> {
    let family: u8 = if dst.len() == 4 { 2 } else { 10 };
    let attr_len = 4 + dst.len();
    let len = 16 + 12 + attr_len;
    let mut m = Vec::with_capacity(len);
    // nlmsghdr: len, type, flags, seq, pid
    m.extend_from_slice(&(len as u32).to_ne_bytes());
    m.extend_from_slice(&RTM_GETROUTE.to_ne_bytes());
    m.extend_from_slice(&NLM_F_REQUEST.to_ne_bytes());
    m.extend_from_slice(&seq.to_ne_bytes());
    m.extend_from_slice(&0u32.to_ne_bytes());
    // rtmsg: family, dst_len, src_len, tos, table, protocol, scope, type, flags
    m.extend_from_slice(&[family, (dst.len() * 8) as u8, 0, 0, 0, 0, 0, 0]);
    m.extend_from_slice(&RTM_F_LOOKUP_TABLE.to_ne_bytes());
    // rtattr RTA_DST
    m.extend_from_slice(&(attr_len as u16).to_ne_bytes());
    m.extend_from_slice(&RTA_DST.to_ne_bytes());
    m.extend_from_slice(dst);
    m
}

/// The output interface index of the route reply in `buf`, or None for an
/// error reply (no route: `ENETUNREACH`) or no such attribute.
pub fn route_oif(buf: &[u8]) -> Option<u32> {
    let u16_at = |i: usize| buf.get(i..i + 2).map(|b| u16::from_ne_bytes([b[0], b[1]]));
    let u32_at =
        |i: usize| buf.get(i..i + 4).map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]));
    let mut at = 0;
    while at + 16 <= buf.len() {
        let len = u32_at(at)? as usize;
        let kind = u16_at(at + 4)?;
        if len < 16 || at + len > buf.len() {
            return None;
        }
        if kind == NLMSG_ERROR {
            return None;
        }
        if kind == RTM_NEWROUTE {
            let mut a = at + 16 + 12;
            while a + 4 <= at + len {
                let alen = u16_at(a)? as usize;
                let akind = u16_at(a + 2)?;
                if alen < 4 {
                    return None;
                }
                if akind == RTA_OIF {
                    return u32_at(a + 4);
                }
                a += (alen + 3) & !3;
            }
            return None;
        }
        at += (len + 3) & !3;
    }
    None
}

// D-Bus (the D-Bus Specification, "Message Protocol"): enough of the wire
// format to call a method and read a property: little-endian messages,
// strings, object paths, signatures and variants of a basic type.

/// A value a reply carried.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    U32(u32),
    I32(i32),
    F64(f64),
    Str(String),
}

fn pad(buf: &mut Vec<u8>, to: usize) {
    while buf.len() % to != 0 {
        buf.push(0);
    }
}

fn put_str(buf: &mut Vec<u8>, s: &str) {
    pad(buf, 4);
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

fn put_sig(buf: &mut Vec<u8>, s: &str) {
    buf.push(s.len() as u8);
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

/// A method call: `destination`, `path`, `interface`, `member`, and its
/// string arguments (signature `s…`), with serial `serial`.
pub fn method_call(
    serial: u32,
    destination: &str,
    path: &str,
    interface: &str,
    member: &str,
    args: &[&str],
) -> Vec<u8> {
    let mut body = Vec::new();
    for a in args {
        put_str(&mut body, a);
    }
    let mut fields = Vec::new();
    let mut field = |code: u8, sig: &str, value: &str| {
        pad(&mut fields, 8);
        fields.push(code);
        put_sig(&mut fields, sig);
        if sig == "g" {
            put_sig(&mut fields, value);
        } else {
            put_str(&mut fields, value);
        }
    };
    field(1, "o", path);
    field(2, "s", interface);
    field(3, "s", member);
    field(6, "s", destination);
    if !args.is_empty() {
        field(8, "g", &"s".repeat(args.len()));
    }
    let mut m = vec![b'l', 1, 0, 1];
    m.extend_from_slice(&(body.len() as u32).to_le_bytes());
    m.extend_from_slice(&serial.to_le_bytes());
    m.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    m.extend_from_slice(&fields);
    pad(&mut m, 8);
    m.extend_from_slice(&body);
    m
}

/// A message read from the bus: its type, the serial it replies to, its
/// member and path (a signal's), its signature and its body.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Message {
    pub kind: u8,
    pub reply_serial: Option<u32>,
    pub member: String,
    pub path: String,
    pub interface: String,
    pub signature: String,
    pub body: Vec<u8>,
}

/// The length of the message starting `head` (its first 16 bytes), or None
/// for a message this reader does not take (big-endian).
pub fn message_len(head: &[u8; 16]) -> Option<usize> {
    if head[0] != b'l' {
        return None;
    }
    let body = u32::from_le_bytes(head[4..8].try_into().ok()?) as usize;
    let fields = u32::from_le_bytes(head[12..16].try_into().ok()?) as usize;
    Some((16 + fields).div_ceil(8) * 8 + body)
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn align(&mut self, to: usize) {
        self.at = self.at.div_ceil(to) * to;
    }
    fn u8(&mut self) -> Option<u8> {
        let v = *self.buf.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    fn u32(&mut self) -> Option<u32> {
        self.align(4);
        let v = u32::from_le_bytes(self.buf.get(self.at..self.at + 4)?.try_into().ok()?);
        self.at += 4;
        Some(v)
    }
    fn f64(&mut self) -> Option<f64> {
        self.align(8);
        let v = f64::from_le_bytes(self.buf.get(self.at..self.at + 8)?.try_into().ok()?);
        self.at += 8;
        Some(v)
    }
    fn str(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        let s = std::str::from_utf8(self.buf.get(self.at..self.at + len)?).ok()?;
        self.at += len + 1;
        Some(s.to_owned())
    }
    fn sig(&mut self) -> Option<String> {
        let len = self.u8()? as usize;
        let s = std::str::from_utf8(self.buf.get(self.at..self.at + len)?).ok()?;
        self.at += len + 1;
        Some(s.to_owned())
    }
    /// A value of the one-character basic type `sig`.
    fn value(&mut self, sig: &str) -> Option<Value> {
        Some(match sig {
            "b" => Value::Bool(self.u32()? != 0),
            "u" => Value::U32(self.u32()?),
            "i" => Value::I32(self.u32()? as i32),
            "d" => Value::F64(self.f64()?),
            "s" | "o" => Value::Str(self.str()?),
            "g" => Value::Str(self.sig()?),
            "y" => Value::U32(self.u8()? as u32),
            _ => return None,
        })
    }
    fn variant(&mut self) -> Option<Value> {
        let sig = self.sig()?;
        self.value(&sig)
    }
}

/// The message in `buf`, a whole one as [`message_len`] measured it.
pub fn parse(buf: &[u8]) -> Option<Message> {
    let head: &[u8; 16] = buf.get(..16)?.try_into().ok()?;
    let total = message_len(head)?;
    let body_len = u32::from_le_bytes(head[4..8].try_into().ok()?) as usize;
    let fields_end = 16 + u32::from_le_bytes(head[12..16].try_into().ok()?) as usize;
    let mut m = Message {
        kind: head[1],
        ..Message::default()
    };
    let mut r = Reader { buf, at: 16 };
    while r.at < fields_end {
        r.align(8);
        if r.at >= fields_end {
            break;
        }
        let code = r.u8()?;
        let value = r.variant()?;
        match (code, value) {
            (1, Value::Str(s)) => m.path = s,
            (2, Value::Str(s)) => m.interface = s,
            (3, Value::Str(s)) => m.member = s,
            (5, Value::U32(n)) => m.reply_serial = Some(n),
            (8, Value::Str(s)) => m.signature = s,
            _ => {}
        }
    }
    m.body = buf.get(total - body_len..total)?.to_vec();
    Some(m)
}

/// The values of a reply's body whose signature is `v` or a run of basic
/// types (`b`, `u`, `s`, …).
pub fn body_values(m: &Message) -> Option<Vec<Value>> {
    let mut r = Reader {
        buf: &m.body,
        at: 0,
    };
    let mut out = Vec::new();
    for c in m.signature.chars() {
        out.push(if c == 'v' {
            r.variant()?
        } else {
            r.value(&c.to_string())?
        });
    }
    Some(out)
}

/// A signal's or a reply's message type.
pub const METHOD_RETURN: u8 = 2;
pub const ERROR: u8 = 3;
pub const SIGNAL: u8 = 4;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A tree of files under a new directory: `(relative path, contents)`.
    fn tree(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("exact-device-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        root
    }

    #[test]
    fn a_laptops_battery_a_desktop_with_none_and_a_mouses_are_told_apart() {
        let laptop = tree(
            "laptop",
            &[
                ("AC/type", "Mains\n"),
                ("AC/online", "1\n"),
                ("BAT0/type", "Battery\n"),
                ("BAT0/scope", "System\n"),
                ("BAT0/capacity", "42\n"),
                ("BAT0/status", "Not charging\n"),
            ],
        );
        assert_eq!(
            sysfs_battery(&laptop),
            Some(Battery {
                level: 0.42,
                charging: true
            }),
            "a battery held at a limit is charging"
        );
        fs::write(laptop.join("BAT0/status"), "Discharging\n").unwrap();
        assert_eq!(sysfs_battery(&laptop).map(|b| b.charging), Some(false));
        let desktop = tree("desktop", &[("AC/type", "Mains\n"), ("AC/online", "1\n")]);
        assert_eq!(sysfs_battery(&desktop), None);
        let mouse = tree(
            "mouse",
            &[
                ("hidpp_battery_0/type", "Battery\n"),
                ("hidpp_battery_0/scope", "Device\n"),
                ("hidpp_battery_0/capacity", "80\n"),
            ],
        );
        assert_eq!(sysfs_battery(&mouse), None, "a mouse's battery is not the system's");
        let energy = tree(
            "energy",
            &[
                ("BAT1/type", "Battery\n"),
                ("BAT1/energy_now", "25004\n"),
                ("BAT1/energy_full", "50000\n"),
                ("BAT1/status", "Charging\n"),
            ],
        );
        assert_eq!(sysfs_battery(&energy).map(|b| b.level), Some(0.5));
        assert_eq!(sysfs_battery(Path::new("/no/such/sys")), None);
        for root in [laptop, desktop, mouse, energy] {
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn upowers_states_map_one_by_one() {
        let charging: Vec<bool> = (0..=6).map(|s| upower_battery(true, 50.0, s).charging).collect();
        assert_eq!(charging, [true, true, false, false, true, true, false]);
        assert_eq!(upower_battery(true, 87.456, 2).level, 0.87);
        assert_eq!(upower_battery(false, 0.0, 0), NO_BATTERY);
    }

    #[test]
    fn links_are_classified_tunnel_first() {
        let net = tree(
            "net",
            &[
                ("wlan0/uevent", "DEVTYPE=wlan\nINTERFACE=wlan0\n"),
                ("wlan0/phy80211/name", "phy0\n"),
                ("wlan0/type", "1\n"),
                ("eth0/uevent", "INTERFACE=eth0\n"),
                ("eth0/type", "1\n"),
                ("wg0/uevent", "DEVTYPE=wireguard\nINTERFACE=wg0\n"),
                ("wg0/type", "65534\n"),
                ("tun0/tun_flags", "0x1001\n"),
                ("tun0/type", "65534\n"),
                ("tap0/tun_flags", "0x1002\n"),
                ("tap0/type", "1\n"),
                ("wwan0/uevent", "DEVTYPE=wwan\n"),
                ("wwan0/type", "65534\n"),
            ],
        );
        let kinds: Vec<_> = ["wlan0", "eth0", "wg0", "tun0", "tap0", "wwan0", "lo"]
            .iter()
            .map(|n| link_type(&net, n))
            .collect();
        assert_eq!(kinds, ["wifi", "ethernet", "other", "other", "other", "cellular", "other"]);
        let _ = fs::remove_dir_all(net);
    }

    /// A route reply as the kernel writes it: nlmsghdr, rtmsg, then RTA_TABLE and RTA_OIF.
    fn reply(family: u8, oif: u32) -> Vec<u8> {
        let mut m = Vec::new();
        let len: u32 = 16 + 12 + 8 + 8;
        m.extend_from_slice(&len.to_ne_bytes());
        m.extend_from_slice(&RTM_NEWROUTE.to_ne_bytes());
        m.extend_from_slice(&0u16.to_ne_bytes());
        m.extend_from_slice(&7u32.to_ne_bytes());
        m.extend_from_slice(&0u32.to_ne_bytes());
        m.extend_from_slice(&[family, 128, 0, 0, 254, 0, 0, 1]);
        m.extend_from_slice(&0u32.to_ne_bytes());
        m.extend_from_slice(&8u16.to_ne_bytes());
        m.extend_from_slice(&15u16.to_ne_bytes()); // RTA_TABLE
        m.extend_from_slice(&51820u32.to_ne_bytes()); // wg-quick's table
        m.extend_from_slice(&8u16.to_ne_bytes());
        m.extend_from_slice(&RTA_OIF.to_ne_bytes());
        m.extend_from_slice(&oif.to_ne_bytes());
        m
    }

    #[test]
    fn a_route_request_is_well_formed_and_its_reply_names_the_interface() {
        let v6 = [0x20, 0x01, 0x48, 0x60, 0x48, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0x88];
        let request = route_request(&v6, 7);
        assert_eq!(request.len(), 16 + 12 + 20);
        assert_eq!(u32::from_ne_bytes(request[..4].try_into().unwrap()) as usize, request.len());
        assert_eq!(request[16], 10, "AF_INET6");
        assert_eq!(request[17], 128, "a host route's prefix");
        // An IPv6-only Wi-Fi link, and a WireGuard default route in its own table.
        assert_eq!(route_oif(&reply(10, 3)), Some(3));
        assert_eq!(route_oif(&reply(2, 9)), Some(9));
        // No route: an NLMSG_ERROR reply carrying -ENETUNREACH.
        let mut error = Vec::new();
        error.extend_from_slice(&36u32.to_ne_bytes());
        error.extend_from_slice(&NLMSG_ERROR.to_ne_bytes());
        error.extend_from_slice(&[0; 10]);
        error.extend_from_slice(&(-101i32).to_ne_bytes());
        error.extend_from_slice(&request[..16]);
        assert_eq!(route_oif(&error), None);
    }

    #[test]
    fn a_property_call_is_framed_and_its_variant_reply_read() {
        let call = method_call(
            3,
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower/devices/DisplayDevice",
            "org.freedesktop.DBus.Properties",
            "Get",
            &["org.freedesktop.UPower.Device", "Percentage"],
        );
        let head: [u8; 16] = call[..16].try_into().unwrap();
        assert_eq!(message_len(&head), Some(call.len()));
        let parsed = parse(&call).unwrap();
        assert_eq!(parsed.member, "Get");
        assert_eq!(parsed.signature, "ss");
        assert_eq!(
            body_values(&parsed).unwrap(),
            [
                Value::Str("org.freedesktop.UPower.Device".into()),
                Value::Str("Percentage".into())
            ]
        );
        // The reply: a method return to serial 3 whose body is the variant `d` 87.5.
        let mut fields = Vec::new();
        fields.extend_from_slice(&[5, 1, b'u', 0]);
        fields.extend_from_slice(&3u32.to_le_bytes());
        fields.extend_from_slice(&[8, 1, b'g', 0, 1, b'v', 0]);
        let mut body = vec![1, b'd', 0, 0, 0, 0, 0, 0];
        body.extend_from_slice(&87.5f64.to_le_bytes());
        let mut m = vec![b'l', METHOD_RETURN, 1, 1];
        m.extend_from_slice(&(body.len() as u32).to_le_bytes());
        m.extend_from_slice(&9u32.to_le_bytes());
        m.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        m.extend_from_slice(&fields);
        while m.len() % 8 != 0 {
            m.push(0);
        }
        m.extend_from_slice(&body);
        let reply = parse(&m).unwrap();
        assert_eq!((reply.kind, reply.reply_serial), (METHOD_RETURN, Some(3)));
        assert_eq!(body_values(&reply).unwrap(), [Value::F64(87.5)]);
    }
}
