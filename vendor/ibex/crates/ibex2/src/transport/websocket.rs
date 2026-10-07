//! A WebSocket client off Apple (RFC 6455): TCP from the standard
//! library, TLS from the same rustls and trust store as `RustlsHttpTransport`.
//! One bounded writer queue serializes every masked client frame and accounts
//! queued data; the reader joins fragments and admits control replies without
//! ever blocking behind that writer.
//! @ref LLP 0057#3-the-boundary — the platform owns the socket and TLS

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::websocket::{
    Event, Incoming, Message, MessageSender, MessageSource, SocketTransport,
};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

const MAX_HEAD: usize = 16 << 10;
const FRAGMENT: usize = 16 << 10;
const MAX_OUTBOUND_BYTES: usize = 16 << 20;
const MAX_OUTBOUND_MESSAGES: usize = 256;
// @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — data and control frames share one bounded command channel
const MAX_OUTBOUND_COMMANDS: usize = MAX_OUTBOUND_MESSAGES + 16;
const OPEN: u8 = 1;
const CLOSING: u8 = 2;
const CLOSED: u8 = 3;

/// Plaintext `ws:` and rustls `wss:`, one connection per socket. The trust
/// store loads on the first `wss:` open, never at construction (a host that
/// opens no socket pays nothing at boot).
#[derive(Default)]
pub struct TcpSocketTransport {
    tls: std::sync::OnceLock<Arc<rustls::ClientConfig>>,
    #[cfg(test)]
    writer_gate: Option<Arc<(Mutex<bool>, std::sync::Condvar)>>,
}

impl TcpSocketTransport {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn with_tls(config: Arc<rustls::ClientConfig>) -> Self {
        let tls = std::sync::OnceLock::new();
        tls.set(config).expect("a fresh TLS configuration");
        Self {
            tls,
            writer_gate: None,
        }
    }

    #[cfg(test)]
    fn with_writer_gate(gate: Arc<(Mutex<bool>, std::sync::Condvar)>) -> Self {
        Self {
            tls: std::sync::OnceLock::new(),
            writer_gate: Some(gate),
        }
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

/// rustls keeps read and write protocol state in one connection, so the two
/// socket halves cannot use independent `StreamOwned`s. Give a queued writer
/// priority over the receive loop instead: otherwise a timed-out reader can
/// immediately reacquire an unfair platform mutex forever. Linux's mutex does
/// exactly that under contention; Darwin happened to hand the lock over.
/// @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — portable framing has one serialized rustls connection
struct SharedWire {
    wire: Mutex<Wire>,
    writer_waiting: AtomicBool,
}

impl SharedWire {
    fn new(wire: Wire) -> Self {
        Self {
            wire: Mutex::new(wire),
            writer_waiting: AtomicBool::new(false),
        }
    }

    fn read(&self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            while self.writer_waiting.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            let mut wire = self.wire.lock().expect("socket wire poisoned");
            // Close the check-to-lock race. A writer which announced itself
            // while this reader acquired the mutex gets the next turn.
            if self.writer_waiting.load(Ordering::Acquire) {
                drop(wire);
                continue;
            }
            return wire.read(out);
        }
    }

    fn write(
        &self,
        operation: impl FnOnce(&mut Wire) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        self.writer_waiting.store(true, Ordering::Release);
        let result = operation(&mut self.wire.lock().expect("socket wire poisoned"));
        self.writer_waiting.store(false, Ordering::Release);
        result
    }
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
        self.connect_with_protocols(url, max_message, signal, &[])
    }

    fn connect_with_protocols(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
        protocols: &[String],
    ) -> Result<Box<dyn MessageSource>, HostError> {
        self.open_socket(url, max_message, signal, protocols)
            .map(|socket| Box::new(socket) as Box<dyn MessageSource>)
    }
}

impl TcpSocketTransport {
    fn open_socket(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
        protocols: &[String],
    ) -> Result<Socket, HostError> {
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
        let (buffered, protocol) =
            handshake(&mut wire, url, protocols).map_err(|e| match signal.check() {
                Err(aborted) => aborted,
                Ok(()) => e,
            })?;
        // Windows clones do not share these timeout options. Configure the
        // retained I/O handle, not a handle used only to shut it down.
        let tcp = match &wire {
            Wire::Plain(tcp) => tcp,
            Wire::Tls(tls) => &tls.sock,
        };
        tcp.set_read_timeout(Some(Duration::from_millis(25)))
            .map_err(failed)?;
        // A writer holds the shared wire while a frame drains. If the peer
        // stops reading, that write must not block forever: the stall is
        // bounded, and then the connection fails like any other write error.
        tcp.set_write_timeout(Some(WRITE_STALL_TIMEOUT))
            .map_err(failed)?;
        let wire = Arc::new(SharedWire::new(wire));
        let buffered_amount = Arc::new(AtomicUsize::new(0));
        let send_state = Arc::new(Mutex::new(SendState {
            phase: OPEN,
            queued_bytes: 0,
            queued_messages: 0,
        }));
        let (commands, outgoing) = mpsc::sync_channel(MAX_OUTBOUND_COMMANDS);
        let sender = Arc::new(TcpSender {
            commands,
            buffered: Arc::clone(&buffered_amount),
            state: Arc::clone(&send_state),
            shutdown: shutdown.try_clone().map_err(failed)?,
        });
        let writer_wire = Arc::clone(&wire);
        let writer_shutdown = shutdown.try_clone().map_err(failed)?;
        #[cfg(test)]
        let writer_gate = self.writer_gate.clone();
        std::thread::spawn(move || {
            #[cfg(test)]
            if let Some(gate) = writer_gate {
                let (lock, ready) = &*gate;
                let mut open = lock.lock().expect("WebSocket writer gate poisoned");
                while !*open {
                    open = ready.wait(open).expect("WebSocket writer gate poisoned");
                }
            }
            writer_loop(
                outgoing,
                writer_wire,
                writer_shutdown,
                buffered_amount,
                send_state,
            )
        });
        Ok(Socket {
            wire,
            buffered,
            at: 0,
            limit: max_message,
            shutdown,
            signal: signal.clone(),
            _registration: registration,
            sender,
            protocol,
        })
    }
}

/// Send the opening handshake and check the answer. Returns bytes read past
/// the head (the first frames, if the server was quick).
fn handshake(
    wire: &mut Wire,
    url: &url::Url,
    protocols: &[String],
) -> Result<(Vec<u8>, String), HostError> {
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).map_err(failed)?;
    use base64::Engine as _;
    let key = base64::engine::general_purpose::STANDARD.encode(nonce);
    let serialized_host = match url.host() {
        Some(url::Host::Ipv6(address)) => format!("[{address}]"),
        Some(url::Host::Ipv4(address)) => address.to_string(),
        Some(url::Host::Domain(domain)) => domain.to_string(),
        None => String::new(),
    };
    let host = match url.port() {
        Some(port) => format!("{serialized_host}:{port}"),
        None => serialized_host,
    };
    let target = match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_string(),
    };
    let protocols_header = if protocols.is_empty() {
        String::new()
    } else {
        format!("Sec-WebSocket-Protocol: {}\r\n", protocols.join(", "))
    };
    let head = format!(
        "GET {target} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n{protocols_header}\r\n"
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
    if header("sec-websocket-extensions").is_some() {
        return Err(failed("the server chose an extension never offered"));
    }
    let selected = header("sec-websocket-protocol").unwrap_or_default();
    if !selected.is_empty() && !protocols.iter().any(|value| value == &selected) {
        return Err(failed("the server chose a subprotocol never offered"));
    }
    Ok((bytes[end..].to_vec(), selected))
}

struct Socket {
    wire: Arc<SharedWire>,
    buffered: Vec<u8>,
    at: usize,
    limit: usize,
    shutdown: TcpStream,
    signal: AbortSignal,
    _registration: AbortRegistration,
    sender: Arc<TcpSender>,
    protocol: String,
}

fn protocol(what: &str) -> HostError {
    HostError::Failed(format!("the socket broke the protocol: {what}"))
}

enum Command {
    Data {
        opcode: u8,
        payload: Vec<u8>,
        accounted: usize,
    },
    Control {
        opcode: u8,
        payload: Vec<u8>,
    },
}

struct TcpSender {
    commands: mpsc::SyncSender<Command>,
    buffered: Arc<AtomicUsize>,
    state: Arc<Mutex<SendState>>,
    shutdown: TcpStream,
}

struct SendState {
    phase: u8,
    queued_bytes: usize,
    queued_messages: usize,
}

enum CommandQueueError {
    Full,
    Closed,
}

impl CommandQueueError {
    fn host_error(self) -> HostError {
        match self {
            Self::Full => HostError::Failed("the socket's outbound command queue is full".into()),
            Self::Closed => HostError::Failed("the socket is closed".into()),
        }
    }
}

impl TcpSender {
    fn queue(&self, state: &mut SendState, command: Command) -> Result<(), CommandQueueError> {
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                // A peer that does not read can prevent even a close frame
                // from draining. Fail abruptly instead of adding an unbounded
                // control-frame escape hatch beside the data quotas.
                state.phase = CLOSED;
                let _ = self.shutdown.shutdown(Shutdown::Both);
                Err(CommandQueueError::Full)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                state.phase = CLOSED;
                let _ = self.shutdown.shutdown(Shutdown::Both);
                Err(CommandQueueError::Closed)
            }
        }
    }

    fn enqueue(&self, opcode: u8, payload: &[u8]) -> Result<(), HostError> {
        saturating_add(&self.buffered, payload.len());
        let mut state = self.state.lock().expect("WebSocket sender poisoned");
        if state.phase != OPEN {
            return Ok(());
        }
        if state.queued_bytes.saturating_add(payload.len()) > MAX_OUTBOUND_BYTES
            || state.queued_messages >= MAX_OUTBOUND_MESSAGES
        {
            // WHATWG says a full implementation buffer flags the socket as
            // full and closes the connection. An abrupt local failure keeps
            // memory bounded when even a close frame could sit behind a
            // non-reading peer.
            state.phase = CLOSED;
            drop(state);
            let _ = self.shutdown.shutdown(Shutdown::Both);
            return Ok(());
        }
        let queued = self.queue(
            &mut state,
            Command::Data {
                opcode,
                payload: payload.to_vec(),
                accounted: payload.len(),
            },
        );
        if let Err(error) = queued {
            return match error {
                // As with the byte/message quota above, the send which finds
                // a full implementation buffer fails the connection without
                // synchronously throwing into script.
                CommandQueueError::Full => Ok(()),
                CommandQueueError::Closed => Err(error.host_error()),
            };
        }
        state.queued_bytes = state.queued_bytes.saturating_add(payload.len());
        state.queued_messages += 1;
        Ok(())
    }

    fn control(&self, opcode: u8, payload: Vec<u8>) -> Result<(), HostError> {
        let mut state = self.state.lock().expect("WebSocket sender poisoned");
        if state.phase == OPEN {
            self.queue(&mut state, Command::Control { opcode, payload })
                .map_err(CommandQueueError::host_error)?;
        }
        Ok(())
    }

    fn mark_closed(&self) {
        self.state.lock().expect("WebSocket sender poisoned").phase = CLOSED;
    }

    fn phase(&self) -> u8 {
        self.state.lock().expect("WebSocket sender poisoned").phase
    }

    fn peer_close(&self, payload: Vec<u8>) {
        let mut state = self.state.lock().expect("WebSocket sender poisoned");
        if state.phase == OPEN {
            state.phase = CLOSING;
            let _ = self.queue(
                &mut state,
                Command::Control {
                    opcode: 0x8,
                    payload,
                },
            );
        }
        state.phase = CLOSED;
    }

    fn fail_too_large(&self) {
        let mut state = self.state.lock().expect("WebSocket sender poisoned");
        if state.phase != OPEN {
            return;
        }
        state.phase = CLOSING;
        let _ = self.queue(
            &mut state,
            Command::Control {
                opcode: 0x8,
                payload: 1009u16.to_be_bytes().to_vec(),
            },
        );
    }
}

impl MessageSender for TcpSender {
    fn send_text(&self, text: &str) -> Result<(), HostError> {
        self.enqueue(0x1, text.as_bytes())
    }

    fn send_binary(&self, bytes: &[u8]) -> Result<(), HostError> {
        self.enqueue(0x2, bytes)
    }

    fn close(&self, code: Option<u16>, reason: &str) -> Result<(), HostError> {
        let mut state = self.state.lock().expect("WebSocket sender poisoned");
        if state.phase != OPEN {
            return Ok(());
        }
        state.phase = CLOSING;
        let mut payload = Vec::new();
        if let Some(code) = code {
            payload.extend_from_slice(&code.to_be_bytes());
            payload.extend_from_slice(reason.as_bytes());
        }
        self.queue(
            &mut state,
            Command::Control {
                opcode: 0x8,
                payload,
            },
        )
        .map_err(CommandQueueError::host_error)
    }

    fn buffered_amount(&self) -> usize {
        self.buffered.load(Ordering::Acquire)
    }
}

fn saturating_add(amount: &AtomicUsize, bytes: usize) {
    let _ = amount.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
        Some(current.saturating_add(bytes))
    });
}

fn writer_loop(
    commands: mpsc::Receiver<Command>,
    wire: Arc<SharedWire>,
    shutdown: TcpStream,
    buffered: Arc<AtomicUsize>,
    state: Arc<Mutex<SendState>>,
) {
    let mut close_written = false;
    while let Ok(command) = commands.recv() {
        let (result, accounted, closing) = match command {
            Command::Data {
                opcode,
                payload,
                accounted,
            } if !close_written => (write_message(&wire, opcode, &payload), accounted, false),
            Command::Data { .. } => {
                // A data command that lost a race with a written close is
                // discarded and remains in bufferedAmount permanently.
                continue;
            }
            Command::Control { opcode, payload } => {
                let result = write_one(&wire, true, opcode, &payload);
                (result, 0, opcode == 0x8)
            }
        };
        if result.is_err() {
            state.lock().expect("WebSocket sender poisoned").phase = CLOSED;
            let _ = shutdown.shutdown(Shutdown::Both);
            return;
        }
        if accounted != 0 {
            buffered.fetch_sub(accounted, Ordering::AcqRel);
            let mut state = state.lock().expect("WebSocket sender poisoned");
            state.queued_bytes = state.queued_bytes.saturating_sub(accounted);
            state.queued_messages = state.queued_messages.saturating_sub(1);
        }
        if closing {
            close_written = true;
        }
    }
}

fn write_message(wire: &SharedWire, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    if payload.is_empty() {
        return write_one(wire, true, opcode, payload);
    }
    // One fragment per turn on the wire. Only this writer thread sends, so
    // fragments stay contiguous on the wire; the reader may run in between.
    for (index, chunk) in payload.chunks(FRAGMENT).enumerate() {
        let fin = (index + 1) * FRAGMENT >= payload.len();
        write_one(wire, fin, if index == 0 { opcode } else { 0 }, chunk)?;
    }
    Ok(())
}

/// How long one frame may wait for a peer that is not reading before the
/// connection fails. Short in unit tests so the stall path is exercised.
#[cfg(not(test))]
const WRITE_STALL_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(test)]
const WRITE_STALL_TIMEOUT: Duration = Duration::from_millis(500);

fn write_one(wire: &SharedWire, fin: bool, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let frame = masked_frame(fin, opcode, payload)?;
    wire.write(|wire| {
        wire.write_all(&frame)?;
        wire.flush()
    })
}

fn masked_frame(fin: bool, opcode: u8, payload: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut mask = [0u8; 4];
    getrandom::getrandom(&mut mask).map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut frame = Vec::with_capacity(payload.len().saturating_add(14));
    frame.push((if fin { 0x80 } else { 0 }) | opcode);
    match payload.len() {
        length if length < 126 => frame.push(0x80 | length as u8),
        length if length <= u16::MAX as usize => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(&mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    Ok(frame)
}

enum Received {
    Text(String),
    Binary(Vec<u8>),
    BinaryLength(usize),
    TooLarge,
    Closed { code: u16, reason: String },
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
            let read = self.wire.read(&mut chunk[..want]);
            match read {
                // An abort shuts the connection down: that end is ours.
                Ok(0) => return self.signal.check().map(|()| None),
                Ok(k) => out.extend_from_slice(&chunk[..k]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    // Reading and writing share rustls's connection state.
                    // Yield after releasing the lock so an outbound burst is
                    // not starved by this receive loop immediately relocking.
                    std::thread::yield_now();
                    continue;
                }
                Err(_) if self.signal.aborted() => return Err(self.signal.check().unwrap_err()),
                // A reset is the far side going away: 1006, as a browser says.
                Err(_) => return Ok(None),
            }
        }
        Ok(Some(out))
    }

    fn receive(&mut self, binary_payload: bool) -> Result<Received, HostError> {
        if self.sender.phase() == CLOSED {
            return Ok(Received::Closed {
                code: 1006,
                reason: String::new(),
            });
        }
        let mut message: Option<(u8, Vec<u8>)> = None;
        loop {
            self.signal.check()?;
            let Some(head) = self.take(2)? else {
                self.sender.mark_closed();
                return Ok(Received::Closed {
                    code: 1006,
                    reason: String::new(),
                });
            };
            let (fin, opcode) = (head[0] & 0x80 != 0, head[0] & 0x0f);
            if head[0] & 0x70 != 0 || head[1] & 0x80 != 0 {
                return Err(protocol("reserved bits, or a masked server frame"));
            }
            let len = match head[1] & 0x7f {
                126 => match self.take(2)? {
                    Some(b) => u16::from_be_bytes([b[0], b[1]]) as u64,
                    None => {
                        self.sender.mark_closed();
                        return Ok(Received::Closed {
                            code: 1006,
                            reason: String::new(),
                        });
                    }
                },
                127 => match self.take(8)? {
                    Some(b) => u64::from_be_bytes(b.try_into().unwrap()),
                    None => {
                        self.sender.mark_closed();
                        return Ok(Received::Closed {
                            code: 1006,
                            reason: String::new(),
                        });
                    }
                },
                n => n as u64,
            };
            if opcode >= 8 {
                if !fin || len > 125 {
                    return Err(protocol("a fragmented or long control frame"));
                }
                let Some(payload) = self.take(len as usize)? else {
                    self.sender.mark_closed();
                    return Ok(Received::Closed {
                        code: 1006,
                        reason: String::new(),
                    });
                };
                match opcode {
                    0x8 => {
                        let code = match payload.len() {
                            0 => 1005,
                            1 => return Err(protocol("a one-byte close")),
                            _ => u16::from_be_bytes([payload[0], payload[1]]),
                        };
                        if code != 1005
                            && (!(1000..=4999).contains(&code)
                                || matches!(code, 1004 | 1005 | 1006 | 1015))
                        {
                            return Err(protocol("an invalid close code"));
                        }
                        let reason = std::str::from_utf8(payload.get(2..).unwrap_or(&[]))
                            .map_err(|_| protocol("a close reason that is not UTF-8"))?;
                        let echo = if code == 1005 {
                            vec![]
                        } else {
                            payload.clone()
                        };
                        self.sender.peer_close(echo);
                        return Ok(Received::Closed {
                            code,
                            reason: reason.to_string(),
                        });
                    }
                    0x9 => self.sender.control(0xA, payload)?,
                    0xA => {}
                    _ => return Err(protocol("an unknown control opcode")),
                }
                continue;
            }
            // Preserve the receive-only API exact2 already consumes: the
            // first binary frame's declared length is returned immediately,
            // before its payload is read or allocated. Event consumers use
            // the payload-bearing path below instead.
            if opcode == 0x2 && message.is_none() && !binary_payload {
                return Ok(Received::BinaryLength(len as usize));
            }
            match (opcode, &message) {
                (0x1, None) | (0x2, None) => message = Some((opcode, Vec::new())),
                (0x0, Some(_)) => {}
                _ => return Err(protocol("a frame out of sequence")),
            }
            let so_far = message.as_ref().map_or(0, |(_, bytes)| bytes.len()) as u64;
            if so_far + len > self.limit as u64 {
                self.sender.fail_too_large();
                return Ok(Received::TooLarge);
            }
            let Some(payload) = self.take(len as usize)? else {
                self.sender.mark_closed();
                return Ok(Received::Closed {
                    code: 1006,
                    reason: String::new(),
                });
            };
            let (_, whole) = message.as_mut().unwrap();
            whole.extend_from_slice(&payload);
            if fin {
                let (kind, bytes) = message.take().unwrap();
                return if kind == 0x1 {
                    String::from_utf8(bytes)
                        .map(Received::Text)
                        .map_err(|_| protocol("a text message that is not UTF-8"))
                } else {
                    Ok(Received::Binary(bytes))
                };
            }
        }
    }
}

impl MessageSource for Socket {
    fn next(&mut self) -> Result<Incoming, HostError> {
        Ok(match self.receive(false)? {
            Received::Text(text) => Incoming::Text(text),
            Received::Binary(bytes) => Incoming::Binary(bytes.len()),
            Received::BinaryLength(length) => Incoming::Binary(length),
            Received::TooLarge => Incoming::TooLarge,
            Received::Closed { code, reason } => Incoming::Closed { code, reason },
        })
    }

    fn next_event(&mut self) -> Result<Event, HostError> {
        Ok(match self.receive(true)? {
            Received::Text(text) => Event::Message(Message::Text(text)),
            Received::Binary(bytes) => Event::Message(Message::Binary(bytes)),
            Received::BinaryLength(_) => unreachable!("event receive requested binary bytes"),
            Received::TooLarge => {
                return Err(HostError::Failed(
                    "the socket message exceeded its configured limit".into(),
                ));
            }
            Received::Closed { code, reason } => Event::Close {
                code,
                reason,
                was_clean: code != 1006,
            },
        })
    }

    fn sender(&self) -> Option<Arc<dyn MessageSender>> {
        Some(self.sender.clone())
    }

    fn protocol(&self) -> &str {
        &self.protocol
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if self.sender.phase() == OPEN && !self.signal.aborted() {
            let _ = self.sender.close(Some(1000), "");
        }
        self.sender.mark_closed();
        let _ = self.shutdown.shutdown(Shutdown::Both);
    }
}

#[cfg(any(test, feature = "test-support"))]
#[path = "websocket_tests.rs"]
pub mod tests;
