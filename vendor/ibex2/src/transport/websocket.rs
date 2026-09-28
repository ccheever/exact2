//! A listening WebSocket client off Apple (RFC 6455): TCP from the standard
//! library, TLS from the same rustls and trust store as `RustlsHttpTransport`.
//! It sends only what the protocol requires — the opening handshake, a pong
//! for each ping, and the closing handshake — always masked, as a client must.
//! @ref LLP 0057#3-the-boundary — the platform owns the socket and TLS

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::websocket::{Incoming, MessageSource, SocketTransport};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

const MAX_HEAD: usize = 16 << 10;

/// Plaintext `ws:` and rustls `wss:`, one connection per socket. The trust
/// store loads on the first `wss:` open, never at construction (a host that
/// opens no socket pays nothing at boot).
#[derive(Default)]
pub struct TcpSocketTransport {
    tls: std::sync::OnceLock<Arc<rustls::ClientConfig>>,
}

impl TcpSocketTransport {
    pub fn new() -> Self {
        Self::default()
    }

    fn tls(&self) -> Arc<rustls::ClientConfig> {
        self.tls.get_or_init(tls_config).clone()
    }
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().unwrap_or_default() {
            let _ = roots.add(cert);
        }
        if roots.is_empty() {
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        Arc::new(tls)
    }
}

fn failed(what: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("the socket did not open: {what}"))
}

enum Wire {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}
impl Read for Wire {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Wire::Plain(s) => s.read(out),
            Wire::Tls(s) => s.read(out),
        }
    }
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Wire::Plain(s) => s.write(bytes),
            Wire::Tls(s) => s.write(bytes),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Wire::Plain(s) => s.flush(),
            Wire::Tls(s) => s.flush(),
        }
    }
}

impl SocketTransport for TcpSocketTransport {
    fn connect(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        let host = url.host_str().ok_or_else(|| failed("no host"))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| failed("no port"))?;
        let addrs: Vec<_> = (host.trim_start_matches('[').trim_end_matches(']'), port)
            .to_socket_addrs()
            .map_err(failed)?
            .collect();
        signal.check()?;
        let mut last = failed("no resolved address");
        let mut tcp = None;
        for address in addrs {
            signal.check()?;
            match super::rustls_http::connect_socket(address, Duration::from_secs(10), signal) {
                Ok(s) => {
                    tcp = Some(s);
                    break;
                }
                Err(e) => last = failed(e),
            }
        }
        let tcp = tcp.ok_or(last)?;
        tcp.set_nodelay(true).map_err(failed)?;
        // Aborting shuts the connection down, which ends any blocked read.
        let registration = {
            let socket = tcp.try_clone().map_err(failed)?;
            signal.register(move || {
                let _ = socket.shutdown(Shutdown::Both);
            })
        };
        signal.check()?;
        let shutdown = tcp.try_clone().map_err(failed)?;
        tcp.set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(failed)?;
        let mut wire = if url.scheme() == "wss" {
            let name = rustls::pki_types::ServerName::try_from(
                host.trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_string(),
            )
            .map_err(failed)?;
            let tls = rustls::ClientConnection::new(self.tls(), name).map_err(failed)?;
            Wire::Tls(Box::new(rustls::StreamOwned::new(tls, tcp)))
        } else {
            Wire::Plain(tcp)
        };
        let buffered = handshake(&mut wire, url).map_err(|e| match signal.check() {
            Err(aborted) => aborted,
            Ok(()) => e,
        })?;
        shutdown.set_read_timeout(None).map_err(failed)?;
        Ok(Box::new(Socket {
            wire,
            buffered,
            at: 0,
            limit: max_message,
            shutdown,
            signal: signal.clone(),
            _registration: registration,
            closed: false,
        }))
    }
}

/// Send the opening handshake and check the answer. Returns bytes read past
/// the head (the first frames, if the server was quick).
fn handshake(wire: &mut Wire, url: &url::Url) -> Result<Vec<u8>, HostError> {
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).map_err(failed)?;
    use base64::Engine as _;
    let key = base64::engine::general_purpose::STANDARD.encode(nonce);
    let host = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_string(),
    };
    let target = match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_string(),
    };
    let head = format!(
        "GET {target} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    wire.write_all(head.as_bytes()).map_err(failed)?;
    wire.flush().map_err(failed)?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if bytes.len() > MAX_HEAD {
            return Err(failed("the handshake's headers exceed 16 KiB"));
        }
        match wire.read(&mut chunk).map_err(failed)? {
            0 => return Err(failed("the connection closed during the handshake")),
            n => bytes.extend_from_slice(&chunk[..n]),
        }
    };
    let text = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let mut lines = text.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| failed("not an HTTP answer"))?;
    if status != 101 {
        return Err(failed(format!("HTTP {status}")));
    }
    let header = |name: &str| {
        text.split("\r\n").skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    let expected = crate::stdlib::websocket::accept_key(&key);
    let upgrade = header("upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let connection = header("connection").is_some_and(|v| {
        v.split(',')
            .any(|t| t.trim().eq_ignore_ascii_case("upgrade"))
    });
    if !upgrade || !connection || header("sec-websocket-accept").as_deref() != Some(&expected) {
        return Err(failed("the server's handshake was not a WebSocket upgrade"));
    }
    // Nothing was offered, so nothing may be accepted.
    if header("sec-websocket-extensions").is_some() || header("sec-websocket-protocol").is_some() {
        return Err(failed(
            "the server chose an extension or protocol never offered",
        ));
    }
    Ok(bytes[end..].to_vec())
}

struct Socket {
    wire: Wire,
    buffered: Vec<u8>,
    at: usize,
    limit: usize,
    shutdown: TcpStream,
    signal: AbortSignal,
    _registration: AbortRegistration,
    closed: bool,
}

fn protocol(what: &str) -> HostError {
    HostError::Failed(format!("the socket broke the protocol: {what}"))
}

impl Socket {
    /// Exactly `n` bytes, or `None` at the end of the connection.
    fn take(&mut self, n: usize) -> Result<Option<Vec<u8>>, HostError> {
        let mut out = Vec::with_capacity(n);
        let from_buffer = (self.buffered.len() - self.at).min(n);
        out.extend_from_slice(&self.buffered[self.at..self.at + from_buffer]);
        self.at += from_buffer;
        if self.at == self.buffered.len() {
            self.buffered.clear();
            self.at = 0;
        }
        let mut chunk = [0u8; 16 << 10];
        while out.len() < n {
            let want = (n - out.len()).min(chunk.len());
            match self.wire.read(&mut chunk[..want]) {
                // An abort shuts the connection down: that end is ours.
                Ok(0) => return self.signal.check().map(|()| None),
                Ok(k) => out.extend_from_slice(&chunk[..k]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) if self.signal.aborted() => return Err(self.signal.check().unwrap_err()),
                // A reset is the far side going away: 1006, as a browser says.
                Err(_) => return Ok(None),
            }
        }
        Ok(Some(out))
    }

    /// Send a control frame (masked, as a client must).
    fn send(&mut self, opcode: u8, payload: &[u8]) {
        let mut mask = [0u8; 4];
        let _ = getrandom::getrandom(&mut mask);
        let mut frame = vec![0x80 | opcode, 0x80 | payload.len() as u8];
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        let _ = self.wire.write_all(&frame).and_then(|_| self.wire.flush());
    }

    fn closed(&mut self, code: u16, reason: String) -> Incoming {
        self.closed = true;
        Incoming::Closed { code, reason }
    }
}

impl MessageSource for Socket {
    fn next(&mut self) -> Result<Incoming, HostError> {
        if self.closed {
            return Ok(Incoming::Closed {
                code: 1006,
                reason: String::new(),
            });
        }
        let mut message: Option<Vec<u8>> = None;
        loop {
            self.signal.check()?;
            let Some(head) = self.take(2)? else {
                return Ok(self.closed(1006, String::new()));
            };
            let (fin, opcode) = (head[0] & 0x80 != 0, head[0] & 0x0f);
            if head[0] & 0x70 != 0 || head[1] & 0x80 != 0 {
                return Err(protocol("reserved bits, or a masked server frame"));
            }
            let len = match head[1] & 0x7f {
                126 => match self.take(2)? {
                    Some(b) => u16::from_be_bytes([b[0], b[1]]) as u64,
                    None => return Ok(self.closed(1006, String::new())),
                },
                127 => match self.take(8)? {
                    Some(b) => u64::from_be_bytes(b.try_into().unwrap()),
                    None => return Ok(self.closed(1006, String::new())),
                },
                n => n as u64,
            };
            if opcode >= 8 {
                if !fin || len > 125 {
                    return Err(protocol("a fragmented or long control frame"));
                }
                let Some(payload) = self.take(len as usize)? else {
                    return Ok(self.closed(1006, String::new()));
                };
                match opcode {
                    0x8 => {
                        let code = match payload.len() {
                            0 => 1005,
                            1 => return Err(protocol("a one-byte close")),
                            _ => u16::from_be_bytes([payload[0], payload[1]]),
                        };
                        let reason = String::from_utf8_lossy(payload.get(2..).unwrap_or(&[]));
                        let echo = if code == 1005 {
                            vec![]
                        } else {
                            payload[..2].to_vec()
                        };
                        self.send(0x8, &echo);
                        return Ok(self.closed(code, reason.into_owned()));
                    }
                    0x9 => self.send(0xA, &payload),
                    0xA => {}
                    _ => return Err(protocol("an unknown control opcode")),
                }
                continue;
            }
            match (opcode, &message) {
                (0x2, None) => return Ok(Incoming::Binary(len as usize)),
                (0x1, None) => message = Some(Vec::new()),
                (0x0, Some(_)) => {}
                _ => return Err(protocol("a frame out of sequence")),
            }
            let so_far = message.as_ref().map_or(0, Vec::len) as u64;
            if so_far + len > self.limit as u64 {
                return Ok(Incoming::TooLarge);
            }
            let Some(payload) = self.take(len as usize)? else {
                return Ok(self.closed(1006, String::new()));
            };
            let whole = message.as_mut().unwrap();
            whole.extend_from_slice(&payload);
            if fin {
                let bytes = message.take().unwrap();
                return String::from_utf8(bytes)
                    .map(Incoming::Text)
                    .map_err(|_| protocol("a text message that is not UTF-8"));
            }
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        // Say goodbye if the far side has not, then hang up either way.
        if !self.closed && !self.signal.aborted() {
            self.send(0x8, &1000u16.to_be_bytes());
        }
        let _ = self.shutdown.shutdown(Shutdown::Both);
    }
}

#[cfg(test)]
#[path = "websocket_tests.rs"]
pub(crate) mod tests;
