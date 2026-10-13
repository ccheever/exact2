//! Asynchronous fetch (LLP 0068.000 §2): a transport starts one exchange
//! and reports it through a [`Sink`]; [`FetchTask`] is `fetch_stream`'s
//! redirect loop as a state machine over it, so a native async caller holds no thread
//! while an exchange is on the wire.
//!
//! The semantics are `fetch.rs`'s, shared rather than copied: the grant
//! check on every hop (`admit_hop`), the redirect rules (`redirect_step`),
//! the request guard, and the body ceiling enforced while bytes arrive. What
//! is new is only who waits: nobody. A transport calls the sink on one
//! serial context per exchange; the task decides at `head` whether the body
//! is wanted, and reports its outcome once, through [`FetchEvents`].

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

// @ref LLP 0068.000#3-ownership-callbacks-and-cancellation — callback and resource lifetime contract
// @ref LLP 0068.000#4-limits-flow-and-deadlines — bounded collection; deadlines belong to consumers

/// Contain an external callback panic without skipping retirement. Native FFI
/// adapters apply the same boundary to arbitrary low-level sinks.
pub(crate) fn contain(f: impl FnOnce()) {
    let _ = catch_unwind(AssertUnwindSafe(f));
}

use crate::boundary::HostError;
use crate::grant::GrantSet;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::fetch::{admit_hop, over_limit, prepare_body, redirect_step, Headers, Request};

/// What a sink tells the transport after `head` or `data`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Keep delivering the body.
    Continue,
    /// Stop the exchange: the sink has recorded why (a ceiling, a header
    /// cap). The transport's later `end` changes nothing.
    Stop,
    /// Stop the body of a redirect that will be followed. Not an outcome:
    /// the hop's `end` is ignored, and the task starts the next hop.
    Skip,
}

/// A response's status line and headers, before any body byte.
#[derive(Debug, Clone)]
pub struct Head {
    pub status: u16,
    pub headers: Headers,
    /// `Content-Length`, when the response declared one.
    pub declared_length: Option<u64>,
}

/// What a transport learned about one exchange's connection, from the
/// platform's metrics (`NSURLSessionTaskMetrics` on Apple). It may arrive
/// after `end`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HopMetrics {
    /// `h2`, `h3`, `http/1.1`, as the platform names it.
    pub protocol: Option<String>,
    /// Whether the exchange rode an already-open connection.
    pub reused: Option<bool>,
    /// The connection's local and remote endpoints, `address:port`: two
    /// exchanges on one connection report the same pair.
    pub local: Option<String>,
    pub remote: Option<String>,
    /// From the task's start (`resume`) to its first transaction's fetch
    /// start, in milliseconds: time the platform queued it.
    pub queued_ms: Option<f64>,
}

/// One exchange's events, on one serial context, never concurrently, and
/// never inline inside `start` or activation. Delivery on another context may
/// overlap activation returning. Cancellation can trigger retirement before activation.
pub trait Sink: Send {
    /// The response's head. A stop decided here sends no body byte.
    fn head(&mut self, head: Head) -> Flow;
    /// Body bytes, in order.
    fn data(&mut self, bytes: &[u8]) -> Flow;
    /// The exchange's physical end, exactly once.
    fn end(&mut self, result: Result<(), HostError>);
    /// The exchange's metrics, at most once, before or after `end`.
    fn metrics(&mut self, _metrics: HopMetrics) {}
}

/// A request body shared by every hop that sends it and lent to the
/// platform without a copy. `released` runs when the last holder lets go:
/// the platform's own reference included.
pub struct Upload {
    bytes: Vec<u8>,
    /// Behind a mutex only so `Upload` is `Sync`; `Drop` has it alone.
    released: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Upload {
    pub fn new(bytes: Vec<u8>, released: Option<Box<dyn FnOnce() + Send>>) -> Arc<Upload> {
        Arc::new(Upload {
            bytes,
            released: Mutex::new(released),
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        if let Some(released) = self.released.get_mut().ok().and_then(Option::take) {
            contain(released);
        }
    }
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Upload({} bytes)", self.bytes.len())
    }
}

/// One hop as a transport sends it: headers already guarded, no redirects.
#[derive(Debug, Clone)]
pub struct HopRequest {
    pub method: String,
    pub url: String,
    pub headers: Headers,
    pub body: Option<Arc<Upload>>,
    /// The body ceiling, which `FetchTask`'s sink enforces; a transport may
    /// also refuse early.
    pub max_body: usize,
    /// The platform's own idle timeout.
    pub timeout: Option<std::time::Duration>,
}

/// A started exchange: what activates and what cancels it. Dropping it
/// does neither.
pub struct Exchange {
    activate: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    cancel: Box<dyn Fn() + Send + Sync>,
    cancelled: std::sync::atomic::AtomicBool,
}

impl Exchange {
    /// An exchange that delivers when `activate` runs, and stops on `cancel`.
    pub fn new(
        activate: impl FnOnce() + Send + 'static,
        cancel: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            activate: Mutex::new(Some(Box::new(activate))),
            cancel: Box::new(cancel),
            cancelled: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Let the transport deliver: the caller has recorded the exchange.
    /// Only the first call does anything. Capacity was reserved at preparation;
    /// activation cannot refuse it. Call outside the consumer state lock.
    pub fn activate(&self) {
        let activate = self.activate.lock().ok().and_then(|mut a| a.take());
        if let Some(activate) = activate {
            activate();
        }
    }

    /// Stop the exchange and schedule retirement even if never activated. A
    /// preactivation cancellation sends nothing. Both operations are idempotent.
    pub fn cancel(&self) {
        if !self
            .cancelled
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            (self.cancel)();
        }
        self.activate();
    }
}

/// A transport that starts exchanges without waiting for them.
pub trait AsyncTransport: Send + Sync {
    /// Prepare one exchange. Nothing reaches the sink before the caller
    /// activates it (`Exchange::activate`), so a caller can record it first,
    /// and never on the caller's thread inside `start` or `activate`: a
    /// failure known at once is delivered later, as `end`.
    /// Reserve capacity before returning `Ok`. Refusal returns `Err` without
    /// an accepted exchange or sink callback.
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Result<Exchange, HostError>;
}

/// A fetch's final response, collected within its ceiling.
#[derive(Debug, Clone)]
pub struct Collected {
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
    pub url: String,
    pub redirected: bool,
}

/// The caps every hop's headers are held to (LLP 0068.000 §2).
#[derive(Debug, Clone, Copy)]
pub struct HeaderCaps {
    pub bytes: usize,
    pub entries: usize,
}

/// What a fetch reports, from its transport's contexts.
pub trait FetchEvents: Send + Sync {
    /// The fetch's outcome, once. A stop or an abort reports it before the
    /// hop's physical end.
    fn outcome(&self, result: Result<Collected, HostError>);
    /// A hop's physical end, once for every hop started.
    fn hop_ended(&self) {}
    /// A hop's metrics, as the transport reports them, with the URL that
    /// hop sent: a late report from an earlier hop names its own origin.
    fn metrics(&self, _url: &str, _metrics: HopMetrics) {}
    /// The next hop of a redirect is ready: start it, now or later.
    fn next_hop(&self, start: Box<dyn FnOnce() + Send>) {
        start()
    }
}

/// `fetch_stream`'s loop as a state machine (LLP 0068.000 §2).
pub struct FetchTask {
    state: Mutex<TaskState>,
    transport: Arc<dyn AsyncTransport>,
    grants: Arc<GrantSet>,
    events: Arc<dyn FetchEvents>,
    caps: HeaderCaps,
}

struct TaskState {
    /// The request as the next hop sends it; its body is `upload`.
    current: Request,
    upload: Option<Arc<Upload>>,
    redirects: u32,
    redirected: bool,
    /// The hop now running; a late event from an earlier one is ignored.
    hop: u64,
    /// The last hop whose transport ended: an exchange handed back after
    /// its own end is not kept.
    ended: u64,
    exchange: Option<ExchangeRef>,
    /// The outcome is reported: everything after changes nothing.
    done: bool,
    /// The redirect the running hop's `head` chose to follow.
    next: Option<crate::stdlib::fetch::RedirectStep>,
    /// The final response's head and body so far.
    response: Option<(u16, Headers)>,
    body: Vec<u8>,
    abort: AbortSignal,
    _registration: Option<AbortRegistration>,
}

/// The one spelling of an aborted fetch.
pub fn aborted() -> HostError {
    HostError::Failed("AbortError: The operation was aborted".into())
}

impl FetchTask {
    /// Start `request`. `upload` is its body when the caller charged it
    /// (it wins over `request.body`); `signal` aborts it.
    pub fn start(
        transport: Arc<dyn AsyncTransport>,
        grants: Arc<GrantSet>,
        request: Request,
        upload: Option<Arc<Upload>>,
        signal: AbortSignal,
        caps: HeaderCaps,
        events: Arc<dyn FetchEvents>,
    ) -> Arc<FetchTask> {
        let prepared = if request.body_limit() > isize::MAX as usize
            || caps.bytes > isize::MAX as usize
            || caps.entries > isize::MAX as usize
        {
            Err(HostError::InvalidArgument(
                "asynchronous fetch requires finite allocation limits".into(),
            ))
        } else {
            prepare_body(request)
        };
        let failure = prepared.as_ref().err().cloned();
        let mut current = prepared.unwrap_or_else(|_| Request::get(""));
        let upload = upload.or_else(|| current.body.take().map(|b| Upload::new(b, None)));
        current.body = None;
        let task = Arc::new(FetchTask {
            state: Mutex::new(TaskState {
                current,
                upload,
                redirects: 0,
                redirected: false,
                hop: 0,
                ended: 0,
                exchange: None,
                done: false,
                next: None,
                response: None,
                body: Vec::new(),
                abort: signal.clone(),
                _registration: None,
            }),
            transport,
            grants,
            events,
            caps,
        });
        if let Some(failure) = failure {
            task.finish(Err(failure));
            return task;
        }
        let weak = Arc::downgrade(&task);
        let registration = signal.register(move || {
            if let Some(task) = weak.upgrade() {
                task.abort();
            }
        });
        task.state.lock().unwrap()._registration = Some(registration);
        task.clone().hop();
        task
    }

    /// Abort: the outcome is Aborted now, reported, then the exchange is
    /// cancelled, so the report comes before the hop's physical end.
    pub fn abort(&self) {
        let released = {
            let mut state = self.state.lock().unwrap();
            let Some(mut released) = state.decide() else {
                return;
            };
            released.exchange = state.exchange.take();
            released
        };
        let Released {
            upload,
            exchange,
            body,
        } = released;
        drop((upload, body));
        contain(|| self.events.outcome(Err(aborted())));
        if let Some(exchange) = exchange {
            exchange.cancel();
        }
    }

    /// Decide `result` as the outcome, if nothing has, and report it. The
    /// caller holds no lock.
    fn finish(&self, result: Result<Collected, HostError>) {
        let released = self.state.lock().unwrap().decide();
        if let Some(released) = released {
            drop(released);
            contain(|| self.events.outcome(result));
        }
    }

    /// Start the hop `current` describes.
    fn hop(self: Arc<Self>) {
        let (request, hop) = {
            let mut state = self.state.lock().unwrap();
            if state.done {
                return;
            }
            if state.abort.aborted() {
                drop(state);
                return self.finish(Err(aborted()));
            }
            if let Err(e) = admit_hop(&self.grants, &state.current.url, state.redirected) {
                drop(state);
                return self.finish(Err(e));
            }
            state.hop += 1;
            state.response = None;
            state.body = Vec::new();
            let current = &state.current;
            let request = HopRequest {
                method: current.method.clone(),
                url: current.url.clone(),
                // The request guard applies where a header list becomes a
                // request, as in `fetch_stream`.
                headers: current.headers.for_request(),
                body: state.upload.clone(),
                max_body: current.body_limit(),
                timeout: current.timeout,
            };
            (request, state.hop)
        };
        let sink = Box::new(HopSink {
            task: Some(self.clone()),
            events: self.events.clone(),
            hop,
            url: request.url.clone(),
        });
        let exchange = match self.transport.start(request, sink) {
            Ok(exchange) => Arc::new(exchange),
            Err(error) => return self.finish(Err(error)),
        };
        // Recorded before it may deliver; activated whatever happened since,
        // so that its sink gets an end.
        let wanted = {
            let mut state = self.state.lock().unwrap();
            let wanted = !state.done && state.hop == hop && state.ended < hop;
            if wanted {
                state.exchange = Some(ExchangeRef(exchange.clone()));
            }
            wanted
        };
        // An unwanted one is cancelled first, so activating it sends nothing.
        if !wanted {
            exchange.cancel();
        }
        exchange.activate();
    }
}

/// What a decision lets go: destroyed after the lock is released, since an
/// upload's release or an exchange's platform release may re-enter (an
/// abort, a caller's accounting).
#[derive(Default)]
struct Released {
    upload: Option<Arc<Upload>>,
    exchange: Option<ExchangeRef>,
    body: Vec<u8>,
}

/// The running hop's exchange, shared with the `hop` call that activates it.
struct ExchangeRef(Arc<Exchange>);
impl std::ops::Deref for ExchangeRef {
    type Target = Exchange;
    fn deref(&self) -> &Exchange {
        &self.0
    }
}

impl TaskState {
    /// Decide the outcome now, if nothing has: what it lets go, or `None`
    /// when an outcome was already decided.
    fn decide(&mut self) -> Option<Released> {
        if self.done {
            return None;
        }
        self.done = true;
        Some(Released {
            upload: self.upload.take(),
            exchange: None,
            body: std::mem::take(&mut self.body),
        })
    }
}

struct HopSink {
    // Released at physical end; late metrics retain only bounded hop metadata
    // and the consumer observer, never the fetch body/upload or transport.
    task: Option<Arc<FetchTask>>,
    events: Arc<dyn FetchEvents>,
    hop: u64,
    /// The URL this hop sent, for its metrics.
    url: String,
}

impl HopSink {
    /// Record a stop's outcome; the transport stops the exchange.
    fn stop(&self, failure: HostError) -> Flow {
        if let Some(task) = &self.task {
            task.finish(Err(failure));
        }
        Flow::Stop
    }
}

impl Sink for HopSink {
    fn head(&mut self, head: Head) -> Flow {
        let Some(task) = &self.task else {
            return Flow::Stop;
        };
        let mut state = task.state.lock().unwrap();
        if state.done || state.hop != self.hop {
            return Flow::Stop;
        }
        let limit = state.current.body_limit();
        // 1. A declared length over the ceiling, for any status with a body,
        //    3xx included (HEAD, 204, 205 and 304 have none).
        let has_body = state.current.method != "HEAD"
            && head.status >= 200
            && !matches!(head.status, 204 | 205 | 304);
        if has_body && head.declared_length.is_some_and(|n| n > limit as u64) {
            drop(state);
            return self.stop(over_limit(limit));
        }
        // 2. Every hop's headers are held to the caps.
        let entries = head.headers.entries();
        let bytes = entries.iter().try_fold(0usize, |n, (k, v)| {
            n.checked_add(k.len())?.checked_add(v.len())
        });
        if bytes.is_none_or(|n| n > task.caps.bytes) || entries.len() > task.caps.entries {
            drop(state);
            return self.stop(HostError::Failed("HTTP headers exceed limit".into()));
        }
        // 3. A redirect that will be followed: its body is not wanted.
        match redirect_step(&state.current, head.status, &head.headers, state.redirects) {
            Err(e) => {
                drop(state);
                self.stop(e)
            }
            Ok(Some(step)) => {
                state.next = Some(step);
                Flow::Skip
            }
            Ok(None) => {
                state.response = Some((head.status, head.headers));
                Flow::Continue
            }
        }
    }

    fn data(&mut self, bytes: &[u8]) -> Flow {
        let Some(task) = &self.task else {
            return Flow::Stop;
        };
        let mut state = task.state.lock().unwrap();
        if state.done || state.hop != self.hop || state.response.is_none() {
            return Flow::Stop;
        }
        let limit = state.current.body_limit();
        if state.body.len().saturating_add(bytes.len()) > limit {
            drop(state);
            return self.stop(over_limit(limit));
        }
        let required = state.body.len() + bytes.len();
        if required > state.body.capacity() {
            // Grow geometrically without requesting capacity beyond the ceiling.
            let capacity = required
                .max(state.body.capacity().saturating_mul(2))
                .min(limit);
            let additional = capacity - state.body.len();
            if state.body.try_reserve_exact(additional).is_err() {
                drop(state);
                return self.stop(HostError::Failed("HTTP response allocation failed".into()));
            }
        }
        state.body.extend_from_slice(bytes);
        Flow::Continue
    }

    fn end(&mut self, result: Result<(), HostError>) {
        let Some(task) = self.task.take() else {
            return;
        };
        enum Then {
            Nothing,
            Next(crate::stdlib::fetch::RedirectStep),
            Outcome(Result<Collected, HostError>),
        }
        let (released, then) = {
            let mut state = task.state.lock().unwrap();
            let current = state.hop == self.hop;
            let mut released = Released::default();
            if current {
                // This hop's resources go with its physical end, whatever
                // was decided before.
                state.ended = self.hop;
                released.exchange = state.exchange.take();
            }
            let then = if !current {
                Then::Nothing
            } else if state.done {
                state.response = None;
                released.body = std::mem::take(&mut state.body);
                Then::Nothing
            } else if let Some(step) = state.next.take() {
                Then::Next(step)
            } else {
                // The outcome is decided here, before any callback, so that
                // nothing those callbacks do can replace it.
                let response = state.response.take();
                let (url, redirected) = (state.current.url.clone(), state.redirected);
                let mut decided = state.decide().expect("not done");
                let body = std::mem::take(&mut decided.body);
                released.upload = decided.upload.take();
                Then::Outcome(match (result, response) {
                    (Ok(()), Some((status, headers))) => Ok(Collected {
                        status,
                        headers,
                        body,
                        url,
                        redirected,
                    }),
                    (Ok(()), None) => Err(HostError::Failed(
                        "TypeError: Failed to fetch — no response".into(),
                    )),
                    (Err(e), _) => Err(e),
                })
            };
            if let Then::Next(step) = &then {
                state.redirects += 1;
                let current = &mut state.current;
                step.apply(&mut current.method, &mut current.headers);
                current.url = step.url.clone();
                if step.drops_body {
                    // The hop before was the last to send the body.
                    released.upload = state.upload.take();
                }
                state.redirected = true;
            }
            (released, then)
        };
        drop(released);
        // The physical end first: a caller's in-flight slot is free before
        // the next hop asks for one.
        contain(|| self.events.hop_ended());
        match then {
            Then::Nothing => {}
            Then::Next(_) => {
                let next = task.clone();
                if catch_unwind(AssertUnwindSafe(|| {
                    self.events.next_hop(Box::new(move || next.hop()))
                }))
                .is_err()
                {
                    task.finish(Err(HostError::Failed(
                        "fetch continuation callback panicked".into(),
                    )));
                }
            }
            Then::Outcome(outcome) => contain(|| self.events.outcome(outcome)),
        }
    }

    fn metrics(&mut self, metrics: HopMetrics) {
        contain(|| self.events.metrics(&self.url, metrics));
    }
}

/// A bounded worker-service job.
pub type Job = Box<dyn FnOnce() + Send>;

/// A reservation whose dispatch cannot refuse or run the job inline. Holding
/// it consumes capacity; dropping it without dispatch releases that capacity.
pub trait ReservedWorker: Send {
    fn dispatch(self: Box<Self>, job: Job);
}

/// Consumer-supplied execution mechanism, without a runtime or queue policy.
/// @ref LLP 0068.000#5-platform-adapters-and-compatibility — reserve before acceptance
pub trait WorkerService: Send + Sync {
    fn reserve(&self) -> Result<Box<dyn ReservedWorker>, HostError>;
}

/// Fixed workers and a finite pending capacity. At most `capacity` prepared
/// or queued jobs plus `threads` executing jobs exist. A queue reservation is
/// released when its worker starts, so an end callback can prepare a redirect
/// even on a one-worker, one-pending-slot service.
/// Dropping the service does not interrupt accepted jobs; reservations retain
/// the queue until dispatched or dropped. Blocking DNS can retain a worker.
pub struct BoundedWorkerPool {
    sender: std::sync::mpsc::Sender<Job>,
    occupied: Arc<std::sync::atomic::AtomicUsize>,
    capacity: usize,
}

impl BoundedWorkerPool {
    pub fn new(threads: usize, capacity: usize) -> std::io::Result<Self> {
        if threads == 0 || capacity == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "worker count and capacity must be positive",
            ));
        }
        let (sender, receiver) = std::sync::mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..threads {
            let receiver = receiver.clone();
            std::thread::Builder::new()
                .name(format!("ibex-http-{index}"))
                .spawn(move || loop {
                    let job = receiver.lock().unwrap().recv();
                    match job {
                        Ok(job) => contain(job),
                        Err(_) => break,
                    }
                })?;
        }
        Ok(Self {
            sender,
            occupied: Arc::default(),
            capacity,
        })
    }
}

struct WorkerPermit(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for WorkerPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
struct PoolReservation {
    sender: std::sync::mpsc::Sender<Job>,
    permit: WorkerPermit,
}
impl ReservedWorker for PoolReservation {
    fn dispatch(self: Box<Self>, job: Job) {
        let Self { sender, permit } = *self;
        // A reservation retains the sender; workers live while any sender
        // exists and contain job panics. Thus this send cannot be refused.
        assert!(
            sender
                .send(Box::new(move || {
                    drop(permit);
                    job();
                }))
                .is_ok(),
            "reserved worker service stopped"
        );
    }
}
impl WorkerService for BoundedWorkerPool {
    fn reserve(&self) -> Result<Box<dyn ReservedWorker>, HostError> {
        use std::sync::atomic::Ordering;
        self.occupied
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < self.capacity).then_some(n + 1)
            })
            .map_err(|_| HostError::Failed("HTTP worker capacity exhausted".into()))?;
        Ok(Box::new(PoolReservation {
            sender: self.sender.clone(),
            permit: WorkerPermit(self.occupied.clone()),
        }))
    }
}

/// A blocking transport using explicitly reserved bounded worker capacity.
/// This is a worker adapter, not thread-free networking. The blocking request
/// currently makes one temporary copy of its upload: budget another upload-sized
/// buffer for every running worker. `Upload`'s release callback observes only
/// the shared original; worker physical retirement bounds the temporary copy.
pub struct BlockingAdapter {
    transport: Arc<dyn crate::stdlib::fetch::Transport>,
    workers: Arc<dyn WorkerService>,
}
impl BlockingAdapter {
    pub fn new(
        transport: Arc<dyn crate::stdlib::fetch::Transport>,
        workers: Arc<dyn WorkerService>,
    ) -> Self {
        Self { transport, workers }
    }
}
impl AsyncTransport for BlockingAdapter {
    fn start(&self, hop: HopRequest, mut sink: Box<dyn Sink>) -> Result<Exchange, HostError> {
        let worker = self.workers.reserve()?;
        let abort = crate::stdlib::abort::AbortController::new();
        let signal = abort.signal();
        let transport = self.transport.clone();
        Ok(Exchange::new(
            move || {
                worker.dispatch(Box::new(move || {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        run_blocking(&*transport, hop, &mut *sink, &signal)
                    }))
                    .unwrap_or_else(|_| {
                        Err(HostError::Failed("HTTP transport callback panicked".into()))
                    });
                    contain(|| sink.end(result));
                    contain(|| drop(sink));
                }))
            },
            move || abort.abort(),
        ))
    }
}

/// One blocking exchange, reported through `sink`. Its body, an upload,
/// is held until its end has been told, as a platform holds what it sends
/// until the exchange is over; an abort stops the read between chunks.
fn run_blocking(
    transport: &dyn crate::stdlib::fetch::Transport,
    hop: HopRequest,
    sink: &mut dyn Sink,
    signal: &AbortSignal,
) -> Result<(), HostError> {
    signal.check()?;
    let mut request = Request::get(&hop.url);
    request.method = hop.method;
    request.headers = hop.headers;
    request.body = hop.body.as_ref().map(|b| b.bytes().to_vec());
    request.max_body = Some(hop.max_body);
    request.timeout = hop.timeout;
    request.redirect = crate::stdlib::fetch::RedirectMode::Manual;
    let _sending = hop.body;
    let mut response = transport.open(&request, signal)?;
    drop(request);
    let declared_length = response
        .headers
        .get("content-length")
        .and_then(|n| n.trim().parse().ok());
    let head = Head {
        status: response.status,
        headers: std::mem::take(&mut response.headers),
        declared_length,
    };
    if sink.head(head) != Flow::Continue {
        drop(response);
        return Err(aborted());
    }
    let mut chunk = vec![0; 64 << 10];
    loop {
        if signal.aborted() {
            drop(response);
            return Err(aborted());
        }
        match response.body.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                if sink.data(&chunk[..n]) != Flow::Continue {
                    drop(response);
                    return Err(aborted());
                }
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
#[path = "fetch_async_tests.rs"]
mod tests;
