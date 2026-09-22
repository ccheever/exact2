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
use std::sync::{Arc, Mutex};
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
    idle: Mutex<Vec<Slot>>,
    config: ureq::config::Config,
}
/// Idle agents retain their connection pools. Each in-flight response exclusively
/// owns its agent, so a signal never becomes ambient state shared by requests.
pub struct RustlsHttpTransport {
    pool: Arc<Pool>,
    roots: Roots,
}
impl RustlsHttpTransport {
    pub fn new() -> Self {
        let (tls, roots) = tls_config();
        let config = ureq::Agent::config_builder()
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .timeout_recv_body(Some(Duration::from_secs(15)))
            .tls_config(tls)
            .build();
        Self {
            pool: Arc::new(Pool {
                idle: Mutex::new(Vec::new()),
                config,
            }),
            roots,
        }
    }
    pub fn roots(&self) -> Roots {
        self.roots
    }
    fn lease(&self, signal: &AbortSignal) -> Lease {
        let slot = self.pool.idle.lock().unwrap().pop().unwrap_or_else(|| {
            let context = Arc::new(Mutex::new(Current::default()));
            let connector = ureq::unversioned::transport::ConnectProxyConnector::default()
                .chain(CancellableConnector(context.clone()))
                .chain(ureq::unversioned::transport::RustlsConnector::default());
            let agent = ureq::Agent::with_parts(
                self.pool.config.clone(),
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
        }
    }
}
impl Default for RustlsHttpTransport {
    fn default() -> Self {
        Self::new()
    }
}
fn tls_config() -> (ureq::tls::TlsConfig, Roots) {
    tls_config_from(rustls_native_certs::load_native_certs().unwrap_or_default())
}
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
}
impl Drop for Lease {
    fn drop(&mut self) {
        let slot = self.slot.take().unwrap();
        // Serialized with callbacks: a delayed callback from this lease cannot
        // shut down a connection after it has been handed to another request.
        slot.context.lock().unwrap().signal = None;
        let mut idle = self.pool.idle.lock().unwrap();
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

fn connect_socket(
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
            std::thread::park_timeout(remaining.min(Duration::from_millis(10)));
        }
    }
    socket.set_nonblocking(false)?;
    Ok(socket.into())
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
        let lease = self.lease(signal);
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for (name, value) in request.headers.entries() {
            builder = builder.header(name, value);
        }
        let req = builder
            .body(request.body.as_deref().unwrap_or_default())
            .map_err(failed)?;
        let result = lease.slot.as_ref().unwrap().agent.run(req);
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
    #[test]
    fn trust_uses_platform_certificates_and_falls_back_only_when_none_are_usable() {
        let (_, roots) = tls_config();
        assert!(matches!(roots, Roots::System(n) if n > 0) || roots == Roots::CompiledIn);
        for certs in [vec![], vec![vec![0, 1, 2].into()]] {
            let (tls, roots) = tls_config_from(certs);
            assert_eq!(roots, Roots::CompiledIn);
            assert!(matches!(tls.root_certs(), ureq::tls::RootCerts::WebPki));
        }
    }
}
