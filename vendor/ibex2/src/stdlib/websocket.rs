//! A WebSocket that only listens (LLP 0059.000 §3.12, the receive-only
//! subset built first): the socket is opened by the host, admitted by `net.websocket
//! <origin>`, and read one message at a time. Nothing is sent but what the
//! protocol requires (the handshake, a pong, the closing handshake): the
//! consumer has no outbound frame.
//!
//! The same split as `fetch` (LLP 0057 §3): the grant check and the message
//! vocabulary are here, identical everywhere; the platform owns the socket,
//! TLS and proxies (`NSURLSessionWebSocketTask` on Apple, rustls elsewhere).
use crate::boundary::HostError;
use crate::grant::{GrantSet, Operation, Origin};
use crate::stdlib::abort::AbortSignal;

/// What a socket said next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// One whole text message (fragments joined), UTF-8.
    Text(String),
    /// A binary message: refused by consumers that take text, so its bytes
    /// are never read. The length is what the first frame declared.
    Binary(usize),
    /// A message over the consumer's ceiling; the socket is done.
    TooLarge,
    /// The socket closed: the far side's close code and reason, or 1006
    /// when the connection ended without a closing handshake (as a browser
    /// reports it).
    Closed { code: u16, reason: String },
}

/// An open socket. Dropping it closes the connection.
pub trait MessageSource: Send {
    /// Block for what comes next. An `Err` is an abort or a protocol
    /// violation; every other end is `Closed`.
    fn next(&mut self) -> Result<Incoming, HostError>;
}

/// The platform half: open `url` (already admitted, `ws` or `wss`) and
/// complete the opening handshake. A refused handshake is an `Err` naming
/// the status. Aborting `signal` interrupts the open and every later read.
pub trait SocketTransport: Send + Sync {
    fn connect(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError>;
}

/// Admit and open a socket to `url`, whose messages may be up to
/// `max_message` bytes. The grant is checked on every open, a reconnect
/// included; a WebSocket handshake follows no redirect.
pub fn open(
    transport: &dyn SocketTransport,
    grants: &GrantSet,
    url: &str,
    max_message: usize,
    signal: &AbortSignal,
) -> Result<Box<dyn MessageSource>, HostError> {
    signal.check()?;
    let parsed = url::Url::parse(url)
        .map_err(|e| HostError::Failed(format!("SyntaxError: invalid socket URL: {e}")))?;
    if !matches!(parsed.scheme(), "ws" | "wss") || parsed.fragment().is_some() {
        return Err(HostError::Failed(
            "SyntaxError: a socket URL is ws: or wss: with no fragment".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| HostError::Failed("SyntaxError: socket URL has no host".into()))?;
    let port = parsed.port_or_known_default().unwrap_or(0);
    let origin = Origin::new(parsed.scheme(), host, port);
    crate::boundary::admit(grants, &Operation::WebSocket { origin })?;
    transport.connect(&parsed, max_message, signal)
}

/// The `Sec-WebSocket-Accept` a server answers `key` with (RFC 6455 §4.2.2),
/// for a client checking a handshake and a test peer making one.
pub fn accept_key(key: &str) -> String {
    use base64::Engine as _;
    let input = format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::engine::general_purpose::STANDARD.encode(sha1(input.as_bytes()))
}

/// SHA-1 (FIPS 180-4), for the one place RFC 6455 needs it: the handshake's
/// accept key. Not used for anything that needs a secure hash.
pub(crate) fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut data = input.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&((input.len() as u64) * 8).to_be_bytes());
    for block in data.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// This build's platform socket.
pub fn default_transport() -> Box<dyn SocketTransport> {
    #[cfg(target_vendor = "apple")]
    {
        Box::new(crate::transport::darwin_websocket::DarwinSocketTransport)
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        Box::new(crate::transport::websocket::TcpSocketTransport::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Never;
    impl SocketTransport for Never {
        fn connect(
            &self,
            _: &url::Url,
            _: usize,
            _: &AbortSignal,
        ) -> Result<Box<dyn MessageSource>, HostError> {
            Err(HostError::Failed("reached the transport".into()))
        }
    }

    fn opened(grants: &str, url: &str) -> String {
        let grants = GrantSet::parse(grants).unwrap();
        match open(&Never, &grants, url, 1024, &AbortSignal::default()) {
            Ok(_) => unreachable!(),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn a_socket_is_admitted_by_its_own_grant_and_nothing_else() {
        let url = "wss://jetstream2.us-east.bsky.network/subscribe?wantedCollections=a";
        let socket = "net.websocket wss://jetstream2.us-east.bsky.network";
        assert_eq!(opened(socket, url), "reached the transport");
        // A fetch grant for the same host admits no socket, and a socket
        // grant is scheme and port exact.
        let fetch = "net.fetch https://jetstream2.us-east.bsky.network";
        assert_eq!(opened(fetch, url), "denied: net.websocket");
        assert_eq!(
            opened("net.websocket ws://jetstream2.us-east.bsky.network", url),
            "denied: net.websocket"
        );
        assert_eq!(
            opened("net.websocket ws://127.0.0.1:4350", "ws://127.0.0.1:4351/x"),
            "denied: net.websocket"
        );
        assert!(opened(socket, "https://jetstream2.us-east.bsky.network/").contains("ws: or wss:"));
        assert!(opened(socket, "wss://jetstream2.us-east.bsky.network/#x").contains("fragment"));
    }
}
