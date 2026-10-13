//! A WebSocket client off Apple (RFC 6455): TCP from the standard
//! library, TLS from the same rustls and trust store as `RustlsHttpTransport`.
//! One readiness-driven I/O pump owns each socket and rustls connection. It
//! multiplexes the bounded outbound queue with requested receives, so control
//! frames remain live while a large data message drains.
//! @ref LLP 0057#3-the-boundary — the platform owns the socket and TLS

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::websocket::{
    Event, Incoming, Message, MessageSender, MessageSource, SocketTransport,
};
use mio::event::Source;
use mio::net::TcpStream as MioTcpStream;
use mio::{Events, Interest, Poll, Token, Waker};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream as StdTcpStream, ToSocketAddrs};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};

const MAX_HEAD: usize = 16 << 10;
const FRAGMENT: usize = 16 << 10;
const MAX_OUTBOUND_BYTES: usize = 16 << 20;
const MAX_OUTBOUND_MESSAGES: usize = 256;
// @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — data and control frames share one bounded command channel
const MAX_OUTBOUND_COMMANDS: usize = MAX_OUTBOUND_MESSAGES + 16;
const MAX_READ_FRAMES_PER_TURN: usize = 32;
const SOCKET_TOKEN: Token = Token(0);
const WAKE_TOKEN: Token = Token(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionState {
    Open,
    Closing,
    Closed,
}

/// Plaintext `ws:` and rustls `wss:`, one connection per socket. The trust
/// store loads on the first `wss:` open, never at construction (a host that
/// opens no socket pays nothing at boot).
#[derive(Default)]
pub struct TcpSocketTransport {
    tls: std::sync::OnceLock<Arc<rustls::ClientConfig>>,
    #[cfg(test)]
    writer_gate: Option<Arc<(Mutex<bool>, std::sync::Condvar)>>,
    #[cfg(test)]
    pump_observer: Option<Arc<PumpObserver>>,
    #[cfg(test)]
    zero_socket_writes: bool,
}

#[cfg(test)]
#[derive(Default)]
struct PumpObserver {
    parked: std::sync::atomic::AtomicBool,
    exited: std::sync::atomic::AtomicBool,
    close_received: std::sync::atomic::AtomicBool,
    terminal_draining: std::sync::atomic::AtomicBool,
    returns: AtomicUsize,
    network_bytes: AtomicUsize,
    last_write_progress: Mutex<Option<Instant>>,
    write_blocked: std::sync::atomic::AtomicBool,
    peer_fin: std::sync::atomic::AtomicBool,
    send_state: std::sync::OnceLock<Arc<Mutex<SendState>>>,
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
            pump_observer: None,
            zero_socket_writes: false,
        }
    }

    #[cfg(test)]
    fn with_writer_gate(gate: Arc<(Mutex<bool>, std::sync::Condvar)>) -> Self {
        Self {
            tls: std::sync::OnceLock::new(),
            writer_gate: Some(gate),
            pump_observer: None,
            zero_socket_writes: false,
        }
    }

    #[cfg(test)]
    fn with_pump_observer(observer: Arc<PumpObserver>) -> Self {
        Self {
            tls: std::sync::OnceLock::new(),
            writer_gate: None,
            pump_observer: Some(observer),
            zero_socket_writes: false,
        }
    }

    #[cfg(test)]
    fn with_tls_and_pump_observer(
        config: Arc<rustls::ClientConfig>,
        observer: Arc<PumpObserver>,
    ) -> Self {
        let tls = std::sync::OnceLock::new();
        tls.set(config).expect("a fresh TLS configuration");
        Self {
            tls,
            writer_gate: None,
            pump_observer: Some(observer),
            zero_socket_writes: false,
        }
    }

    #[cfg(test)]
    fn with_tls_zero_socket_writes(config: Arc<rustls::ClientConfig>) -> Self {
        let tls = std::sync::OnceLock::new();
        tls.set(config).expect("a fresh TLS configuration");
        Self {
            tls,
            writer_gate: None,
            pump_observer: None,
            zero_socket_writes: true,
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

enum HandshakeWire {
    Plain(StdTcpStream),
    Tls {
        stream: Box<rustls::StreamOwned<rustls::ClientConnection, StdTcpStream>>,
    },
}

impl HandshakeWire {
    fn into_nonblocking(self, #[cfg(test)] zero_socket_writes: bool) -> std::io::Result<Wire> {
        let convert = |tcp: StdTcpStream| {
            tcp.set_read_timeout(None)?;
            tcp.set_write_timeout(None)?;
            tcp.set_nonblocking(true)?;
            Ok::<_, std::io::Error>(PumpSocket {
                tcp: MioTcpStream::from_std(tcp),
                #[cfg(test)]
                zero_writes: zero_socket_writes,
            })
        };
        match self {
            Self::Plain(tcp) => convert(tcp).map(Wire::Plain),
            Self::Tls { stream } => {
                let rustls::StreamOwned { conn, sock } = *stream;
                Ok(Wire::Tls {
                    conn: Box::new(conn),
                    sock: convert(sock)?,
                })
            }
        }
    }
}

struct PumpSocket {
    tcp: MioTcpStream,
    #[cfg(test)]
    zero_writes: bool,
}

impl Read for PumpSocket {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.tcp.read(bytes)
    }
}

impl Write for PumpSocket {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        #[cfg(test)]
        if self.zero_writes && !bytes.is_empty() {
            return Ok(0);
        }
        self.tcp.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.tcp.flush()
    }
}

impl Source for PumpSocket {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: Token,
        interests: Interest,
    ) -> std::io::Result<()> {
        self.tcp.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: Token,
        interests: Interest,
    ) -> std::io::Result<()> {
        self.tcp.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        self.tcp.deregister(registry)
    }
}

impl Read for HandshakeWire {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(out),
            Self::Tls { stream } => stream.read(out),
        }
    }
}
impl Write for HandshakeWire {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(bytes),
            Self::Tls { stream } => stream.write(bytes),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls { stream } => stream.flush(),
        }
    }
}

enum Wire {
    Plain(PumpSocket),
    Tls {
        conn: Box<rustls::ClientConnection>,
        sock: PumpSocket,
    },
}

impl Wire {
    fn tcp_mut(&mut self) -> &mut PumpSocket {
        match self {
            Self::Plain(tcp) => tcp,
            Self::Tls { sock, .. } => sock,
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
        // Establish both readiness resources before a server can observe a TCP
        // connection or successful WebSocket upgrade. The Waker is mio's
        // platform primitive (EVFILT_USER/eventfd/IOCP), not a second socket.
        let poll = Poll::new().map_err(failed)?;
        let wake = Arc::new(Waker::new(poll.registry(), WAKE_TOKEN).map_err(failed)?);
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
        // Aborting wakes Poll even when the connection is completely idle.
        // Shutdown is retained as a second, idempotent way to make socket I/O
        // observe cancellation if it races the wake.
        let registration = {
            let socket = tcp.try_clone().map_err(failed)?;
            let wake = Arc::clone(&wake);
            signal.register(move || {
                let _ = socket.shutdown(Shutdown::Both);
                let _ = wake.wake();
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
            HandshakeWire::Tls {
                stream: Box::new(rustls::StreamOwned::new(tls, tcp)),
            }
        } else {
            HandshakeWire::Plain(tcp)
        };
        let (buffered, protocol) =
            handshake(&mut wire, url, protocols).map_err(|e| match signal.check() {
                Err(aborted) => aborted,
                Ok(()) => e,
            })?;
        // The connector (including Windows' select + SO_ERROR path) has
        // already proved this socket connected. Convert that exact retained
        // handle after the bounded blocking handshake; mio does not reconnect.
        let mut wire = wire
            .into_nonblocking(
                #[cfg(test)]
                self.zero_socket_writes,
            )
            .map_err(failed)?;
        poll.registry()
            .register(
                wire.tcp_mut(),
                SOCKET_TOKEN,
                Interest::READABLE.add(Interest::WRITABLE),
            )
            .map_err(failed)?;
        let buffered_amount = Arc::new(AtomicUsize::new(0));
        let command_count = Arc::new(AtomicUsize::new(0));
        let send_state = Arc::new(Mutex::new(SendState {
            admission: AdmissionState::Open,
            queued_bytes: 0,
            queued_messages: 0,
            terminal: None,
            graceful_teardown: false,
            #[cfg(test)]
            history: Vec::new(),
        }));
        let (commands, outgoing) = mpsc::sync_channel(MAX_OUTBOUND_COMMANDS);
        let (requests, incoming) = mpsc::sync_channel(1);
        let sender = Arc::new(TcpSender {
            commands,
            wake: Arc::clone(&wake),
            command_count: Arc::clone(&command_count),
            buffered: Arc::clone(&buffered_amount),
            state: Arc::clone(&send_state),
            shutdown: shutdown.try_clone().map_err(failed)?,
        });
        let pump_shutdown = shutdown.try_clone().map_err(failed)?;
        let pump_signal = signal.clone();
        #[cfg(test)]
        let writer_gate = self.writer_gate.clone();
        #[cfg(test)]
        let pump_observer = self.pump_observer.clone();
        let pump_wake = Arc::clone(&wake);
        std::thread::spawn(move || {
            pump_loop(
                wire,
                buffered,
                max_message,
                outgoing,
                incoming,
                poll,
                pump_wake,
                pump_shutdown,
                buffered_amount,
                command_count,
                send_state,
                pump_signal,
                #[cfg(test)]
                writer_gate,
                #[cfg(test)]
                pump_observer,
            )
        });
        Ok(Socket {
            requests,
            wake,
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
    wire: &mut HandshakeWire,
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
    requests: mpsc::SyncSender<ReceiveRequest>,
    wake: Arc<Waker>,
    shutdown: StdTcpStream,
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
    wake: Arc<Waker>,
    command_count: Arc<AtomicUsize>,
    buffered: Arc<AtomicUsize>,
    state: Arc<Mutex<SendState>>,
    shutdown: StdTcpStream,
}

struct SendState {
    admission: AdmissionState,
    queued_bytes: usize,
    queued_messages: usize,
    terminal: Option<TerminalLatch>,
    /// The pump has handed its final frame to TCP (or is already
    /// half-closing) and owns a graceful TCP teardown from here on.
    graceful_teardown: bool,
    #[cfg(test)]
    history: Vec<TerminalStep>,
}

/// Test-only record of terminal-latch transitions, in the order the pump made
/// them under the send-state lock.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalStep {
    LatchedDeferred,
    LatchedDeliverable,
    CloseSent,
    Deliverable,
}

struct TerminalLatch {
    result: Result<Received, HostError>,
    delivered: bool,
    deliverable: bool,
    peer_close: bool,
}

enum CommandQueueError {
    Full,
    Closed,
    Wake(std::io::Error),
}

impl CommandQueueError {
    fn host_error(self) -> HostError {
        match self {
            Self::Full => HostError::Failed("the socket's outbound command queue is full".into()),
            Self::Closed => HostError::Failed("the socket is closed".into()),
            Self::Wake(error) => {
                HostError::Failed(format!("the socket wake notification failed: {error}"))
            }
        }
    }
}

impl TcpSender {
    fn queue(&self, state: &mut SendState, command: Command) -> Result<(), CommandQueueError> {
        if self
            .command_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_OUTBOUND_COMMANDS).then_some(count + 1)
            })
            .is_err()
        {
            state.admission = AdmissionState::Closed;
            let _ = self.shutdown.shutdown(Shutdown::Both);
            let _ = self.wake.wake();
            return Err(CommandQueueError::Full);
        }
        match self.commands.try_send(command) {
            Ok(()) => {
                if let Err(error) = self.wake.wake() {
                    state.admission = AdmissionState::Closed;
                    let _ = self.shutdown.shutdown(Shutdown::Both);
                    Err(CommandQueueError::Wake(error))
                } else {
                    Ok(())
                }
            }
            Err(mpsc::TrySendError::Full(_)) => {
                self.command_count.fetch_sub(1, Ordering::AcqRel);
                // A peer that does not read can prevent even a close frame
                // from draining. Fail abruptly instead of adding an unbounded
                // control-frame escape hatch beside the data quotas.
                state.admission = AdmissionState::Closed;
                let _ = self.shutdown.shutdown(Shutdown::Both);
                let _ = self.wake.wake();
                Err(CommandQueueError::Full)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.command_count.fetch_sub(1, Ordering::AcqRel);
                state.admission = AdmissionState::Closed;
                let _ = self.shutdown.shutdown(Shutdown::Both);
                let _ = self.wake.wake();
                Err(CommandQueueError::Closed)
            }
        }
    }

    fn enqueue(&self, opcode: u8, payload: &[u8]) -> Result<(), HostError> {
        saturating_add(&self.buffered, payload.len());
        let mut state = self.state.lock().expect("WebSocket sender poisoned");
        if state.admission != AdmissionState::Open {
            return Ok(());
        }
        if state.queued_bytes.saturating_add(payload.len()) > MAX_OUTBOUND_BYTES
            || state.queued_messages >= MAX_OUTBOUND_MESSAGES
        {
            // WHATWG says a full implementation buffer flags the socket as
            // full and closes the connection. An abrupt local failure keeps
            // memory bounded when even a close frame could sit behind a
            // non-reading peer.
            state.admission = AdmissionState::Closed;
            drop(state);
            let _ = self.shutdown.shutdown(Shutdown::Both);
            let _ = self.wake.wake();
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
                CommandQueueError::Closed | CommandQueueError::Wake(_) => Err(error.host_error()),
            };
        }
        state.queued_bytes = state.queued_bytes.saturating_add(payload.len());
        state.queued_messages += 1;
        Ok(())
    }

    fn mark_closed(&self) {
        self.state
            .lock()
            .expect("WebSocket sender poisoned")
            .admission = AdmissionState::Closed;
        let _ = self.wake.wake();
    }

    fn admission(&self) -> AdmissionState {
        self.state
            .lock()
            .expect("WebSocket sender poisoned")
            .admission
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
        if state.admission != AdmissionState::Open {
            return Ok(());
        }
        state.admission = AdmissionState::Closing;
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

/// How long one frame may wait for a peer that is not reading before the
/// connection fails. Short in unit tests so the stall path is exercised.
#[cfg(not(test))]
const WRITE_STALL_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(test)]
const WRITE_STALL_TIMEOUT: Duration = Duration::from_millis(500);

struct ReceiveRequest {
    binary_payload: bool,
    reply: mpsc::SyncSender<Result<Received, HostError>>,
}

struct ReceiveOperation {
    binary_payload: bool,
    message: Option<(u8, Vec<u8>)>,
}

struct Input {
    bytes: Vec<u8>,
    at: usize,
}

impl Input {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, at: 0 }
    }

    fn available(&self) -> &[u8] {
        &self.bytes[self.at..]
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.at == self.bytes.len() {
            self.bytes.clear();
            self.at = 0;
        } else if self.at > 0 && self.at >= self.bytes.len() / 2 {
            self.bytes.drain(..self.at);
            self.at = 0;
        }
        self.bytes.extend_from_slice(bytes);
    }

    fn take(&mut self, count: usize) -> Vec<u8> {
        let start = self.at;
        self.at += count;
        self.bytes[start..start + count].to_vec()
    }

    fn discard(&mut self, count: usize) {
        self.at += count;
    }
}

struct ActiveData {
    opcode: u8,
    payload: Vec<u8>,
    accounted: usize,
    next: usize,
}

impl ActiveData {
    fn frame(&mut self) -> std::io::Result<(Vec<u8>, bool)> {
        if self.payload.is_empty() {
            return masked_frame(true, self.opcode, &[]).map(|frame| (frame, true));
        }
        let start = self.next;
        let end = start.saturating_add(FRAGMENT).min(self.payload.len());
        self.next = end;
        let final_fragment = end == self.payload.len();
        masked_frame(
            final_fragment,
            if start == 0 { self.opcode } else { 0 },
            &self.payload[start..end],
        )
        .map(|frame| (frame, final_fragment))
    }
}

enum FrameKind {
    Data { final_fragment: bool },
    Control { opcode: u8 },
}

struct OutgoingFrame {
    bytes: Vec<u8>,
    at: usize,
    kind: FrameKind,
}

struct Control {
    opcode: u8,
    payload: Vec<u8>,
}

enum Parse {
    NeedData(usize),
    Received(Received),
    Yield,
}

enum ControlParse {
    NeedData(usize),
    Data,
    Progress,
    Closed(Received),
    Yield,
}

enum ReadProgress {
    Bytes,
    Eof,
    Blocked,
}

enum WriteProgress {
    Blocked,
    Retry,
    Network(usize),
    Eof,
}

impl Wire {
    fn wants_write(&self) -> bool {
        match self {
            Self::Plain(_) => false,
            Self::Tls { conn, .. } => conn.wants_write(),
        }
    }

    fn read_into(&mut self, input: &mut Input, wanted: usize) -> std::io::Result<ReadProgress> {
        let mut bytes = [0; 16 << 10];
        let wanted = wanted.min(bytes.len());
        match self {
            Self::Plain(tcp) => match tcp.read(&mut bytes[..wanted]) {
                Ok(0) => Ok(ReadProgress::Eof),
                Ok(count) => {
                    input.append(&bytes[..count]);
                    Ok(ReadProgress::Bytes)
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    Ok(ReadProgress::Blocked)
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                    Ok(ReadProgress::Bytes)
                }
                Err(error) => Err(error),
            },
            Self::Tls { conn, sock } => match conn.reader().read(&mut bytes[..wanted]) {
                Ok(0) => Ok(ReadProgress::Eof),
                Ok(count) => {
                    input.append(&bytes[..count]);
                    Ok(ReadProgress::Bytes)
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    match conn.read_tls(sock) {
                        Ok(0) => Ok(ReadProgress::Eof),
                        Ok(_) => {
                            conn.process_new_packets().map_err(|error| {
                                std::io::Error::new(std::io::ErrorKind::InvalidData, error)
                            })?;
                            Ok(ReadProgress::Bytes)
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            Ok(ReadProgress::Blocked)
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                            Ok(ReadProgress::Bytes)
                        }
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            },
        }
    }

    fn write_frame(&mut self, frame: &mut OutgoingFrame) -> std::io::Result<WriteProgress> {
        match self {
            Self::Plain(_) if frame.at == frame.bytes.len() => Ok(WriteProgress::Blocked),
            Self::Plain(tcp) => match tcp.write(&frame.bytes[frame.at..]) {
                Ok(0) => Err(std::io::ErrorKind::WriteZero.into()),
                Ok(count) => {
                    frame.at += count;
                    Ok(WriteProgress::Network(count))
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    Ok(WriteProgress::Blocked)
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                    Ok(WriteProgress::Retry)
                }
                Err(error) => Err(error),
            },
            Self::Tls { conn, sock } => {
                if conn.wants_write() {
                    return write_tls_progress(conn, sock);
                }
                if frame.at == frame.bytes.len() {
                    return Ok(WriteProgress::Blocked);
                }
                let count = conn.writer().write(&frame.bytes[frame.at..])?;
                if count == 0 {
                    return Err(std::io::ErrorKind::WriteZero.into());
                }
                frame.at += count;
                Ok(WriteProgress::Retry)
            }
        }
    }

    fn frame_flushed(&self, frame: &OutgoingFrame) -> bool {
        frame.at == frame.bytes.len() && !self.wants_write()
    }

    fn write_tls_pending(&mut self) -> std::io::Result<WriteProgress> {
        let Self::Tls { conn, sock } = self else {
            return Ok(WriteProgress::Blocked);
        };
        write_tls_progress(conn, sock)
    }
}

fn write_tls_progress(
    connection: &mut rustls::ClientConnection,
    socket: &mut impl Write,
) -> std::io::Result<WriteProgress> {
    match connection.write_tls(socket) {
        Ok(0) => Ok(WriteProgress::Eof),
        Ok(count) => Ok(WriteProgress::Network(count)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(WriteProgress::Blocked),
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => Ok(WriteProgress::Retry),
        Err(error) => Err(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_available(
    operation: &mut ReceiveOperation,
    input: &mut Input,
    controls: &mut std::collections::VecDeque<Control>,
    command_count: &AtomicUsize,
    state: &Mutex<SendState>,
    limit: usize,
    close_started: bool,
    frames_left: &mut usize,
) -> Result<Parse, HostError> {
    loop {
        if *frames_left == 0 {
            return Ok(Parse::Yield);
        }
        let bytes = input.available();
        if bytes.len() < 2 {
            return Ok(Parse::NeedData(2 - bytes.len()));
        }
        let (fin, opcode) = (bytes[0] & 0x80 != 0, bytes[0] & 0x0f);
        if bytes[0] & 0x70 != 0 || bytes[1] & 0x80 != 0 {
            return Err(protocol("reserved bits, or a masked server frame"));
        }
        let (head, length) = match bytes[1] & 0x7f {
            126 if bytes.len() < 4 => return Ok(Parse::NeedData(4 - bytes.len())),
            126 => (4, u16::from_be_bytes([bytes[2], bytes[3]]) as u64),
            127 if bytes.len() < 10 => return Ok(Parse::NeedData(10 - bytes.len())),
            127 => (10, u64::from_be_bytes(bytes[2..10].try_into().unwrap())),
            length => (2, length as u64),
        };
        if opcode >= 8 && (!fin || length > 125) {
            return Err(protocol("a fragmented or long control frame"));
        }
        // Preserve the receive-only API exact2 already consumes: the first
        // binary frame's declared length returns before its payload is read or
        // allocated. Event consumers use the payload-bearing path below.
        if opcode == 0x2 && operation.message.is_none() && !operation.binary_payload {
            input.discard(head);
            return Ok(Parse::Received(Received::BinaryLength(length as usize)));
        }
        if opcode < 8 {
            match (opcode, &operation.message) {
                (0x1, None) | (0x2, None) => {}
                (0x0, Some(_)) => {}
                _ => return Err(protocol("a frame out of sequence")),
            }
            let so_far = operation
                .message
                .as_ref()
                .map_or(0, |(_, message)| message.len()) as u64;
            if so_far.saturating_add(length) > limit as u64 {
                input.discard(head);
                let mut send = state.lock().expect("WebSocket sender poisoned");
                if !close_started {
                    replace_controls_with_close(
                        controls,
                        command_count,
                        1009u16.to_be_bytes().to_vec(),
                    )?;
                    send.admission = AdmissionState::Closing;
                }
                return Ok(Parse::Received(Received::TooLarge));
            }
        }
        let Ok(length) = usize::try_from(length) else {
            return Err(protocol("a frame length that does not fit this platform"));
        };
        let frame_length = head.saturating_add(length);
        if input.available().len() < frame_length {
            return Ok(Parse::NeedData(frame_length - input.available().len()));
        }
        input.discard(head);
        let payload = input.take(length);
        *frames_left -= 1;
        if opcode >= 8 {
            match opcode {
                0x8 => {
                    let (code, reason) = parse_close_payload(&payload)?;
                    let echo = if code == 1005 {
                        vec![]
                    } else {
                        payload.clone()
                    };
                    let mut send = state.lock().expect("WebSocket sender poisoned");
                    if !close_started {
                        replace_controls_with_close(controls, command_count, echo)?;
                    }
                    send.admission = AdmissionState::Closed;
                    return Ok(Parse::Received(Received::Closed { code, reason }));
                }
                0x9 => {
                    if !close_started {
                        reserve_control(command_count)?;
                        controls.push_back(Control {
                            opcode: 0xA,
                            payload,
                        });
                    }
                }
                0xA => {}
                _ => return Err(protocol("an unknown control opcode")),
            }
            continue;
        }
        if matches!(opcode, 0x1 | 0x2) {
            operation.message = Some((opcode, Vec::new()));
        }
        let (_, whole) = operation.message.as_mut().unwrap();
        whole.extend_from_slice(&payload);
        if fin {
            let (kind, bytes) = operation.message.take().unwrap();
            let received = if kind == 0x1 {
                Received::Text(
                    String::from_utf8(bytes)
                        .map_err(|_| protocol("a text message that is not UTF-8"))?,
                )
            } else {
                Received::Binary(bytes)
            };
            return Ok(Parse::Received(received));
        }
    }
}

fn parse_control_prefix(
    input: &mut Input,
    controls: &mut std::collections::VecDeque<Control>,
    command_count: &AtomicUsize,
    state: &Mutex<SendState>,
    close_started: bool,
    frames_left: &mut usize,
) -> Result<ControlParse, HostError> {
    let mut progressed = false;
    loop {
        if *frames_left == 0 {
            return Ok(ControlParse::Yield);
        }
        let bytes = input.available();
        if bytes.len() < 2 {
            return Ok(if progressed {
                ControlParse::Progress
            } else {
                ControlParse::NeedData(2 - bytes.len())
            });
        }
        let (fin, opcode) = (bytes[0] & 0x80 != 0, bytes[0] & 0x0f);
        if opcode < 8 {
            return Ok(if progressed {
                ControlParse::Progress
            } else {
                ControlParse::Data
            });
        }
        if bytes[0] & 0x70 != 0 || bytes[1] & 0x80 != 0 {
            return Err(protocol("reserved bits, or a masked server frame"));
        }
        let (head, length): (usize, usize) = match bytes[1] & 0x7f {
            126 if bytes.len() < 4 => return Ok(ControlParse::NeedData(4 - bytes.len())),
            126 => (4, u16::from_be_bytes([bytes[2], bytes[3]]) as usize),
            127 if bytes.len() < 10 => return Ok(ControlParse::NeedData(10 - bytes.len())),
            127 => {
                let length = u64::from_be_bytes(bytes[2..10].try_into().unwrap());
                let length = usize::try_from(length)
                    .map_err(|_| protocol("a frame length that does not fit this platform"))?;
                (10, length)
            }
            length => (2, length as usize),
        };
        if !fin || length > 125 {
            return Err(protocol("a fragmented or long control frame"));
        }
        let frame_length = head.saturating_add(length);
        if bytes.len() < frame_length {
            return Ok(ControlParse::NeedData(frame_length - bytes.len()));
        }
        input.discard(head);
        let payload = input.take(length);
        *frames_left -= 1;
        progressed = true;
        match opcode {
            0x8 => {
                let (code, reason) = parse_close_payload(&payload)?;
                let echo = if code == 1005 { Vec::new() } else { payload };
                let mut send = state.lock().expect("WebSocket sender poisoned");
                if !close_started {
                    replace_controls_with_close(controls, command_count, echo)?;
                }
                send.admission = AdmissionState::Closed;
                return Ok(ControlParse::Closed(Received::Closed { code, reason }));
            }
            0x9 => {
                if !close_started {
                    reserve_control(command_count)?;
                    controls.push_back(Control {
                        opcode: 0xA,
                        payload,
                    });
                }
            }
            0xA => {}
            _ => return Err(protocol("an unknown control opcode")),
        }
    }
}

fn parse_close_payload(payload: &[u8]) -> Result<(u16, String), HostError> {
    let code = match payload.len() {
        0 => 1005,
        1 => return Err(protocol("a one-byte close")),
        _ => u16::from_be_bytes([payload[0], payload[1]]),
    };
    if code != 1005 && (!(1000..=4999).contains(&code) || matches!(code, 1004 | 1005 | 1006 | 1015))
    {
        return Err(protocol("an invalid close code"));
    }
    let reason = std::str::from_utf8(payload.get(2..).unwrap_or(&[]))
        .map_err(|_| protocol("a close reason that is not UTF-8"))?
        .to_string();
    Ok((code, reason))
}

fn drop_controls(controls: &mut std::collections::VecDeque<Control>, command_count: &AtomicUsize) {
    command_count.fetch_sub(controls.len(), Ordering::AcqRel);
    controls.clear();
}

fn replace_controls_with_close(
    controls: &mut std::collections::VecDeque<Control>,
    command_count: &AtomicUsize,
    payload: Vec<u8>,
) -> Result<(), HostError> {
    drop_controls(controls, command_count);
    reserve_control(command_count)?;
    controls.push_front(Control {
        opcode: 0x8,
        payload,
    });
    Ok(())
}

fn reserve_control(command_count: &AtomicUsize) -> Result<(), HostError> {
    command_count
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < MAX_OUTBOUND_COMMANDS).then_some(count + 1)
        })
        .map(|_| ())
        .map_err(|_| HostError::Failed("the socket's outbound command queue is full".into()))
}

fn finish_data(
    active: &mut Option<ActiveData>,
    buffered: &AtomicUsize,
    command_count: &AtomicUsize,
    state: &Mutex<SendState>,
) {
    let Some(data) = active.take() else {
        return;
    };
    if data.accounted != 0 {
        buffered.fetch_sub(data.accounted, Ordering::AcqRel);
    }
    command_count.fetch_sub(1, Ordering::AcqRel);
    let mut state = state.lock().expect("WebSocket sender poisoned");
    state.queued_bytes = state.queued_bytes.saturating_sub(data.accounted);
    state.queued_messages = state.queued_messages.saturating_sub(1);
}

fn latch_terminal(
    result: Result<Received, HostError>,
    deliverable: bool,
    state: &Mutex<SendState>,
) {
    let peer_close = matches!(
        &result,
        Ok(Received::Closed { code, .. }) if *code != 1006
    );
    let mut send = state.lock().expect("WebSocket sender poisoned");
    send.admission = AdmissionState::Closed;
    if let Some(terminal) = send.terminal.as_mut() {
        if terminal.peer_close || !peer_close {
            #[cfg(test)]
            let became_deliverable = deliverable && !terminal.deliverable;
            terminal.deliverable |= deliverable;
            #[cfg(test)]
            if became_deliverable {
                send.history.push(TerminalStep::Deliverable);
            }
            return;
        }
    }
    #[cfg(test)]
    send.history.push(if deliverable {
        TerminalStep::LatchedDeliverable
    } else {
        TerminalStep::LatchedDeferred
    });
    send.terminal = Some(TerminalLatch {
        result,
        delivered: false,
        deliverable,
        peer_close,
    });
}

fn deliver_terminal(
    request: &mut Option<(ReceiveRequest, ReceiveOperation)>,
    requests: &mpsc::Receiver<ReceiveRequest>,
    state: &Mutex<SendState>,
) {
    // Admission holds this same state lock through queue insertion. Once the
    // pump owns it, every request which observed OPEN is either active or
    // visible in `requests`, so a racing EOF cannot turn into channel closure.
    let delivery = {
        let mut send = state.lock().expect("WebSocket sender poisoned");
        let terminal = send.terminal.as_mut();
        let Some(terminal) =
            terminal.filter(|terminal| terminal.deliverable && !terminal.delivered)
        else {
            return;
        };
        let pending = request
            .take()
            .map(|(request, _)| request)
            .or_else(|| requests.try_recv().ok());
        terminal.delivered = pending.is_some();
        pending.map(|pending| (pending, terminal.result.clone()))
    };
    let Some((request, result)) = delivery else {
        return;
    };
    let _ = request.reply.send(result);
}

fn defer_terminal(result: Result<Received, HostError>, state: &Mutex<SendState>) {
    latch_terminal(result, false, state);
}

fn release_terminal(
    request: &mut Option<(ReceiveRequest, ReceiveOperation)>,
    requests: &mpsc::Receiver<ReceiveRequest>,
    state: &Mutex<SendState>,
) {
    {
        let mut send = state.lock().expect("WebSocket sender poisoned");
        // Set in the same critical section that can make the terminal
        // deliverable, so a caller dropping the socket on that result never
        // races the pump into a resetting shutdown.
        send.graceful_teardown = true;
        #[cfg(test)]
        send.history.push(TerminalStep::CloseSent);
        if let Some(terminal) = send.terminal.as_mut() {
            #[cfg(test)]
            let became_deliverable = !terminal.deliverable;
            terminal.deliverable = true;
            #[cfg(test)]
            if became_deliverable {
                send.history.push(TerminalStep::Deliverable);
            }
        }
    }
    deliver_terminal(request, requests, state);
}

fn publish_terminal(
    request: &mut Option<(ReceiveRequest, ReceiveOperation)>,
    requests: &mpsc::Receiver<ReceiveRequest>,
    result: Result<Received, HostError>,
    state: &Mutex<SendState>,
) {
    latch_terminal(result, true, state);
    deliver_terminal(request, requests, state);
}

fn fail_pump(
    request: &mut Option<(ReceiveRequest, ReceiveOperation)>,
    requests: &mpsc::Receiver<ReceiveRequest>,
    error: HostError,
    state: &Mutex<SendState>,
    shutdown: &StdTcpStream,
) {
    publish_terminal(request, requests, Err(error), state);
    let _ = shutdown.shutdown(Shutdown::Both);
}

fn abnormal_close() -> Received {
    Received::Closed {
        code: 1006,
        reason: String::new(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadSide {
    Open,
    Closing,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteSide {
    Open,
    Done,
}

struct CloseState {
    read: ReadSide,
    write: WriteSide,
    close_started: bool,
    close_sent: bool,
    close_received: bool,
    finish_after_close_sent: bool,
    peer_fin: bool,
}

impl CloseState {
    fn new() -> Self {
        Self {
            read: ReadSide::Open,
            write: WriteSide::Open,
            close_started: false,
            close_sent: false,
            close_received: false,
            finish_after_close_sent: false,
            peer_fin: false,
        }
    }

    fn receive_close(&mut self) {
        self.close_received = true;
        self.finish_after_close_sent = true;
        self.read = ReadSide::Done;
    }
}

#[derive(Default)]
struct TerminalDrain {
    fragmented: Option<u8>,
    message_len: u64,
    data_left: usize,
}

fn begin_terminal_drain(
    close: &mut CloseState,
    drain: &mut TerminalDrain,
    request: &mut Option<(ReceiveRequest, ReceiveOperation)>,
    #[cfg(test)] observer: Option<&PumpObserver>,
) {
    if close.read != ReadSide::Open {
        return;
    }
    if let Some((_, operation)) = request.as_mut() {
        if let Some((opcode, message)) = operation.message.take() {
            drain.fragmented = Some(opcode);
            drain.message_len = message.len() as u64;
        }
    }
    close.read = ReadSide::Closing;
    #[cfg(test)]
    if let Some(observer) = observer {
        observer.terminal_draining.store(true, Ordering::Release);
    }
}

enum DrainParse {
    NeedData(usize),
    Progress,
    Closed(Received),
    TooLarge,
    Yield,
}

#[allow(clippy::too_many_arguments)]
fn parse_terminal_drain(
    drain: &mut TerminalDrain,
    input: &mut Input,
    controls: &mut std::collections::VecDeque<Control>,
    command_count: &AtomicUsize,
    state: &Mutex<SendState>,
    limit: usize,
    close_started: bool,
    frames_left: &mut usize,
) -> Result<DrainParse, HostError> {
    let mut progressed = false;
    loop {
        if drain.data_left != 0 {
            let discarded = drain.data_left.min(input.available().len());
            input.discard(discarded);
            drain.data_left -= discarded;
            progressed |= discarded != 0;
            if drain.data_left != 0 {
                return Ok(DrainParse::NeedData(drain.data_left.min(FRAGMENT)));
            }
            *frames_left -= 1;
        }
        if *frames_left == 0 {
            return Ok(DrainParse::Yield);
        }
        let bytes = input.available();
        if bytes.len() < 2 {
            return Ok(if progressed {
                DrainParse::Progress
            } else {
                DrainParse::NeedData(2 - bytes.len())
            });
        }
        let (fin, opcode) = (bytes[0] & 0x80 != 0, bytes[0] & 0x0f);
        if bytes[0] & 0x70 != 0 || bytes[1] & 0x80 != 0 {
            return Err(protocol("reserved bits, or a masked server frame"));
        }
        let (head, length) = match bytes[1] & 0x7f {
            126 if bytes.len() < 4 => return Ok(DrainParse::NeedData(4 - bytes.len())),
            126 => (4, u16::from_be_bytes([bytes[2], bytes[3]]) as u64),
            127 if bytes.len() < 10 => return Ok(DrainParse::NeedData(10 - bytes.len())),
            127 => (10, u64::from_be_bytes(bytes[2..10].try_into().unwrap())),
            length => (2, length as u64),
        };
        if opcode >= 8 {
            if !fin || length > 125 {
                return Err(protocol("a fragmented or long control frame"));
            }
            let length = length as usize;
            let frame_length = head + length;
            if bytes.len() < frame_length {
                return Ok(DrainParse::NeedData(frame_length - bytes.len()));
            }
            input.discard(head);
            let payload = input.take(length);
            *frames_left -= 1;
            progressed = true;
            match opcode {
                0x8 => {
                    let (code, reason) = parse_close_payload(&payload)?;
                    if !close_started {
                        let echo = if code == 1005 { Vec::new() } else { payload };
                        replace_controls_with_close(controls, command_count, echo)?;
                    }
                    state.lock().expect("WebSocket sender poisoned").admission =
                        AdmissionState::Closed;
                    return Ok(DrainParse::Closed(Received::Closed { code, reason }));
                }
                0x9 if !close_started => {
                    reserve_control(command_count)?;
                    controls.push_back(Control {
                        opcode: 0xA,
                        payload,
                    });
                }
                0x9 | 0xA => {}
                _ => return Err(protocol("an unknown control opcode")),
            }
            continue;
        }
        if !matches!(opcode, 0x0..=0x2) {
            return Err(protocol("an unknown data opcode"));
        }
        match (opcode, drain.fragmented) {
            (0x1 | 0x2, None) => {
                drain.message_len = length;
                drain.fragmented = (!fin).then_some(opcode);
            }
            (0x0, Some(kind)) => {
                drain.message_len = drain.message_len.saturating_add(length);
                if fin {
                    drain.fragmented = None;
                } else {
                    drain.fragmented = Some(kind);
                }
            }
            _ => return Err(protocol("a frame out of sequence")),
        }
        if drain.message_len > limit as u64 {
            input.discard(head);
            let mut send = state.lock().expect("WebSocket sender poisoned");
            if !close_started {
                replace_controls_with_close(
                    controls,
                    command_count,
                    1009u16.to_be_bytes().to_vec(),
                )?;
                send.admission = AdmissionState::Closing;
            }
            return Ok(DrainParse::TooLarge);
        }
        if fin {
            drain.message_len = 0;
        }
        let length = usize::try_from(length)
            .map_err(|_| protocol("a frame length that does not fit this platform"))?;
        input.discard(head);
        drain.data_left = length;
        progressed = true;
        if drain.data_left == 0 {
            *frames_left -= 1;
        }
    }
}

fn stop_polling_tcp(poll: &Poll, wire: &mut Wire, registered: &mut bool) -> Result<(), HostError> {
    if *registered {
        poll.registry()
            .deregister(wire.tcp_mut())
            .map_err(|error| {
                HostError::Failed(format!(
                    "the socket readiness deregistration failed: {error}"
                ))
            })?;
        *registered = false;
    }
    Ok(())
}

fn poll_writes_only(
    poll: &Poll,
    wire: &mut Wire,
    write_only: &mut bool,
) -> Result<bool, HostError> {
    if !*write_only {
        poll.registry()
            .reregister(wire.tcp_mut(), SOCKET_TOKEN, Interest::WRITABLE)
            .map_err(|error| {
                HostError::Failed(format!("the socket readiness registration failed: {error}"))
            })?;
        *write_only = true;
        return Ok(true);
    }
    Ok(false)
}

fn abandon_active(active: &mut Option<ActiveData>, command_count: &AtomicUsize) {
    if active.take().is_some() {
        command_count.fetch_sub(1, Ordering::AcqRel);
    }
}

fn is_close_frame(frame: &Option<OutgoingFrame>) -> bool {
    matches!(
        frame,
        Some(OutgoingFrame {
            kind: FrameKind::Control { opcode: 0x8 },
            ..
        })
    )
}

fn discard_unsent_close_frame(
    frame: &mut Option<OutgoingFrame>,
    wire: &Wire,
    command_count: &AtomicUsize,
) {
    let unsent_close = matches!(
        frame,
        Some(OutgoingFrame {
            at: 0,
            kind: FrameKind::Control { opcode: 0x8 },
            ..
        })
    ) && !wire.wants_write();
    if unsent_close {
        frame.take();
        command_count.fetch_sub(1, Ordering::AcqRel);
    }
}

fn drain_commands(
    commands: &mpsc::Receiver<Command>,
    controls: &mut std::collections::VecDeque<Control>,
    command_count: &AtomicUsize,
    mut retain_close: bool,
) {
    retain_close &= !controls.iter().any(|control| control.opcode == 0x8);
    while let Ok(command) = commands.try_recv() {
        match command {
            Command::Control {
                opcode: 0x8,
                payload,
            } if retain_close => {
                controls.push_back(Control {
                    opcode: 0x8,
                    payload,
                });
                retain_close = false;
            }
            Command::Data { .. } | Command::Control { .. } => {
                command_count.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
}

/// How long a finished pump keeps reading after its half-close while it waits
/// for the peer's FIN.
#[cfg(not(test))]
const CLOSE_LINGER: Duration = Duration::from_secs(5);
#[cfg(test)]
const CLOSE_LINGER: Duration = Duration::from_secs(2);

/// Close TCP cleanly once the WebSocket exchange is finished. Every frame the
/// pump will ever write is already in the kernel, possibly behind a slow
/// reader. Half-close writing so FIN follows those bytes, then read and discard
/// until the peer's FIN, abort, or `CLOSE_LINGER`. Shutting down the receive
/// side or closing the last handle while inbound bytes are unread instead
/// sends RST (every platform on close; Windows also on `SD_RECEIVE`), and RST
/// discards our unsent send queue — including the Close or 1009 the peer has
/// not read yet.
// @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — half-close and drain before teardown
#[allow(clippy::too_many_arguments)]
fn linger_close(
    poll: &mut Poll,
    events: &mut Events,
    wire: &mut Wire,
    registered: &mut bool,
    shutdown: &StdTcpStream,
    signal: &AbortSignal,
    state: &Mutex<SendState>,
) {
    state
        .lock()
        .expect("WebSocket sender poisoned")
        .graceful_teardown = true;
    let _ = shutdown.shutdown(Shutdown::Write);
    let deadline = Instant::now() + CLOSE_LINGER;
    let interest = if *registered {
        poll.registry()
            .reregister(wire.tcp_mut(), SOCKET_TOKEN, Interest::READABLE)
    } else {
        poll.registry()
            .register(wire.tcp_mut(), SOCKET_TOKEN, Interest::READABLE)
    };
    if interest.is_ok() {
        *registered = true;
        let mut scratch = [0u8; 16 << 10];
        'linger: loop {
            if signal.aborted() {
                break;
            }
            loop {
                // A peer that keeps sending never yields WouldBlock, so the
                // cap and abort are checked after every read, not only when
                // the socket goes quiet.
                if signal.aborted() || Instant::now() >= deadline {
                    break 'linger;
                }
                match wire.tcp_mut().read(&mut scratch) {
                    Ok(0) => break 'linger,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break 'linger,
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match poll.poll(events, Some(left)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    }
    let _ = stop_polling_tcp(poll, wire, registered);
    let _ = shutdown.shutdown(Shutdown::Both);
}

#[cfg(test)]
struct PumpExit(Option<Arc<PumpObserver>>);

#[cfg(test)]
impl Drop for PumpExit {
    fn drop(&mut self) {
        if let Some(observer) = &self.0 {
            observer.parked.store(false, Ordering::Release);
            observer.exited.store(true, Ordering::Release);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pump_loop(
    mut wire: Wire,
    buffered_input: Vec<u8>,
    limit: usize,
    commands: mpsc::Receiver<Command>,
    requests: mpsc::Receiver<ReceiveRequest>,
    mut poll: Poll,
    _wake: Arc<Waker>,
    shutdown: StdTcpStream,
    buffered: Arc<AtomicUsize>,
    command_count: Arc<AtomicUsize>,
    state: Arc<Mutex<SendState>>,
    signal: AbortSignal,
    #[cfg(test)] writer_gate: Option<Arc<(Mutex<bool>, std::sync::Condvar)>>,
    #[cfg(test)] pump_observer: Option<Arc<PumpObserver>>,
) {
    #[cfg(test)]
    let _exit = PumpExit(pump_observer.clone());
    #[cfg(test)]
    if let Some(observer) = &pump_observer {
        let _ = observer.send_state.set(Arc::clone(&state));
    }
    let mut events = Events::with_capacity(8);
    let mut input = Input::new(buffered_input);
    let mut terminal_drain = TerminalDrain::default();
    let mut request: Option<(ReceiveRequest, ReceiveOperation)> = None;
    let mut controls = std::collections::VecDeque::new();
    let mut active: Option<ActiveData> = None;
    let mut frame: Option<OutgoingFrame> = None;
    let mut close = CloseState::new();
    let mut read_ready = !input.available().is_empty();
    let mut write_ready = true;
    let mut tcp_registered = true;
    let mut write_only = false;
    let mut stall_deadline: Option<Instant> = None;

    loop {
        let mut reschedule_read = false;
        if let Err(error) = signal.check() {
            fail_pump(&mut request, &requests, error, &state, &shutdown);
            return;
        }
        if stall_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            fail_pump(
                &mut request,
                &requests,
                HostError::Failed("the socket write made no progress before its deadline".into()),
                &state,
                &shutdown,
            );
            return;
        }

        let stop_data = close.read != ReadSide::Open || close.finish_after_close_sent;
        if close.close_sent {
            drain_commands(&commands, &mut controls, &command_count, false);
        } else if stop_data {
            drain_commands(
                &commands,
                &mut controls,
                &command_count,
                !is_close_frame(&frame),
            );
        }
        if stop_data && frame.is_none() {
            abandon_active(&mut active, &command_count);
        }

        if request.is_none() {
            match requests.try_recv() {
                Ok(next) => {
                    let binary_payload = next.binary_payload;
                    request = Some((
                        next,
                        ReceiveOperation {
                            binary_payload,
                            message: None,
                        },
                    ));
                    // A readable edge may have been paused while there was no
                    // receive demand. A nonblocking read rechecks it safely.
                    read_ready = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    if close.close_sent {
                        linger_close(
                            &mut poll,
                            &mut events,
                            &mut wire,
                            &mut tcp_registered,
                            &shutdown,
                            &signal,
                            &state,
                        );
                    } else {
                        let _ = shutdown.shutdown(Shutdown::Both);
                    }
                    return;
                }
            }
        }

        let mut read_wanted = 0;
        let mut read_frames = MAX_READ_FRAMES_PER_TURN;
        if close.read == ReadSide::Open {
            if let Some((_, operation)) = request.as_mut() {
                match parse_available(
                    operation,
                    &mut input,
                    &mut controls,
                    &command_count,
                    &state,
                    limit,
                    close.close_started,
                    &mut read_frames,
                ) {
                    Ok(Parse::Received(received)) => {
                        match received {
                            received @ Received::Closed { .. } => {
                                close.receive_close();
                                #[cfg(test)]
                                if let Some(observer) = &pump_observer {
                                    observer.close_received.store(true, Ordering::Release);
                                }
                                if close.close_sent || close.write == WriteSide::Done {
                                    publish_terminal(&mut request, &requests, Ok(received), &state);
                                } else {
                                    defer_terminal(Ok(received), &state);
                                }
                            }
                            Received::TooLarge => {
                                discard_unsent_close_frame(&mut frame, &wire, &command_count);
                                close.read = ReadSide::Done;
                                close.finish_after_close_sent = true;
                                if close.close_sent || close.write == WriteSide::Done {
                                    publish_terminal(
                                        &mut request,
                                        &requests,
                                        Ok(Received::TooLarge),
                                        &state,
                                    );
                                } else {
                                    defer_terminal(Ok(Received::TooLarge), &state);
                                }
                            }
                            received => {
                                let (finished, _) = request.take().unwrap();
                                let _ = finished.reply.send(Ok(received));
                            }
                        }
                        reschedule_read = true;
                    }
                    Ok(Parse::NeedData(wanted)) => read_wanted = wanted,
                    Ok(Parse::Yield) => reschedule_read = true,
                    Err(error) => {
                        fail_pump(&mut request, &requests, error, &state, &shutdown);
                        return;
                    }
                }
            }
        }

        if close.read == ReadSide::Closing && !reschedule_read {
            match parse_terminal_drain(
                &mut terminal_drain,
                &mut input,
                &mut controls,
                &command_count,
                &state,
                limit,
                close.close_started,
                &mut read_frames,
            ) {
                Ok(DrainParse::NeedData(wanted)) => read_wanted = wanted,
                Ok(DrainParse::Progress | DrainParse::Yield) => reschedule_read = true,
                Ok(DrainParse::Closed(received)) => {
                    close.receive_close();
                    #[cfg(test)]
                    if let Some(observer) = &pump_observer {
                        observer.close_received.store(true, Ordering::Release);
                    }
                    if close.close_sent || close.write == WriteSide::Done {
                        publish_terminal(&mut request, &requests, Ok(received), &state);
                    } else {
                        defer_terminal(Ok(received), &state);
                    }
                    reschedule_read = true;
                }
                Ok(DrainParse::TooLarge) => {
                    discard_unsent_close_frame(&mut frame, &wire, &command_count);
                    close.read = ReadSide::Done;
                    close.finish_after_close_sent = true;
                    if close.close_sent || close.write == WriteSide::Done {
                        publish_terminal(&mut request, &requests, Ok(Received::TooLarge), &state);
                    } else {
                        defer_terminal(Ok(Received::TooLarge), &state);
                    }
                    reschedule_read = true;
                }
                Err(error) => {
                    fail_pump(&mut request, &requests, error, &state, &shutdown);
                    return;
                }
            }
        }

        let background_read = close.read == ReadSide::Closing
            || close.peer_fin
            || close.close_sent
            || command_count.load(Ordering::Acquire) != 0
            || frame.is_some()
            || wire.wants_write()
            || active.is_some();
        if request.is_none() && close.read == ReadSide::Open && !reschedule_read && background_read
        {
            let mut frames = MAX_READ_FRAMES_PER_TURN;
            match parse_control_prefix(
                &mut input,
                &mut controls,
                &command_count,
                &state,
                close.close_started,
                &mut frames,
            ) {
                Ok(ControlParse::NeedData(wanted)) => read_wanted = wanted,
                Ok(ControlParse::Data) => {
                    // Preserve receive backpressure and the binary
                    // head-only contract until demand is posted.
                    read_ready = false;
                }
                Ok(ControlParse::Progress | ControlParse::Yield) => {
                    reschedule_read = true;
                }
                Ok(ControlParse::Closed(received)) => {
                    close.receive_close();
                    #[cfg(test)]
                    if let Some(observer) = &pump_observer {
                        observer.close_received.store(true, Ordering::Release);
                    }
                    if close.close_sent || close.write == WriteSide::Done {
                        publish_terminal(&mut request, &requests, Ok(received), &state);
                    } else {
                        defer_terminal(Ok(received), &state);
                    }
                    reschedule_read = true;
                }
                Err(error) => {
                    fail_pump(&mut request, &requests, error, &state, &shutdown);
                    return;
                }
            }
        }

        let should_read = read_ready
            && close.read != ReadSide::Done
            && !reschedule_read
            && (request.is_some() || background_read);
        if should_read {
            let wanted = read_wanted.clamp(1, FRAGMENT);
            match wire.read_into(&mut input, wanted) {
                Ok(ReadProgress::Bytes) => reschedule_read = true,
                Ok(ReadProgress::Blocked) if close.read == ReadSide::Open => read_ready = false,
                Ok(ReadProgress::Blocked | ReadProgress::Eof) | Err(_) => {
                    if signal.aborted() {
                        fail_pump(
                            &mut request,
                            &requests,
                            signal.check().unwrap_err(),
                            &state,
                            &shutdown,
                        );
                        return;
                    }
                    close.read = ReadSide::Done;
                    if !close.close_received {
                        let admitted_close =
                            state.lock().expect("WebSocket sender poisoned").admission
                                == AdmissionState::Closing;
                        if admitted_close && !close.close_sent && close.write == WriteSide::Open {
                            defer_terminal(Ok(abnormal_close()), &state);
                        } else {
                            publish_terminal(&mut request, &requests, Ok(abnormal_close()), &state);
                        }
                    }
                    reschedule_read = true;
                }
            }
        }

        // A write-side close is independent of a peer FIN. Give readable work
        // above one turn to retain a Close, then stop because no reply can be
        // put on the wire.
        if close.write == WriteSide::Done {
            begin_terminal_drain(
                &mut close,
                &mut terminal_drain,
                &mut request,
                #[cfg(test)]
                pump_observer.as_deref(),
            );
            if close.read == ReadSide::Closing && !close.close_received {
                continue;
            }
            publish_terminal(&mut request, &requests, Ok(abnormal_close()), &state);
            let _ = stop_polling_tcp(&poll, &mut wire, &mut tcp_registered);
            let _ = shutdown.shutdown(Shutdown::Both);
            return;
        }

        let stop_data = close.read != ReadSide::Open || close.finish_after_close_sent;
        if stop_data && frame.is_none() {
            abandon_active(&mut active, &command_count);
        }
        if frame.is_none() && !wire.wants_write() && !close.close_sent {
            if let Some(control) = controls.pop_front() {
                match masked_frame(true, control.opcode, &control.payload) {
                    Ok(bytes) => {
                        frame = Some(OutgoingFrame {
                            bytes,
                            at: 0,
                            kind: FrameKind::Control {
                                opcode: control.opcode,
                            },
                        });
                    }
                    Err(error) => {
                        fail_pump(
                            &mut request,
                            &requests,
                            HostError::Failed(error.to_string()),
                            &state,
                            &shutdown,
                        );
                        return;
                    }
                }
            } else if !stop_data {
                if active.is_none() {
                    match commands.try_recv() {
                        Ok(Command::Data {
                            opcode,
                            payload,
                            accounted,
                        }) => {
                            active = Some(ActiveData {
                                opcode,
                                payload,
                                accounted,
                                next: 0,
                            });
                        }
                        Ok(Command::Control { opcode, payload }) => {
                            match masked_frame(true, opcode, &payload) {
                                Ok(bytes) => {
                                    frame = Some(OutgoingFrame {
                                        bytes,
                                        at: 0,
                                        kind: FrameKind::Control { opcode },
                                    });
                                }
                                Err(error) => {
                                    fail_pump(
                                        &mut request,
                                        &requests,
                                        HostError::Failed(error.to_string()),
                                        &state,
                                        &shutdown,
                                    );
                                    return;
                                }
                            }
                        }
                        Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {}
                    }
                }
                if frame.is_none() {
                    if let Some(data) = active.as_mut() {
                        match data.frame() {
                            Ok((bytes, final_fragment)) => {
                                frame = Some(OutgoingFrame {
                                    bytes,
                                    at: 0,
                                    kind: FrameKind::Data { final_fragment },
                                });
                            }
                            Err(error) => {
                                fail_pump(
                                    &mut request,
                                    &requests,
                                    HostError::Failed(error.to_string()),
                                    &state,
                                    &shutdown,
                                );
                                return;
                            }
                        }
                    }
                }
            }
        }

        let writes_allowed = {
            #[cfg(test)]
            {
                writer_gate
                    .as_ref()
                    .is_none_or(|gate| *gate.0.lock().expect("WebSocket writer gate poisoned"))
            }
            #[cfg(not(test))]
            {
                true
            }
        };
        let pending_before_write = frame.is_some() || wire.wants_write();
        if pending_before_write {
            stall_deadline.get_or_insert_with(|| Instant::now() + WRITE_STALL_TIMEOUT);
        }
        let mut retry_write = false;
        if writes_allowed {
            if let Some(outgoing) = frame.as_mut() {
                let can_attempt = match &wire {
                    Wire::Plain(_) => write_ready,
                    Wire::Tls { conn, .. } => !conn.wants_write() || write_ready,
                };
                let close_frame = matches!(&outgoing.kind, FrameKind::Control { opcode: 0x8 });
                let at_before = outgoing.at;
                match can_attempt.then(|| wire.write_frame(outgoing)).transpose() {
                    Ok(None) => {}
                    Ok(Some(WriteProgress::Eof)) => {
                        close.write = WriteSide::Done;
                        begin_terminal_drain(
                            &mut close,
                            &mut terminal_drain,
                            &mut request,
                            #[cfg(test)]
                            pump_observer.as_deref(),
                        );
                        defer_terminal(Ok(abnormal_close()), &state);
                    }
                    Ok(Some(WriteProgress::Network(_bytes))) => {
                        let progressed_at = Instant::now();
                        stall_deadline = Some(progressed_at + WRITE_STALL_TIMEOUT);
                        retry_write = true;
                        #[cfg(test)]
                        if let Some(observer) = &pump_observer {
                            observer.network_bytes.fetch_add(_bytes, Ordering::AcqRel);
                            observer.write_blocked.store(false, Ordering::Release);
                            *observer
                                .last_write_progress
                                .lock()
                                .expect("WebSocket progress observer poisoned") =
                                Some(progressed_at);
                        }
                    }
                    Ok(Some(WriteProgress::Retry)) => retry_write = true,
                    Ok(Some(WriteProgress::Blocked)) => {
                        write_ready = false;
                        #[cfg(test)]
                        if let Some(observer) = &pump_observer {
                            observer.write_blocked.store(true, Ordering::Release);
                        }
                    }
                    Err(error) => {
                        close.write = WriteSide::Done;
                        begin_terminal_drain(
                            &mut close,
                            &mut terminal_drain,
                            &mut request,
                            #[cfg(test)]
                            pump_observer.as_deref(),
                        );
                        defer_terminal(
                            Err(HostError::Failed(format!(
                                "the socket write failed: {error}"
                            ))),
                            &state,
                        );
                    }
                }
                if close_frame && outgoing.at != at_before {
                    close.close_started = true;
                }
                if close.write == WriteSide::Open && wire.frame_flushed(outgoing) {
                    let completed = frame.take().unwrap();
                    match completed.kind {
                        FrameKind::Data { final_fragment } if final_fragment => {
                            finish_data(&mut active, &buffered, &command_count, &state);
                        }
                        FrameKind::Data { .. } if stop_data => {
                            abandon_active(&mut active, &command_count);
                        }
                        FrameKind::Data { .. } => {}
                        FrameKind::Control { opcode: 0x8 } => {
                            command_count.fetch_sub(1, Ordering::AcqRel);
                            close.close_started = true;
                            close.close_sent = true;
                            drop_controls(&mut controls, &command_count);
                            abandon_active(&mut active, &command_count);
                            drain_commands(&commands, &mut controls, &command_count, false);
                            release_terminal(&mut request, &requests, &state);
                        }
                        FrameKind::Control { .. } => {
                            command_count.fetch_sub(1, Ordering::AcqRel);
                        }
                    }
                    retry_write = true;
                }
            } else if wire.wants_write() && write_ready {
                match wire.write_tls_pending() {
                    Ok(WriteProgress::Eof) => {
                        close.write = WriteSide::Done;
                        begin_terminal_drain(
                            &mut close,
                            &mut terminal_drain,
                            &mut request,
                            #[cfg(test)]
                            pump_observer.as_deref(),
                        );
                        defer_terminal(Ok(abnormal_close()), &state);
                    }
                    Ok(WriteProgress::Network(_bytes)) => {
                        let progressed_at = Instant::now();
                        stall_deadline = Some(progressed_at + WRITE_STALL_TIMEOUT);
                        retry_write = true;
                        #[cfg(test)]
                        if let Some(observer) = &pump_observer {
                            observer.network_bytes.fetch_add(_bytes, Ordering::AcqRel);
                            observer.write_blocked.store(false, Ordering::Release);
                            *observer
                                .last_write_progress
                                .lock()
                                .expect("WebSocket progress observer poisoned") =
                                Some(progressed_at);
                        }
                    }
                    Ok(WriteProgress::Retry) => retry_write = true,
                    Ok(WriteProgress::Blocked) => {
                        write_ready = false;
                        #[cfg(test)]
                        if let Some(observer) = &pump_observer {
                            observer.write_blocked.store(true, Ordering::Release);
                        }
                    }
                    Err(error) => {
                        close.write = WriteSide::Done;
                        begin_terminal_drain(
                            &mut close,
                            &mut terminal_drain,
                            &mut request,
                            #[cfg(test)]
                            pump_observer.as_deref(),
                        );
                        defer_terminal(
                            Err(HostError::Failed(format!(
                                "the socket write failed: {error}"
                            ))),
                            &state,
                        );
                    }
                }
            }
        }

        if close.write == WriteSide::Done {
            begin_terminal_drain(
                &mut close,
                &mut terminal_drain,
                &mut request,
                #[cfg(test)]
                pump_observer.as_deref(),
            );
            if close.read == ReadSide::Closing && !close.close_received {
                continue;
            }
            publish_terminal(&mut request, &requests, Ok(abnormal_close()), &state);
            let _ = stop_polling_tcp(&poll, &mut wire, &mut tcp_registered);
            let _ = shutdown.shutdown(Shutdown::Both);
            return;
        }
        let pending_write = frame.is_some() || wire.wants_write();
        if !pending_write {
            stall_deadline = None;
        }
        if (close.finish_after_close_sent && close.close_sent && !pending_write)
            || (close.read == ReadSide::Done
                && !pending_write
                && controls.is_empty()
                && command_count.load(Ordering::Acquire) == 0)
        {
            linger_close(
                &mut poll,
                &mut events,
                &mut wire,
                &mut tcp_registered,
                &shutdown,
                &signal,
                &state,
            );
            return;
        }

        // Read/parse work is bounded. Writes and their no-progress deadline
        // get a turn before another local input batch.
        if reschedule_read || retry_write {
            continue;
        }
        if command_count.load(Ordering::Acquire) != 0 && !pending_write {
            continue;
        }
        if close.read == ReadSide::Done {
            match poll_writes_only(&poll, &mut wire, &mut write_only) {
                Ok(true) => {
                    // mio readiness is edge-triggered on epoll. The socket may
                    // already be writable when EPOLL_CTL_MOD changes the
                    // interest set, so probe it before waiting for an edge
                    // which need not recur.
                    write_ready = true;
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    fail_pump(&mut request, &requests, error, &state, &shutdown);
                    return;
                }
            }
        }
        #[cfg(test)]
        if let Some(observer) = &pump_observer {
            observer.parked.store(true, Ordering::Release);
        }
        let timeout =
            stall_deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let waited = poll.poll(&mut events, timeout);
        #[cfg(test)]
        if let Some(observer) = &pump_observer {
            observer.parked.store(false, Ordering::Release);
            observer.returns.fetch_add(1, Ordering::AcqRel);
        }
        match waited {
            Ok(()) => {
                // Waker and socket readiness can be coalesced. After every
                // wake, retry nonblocking I/O and drain it to WouldBlock so a
                // readiness edge cannot be consumed only by the poll call.
                read_ready = true;
                write_ready = true;
                for event in &events {
                    if event.token() != SOCKET_TOKEN {
                        continue;
                    }
                    read_ready |= event.is_readable();
                    write_ready |= event.is_writable();
                    if event.is_read_closed() && close.read != ReadSide::Done {
                        // A peer FIN is an in-order end of the byte stream,
                        // not a terminal condition: data frames ahead of it
                        // still belong to receive demand. Only parse leading
                        // control frames without demand; EOF itself is
                        // processed when a read reaches it.
                        // @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — read FIN keeps queued messages
                        close.peer_fin = true;
                        read_ready = true;
                        #[cfg(test)]
                        if let Some(observer) = &pump_observer {
                            observer.peer_fin.store(true, Ordering::Release);
                        }
                    }
                    if event.is_write_closed() {
                        close.write = WriteSide::Done;
                        begin_terminal_drain(
                            &mut close,
                            &mut terminal_drain,
                            &mut request,
                            #[cfg(test)]
                            pump_observer.as_deref(),
                        );
                        read_ready = true;
                    }
                    if event.is_error() {
                        // The next nonblocking operation obtains the concrete
                        // error without treating a read HUP as a write HUP.
                        read_ready = true;
                        write_ready = true;
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                fail_pump(
                    &mut request,
                    &requests,
                    HostError::Failed(format!("the socket readiness wait failed: {error}")),
                    &state,
                    &shutdown,
                );
                return;
            }
        }
    }
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

#[derive(Clone)]
enum Received {
    Text(String),
    Binary(Vec<u8>),
    BinaryLength(usize),
    TooLarge,
    Closed { code: u16, reason: String },
}

impl Socket {
    fn receive(&mut self, binary_payload: bool) -> Result<Received, HostError> {
        self.signal.check()?;
        let mut state = self.sender.state.lock().expect("WebSocket sender poisoned");
        if let Some(terminal) = state.terminal.as_mut() {
            if terminal.deliverable && !terminal.delivered {
                terminal.delivered = true;
                return terminal.result.clone();
            }
        }
        if state.admission == AdmissionState::Closed
            && state
                .terminal
                .as_ref()
                .is_none_or(|terminal| terminal.deliverable)
        {
            return Ok(abnormal_close());
        }
        let (reply, received) = mpsc::sync_channel(1);
        if self
            .requests
            .send(ReceiveRequest {
                binary_payload,
                reply,
            })
            .is_err()
        {
            state.admission = AdmissionState::Closed;
            return Ok(abnormal_close());
        }
        if let Err(error) = self.wake.wake() {
            state.admission = AdmissionState::Closed;
            drop(state);
            let _ = self.shutdown.shutdown(Shutdown::Both);
            return Err(HostError::Failed(format!(
                "the socket wake notification failed: {error}"
            )));
        }
        drop(state);
        match received.recv() {
            Ok(result) => result,
            Err(_) => {
                self.signal.check()?;
                let mut state = self.sender.state.lock().expect("WebSocket sender poisoned");
                if let Some(terminal) = state.terminal.as_mut() {
                    if terminal.deliverable && !terminal.delivered {
                        terminal.delivered = true;
                        return terminal.result.clone();
                    }
                }
                Ok(abnormal_close())
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
        if self.sender.admission() == AdmissionState::Open && !self.signal.aborted() {
            let _ = self.sender.close(Some(1000), "");
        }
        self.sender.mark_closed();
        // After its Close reaches TCP the pump half-closes and finishes TCP
        // itself; a receive-side shutdown here could reset those queued
        // final bytes.
        let graceful = self
            .sender
            .state
            .lock()
            .expect("WebSocket sender poisoned")
            .graceful_teardown;
        if !graceful {
            let _ = self.shutdown.shutdown(Shutdown::Both);
        }
        let _ = self.wake.wake();
    }
}

#[cfg(any(test, feature = "test-support"))]
#[path = "websocket_tests.rs"]
pub mod tests;
