//! HTTP/1.1 over rustls off Apple. HTTP framing and TLS remain ureq's; the
//! adapter supplies cancellable sockets and exclusive leases of pooled agents.
//! @ref LLP 0068#5-open-questions — the platform transport off Apple

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::fetch::{
    over_limit, Body, BodySource, Headers, Request, StreamingResponse, Transport,
};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout,
    Transport as WireTransport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Roots {
    System(usize),
    CompiledIn,
}
impl std::fmt::Display for Roots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System(n) => write!(f, "the system's trust store ({n} roots)"),
            Self::CompiledIn => write!(f, "no system trust store; the compiled-in roots"),
        }
    }
}

#[derive(Default)]
struct Current {
    generation: u64,
    signal: Option<AbortSignal>,
}
type Context = Arc<Mutex<Current>>;
struct Slot {
    agent: ureq::Agent,
    context: Context,
}
struct Pool {
    plain_idle: Mutex<Vec<Slot>>,
    tls_idle: Mutex<Vec<Slot>>,
}
impl Pool {
    fn idle(&self, tls: bool) -> &Mutex<Vec<Slot>> {
        if tls {
            &self.tls_idle
        } else {
            &self.plain_idle
        }
    }
}
/// Idle agents retain their connection pools. Each in-flight response exclusively
/// owns its agent, so a signal never becomes ambient state shared by requests.
pub struct RustlsHttpTransport {
    pool: Arc<Pool>,
}
impl RustlsHttpTransport {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Pool {
                plain_idle: Mutex::new(Vec::new()),
                tls_idle: Mutex::new(Vec::new()),
            }),
        }
    }
    pub fn roots(&self) -> Roots {
        tls_config().1
    }
    fn lease(&self, signal: &AbortSignal, tls: bool) -> Lease {
        let slot = self
            .pool
            .idle(tls)
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| {
                let context = Arc::new(Mutex::new(Current::default()));
                let connector = ureq::unversioned::transport::ConnectProxyConnector::default()
                    .chain(CancellableConnector(context.clone()))
                    .chain(ureq::unversioned::transport::RustlsConnector::default());
                let agent = ureq::Agent::with_parts(
                    agent_config(tls),
                    connector,
                    Resolver(context.clone()),
                );
                Slot { agent, context }
            });
        {
            let mut current = slot.context.lock().unwrap();
            current.generation = current.generation.wrapping_add(1);
            current.signal = Some(signal.clone());
        }
        Lease {
            slot: Some(slot),
            pool: self.pool.clone(),
            tls,
        }
    }
}
impl Default for RustlsHttpTransport {
    fn default() -> Self {
        Self::new()
    }
}
/// Whether a request needs the TLS-capable agent (with the system roots): its
/// target is HTTPS, or the environment's proxy is reached over TLS.
fn needs_tls(target_https: bool, proxy: Option<&ureq::Proxy>) -> bool {
    target_https || proxy.is_some_and(|proxy| proxy.protocol() == ureq::ProxyProtocol::Https)
}

fn agent_config(tls: bool) -> ureq::config::Config {
    let builder = ureq::Agent::config_builder()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(15)));
    if tls {
        builder.tls_config(tls_config().0.clone()).build()
    } else {
        builder.build()
    }
}

// Every transport in the process shares the one trust-store read. Keeping the
// config out of `new` also keeps constructing an unused host endowment cheap.
// @ref LLP 0068#5-open-questions — native roots are loaded lazily and once per process
fn tls_config() -> &'static (ureq::tls::TlsConfig, Roots) {
    static TLS_CONFIG: OnceLock<(ureq::tls::TlsConfig, Roots)> = OnceLock::new();
    TLS_CONFIG.get_or_init(|| {
        #[cfg(test)]
        NATIVE_CERT_LOADS.fetch_add(1, Ordering::SeqCst);
        tls_config_from(rustls_native_certs::load_native_certs().unwrap_or_default())
    })
}

#[cfg(test)]
static NATIVE_CERT_LOADS: AtomicUsize = AtomicUsize::new(0);

fn tls_config_from(
    certs: Vec<rustls::pki_types::CertificateDer<'static>>,
) -> (ureq::tls::TlsConfig, Roots) {
    let mut store = rustls::RootCertStore::empty();
    let mut certificates = Vec::new();
    for cert in certs {
        if store.add(cert.clone()).is_ok() {
            certificates.push(ureq::tls::Certificate::from_der(cert.as_ref()).to_owned());
        }
    }
    let (root_certs, roots) = if certificates.is_empty() {
        (ureq::tls::RootCerts::WebPki, Roots::CompiledIn)
    } else {
        let count = certificates.len();
        (certificates.into(), Roots::System(count))
    };
    (
        ureq::tls::TlsConfig::builder()
            .root_certs(root_certs)
            .build(),
        roots,
    )
}
struct Lease {
    slot: Option<Slot>,
    pool: Arc<Pool>,
    tls: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let slot = self.slot.take().unwrap();
        // Serialized with callbacks: a delayed callback from this lease cannot
        // shut down a connection after it has been handed to another request.
        slot.context.lock().unwrap().signal = None;
        let mut idle = self.pool.idle(self.tls).lock().unwrap();
        if idle.len() < 8 {
            idle.push(slot);
        }
    }
}
/// Keep system DNS on the caller's executor. ureq's timeout resolver otherwise
/// spawns a detached thread per lookup. OS name resolution itself cannot be
/// interrupted; check cancellation and elapsed time on both sides of it.
struct Resolver(Context);
impl std::fmt::Debug for Resolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SystemResolver")
    }
}
impl ureq::unversioned::resolver::Resolver for Resolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let signal = self.0.lock().unwrap().signal.clone().expect("active lease");
        signal.check().map_err(interrupted)?;
        let started = Instant::now();
        let sync = NextTimeout {
            after: ureq::unversioned::transport::time::Duration::NotHappening,
            reason: timeout.reason,
        };
        let result =
            ureq::unversioned::resolver::DefaultResolver::default().resolve(uri, config, sync);
        signal.check().map_err(interrupted)?;
        if timeout
            .not_zero()
            .is_some_and(|duration| started.elapsed() >= *duration)
        {
            return Err(ureq::Error::Timeout(timeout.reason));
        }
        result
    }
}

#[derive(Clone)]
struct CancellableConnector(Context);
impl std::fmt::Debug for CancellableConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CancellableConnector")
    }
}
fn interrupted(error: HostError) -> ureq::Error {
    std::io::Error::new(std::io::ErrorKind::Interrupted, error.to_string()).into()
}
impl<In: WireTransport> Connector<In> for CancellableConnector {
    type Out = Either<In, CancellableSocket>;
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        if let Some(transport) = chained {
            return Ok(Some(Either::A(transport)));
        }
        let signal = self.0.lock().unwrap().signal.clone().expect("active lease");
        signal.check().map_err(interrupted)?;
        let budget = details
            .timeout
            .not_zero()
            .map(|d| *d)
            .unwrap_or(Duration::from_secs(10));
        let deadline = Instant::now() + budget;
        let mut last = std::io::Error::new(std::io::ErrorKind::NotConnected, "no resolved address");
        let count = details.addrs.len();
        for (index, address) in details.addrs.iter().enumerate() {
            signal.check().map_err(interrupted)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            // Reserve a share for each remaining address, while allowing one
            // connection attempt to complete across arbitrarily many polls.
            let attempt = remaining / (count - index) as u32;
            match connect_socket(*address, attempt, &signal) {
                Ok(stream) => {
                    signal.check().map_err(interrupted)?;
                    stream.set_nodelay(details.config.no_delay())?;
                    let socket = stream.try_clone()?;
                    let buffers = LazyBuffers::new(
                        details.config.input_buffer_size(),
                        details.config.output_buffer_size(),
                    );
                    let mut wire = CancellableSocket {
                        stream,
                        buffers,
                        socket,
                        context: self.0.clone(),
                        generation: None,
                        registration: None,
                    };
                    wire.prepare()?;
                    return Ok(Some(Either::B(wire)));
                }
                Err(error) => {
                    last = error;
                }
            }
        }
        signal.check().map_err(interrupted)?;
        Err(last.into())
    }
}

#[cfg(windows)]
pub(crate) use super::windows_connect::connect_socket;

#[cfg(not(windows))]
pub(crate) fn connect_socket(
    address: std::net::SocketAddr,
    timeout: Duration,
    signal: &AbortSignal,
) -> std::io::Result<TcpStream> {
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(address),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    socket.set_nonblocking(true)?;
    if let Err(error) = socket.connect(&address.into()) {
        let pending = error.kind() == std::io::ErrorKind::WouldBlock;
        #[cfg(unix)]
        let pending = pending || error.raw_os_error() == Some(libc::EINPROGRESS);
        if !pending {
            return Err(error);
        }
        let deadline = Instant::now() + timeout;
        loop {
            signal
                .check()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Interrupted, e.to_string()))?;
            if let Some(error) = socket.take_error()? {
                return Err(error);
            }
            if socket.peer_addr().is_ok() {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            // A bounded wait on the caller's thread, not a second executor or
            // worker; abandoning the attempt closes the one pending socket.
            wait_writable(&socket, remaining.min(Duration::from_millis(50)));
        }
    }
    socket.set_nonblocking(false)?;
    Ok(socket.into())
}
/// Wait until a connecting socket is writable (connected or failed), at most
/// `timeout`. On Unix a `poll(2)`, which returns the moment the connection
/// completes: a sleep between checks (a 10 ms park, which a busy macOS
/// stretched to 50–90 ms) delayed every new connection by at least one sleep
/// (Exact patch 3). The caller rechecks cancellation between waits.
#[cfg(not(windows))]
fn wait_writable(socket: &socket2::Socket, timeout: Duration) {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let mut fd = libc::pollfd {
            fd: socket.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        let ms = timeout.as_millis().clamp(1, i32::MAX as u128) as libc::c_int;
        // SAFETY: one valid pollfd for a descriptor the socket owns, alive
        // for the call.
        unsafe {
            libc::poll(&mut fd, 1, ms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = socket;
        std::thread::park_timeout(timeout.min(Duration::from_millis(10)));
    }
}
struct CancellableSocket {
    stream: TcpStream,
    buffers: LazyBuffers,
    socket: TcpStream,
    context: Context,
    generation: Option<u64>,
    registration: Option<AbortRegistration>,
}
impl std::fmt::Debug for CancellableSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CancellableSocket")
    }
}
impl CancellableSocket {
    fn prepare(&mut self) -> Result<(), ureq::Error> {
        let (generation, signal) = {
            let current = self.context.lock().unwrap();
            (current.generation, current.signal.clone())
        };
        let Some(signal) = signal else {
            return Ok(());
        };
        signal.check().map_err(interrupted)?;
        if self.generation != Some(generation) {
            let socket = self.socket.try_clone()?;
            let context = Arc::downgrade(&self.context);
            self.registration = Some(signal.register(move || {
                if let Some(context) = context.upgrade() {
                    let current = context.lock().unwrap();
                    if current.generation == generation && current.signal.is_some() {
                        let _ = socket.shutdown(Shutdown::Both);
                    }
                }
            }));
            self.generation = Some(generation);
        }
        Ok(())
    }
}
impl WireTransport for CancellableSocket {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }
    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.prepare()?;
        self.stream
            .set_write_timeout(timeout.not_zero().map(|d| *d))?;
        self.stream
            .write_all(&self.buffers.output()[..amount])
            .map_err(ureq::Error::from)
    }
    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.prepare()?;
        self.stream
            .set_read_timeout(timeout.not_zero().map(|d| *d))?;
        let n = self.stream.read(self.buffers.input_append_buf())?;
        self.buffers.input_appended(n);
        Ok(n > 0)
    }
    fn is_open(&mut self) -> bool {
        if self.prepare().is_err() || self.stream.set_nonblocking(true).is_err() {
            return false;
        }
        let open = matches!(self.stream.read(&mut [0]), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock);
        self.stream.set_nonblocking(false).is_ok() && open
    }
}
fn failed(error: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("TypeError: Failed to fetch — {error}"))
}
struct Source {
    reader: Option<ureq::BodyReader<'static>>,
    lease: Option<Lease>,
}
impl BodySource for Source {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, HostError> {
        let result = self.reader.as_mut().unwrap().read(out).map_err(failed);
        if !matches!(result, Ok(n) if n > 0) {
            self.reader.take();
            self.lease.take();
        }
        result
    }
}
// Drop the reader (closing an unfinished response) before returning the lease.
impl Drop for Source {
    fn drop(&mut self) {
        self.reader.take();
        self.lease.take();
    }
}
impl Transport for RustlsHttpTransport {
    fn open(
        &self,
        request: &Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        signal.check()?;
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for (name, value) in request.headers.entries() {
            builder = builder.header(name, value);
        }
        let req = builder
            .body(request.body.as_deref().unwrap_or_default())
            .map_err(failed)?;
        // Plain HTTP agents stay separate so creating or using one does not
        // initialize the process TLS config. An HTTPS request initializes it
        // before constructing the first TLS-capable agent, and so does a plain
        // request through an HTTPS proxy: ureq speaks TLS to that proxy, whose
        // certificate may be trusted only by the system store.
        let lease = self.lease(
            signal,
            needs_tls(
                req.uri().scheme_str() == Some("https"),
                ureq::Proxy::try_from_env().as_ref(),
            ),
        );
        let agent = &lease.slot.as_ref().unwrap().agent;
        // The request's own limit (`Request::timeout`) replaces the agent's
        // 30 s total, 10 s connect and 15 s waits, any of which would end a
        // longer one first. (Name resolution is the system's, unbounded by
        // either: a caller's abort is seen once it returns.)
        let req = match request.timeout {
            Some(limit) => agent
                .configure_request(req)
                .timeout_global(Some(limit))
                .timeout_connect(Some(limit))
                .timeout_send_request(Some(limit))
                .timeout_send_body(Some(limit))
                .timeout_await_100(Some(limit))
                .timeout_recv_response(Some(limit))
                .timeout_recv_body(Some(limit))
                .build(),
            None => req,
        };
        let result = agent.run(req);
        signal.check()?;
        let response = result.map_err(failed)?;
        let status = response.status();
        let mut headers = Headers::new();
        for (name, value) in response.headers() {
            headers.set_response(name.as_str(), value.to_str().map_err(failed)?);
        }
        let limit = request.body_limit();
        if request.method != "HEAD"
            && !matches!(status.as_u16(), 204 | 304)
            && response
                .body()
                .content_length()
                .is_some_and(|n| n > limit as u64)
        {
            return Err(over_limit(limit));
        }
        let reader = response.into_body().into_reader();
        Ok(StreamingResponse {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").into(),
            headers,
            body: Body::new(
                Box::new(Source {
                    reader: Some(reader),
                    lease: Some(lease),
                }),
                limit,
                signal.clone(),
            ),
            url: request.url.clone(),
            redirected: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    fn tls_test_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap()
    }

    #[test]
    fn constructing_many_transports_does_not_load_native_certificates() {
        let _guard = tls_test_lock();
        let before = NATIVE_CERT_LOADS.load(Ordering::SeqCst);
        let transports: Vec<_> = (0..1_000).map(|_| RustlsHttpTransport::new()).collect();
        assert_eq!(NATIVE_CERT_LOADS.load(Ordering::SeqCst), before);
        std::hint::black_box(transports);
    }

    #[test]
    fn trust_uses_platform_certificates_and_falls_back_only_when_none_are_usable() {
        let _guard = tls_test_lock();
        let roots = tls_config().1;
        assert!(matches!(roots, Roots::System(n) if n > 0) || roots == Roots::CompiledIn);
        for certs in [vec![], vec![vec![0, 1, 2].into()]] {
            let (tls, roots) = tls_config_from(certs);
            assert_eq!(roots, Roots::CompiledIn);
            assert!(matches!(tls.root_certs(), ureq::tls::RootCerts::WebPki));
        }
    }
}

#[cfg(test)]
mod tls_selection_tests {
    use super::needs_tls;

    #[test]
    fn a_plain_request_through_an_https_proxy_uses_the_tls_agent() {
        let https_proxy = ureq::Proxy::new("https://proxy.corp:443").unwrap();
        let http_proxy = ureq::Proxy::new("http://proxy.corp:3128").unwrap();
        assert!(needs_tls(false, Some(&https_proxy)));
        assert!(!needs_tls(false, Some(&http_proxy)));
        assert!(!needs_tls(false, None));
        assert!(needs_tls(true, None));
        assert!(needs_tls(true, Some(&http_proxy)));
    }
}
