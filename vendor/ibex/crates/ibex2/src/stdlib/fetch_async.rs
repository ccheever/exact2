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

/// A started exchange: what cancels it. Dropping it does not.
pub struct Exchange {
    cancel: Box<dyn Fn() + Send + Sync>,
}

impl Exchange {
    pub fn new(cancel: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            cancel: Box::new(cancel),
        }
    }

    /// Stop the exchange. Its sink still gets `end`.
    pub fn cancel(&self) {
        (self.cancel)()
    }
}

/// A transport that starts exchanges without waiting for them.
pub trait AsyncTransport: Send + Sync {
    /// Start one exchange. The sink is never called before this returns;
    /// a failure known at once is delivered asynchronously as `end`.
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
    /// A hop's metrics, as the transport reports them.
    fn metrics(&self, _metrics: HopMetrics) {}
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
    exchange: Option<Exchange>,
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

    /// Abort: the exchange is cancelled and the outcome is Aborted now.
    fn abort(&self) {
        let exchange = {
            let mut state = self.state.lock().unwrap();
            if state.done {
                return;
            }
            state.exchange.take()
        };
        if let Some(exchange) = exchange {
            exchange.cancel();
        }
        self.finish(Err(aborted()));
    }

    /// Report the outcome once. The caller holds no lock.
    fn finish(&self, result: Result<Collected, HostError>) {
        {
            let mut state = self.state.lock().unwrap();
            if state.done {
                return;
            }
            state.done = true;
            state.upload = None;
        }
        self.events.outcome(result);
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
            state.body.clear();
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
        });
        let exchange = self.transport.start(request, sink);
        let mut state = self.state.lock().unwrap();
        if state.done || state.hop != hop {
            drop(state);
            exchange.cancel();
        } else {
            state.exchange = Some(exchange);
        }
    }
}

struct HopSink {
    task: Arc<FetchTask>,
    hop: u64,
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
        let has_body = !state.current.method.eq_ignore_ascii_case("HEAD")
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
        self.task.events.hop_ended();
        let mut state = self.task.state.lock().unwrap();
        if state.done || state.hop != self.hop {
            return;
        }
        state.exchange = None;
        if let Some(step) = state.next.take() {
            state.redirects += 1;
            let current = &mut state.current;
            step.apply(&mut current.method, &mut current.headers);
            current.url = step.url.clone();
            if step.drops_body {
                // The hop before was the last to send the body.
                state.upload = None;
            }
            state.redirected = true;
            drop(state);
            let task = self.task.clone();
            self.task.events.next_hop(Box::new(move || task.hop()));
            return;
        }
        let response = state.response.take();
        let body = std::mem::take(&mut state.body);
        let (url, redirected) = (state.current.url.clone(), state.redirected);
        drop(state);
        let outcome = match (result, response) {
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
        };
        self.task.finish(outcome);
    }

    fn metrics(&mut self, metrics: HopMetrics) {
        self.task.events.metrics(metrics);
    }
}

#[cfg(test)]
#[path = "fetch_async_tests.rs"]
mod tests;
