//! LLP 1133.036 D6 at every fetch hop and WebSocket open this executor makes:
//! a reach asked by a tool invocation, a wake or a package's event, whose
//! destination could show the Local Network prompt, is checked against the
//! host's local-network fact before it connects ([`exact_runner::local_network`]).
//!
//! The test of the destination is the host's. Under the agent it is the
//! grant's static test, since the substitute reads no interface (D19). In
//! production on Apple it is TN3179's: a `.local` name; a link-local,
//! multicast or broadcast address; an address on the subnet of an interface
//! that can broadcast (Wi-Fi, Ethernet; not a VPN's point-to-point tunnel or
//! cellular); a unicast name that resolves to one of those. A name is
//! resolved here for the test and again by the transport when it connects,
//! and an answer that changes between the two escapes the test, a declared
//! limit (D6).

use exact_runner::local_network::{self, Reach};
use exact_runner::CallOrigin;
use ibex2::boundary::{HopGate, HostError};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;

struct Gate(Reach);

impl HopGate for Gate {
    fn admit(&self, url: &url::Url) -> Result<(), HostError> {
        let risk = local_network::prompts() && url.host().is_some_and(|h| at_risk(&h, self.0));
        match self.0.refusal(risk) {
            None => Ok(()),
            Some(reason) => Err(HostError::NotAllowed {
                reason: reason.into(),
            }),
        }
    }
}

/// `fetch`, checked at every hop for `reach`: unchanged for a person's or
/// the app's own reach, which the rule never refuses.
pub(super) fn gate_fetch(fetch: ibex2::host::Fetch, reach: Reach) -> ibex2::host::Fetch {
    match reach.origin {
        CallOrigin::App => fetch,
        _ => fetch.with_gate(Arc::new(Gate(reach))),
    }
}

/// `bindings`, their fetch and WebSocket checked for `reach`.
pub(super) fn gate_bindings(
    mut bindings: ibex2::host::Bindings,
    reach: Reach,
) -> ibex2::host::Bindings {
    if reach.origin != CallOrigin::App {
        let gate: Arc<dyn HopGate> = Arc::new(Gate(reach));
        bindings.fetch = bindings.fetch.with_gate(gate.clone());
        bindings.websocket = bindings.websocket.with_gate(gate);
    }
    bindings
}

fn at_risk(host: &url::Host<&str>, reach: Reach) -> bool {
    if reach.substitute.is_some() {
        return Reach::static_risk(&host.to_string());
    }
    match host {
        url::Host::Ipv4(a) => address_risk(IpAddr::V4(*a)),
        url::Host::Ipv6(a) => address_risk(IpAddr::V6(*a)),
        url::Host::Domain(name) => {
            let name = name.strip_suffix('.').unwrap_or(name);
            name.ends_with(".local")
                || (name, 0)
                    .to_socket_addrs()
                    .is_ok_and(|mut found| found.any(|a| address_risk(a.ip())))
        }
    }
}

/// Whether reaching `address` can need local network access (TN3179).
fn address_risk(address: IpAddr) -> bool {
    let address = match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(address, IpAddr::V4),
        v4 => v4,
    };
    match address {
        _ if address.is_loopback() || address.is_unspecified() => false,
        IpAddr::V4(a) if a.is_link_local() || a.is_multicast() || a.is_broadcast() => true,
        IpAddr::V6(a) if a.is_multicast() || (a.segments()[0] & 0xffc0) == 0xfe80 => true,
        _ => interfaces::on_broadcast_subnet(address),
    }
}

#[cfg(target_vendor = "apple")]
#[allow(unsafe_code)] // getifaddrs(3), whose list is read and freed here.
mod interfaces {
    //! The device's broadcast-capable interfaces, by `getifaddrs(3)`.
    use super::IpAddr;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::os::raw::{c_char, c_int, c_uint, c_void};

    #[repr(C)]
    struct IfAddrs {
        next: *mut IfAddrs,
        name: *mut c_char,
        flags: c_uint,
        addr: *mut SockAddr,
        netmask: *mut SockAddr,
        dstaddr: *mut SockAddr,
        data: *mut c_void,
    }

    /// `sockaddr_in` and `sockaddr_in6` share this head on Apple platforms.
    #[repr(C)]
    struct SockAddr {
        len: u8,
        family: u8,
    }

    extern "C" {
        fn getifaddrs(out: *mut *mut IfAddrs) -> c_int;
        fn freeifaddrs(list: *mut IfAddrs);
    }

    const AF_INET: u8 = 2;
    const AF_INET6: u8 = 30;
    const IFF_UP: c_uint = 0x1;
    const IFF_BROADCAST: c_uint = 0x2;
    const IFF_LOOPBACK: c_uint = 0x8;
    const IFF_POINTOPOINT: c_uint = 0x10;

    /// The address `sa` holds, by its family: `sin_addr` at byte 4,
    /// `sin6_addr` at byte 8.
    ///
    /// # Safety
    /// `sa` is null or points at a `sockaddr` of at least its `sa_len` bytes.
    unsafe fn address(sa: *const SockAddr) -> Option<IpAddr> {
        let sa = sa.as_ref()?;
        let bytes = sa as *const SockAddr as *const u8;
        match sa.family {
            AF_INET => Some(IpAddr::V4(Ipv4Addr::from(std::ptr::read_unaligned(
                bytes.add(4) as *const [u8; 4],
            )))),
            AF_INET6 => Some(IpAddr::V6(Ipv6Addr::from(std::ptr::read_unaligned(
                bytes.add(8) as *const [u8; 16],
            )))),
            _ => None,
        }
    }

    fn masked(a: IpAddr, mask: IpAddr) -> Option<u128> {
        match (a, mask) {
            (IpAddr::V4(a), IpAddr::V4(m)) => Some((u32::from(a) & u32::from(m)) as u128),
            (IpAddr::V6(a), IpAddr::V6(m)) => Some(u128::from(a) & u128::from(m)),
            _ => None,
        }
    }

    /// Whether `target` is on the subnet of an interface that is up and can
    /// broadcast, the subnet's broadcast address included.
    pub(super) fn on_broadcast_subnet(target: IpAddr) -> bool {
        let mut list = std::ptr::null_mut();
        // SAFETY: getifaddrs fills `list` on success, and it is freed once.
        if unsafe { getifaddrs(&mut list) } != 0 {
            return false;
        }
        let mut found = false;
        let mut at = list;
        while let Some(entry) = unsafe { at.as_ref() } {
            let wanted = IFF_UP | IFF_BROADCAST;
            let flags = entry.flags;
            if flags & wanted == wanted && flags & (IFF_LOOPBACK | IFF_POINTOPOINT) == 0 {
                // SAFETY: the kernel's entries hold sockaddrs or null.
                let (addr, mask) = unsafe { (address(entry.addr), address(entry.netmask)) };
                if let (Some(addr), Some(mask)) = (addr, mask) {
                    let (subnet, theirs) = (masked(addr, mask), masked(target, mask));
                    found |= subnet.is_some() && subnet == theirs;
                }
            }
            at = entry.next;
        }
        // SAFETY: `list` came from getifaddrs.
        unsafe { freeifaddrs(list) };
        found
    }
}

#[cfg(not(target_vendor = "apple"))]
mod interfaces {
    /// No target but Apple's prompts, so no other host asks.
    pub(super) fn on_broadcast_subnet(_: super::IpAddr) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_local_multicast_and_loopback_addresses_are_judged_without_interfaces() {
        assert!(address_risk("169.254.3.4".parse().unwrap()));
        assert!(address_risk("224.0.0.251".parse().unwrap()));
        assert!(address_risk("255.255.255.255".parse().unwrap()));
        assert!(address_risk("fe80::1".parse().unwrap()));
        assert!(address_risk("::ffff:169.254.3.4".parse().unwrap()));
        assert!(!address_risk("127.0.0.1".parse().unwrap()));
        assert!(!address_risk("::1".parse().unwrap()));
    }

    #[test]
    fn under_the_agent_the_static_test_decides() {
        let reach = Reach {
            origin: CallOrigin::Tool,
            substitute: Some(local_network::State::Prompt),
        };
        let host = |s: &str| url::Url::parse(s).unwrap();
        let risk = |s: &str| at_risk(&host(s).host().unwrap(), reach);
        assert!(risk("http://192.168.1.40/"));
        assert!(risk("http://wled-a1b2c3.local/"));
        assert!(!risk("http://127.0.0.1:4790/"));
        assert!(!risk("https://example.test/"));
    }
}

#[cfg(test)]
#[path = "executor_reach_tests.rs"]
mod executor_tests;
