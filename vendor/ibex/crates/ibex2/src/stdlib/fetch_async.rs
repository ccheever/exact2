//! Asynchronous fetch (exact2 LLP 1120 D1): a transport starts one exchange
//! and reports it through a [`Sink`]; [`FetchTask`] is `fetch_stream`'s
//! redirect loop as a state machine over it, so a caller holds no thread
//! while an exchange is on the wire.
//!
//! The semantics are `fetch.rs`'s, shared rather than copied: the grant
//! check on every hop (`admit_hop`), the redirect rules (`redirect_step`),
//! the request guard, and the body ceiling enforced while bytes arrive. What
//! is new is only who waits: nobody. A transport calls the sink on one
//! serial context per exchange; the task decides at `head` whether the body
//! is wanted, and reports its outcome once, through [`FetchEvents`].

use std::sync::{Arc, Mutex};

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
/// never before `start` has returned.
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
            released();
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
        }
    }

    /// Let the transport deliver: the caller has recorded the exchange.
    /// Every exchange must be activated, a cancelled one included, so that
    /// its sink gets its `end`. Only the first call does anything.
    pub fn activate(&self) {
        let activate = self.activate.lock().ok().and_then(|mut a| a.take());
        if let Some(activate) = activate {
            activate();
        }
    }

    /// Stop the exchange. Its sink still gets `end` once activated; one
    /// cancelled before activation sends nothing.
    pub fn cancel(&self) {
        (self.cancel)()
    }
}

/// A transport that starts exchanges without waiting for them.
pub trait AsyncTransport: Send + Sync {
    /// Prepare one exchange. Nothing reaches the sink before the caller
    /// activates it (`Exchange::activate`), so a caller can record it first,
    /// and never on the caller's thread inside `start` or `activate`: a
    /// failure known at once is delivered later, as `end`.
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Exchange;
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

/// The caps every hop's headers are held to (exact2 LLP 1120 D1).
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

/// `fetch_stream`'s loop as a state machine (exact2 LLP 1120 D1).
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
        let prepared = prepare_body(request);
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

    /// Abort: the outcome is Aborted now, then the exchange is cancelled.
    fn abort(&self) {
        let released = {
            let mut state = self.state.lock().unwrap();
            let Some(mut released) = state.decide() else {
                return;
            };
            released.exchange = state.exchange.take();
            released
        };
        if let Some(exchange) = &released.exchange {
            exchange.cancel();
        }
        drop(released);
        self.events.outcome(Err(aborted()));
    }

    /// Decide `result` as the outcome, if nothing has, and report it. The
    /// caller holds no lock.
    fn finish(&self, result: Result<Collected, HostError>) {
        let released = self.state.lock().unwrap().decide();
        if let Some(released) = released {
            drop(released);
            self.events.outcome(result);
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
            task: self.clone(),
            hop,
            url: request.url.clone(),
        });
        let exchange = Arc::new(self.transport.start(request, sink));
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
    task: Arc<FetchTask>,
    hop: u64,
    /// The URL this hop sent, for its metrics.
    url: String,
}

impl HopSink {
    /// Record a stop's outcome; the transport stops the exchange.
    fn stop(&self, failure: HostError) -> Flow {
        self.task.finish(Err(failure));
        Flow::Stop
    }
}

impl Sink for HopSink {
    fn head(&mut self, head: Head) -> Flow {
        let mut state = self.task.state.lock().unwrap();
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
        if bytes.is_none_or(|n| n > self.task.caps.bytes) || entries.len() > self.task.caps.entries
        {
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
        let mut state = self.task.state.lock().unwrap();
        if state.done || state.hop != self.hop || state.response.is_none() {
            return Flow::Stop;
        }
        let limit = state.current.body_limit();
        if state.body.len().saturating_add(bytes.len()) > limit {
            drop(state);
            return self.stop(over_limit(limit));
        }
        state.body.extend_from_slice(bytes);
        Flow::Continue
    }

    fn end(&mut self, result: Result<(), HostError>) {
        enum Then {
            Nothing,
            Next(crate::stdlib::fetch::RedirectStep),
            Outcome(Result<Collected, HostError>),
        }
        let (released, then) = {
            let mut state = self.task.state.lock().unwrap();
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
        self.task.events.hop_ended();
        match then {
            Then::Nothing => {}
            Then::Next(_) => {
                let task = self.task.clone();
                self.task.events.next_hop(Box::new(move || task.hop()));
            }
            Then::Outcome(outcome) => self.task.events.outcome(outcome),
        }
    }

    fn metrics(&mut self, metrics: HopMetrics) {
        self.task.events.metrics(&self.url, metrics);
    }
}

#[cfg(test)]
#[path = "fetch_async_tests.rs"]
mod tests;
