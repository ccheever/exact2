//! A WebSocket client shared by Rust consumers and the JavaScript binding.
//!
//! The receive-only API (`open`, `MessageSource::next`, and `Incoming`) stays
//! source-compatible. Sending and event watching are additive: transports
//! expose a clonable sender, while a watch publishes the WHATWG lifecycle over
//! L3's executor-independent `Receiver`/`Subscription` pair.
//!
//! @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — one WebSocket contract on both doors
//! @ref LLP 0057.000#l4--websocket--completed-2026-10-04 — Apple keeps its platform task; portable framing stays in Rust

use crate::boundary::HostError;
use crate::grant::{GrantSet, Operation, Origin};
use crate::stdlib::abort::{AbortController, AbortSignal};
use crate::stdlib::events::Subscription;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSING: u8 = 2;
const CLOSED: u8 = 3;

/// What the receive-only Rust API observes next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// One whole text message (fragments joined), UTF-8.
    Text(String),
    /// A binary message. This legacy variant intentionally carries only the
    /// length; [`MessageSource::next_event`] is the payload-bearing path.
    Binary(usize),
    /// A message over the consumer's ceiling; the socket is done.
    TooLarge,
    /// The socket closed: the far side's close code and reason, or 1006
    /// when the connection ended without a closing handshake.
    Closed { code: u16, reason: String },
}

/// A complete WebSocket message for event consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
}

/// The events produced by [`watch`], in transport order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open {
        protocol: String,
    },
    Message(Message),
    Error(String),
    Close {
        code: u16,
        reason: String,
        was_clean: bool,
    },
}

/// The concurrently usable sending half of an open transport socket.
pub trait MessageSender: Send + Sync {
    fn send_text(&self, text: &str) -> Result<(), HostError>;
    fn send_binary(&self, bytes: &[u8]) -> Result<(), HostError>;
    fn close(&self, code: Option<u16>, reason: &str) -> Result<(), HostError>;
    fn buffered_amount(&self) -> usize;
}

/// An open socket. Dropping it closes the connection.
pub trait MessageSource: Send {
    /// Block for what comes next. An `Err` is an abort or a protocol
    /// violation; every other end is `Closed`.
    fn next(&mut self) -> Result<Incoming, HostError>;

    /// The payload-bearing receive path used by watches. Receive-only custom
    /// transports remain source-compatible; they get a precise refusal if a
    /// binary payload is unavailable through their legacy `next` method.
    fn next_event(&mut self) -> Result<Event, HostError> {
        match self.next()? {
            Incoming::Text(text) => Ok(Event::Message(Message::Text(text))),
            Incoming::Binary(_) => Err(HostError::Failed(
                "the socket transport did not expose binary message bytes".into(),
            )),
            Incoming::TooLarge => Err(HostError::Failed(
                "the socket message exceeded its configured limit".into(),
            )),
            Incoming::Closed { code, reason } => Ok(Event::Close {
                code,
                reason,
                was_clean: code != 1006,
            }),
        }
    }

    /// A sending half usable while another thread blocks in `next_event`.
    /// Existing receive-only transports may omit it.
    fn sender(&self) -> Option<Arc<dyn MessageSender>> {
        None
    }

    /// The subprotocol selected by the peer, or the empty string.
    fn protocol(&self) -> &str {
        ""
    }

    fn send_text(&self, text: &str) -> Result<(), HostError> {
        self.sender()
            .ok_or(HostError::Unavailable {
                feature: "WebSocket send on this transport",
            })?
            .send_text(text)
    }

    fn send_binary(&self, bytes: &[u8]) -> Result<(), HostError> {
        self.sender()
            .ok_or(HostError::Unavailable {
                feature: "WebSocket send on this transport",
            })?
            .send_binary(bytes)
    }

    fn close(&self, code: u16, reason: &str) -> Result<(), HostError> {
        validate_close(code, reason)?;
        self.sender()
            .ok_or(HostError::Unavailable {
                feature: "WebSocket send on this transport",
            })?
            .close(Some(code), reason)
    }

    fn buffered_amount(&self) -> usize {
        self.sender().map_or(0, |sender| sender.buffered_amount())
    }
}

/// The platform half: open `url` (already admitted, `ws` or `wss`) and
/// complete the opening handshake. Aborting `signal` interrupts the open and
/// every later read.
pub trait SocketTransport: Send + Sync {
    fn connect(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError>;

    /// Additive protocol-aware open. A receive-only custom transport keeps
    /// working for the empty protocol list.
    fn connect_with_protocols(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
        protocols: &[String],
    ) -> Result<Box<dyn MessageSource>, HostError> {
        if !protocols.is_empty() {
            return Err(HostError::Unavailable {
                feature: "WebSocket subprotocols on this transport",
            });
        }
        self.connect(url, max_message, signal)
    }
}

fn parse_and_admit(grants: &GrantSet, value: &str) -> Result<url::Url, HostError> {
    let parsed = url::Url::parse(value)
        .map_err(|error| HostError::Failed(format!("SyntaxError: invalid socket URL: {error}")))?;
    if !matches!(parsed.scheme(), "ws" | "wss") || parsed.fragment().is_some() {
        return Err(HostError::Failed(
            "SyntaxError: a socket URL is ws: or wss: with no fragment".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| HostError::Failed("SyntaxError: socket URL has no host".into()))?;
    let port = parsed.port_or_known_default().unwrap_or(0);
    crate::boundary::admit(
        grants,
        &Operation::WebSocket {
            origin: Origin::new(parsed.scheme(), host, port),
        },
    )?;
    Ok(parsed)
}

/// Validate the `Sec-WebSocket-Protocol` values before a platform sees them.
/// RFC 6455 uses the HTTP token grammar and WHATWG rejects duplicates.
pub fn validate_protocols(protocols: &[String]) -> Result<(), HostError> {
    fn token(value: &str) -> bool {
        !value.is_empty()
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#'
                            | b'$'
                            | b'%'
                            | b'&'
                            | b'\''
                            | b'*'
                            | b'+'
                            | b'-'
                            | b'.'
                            | b'^'
                            | b'_'
                            | b'`'
                            | b'|'
                            | b'~'
                    )
            })
    }
    for (index, protocol) in protocols.iter().enumerate() {
        if !token(protocol) {
            return Err(HostError::InvalidArgument(
                "a WebSocket protocol must be a non-empty HTTP token".into(),
            ));
        }
        if protocols[..index]
            .iter()
            .any(|other| other.eq_ignore_ascii_case(protocol))
        {
            return Err(HostError::InvalidArgument(
                "WebSocket protocols must not contain duplicates".into(),
            ));
        }
    }
    Ok(())
}

/// Admit and open a socket to `url`, preserving the receive-only Rust API.
pub fn open(
    transport: &dyn SocketTransport,
    grants: &GrantSet,
    url: &str,
    max_message: usize,
    signal: &AbortSignal,
) -> Result<Box<dyn MessageSource>, HostError> {
    open_with_protocols(transport, grants, url, max_message, signal, &[])
}

/// Protocol-aware spelling used by the event and JavaScript projections.
pub fn open_with_protocols(
    transport: &dyn SocketTransport,
    grants: &GrantSet,
    url: &str,
    max_message: usize,
    signal: &AbortSignal,
    protocols: &[String],
) -> Result<Box<dyn MessageSource>, HostError> {
    signal.check()?;
    validate_protocols(protocols)?;
    let parsed = parse_and_admit(grants, url)?;
    transport.connect_with_protocols(&parsed, max_message, signal, protocols)
}

/// A watch's sending/control handle. It is usable before the opening worker
/// finishes so `close()` can cancel a CONNECTING socket.
#[derive(Clone)]
pub struct Connection {
    inner: Arc<ConnectionState>,
}

struct ConnectionState {
    phase: std::sync::atomic::AtomicU8,
    sender: Mutex<Option<Arc<dyn MessageSender>>>,
    discarded: std::sync::atomic::AtomicUsize,
    abort: AbortController,
    local_close_without_code: AtomicBool,
    protocol: Mutex<String>,
    close: Mutex<(u16, String, bool)>,
}

impl Connection {
    fn new() -> Self {
        Self {
            inner: Arc::new(ConnectionState {
                phase: std::sync::atomic::AtomicU8::new(CONNECTING),
                sender: Mutex::new(None),
                discarded: std::sync::atomic::AtomicUsize::new(0),
                abort: AbortController::new(),
                local_close_without_code: AtomicBool::new(false),
                protocol: Mutex::new(String::new()),
                close: Mutex::new((1006, String::new(), false)),
            }),
        }
    }

    pub fn ready_state(&self) -> u8 {
        self.inner.phase.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn protocol(&self) -> String {
        self.inner.protocol.lock().unwrap().clone()
    }

    pub fn close_info(&self) -> (u16, String, bool) {
        self.inner.close.lock().unwrap().clone()
    }

    pub fn buffered_amount(&self) -> usize {
        let transport = self
            .inner
            .sender
            .lock()
            .unwrap()
            .as_ref()
            .map_or(0, |sender| sender.buffered_amount());
        transport.saturating_add(
            self.inner
                .discarded
                .load(std::sync::atomic::Ordering::Acquire),
        )
    }

    pub fn send_text(&self, text: &str) -> Result<(), HostError> {
        self.send(text.as_bytes(), |sender| sender.send_text(text))
    }

    pub fn send_binary(&self, bytes: &[u8]) -> Result<(), HostError> {
        self.send(bytes, |sender| sender.send_binary(bytes))
    }

    fn send(
        &self,
        bytes: &[u8],
        send: impl FnOnce(&dyn MessageSender) -> Result<(), HostError>,
    ) -> Result<(), HostError> {
        match self.ready_state() {
            CONNECTING => Err(HostError::Failed(
                "InvalidStateError: the WebSocket is still connecting".into(),
            )),
            OPEN => {
                let sender = self.inner.sender.lock().unwrap();
                send(
                    sender.as_deref().ok_or_else(|| {
                        HostError::Failed("the socket sender is not ready".into())
                    })?,
                )
            }
            _ => {
                saturating_add(&self.inner.discarded, bytes.len());
                Ok(())
            }
        }
    }

    pub fn close(&self, code: Option<u16>, reason: &str) -> Result<(), HostError> {
        if let Some(code) = code {
            validate_close(code, reason)?;
        } else if !reason.is_empty() {
            return Err(HostError::InvalidArgument(
                "a WebSocket close reason requires a close code".into(),
            ));
        }
        if code.is_none() {
            self.inner
                .local_close_without_code
                .store(true, Ordering::Release);
        }
        loop {
            let phase = self.ready_state();
            if phase >= CLOSING {
                return Ok(());
            }
            let next = if phase == CONNECTING { CLOSED } else { CLOSING };
            if self
                .inner
                .phase
                .compare_exchange(
                    phase,
                    next,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                )
                .is_ok()
            {
                if phase == CONNECTING {
                    self.inner.abort.abort();
                    return Ok(());
                }
                return self
                    .inner
                    .sender
                    .lock()
                    .unwrap()
                    .as_ref()
                    .ok_or_else(|| HostError::Failed("the socket sender is not ready".into()))?
                    .close(code, reason);
            }
        }
    }

    pub fn cancel(&self) {
        self.inner
            .phase
            .store(CLOSED, std::sync::atomic::Ordering::Release);
        self.inner.abort.abort();
    }

    fn finish(&self, code: u16, reason: String, clean: bool) {
        *self.inner.close.lock().unwrap() = (code, reason, clean);
        self.inner
            .phase
            .store(CLOSED, std::sync::atomic::Ordering::Release);
    }
}

fn saturating_add(amount: &std::sync::atomic::AtomicUsize, bytes: usize) {
    let _ = amount.fetch_update(
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
        |current| Some(current.saturating_add(bytes)),
    );
}

/// Start one event-producing socket. Admission happens before the worker is
/// spawned; a denial therefore publishes `error`, then `close(1006)`, without
/// touching the transport.
#[cfg(feature = "bindings")]
pub(crate) fn watch_with(
    transport: Arc<dyn SocketTransport>,
    grants: Arc<GrantSet>,
    url: String,
    protocols: Vec<String>,
    max_message: usize,
    publish: Arc<dyn Fn(Event) -> bool + Send + Sync>,
) -> (Connection, Subscription) {
    watch_with_liveness(
        transport,
        grants,
        url,
        protocols,
        max_message,
        publish,
        Arc::new(AtomicBool::new(true)),
    )
}

fn watch_with_liveness(
    transport: Arc<dyn SocketTransport>,
    grants: Arc<GrantSet>,
    url: String,
    protocols: Vec<String>,
    max_message: usize,
    publish: Arc<dyn Fn(Event) -> bool + Send + Sync>,
    live: Arc<AtomicBool>,
) -> (Connection, Subscription) {
    let connection = Connection::new();
    let cancel = connection.clone();
    // Clearing liveness is the unsubscribe linearization point and happens
    // before abort wakes a blocked connect/read. The gate then waits out a
    // publication already in progress, so none can complete after
    // `unsubscribe` returns. A bounded publisher observes `live` while it
    // waits for space, avoiding a full-queue teardown deadlock.
    let publication_gate = Arc::new(Mutex::new(()));
    let cancel_live = Arc::clone(&live);
    let cancel_gate = Arc::clone(&publication_gate);
    let subscription = Subscription::new(move || {
        cancel_live.store(false, Ordering::Release);
        cancel.cancel();
        let _publication = cancel_gate.lock().expect("WebSocket publication poisoned");
    });
    let publish_live = Arc::clone(&live);
    let worker_live = Arc::clone(&live);
    let publish_gate = Arc::clone(&publication_gate);
    let publish = Arc::new(move |event| {
        let _publication = publish_gate.lock().expect("WebSocket publication poisoned");
        publish_live.load(Ordering::Acquire) && publish(event)
    });
    let parsed = match validate_protocols(&protocols).and_then(|()| parse_and_admit(&grants, &url))
    {
        Ok(parsed) => parsed,
        Err(error) => {
            connection.finish(1006, String::new(), false);
            publish(Event::Error(error.to_string()));
            publish(Event::Close {
                code: 1006,
                reason: String::new(),
                was_clean: false,
            });
            return (connection, subscription);
        }
    };

    let worker_connection = connection.clone();
    std::thread::spawn(move || {
        let mut source = match transport.connect_with_protocols(
            &parsed,
            max_message,
            &worker_connection.inner.abort.signal(),
            &protocols,
        ) {
            Ok(source) => source,
            Err(error) => {
                if !worker_live.load(Ordering::Acquire) {
                    return;
                }
                worker_connection.finish(1006, String::new(), false);
                publish(Event::Error(error.to_string()));
                publish(Event::Close {
                    code: 1006,
                    reason: String::new(),
                    was_clean: false,
                });
                return;
            }
        };
        let sender = source.sender();
        let protocol = source.protocol().to_string();
        *worker_connection.inner.sender.lock().unwrap() = sender;
        *worker_connection.inner.protocol.lock().unwrap() = protocol.clone();
        if worker_connection
            .inner
            .phase
            .compare_exchange(
                CONNECTING,
                OPEN,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_err()
        {
            drop(source);
            worker_connection.finish(1006, String::new(), false);
            publish(Event::Error(
                "the WebSocket connection was closed while connecting".into(),
            ));
            publish(Event::Close {
                code: 1006,
                reason: String::new(),
                was_clean: false,
            });
            return;
        }
        if !publish(Event::Open { protocol }) {
            worker_connection.cancel();
            return;
        }
        loop {
            match source.next_event() {
                Ok(Event::Message(message)) => {
                    if worker_connection.ready_state() != OPEN {
                        continue;
                    }
                    if !publish(Event::Message(message)) {
                        worker_connection.cancel();
                        return;
                    }
                }
                Ok(Event::Close {
                    mut code,
                    reason,
                    was_clean,
                }) => {
                    // NSURLSession has no empty close-payload spelling and
                    // sends 1000 for close(). Normalize its echo to the 1005
                    // exposed by the portable empty-payload handshake.
                    if code == 1000
                        && reason.is_empty()
                        && worker_connection
                            .inner
                            .local_close_without_code
                            .load(Ordering::Acquire)
                    {
                        code = 1005;
                    }
                    worker_connection.finish(code, reason.clone(), was_clean);
                    if code == 1006 {
                        publish(Event::Error("the socket closed abnormally".into()));
                    }
                    publish(Event::Close {
                        code,
                        reason,
                        was_clean,
                    });
                    return;
                }
                Ok(_) => {}
                Err(error) => {
                    if !worker_live.load(Ordering::Acquire) {
                        return;
                    }
                    worker_connection.finish(1006, String::new(), false);
                    publish(Event::Error(error.to_string()));
                    publish(Event::Close {
                        code: 1006,
                        reason: String::new(),
                        was_clean: false,
                    });
                    return;
                }
            }
        }
    });
    (connection, subscription)
}

/// Watch a socket from Rust without selecting an executor or future type.
pub fn watch(
    transport: Arc<dyn SocketTransport>,
    grants: Arc<GrantSet>,
    url: String,
    protocols: Vec<String>,
    max_message: usize,
) -> (Connection, mpsc::Receiver<Event>, Subscription) {
    // Two slots hold the terminal error/close pair. A host that stops reading
    // backpressures the socket worker instead of accumulating messages.
    let (sender, receiver) = mpsc::sync_channel(2);
    let live = Arc::new(AtomicBool::new(true));
    let publish_live = Arc::clone(&live);
    let publish = Arc::new(move |mut event| loop {
        if !publish_live.load(Ordering::Acquire) {
            return false;
        }
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(mpsc::TrySendError::Full(returned)) => {
                event = returned;
                std::thread::yield_now();
            }
            Err(mpsc::TrySendError::Disconnected(_)) => return false,
        }
    });
    let (connection, subscription) = watch_with_liveness(
        transport,
        grants,
        url,
        protocols,
        max_message,
        publish,
        live,
    );
    (connection, receiver, subscription)
}

pub fn validate_close(code: u16, reason: &str) -> Result<(), HostError> {
    if code != 1000 && !(3000..=4999).contains(&code) {
        return Err(HostError::InvalidArgument(
            "a WebSocket close code is 1000 or in 3000..=4999".into(),
        ));
    }
    if reason.len() > 123 {
        return Err(HostError::InvalidArgument(
            "a WebSocket close reason is at most 123 UTF-8 bytes".into(),
        ));
    }
    Ok(())
}

/// The `Sec-WebSocket-Accept` a server answers `key` with (RFC 6455 §4.2.2).
pub fn accept_key(key: &str) -> String {
    use base64::Engine as _;
    let input = format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::engine::general_purpose::STANDARD.encode(sha1(input.as_bytes()))
}

/// SHA-1 (FIPS 180-4), used only for RFC 6455's opening handshake.
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

#[cfg(not(feature = "websocket"))]
struct UnavailableSocketTransport;

#[cfg(not(feature = "websocket"))]
impl SocketTransport for UnavailableSocketTransport {
    fn connect(
        &self,
        _: &url::Url,
        _: usize,
        _: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        Err(HostError::Unavailable {
            feature: "websocket",
        })
    }
}

/// This build's platform socket. The shape remains present with the feature
/// off and refuses with [`HostError::Unavailable`].
pub fn default_transport() -> Box<dyn SocketTransport> {
    #[cfg(all(feature = "websocket", target_vendor = "apple"))]
    {
        Box::new(crate::transport::darwin_websocket::DarwinSocketTransport)
    }
    #[cfg(all(feature = "websocket", not(target_vendor = "apple")))]
    {
        Box::new(crate::transport::websocket::TcpSocketTransport::new())
    }
    #[cfg(not(feature = "websocket"))]
    {
        Box::new(UnavailableSocketTransport)
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
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn a_socket_is_admitted_by_its_own_grant_and_nothing_else() {
        let url = "wss://jetstream2.us-east.bsky.network/subscribe?wantedCollections=a";
        let socket = "net.websocket wss://jetstream2.us-east.bsky.network";
        assert_eq!(opened(socket, url), "reached the transport");
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

    #[test]
    fn close_validation_is_the_whatwg_script_subset() {
        for code in [1000, 3000, 4999] {
            assert_eq!(validate_close(code, "ok"), Ok(()));
        }
        for code in [0, 999, 1001, 2999, 5000, u16::MAX] {
            assert!(matches!(
                validate_close(code, ""),
                Err(HostError::InvalidArgument(_))
            ));
        }
        assert_eq!(validate_close(1000, &"é".repeat(61)), Ok(()));
        assert!(validate_close(1000, &"é".repeat(62)).is_err());
    }

    #[test]
    fn subprotocols_are_tokens_and_unique_before_transport() {
        assert_eq!(
            validate_protocols(&["chat".into(), "superchat.v2".into()]),
            Ok(())
        );
        for protocols in [
            vec!["".into()],
            vec!["has space".into()],
            vec!["line\nbreak".into()],
            vec!["same".into(), "SAME".into()],
        ] {
            assert!(validate_protocols(&protocols).is_err(), "{protocols:?}");
        }
    }

    #[test]
    fn a_denied_watch_reports_error_then_close_without_opening() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Counting(Arc<AtomicUsize>);
        impl SocketTransport for Counting {
            fn connect(
                &self,
                _: &url::Url,
                _: usize,
                _: &AbortSignal,
            ) -> Result<Box<dyn MessageSource>, HostError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err(HostError::Failed("transport was reached".into()))
            }
        }

        let opens = Arc::new(AtomicUsize::new(0));
        let (connection, events, _subscription) = watch(
            Arc::new(Counting(Arc::clone(&opens))),
            Arc::new(GrantSet::none()),
            "ws://127.0.0.1:9/".into(),
            Vec::new(),
            1024,
        );
        assert!(matches!(events.recv().unwrap(), Event::Error(_)));
        assert_eq!(
            events.recv().unwrap(),
            Event::Close {
                code: 1006,
                reason: String::new(),
                was_clean: false,
            }
        );
        assert_eq!(connection.ready_state(), CLOSED);
        assert_eq!(opens.load(Ordering::SeqCst), 0);
    }

    struct Connecting {
        started: Arc<AtomicBool>,
    }

    impl SocketTransport for Connecting {
        fn connect(
            &self,
            _: &url::Url,
            _: usize,
            signal: &AbortSignal,
        ) -> Result<Box<dyn MessageSource>, HostError> {
            self.started.store(true, Ordering::Release);
            while !signal.aborted() {
                std::thread::yield_now();
            }
            Err(signal.check().unwrap_err())
        }
    }

    fn local_grant() -> Arc<GrantSet> {
        Arc::new(GrantSet::parse("net.websocket ws://127.0.0.1:9\n").unwrap())
    }

    #[test]
    fn close_while_connecting_never_publishes_open() {
        let started = Arc::new(AtomicBool::new(false));
        let (connection, events, _subscription) = watch(
            Arc::new(Connecting {
                started: Arc::clone(&started),
            }),
            local_grant(),
            "ws://127.0.0.1:9/".into(),
            Vec::new(),
            1024,
        );
        while !started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        connection.close(None, "").unwrap();
        assert!(matches!(events.recv().unwrap(), Event::Error(_)));
        assert!(matches!(events.recv().unwrap(), Event::Close { .. }));
        assert_eq!(connection.ready_state(), CLOSED);
        assert!(events.try_recv().is_err(), "no stale open may follow close");
    }

    #[test]
    fn unsubscribe_during_connecting_is_silent() {
        let started = Arc::new(AtomicBool::new(false));
        let (_connection, events, subscription) = watch(
            Arc::new(Connecting {
                started: Arc::clone(&started),
            }),
            local_grant(),
            "ws://127.0.0.1:9/".into(),
            Vec::new(),
            1024,
        );
        while !started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        subscription.unsubscribe();
        assert!(matches!(
            events.recv_timeout(std::time::Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    struct BlockedRead {
        signal: AbortSignal,
    }

    impl MessageSource for BlockedRead {
        fn next(&mut self) -> Result<Incoming, HostError> {
            while !self.signal.aborted() {
                std::thread::yield_now();
            }
            Err(self.signal.check().unwrap_err())
        }
    }

    struct OpensThenBlocks;

    impl SocketTransport for OpensThenBlocks {
        fn connect(
            &self,
            _: &url::Url,
            _: usize,
            signal: &AbortSignal,
        ) -> Result<Box<dyn MessageSource>, HostError> {
            Ok(Box::new(BlockedRead {
                signal: signal.clone(),
            }))
        }
    }

    #[test]
    fn unsubscribe_during_a_blocked_open_read_is_silent() {
        let (_connection, events, subscription) = watch(
            Arc::new(OpensThenBlocks),
            local_grant(),
            "ws://127.0.0.1:9/".into(),
            Vec::new(),
            1024,
        );
        assert!(matches!(events.recv().unwrap(), Event::Open { .. }));
        subscription.unsubscribe();
        assert!(matches!(
            events.recv_timeout(std::time::Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    struct FastSource {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl MessageSource for FastSource {
        fn next(&mut self) -> Result<Incoming, HostError> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            Ok(Incoming::Text("fast".into()))
        }
    }

    struct FastTransport(Arc<std::sync::atomic::AtomicUsize>);

    impl SocketTransport for FastTransport {
        fn connect(
            &self,
            _: &url::Url,
            _: usize,
            _: &AbortSignal,
        ) -> Result<Box<dyn MessageSource>, HostError> {
            Ok(Box::new(FastSource {
                calls: Arc::clone(&self.0),
            }))
        }
    }

    #[test]
    fn a_host_that_stops_receiving_backpressures_the_rust_watch() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (_connection, _events, subscription) = watch(
            Arc::new(FastTransport(Arc::clone(&calls))),
            local_grant(),
            "ws://127.0.0.1:9/".into(),
            Vec::new(),
            1024,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while calls.load(Ordering::Acquire) < 2 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            calls.load(Ordering::Acquire),
            2,
            "open plus one message fill the two-slot queue; the next publish pauses"
        );
        subscription.unsubscribe();
    }

    #[cfg(not(feature = "websocket"))]
    #[test]
    fn an_omitted_transport_has_a_named_refusal() {
        let grants = GrantSet::parse("net.websocket ws://example.com\n").unwrap();
        assert_eq!(
            open(
                default_transport().as_ref(),
                &grants,
                "ws://example.com",
                1024,
                &AbortSignal::default()
            )
            .err(),
            Some(HostError::Unavailable {
                feature: "websocket"
            })
        );
    }
}
