//! The local network as a grant names it (LLP 1133.036 D5): decided from a
//! URL's host as written, never from where a name resolves.

use std::net::{Ipv4Addr, Ipv6Addr};

/// The target word of `net.fetch local-network` and `net.websocket
/// local-network`.
pub const LOCAL_NETWORK: &str = "local-network";

/// The word a development build's bake writes after [`LOCAL_NETWORK`]: the
/// line then admits loopback too, so a drive's fixture on this machine stands
/// in for a device on the LAN. A production bake refuses it.
pub const DEVELOPMENT: &str = "development";

/// Whether `host`, as a URL serializes it (lowercase, an IPv6 address in
/// brackets), is on the local network: an RFC 1918, link-local IPv4,
/// unique-local IPv6 or link-local IPv6 literal, an IPv4-mapped IPv6 address
/// judged by the IPv4 address it carries, or a name ending in `.local`
/// (RFC 6762 §3). With `loopback`, also 127.0.0.0/8, `[::1]` and `localhost`.
pub fn is_local_network(host: &str, loopback: bool) -> bool {
    let host = host.to_ascii_lowercase();
    if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inner
            .parse::<Ipv6Addr>()
            .is_ok_and(|address| local_v6(address, loopback));
    }
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        return local_v4(address, loopback);
    }
    let name = host.strip_suffix('.').unwrap_or(&host);
    name.ends_with(".local") && name.len() > ".local".len() || loopback && name == "localhost"
}

/// Whether `kind` is a DNS-SD service type `net.dns-sd` may name (LLP
/// 1133.036 D1): `_<name>._tcp` or `_<name>._udp`, the name 1 to 15
/// letters, digits and hyphens with at least one letter and no leading,
/// trailing or doubled hyphen (RFC 6335 §5.1; RFC 6763 §7).
pub fn is_service_type(kind: &str) -> bool {
    let Some(name) = (kind
        .strip_suffix("._tcp")
        .or_else(|| kind.strip_suffix("._udp")))
    .and_then(|rest| rest.strip_prefix('_')) else {
        return false;
    };
    (1..=15).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && name.bytes().any(|b| b.is_ascii_alphabetic())
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

fn local_v4(address: Ipv4Addr, loopback: bool) -> bool {
    address.is_private() || address.is_link_local() || loopback && address.is_loopback()
}

fn local_v6(address: Ipv6Addr, loopback: bool) -> bool {
    if let Some(v4) = address.to_ipv4_mapped() {
        return local_v4(v4, loopback);
    }
    let first = address.segments()[0];
    (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80 || loopback && address.is_loopback()
}

#[cfg(test)]
mod tests {
    use super::{is_local_network, is_service_type};

    #[test]
    fn a_service_type_is_rfc_6335_s_name_and_a_transport() {
        for good in [
            "_wled._tcp",
            "_http._tcp",
            "_exactlntest._tcp",
            "_a-b._udp",
            "_x1._tcp",
        ] {
            assert!(is_service_type(good), "{good}");
        }
        for bad in [
            "_wled._sctp",
            "wled._tcp",
            "_._tcp",
            "_abcdefghijklmnop._tcp",
            "_-a._tcp",
            "_a-._tcp",
            "_a--b._tcp",
            "_123._tcp",
            "_wled._tcp.local",
            "_we_ld._tcp",
        ] {
            assert!(!is_service_type(bad), "{bad}");
        }
    }

    #[test]
    fn the_local_network_is_its_private_ranges_and_local_names() {
        for host in [
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.40",
            "169.254.3.4",
            "[fd12:3456::1]",
            "[fc00::1]",
            "[fe80::1]",
            "[::ffff:192.168.1.40]",
            "wled-a1b2c3.local",
            "Desk.LOCAL",
            "nas.local.",
        ] {
            assert!(is_local_network(host, false), "{host}");
        }
        for host in [
            "172.32.0.1",
            "8.8.8.8",
            "100.64.0.1",
            "[2001:db8::1]",
            "[::ffff:8.8.8.8]",
            "nas.home.arpa",
            "local",
            ".local",
            "example.localhost",
            "127.0.0.1",
            "[::1]",
            "localhost",
            "[::ffff:127.0.0.1]",
        ] {
            assert!(!is_local_network(host, false), "{host}");
        }
    }

    #[test]
    fn a_development_build_adds_loopback() {
        for host in [
            "127.0.0.1",
            "127.9.8.7",
            "[::1]",
            "localhost",
            "[::ffff:127.0.0.1]",
        ] {
            assert!(is_local_network(host, true), "{host}");
        }
        assert!(!is_local_network("8.8.8.8", true));
    }
}
