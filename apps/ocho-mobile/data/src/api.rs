//! Fleet's mobile API as the phone reaches it: the pairing a desktop shows
//! under Pair Phone, and the relay URLs every request goes to.
//!
//! A machine running `fleet serve` dials out to the relay; the phone calls
//! `RELAY/m/<machine>/api/…` with the fleet token as a bearer. Every enrolled
//! machine runs the same server with the same token, so when the Mac is
//! asleep the phone asks a peer instead (`model.rs`).

use serde::{Deserialize, Serialize};

/// The relay the grants admit; a pairing naming another is refused.
pub const RELAY: &str = "https://fleet-relay.fly.dev";

/// The long poll's hold, under the relay's 90 s and the server's 30 s cap.
pub const WAIT: &str = "25s";

/// The largest transcript answer read. The host's independent lane holds
/// 32 MiB and charges twice each ceiling: a transcript (12), the poll (4) and
/// a send (8) fit side by side, so none waits on another's long poll.
pub const MAX_BYTES: u32 = 6 << 20;
/// The largest `/api/fleet` answer: the phone's view is ~20 KB, and even a
/// server without it answered 800 KB here.
pub const POLL_BYTES: u32 = 2 << 20;
/// The largest answer to a send: a Codex `codex-client` reply carries the
/// thread's turns with their activity, well past 64 KB once a thread has a
/// history (a send failed "response exceeded the 65536-byte limit").
pub const SEND_BYTES: u32 = 4 << 20;

/// What the phone keeps: `fleet serve --json`'s `{relay, machine, token,
/// name}`. `machine` is the desktop's own machine: home.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    /// The relay's origin.
    pub relay: String,
    /// The paired machine's id.
    pub machine: String,
    /// The fleet token, the same on every peer.
    pub token: String,
    /// The paired machine's name.
    #[serde(default)]
    pub name: String,
    /// Where the paired machine answers on the relay when it connects with
    /// its own key (`k` and 32 hex digits); empty means at its machine id.
    #[serde(default)]
    pub route: String,
}

impl Connection {
    /// The pairing text a desktop shows: the QR's JSON, or the whole
    /// `fleet serve --json` / `--describe` object, which carries the same
    /// fields. Whitespace a paste picked up is tolerated.
    pub fn parse(text: &str) -> Result<Connection, String> {
        let decoded = pairing_json(text.trim());
        let text = decoded.as_str();
        if text.is_empty() {
            return Err("Paste the pairing text from Ocho's Pair Phone.".into());
        }
        let mut c: Connection = serde_json::from_str(text)
            .map_err(|_| "That isn't Ocho pairing text. Copy it from Pair Phone on the desktop.")?;
        c.relay = c.relay.trim().trim_end_matches('/').to_string();
        if c.relay.is_empty() {
            c.relay = RELAY.into();
        }
        if c.relay != RELAY {
            return Err(format!(
                "This app reaches {RELAY}; the pairing names {}.",
                c.relay
            ));
        }
        if c.machine.trim().is_empty() || c.token.trim().is_empty() {
            return Err("The pairing text has no machine or token.".into());
        }
        if !c.route.is_empty() && !is_route(&c.route) {
            return Err("The pairing text names a relay route this app can't use.".into());
        }
        if c.name.is_empty() {
            c.name = c.machine.chars().take(8).collect();
        }
        Ok(c)
    }

    /// `RELAY/m/<via>/api<path>`: `via` is whichever server answers now.
    pub fn url(&self, via: &str, path: &str) -> String {
        format!("{}/m/{}/api{path}", self.relay, encode(via))
    }

    /// The long poll: answers at once when `since` is behind or `instance`
    /// is another server's, else when something changes.
    /// The phone's view (`view=phone`, Fleet's slim answer): a server that
    /// has it holds the poll until the view's `tag` changes; one that does
    /// not answers in full by `since`, as before.
    pub fn fleet_url(&self, via: &str, since: u64, instance: &str, tag: &str) -> String {
        let mut path = format!("/fleet?view=phone&wait={WAIT}&since={since}");
        if !instance.is_empty() {
            path.push_str(&format!("&instance={}", encode(instance)));
        }
        if !tag.is_empty() {
            path.push_str(&format!("&tag={}", encode(tag)));
        }
        self.url(via, &path)
    }

    /// A session's route on the server answering now.
    pub fn session_url(&self, via: &str, machine: &str, session: &str, leaf: &str) -> String {
        self.url(
            via,
            &format!(
                "/machines/{}/sessions/{}/{leaf}",
                encode(machine),
                encode(session)
            ),
        )
    }

    /// The bearer header's value.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

/// The pairing JSON inside what was scanned or pasted: the JSON itself, the
/// QR's link (`https://RELAY/pair#<base64url>`), or the app link the relay's
/// pairing page opens (`ocho://pair/<base64url>`, a location `/pair/<…>`).
pub fn pairing_json(text: &str) -> String {
    let b64 = text
        .split_once("/pair#")
        .filter(|(origin, _)| origin.starts_with("http://") || origin.starts_with("https://"))
        .map(|(_, b64)| b64)
        .or_else(|| text.strip_prefix("ocho://pair/"))
        .or_else(|| text.strip_prefix("/pair/"));
    b64.and_then(base64url)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| text.to_string())
}

/// RFC 4648 base64url, padding optional; `None` for anything else.
fn base64url(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// A key-derived relay address: `k` and 32 lowercase hex digits.
pub fn is_route(s: &str) -> bool {
    s.len() == 33
        && s.starts_with('k')
        && s[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A path segment, percent-encoded (ids are hex and dashes; names are not).
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A route segment as the router hands it back.
pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_reads_the_qr_and_the_describe_object() {
        let qr = r#"{"relay":"https://fleet-relay.fly.dev/","machine":"c494","token":"abc","name":"Mac"}"#;
        let c = Connection::parse(qr).unwrap();
        assert_eq!(c.relay, RELAY);
        assert_eq!(
            c.url("c494", "/health"),
            "https://fleet-relay.fly.dev/m/c494/api/health"
        );
        let describe = r#" {"relay":"https://fleet-relay.fly.dev","machine":"m","name":"Mac","token":"t","payload":"…","qr":["01"]} "#;
        assert_eq!(Connection::parse(describe).unwrap().token, "t");
    }

    #[test]
    fn pairing_reads_the_qr_link_and_the_app_link() {
        let json =
            r#"{"relay":"https://fleet-relay.fly.dev","machine":"mac","token":"t","name":"Café"}"#;
        // base64url of `json`, no padding, as `fleet serve` writes it.
        let b64 = "eyJyZWxheSI6Imh0dHBzOi8vZmxlZXQtcmVsYXkuZmx5LmRldiIsIm1hY2hpbmUiOiJtYWMiLCJ0b2tlbiI6InQiLCJuYW1lIjoiQ2Fmw6kifQ";
        assert_eq!(
            pairing_json(&format!("https://fleet-relay.fly.dev/pair#{b64}")),
            json
        );
        assert_eq!(pairing_json(&format!("ocho://pair/{b64}")), json);
        assert_eq!(
            Connection::parse(&format!("/pair/{b64}")).unwrap().name,
            "Café"
        );
        assert_eq!(
            pairing_json("https://example.com/other"),
            "https://example.com/other"
        );
    }

    #[test]
    fn pairing_refuses_what_it_cannot_use() {
        assert!(Connection::parse("").is_err());
        assert!(Connection::parse("hello").is_err());
        assert!(
            Connection::parse(r#"{"relay":"https://evil.example","machine":"m","token":"t"}"#)
                .is_err()
        );
        assert!(Connection::parse(r#"{"machine":"m"}"#).is_err());
    }

    #[test]
    fn segments_round_trip() {
        assert_eq!(encode("a b/c"), "a%20b%2Fc");
        assert_eq!(decode("a%20b%2Fc"), "a b/c");
        assert_eq!(decode("100%"), "100%");
    }
}
