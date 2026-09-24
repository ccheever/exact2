//! Native transport scheduling shared by Apple and Linux (LLP 1041 D1–D4).
//! Only explicitly independent HTTP leaves the ordered lane. Each worker has
//! its own bindings and transport, so held data cannot consume control leases.
use exact_runner::{
    FailureKind, HttpScheduling, Outcome, Reply, Request, RequestOut, Response, Work as OwnedWork,
};
use ibex2::stdlib::abort::AbortController;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};

const WORKERS: usize = 3;
const MAX_WORKERS: usize = 48; // Includes retired workers until they actually exit.
static LIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
const COUNTS: [usize; 2] = [16, 128];
const BYTES: [usize; 2] = [512 << 20, 32 << 20];
const MAX_REQUEST: usize = 4 << 20;
const MAX_BODY: usize = 64 << 20;
const MAX_HEADERS: usize = 64 << 10;

type Work = Box<dyn FnOnce() -> Outcome + Send>;
type Wake = Box<dyn Fn() + Send + Sync>;
struct Job {
    ticket: u64,
    request: Request,
    forced: bool,
    work: Option<OwnedWork>,
    bytes: usize,
}
struct Completed {
    ticket: u64,
    outcome: Outcome,
    bytes: usize,
}
#[derive(Default)]
struct State {
    jobs: [VecDeque<Job>; 2],
    completed: [VecDeque<Completed>; 2],
    counts: [usize; 2],
    bytes: [usize; 2],
    next: usize,
    ordered: VecDeque<u64>,
    retired: bool,
    ordered_barrier: bool,
    notified: bool,
    wake: Option<Wake>,
}
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
    abort: AbortController,
}

/// Count/byte reservations last until the UI takes the result, not merely
/// until transport finishes. Rejections return directly to the host: there
/// is no unbounded queue of overload failures. Byte reservations cover owned
/// request/result buffers, NOT arbitrary closure captures or allocations during
/// native work. Those trusted-source costs are count/worker bounded only.
pub(super) struct Core {
    shared: Arc<Shared>,
    disabled: bool,
}

impl Core {
    pub(super) fn start(bindings: Option<ibex2::host::Bindings>, grants: &str, wake: Wake) -> Self {
        // Additional transports are constructed on their own threads, not
        // while the presenter is trying to publish its first frame.
        Self::with_owners(vec![bindings, None, None], grants, wake)
    }

    fn with_owners(owners: Vec<Option<ibex2::host::Bindings>>, grants: &str, wake: Wake) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                wake: Some(wake),
                ..State::default()
            }),
            ready: Condvar::new(),
            abort: AbortController::new(),
        });
        let reserved = LIVE_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n + WORKERS <= MAX_WORKERS).then_some(n + WORKERS)
            })
            .is_ok();
        let mut core = Self {
            shared,
            disabled: !reserved,
        };
        if !reserved {
            return core;
        }
        for (index, bindings) in owners.into_iter().enumerate() {
            let shared = core.shared.clone();
            let grants = grants.to_string();
            let guard = WorkerSlot;
            let spawned = std::thread::Builder::new()
                .name(format!("exact-io-{index}"))
                .spawn(move || {
                    let _guard = guard;
                    worker(shared, usize::from(index != 0), bindings, grants);
                });
            if spawned.is_err() {
                core.disabled = true;
            }
        }
        // On partial spawn failure keep the wake alive to deliver admission
        // refusals. No jobs are admitted; any started workers retire on Drop.
        core
    }

    #[cfg(test)]
    pub(super) fn run(&self, r: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.run_owned(r, work.map(OwnedWork::Now))
    }

    pub(super) fn run_owned(
        &self,
        r: RequestOut,
        work: Option<OwnedWork>,
    ) -> Result<(), &'static str> {
        let ordered = r.request.is_ordered();
        let mut state = self.shared.state.lock().unwrap();
        let admitted = (|| {
            let (lane, bytes) = reservation(&r.request)?;
            if self.disabled {
                return Err("native executor worker limit reached");
            }
            if state.retired {
                return Err("native executor retired");
            }
            if ordered && state.ordered_barrier {
                return Err("earlier ordered admission refusal must settle first");
            }
            if state.counts[lane] >= COUNTS[lane]
                || bytes > BYTES[lane].saturating_sub(state.bytes[lane])
            {
                return Err("native executor admission limit reached");
            }
            Ok((lane, bytes))
        })();
        let (lane, bytes) = match admitted {
            Ok(value) => value,
            Err(reason) => {
                // Failure parsing can mutate Store too. Refusals live on
                // runner tickets, but fence later ordered work here until
                // the host has settled/forgotten all those tickets.
                if ordered {
                    state.ordered_barrier = true;
                }
                return Err(reason);
            }
        };
        if ordered {
            state.ordered.push_back(r.ticket);
        }
        state.counts[lane] += 1;
        state.bytes[lane] += bytes;
        state.jobs[lane].push_back(Job {
            ticket: r.ticket,
            request: r.request,
            forced: r.forced,
            work,
            bytes,
        });
        self.shared.ready.notify_all();
        Ok(())
    }

    /// One complete transaction per pump. Alternate ready lanes; never drain
    /// and lose later results when parsing one reply fails.
    pub(super) fn drain(&self) -> Vec<(u64, Outcome)> {
        let mut state = self.shared.state.lock().unwrap();
        let ready = |lane: usize, state: &State| {
            if lane == 0 {
                state.ordered.front().and_then(|ticket| {
                    state.completed[0]
                        .iter()
                        .position(|done| done.ticket == *ticket)
                })
            } else {
                (!state.completed[1].is_empty()).then_some(0)
            }
        };
        let first = state.next;
        let (lane, index) = if let Some(index) = ready(first, &state) {
            (first, index)
        } else if let Some(index) = ready(1 - first, &state) {
            (1 - first, index)
        } else {
            return vec![];
        };
        let done = state.completed[lane]
            .remove(index)
            .expect("ready completion");
        if lane == 0 {
            state.ordered.pop_front();
        }
        state.next = 1 - lane;
        state.counts[lane] -= 1;
        state.bytes[lane] -= done.bytes;
        if has_ready(&state) {
            wake(&mut state);
        }
        vec![(done.ticket, done.outcome)]
    }

    pub(super) fn ordered_idle(&self) -> bool {
        self.shared.state.lock().unwrap().counts[0] == 0
    }

    pub(super) fn resume_ordered(&self) {
        self.shared.state.lock().unwrap().ordered_barrier = false;
    }

    pub(super) fn notify(&self) {
        wake(&mut self.shared.state.lock().unwrap());
    }

    pub(super) fn begin_pump(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.notified = false;
        // A refusal may use this turn instead of a completion. Preserve the
        // completion wake even in that case. An extra empty pump is harmless.
        if has_ready(&state) {
            wake(&mut state);
        }
    }

    fn retire(&mut self) {
        {
            let mut state = self.shared.state.lock().unwrap();
            state.retired = true;
            // Synchronizes with all wake callbacks. None can start after this
            // returns; platform callbacks must only enqueue onto their UI loop.
            state.wake = None;
            // Queued closures are destroyed by their executor owner. Doing
            // it here could run an arbitrary destructor on the UI thread.
            for queue in &mut state.completed {
                queue.clear();
            }
        }
        self.shared.ready.notify_all();
        self.shared.abort.abort();
        // Never join arbitrary native work on the UI thread. Its WorkerSlot
        // remains charged until return, including across repeated replacement.
    }
}
impl Drop for Core {
    fn drop(&mut self) {
        self.retire();
    }
}
struct WorkerSlot;
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        LIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}
fn has_ready(state: &State) -> bool {
    !state.completed[1].is_empty()
        || state
            .ordered
            .front()
            .is_some_and(|ticket| state.completed[0].iter().any(|done| done.ticket == *ticket))
}

fn complete(shared: &Shared, lane: usize, ticket: u64, bytes: usize, outcome: Outcome) {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return;
    }
    state.completed[lane].push_back(Completed {
        ticket,
        outcome: bounded_outcome(outcome, bytes),
        bytes,
    });
    if has_ready(&state) {
        wake(&mut state);
    }
}

fn wake(state: &mut State) {
    if !state.retired && !state.notified {
        state.notified = true;
        if let Some(wake) = &state.wake {
            wake();
        }
    }
}

fn reservation(request: &Request) -> Result<(usize, usize), &'static str> {
    let (lane, body) = match request.http {
        HttpScheduling::Ordered => (0, MAX_BODY),
        HttpScheduling::Independent { max_response_bytes } => {
            if request.storage.is_some() || request.continuation.is_some() {
                return Err("only HTTP may opt into independent transport");
            }
            if max_response_bytes == 0 || max_response_bytes as usize > MAX_BODY {
                return Err("independent HTTP response limit must be 1..=64 MiB");
            }
            (1, max_response_bytes as usize)
        }
    };
    let mut bytes = std::mem::size_of::<Job>();
    // Capacity, not length: a caller cannot hide an oversized retained buffer.
    let sizes = [
        request.url.capacity(),
        request.method.capacity(),
        request.body.capacity(),
        request.grants.as_ref().map_or(0, String::capacity),
        request.storage.as_ref().map_or(0, Vec::capacity),
        request
            .headers
            .capacity()
            .saturating_mul(std::mem::size_of::<(String, String)>()),
    ];
    for n in sizes.into_iter().chain(
        request
            .headers
            .iter()
            .flat_map(|(k, v)| [k.capacity(), v.capacity()]),
    ) {
        bytes = bytes.checked_add(n).ok_or("request size overflow")?;
    }
    if bytes > MAX_REQUEST {
        return Err("native request exceeds 4 MiB");
    }
    // Vec growth while collecting can reserve up to twice the body ceiling;
    // headers, error text and queue bookkeeping have a separate allowance.
    Ok((
        lane,
        bytes + body.saturating_mul(2).max(32 << 10) + MAX_HEADERS * 2,
    ))
}

fn failed(kind: FailureKind, message: impl Into<String>) -> Outcome {
    Outcome::Failed {
        kind,
        message: message.into(),
    }
}
fn worker(
    shared: Arc<Shared>,
    lane: usize,
    bindings: Option<ibex2::host::Bindings>,
    grants: String,
) {
    let parsed = || ibex2::grant::GrantSet::parse(&exact_runner::io_grants(&grants));
    let bindings = if lane == 1 && bindings.is_none() {
        parsed().ok().map(|g| ibex2::host::Host::new().endow(g))
    } else {
        bindings
    };
    // Why a request is refused when this owner holds no bindings: grants
    // that do not parse are named, never reported as absent.
    let unbound = match (&bindings, parsed()) {
        (None, Err(e)) => format!("the app's grants did not parse: {e}"),
        _ => "the app declares no grants".to_string(),
    };
    loop {
        let job = {
            let mut state = shared.state.lock().unwrap();
            loop {
                if state.retired {
                    let abandoned = std::mem::take(&mut state.jobs[lane]);
                    drop(state);
                    drop(abandoned);
                    return;
                }
                if let Some(job) = state.jobs[lane].pop_front() {
                    break job;
                }
                state = shared.ready.wait(state).unwrap();
            }
        };
        let Job {
            ticket,
            request,
            forced,
            work,
            bytes,
        } = job;
        let work = match work {
            Some(OwnedWork::Later(hand))
                if request.continuation.is_some() && request.storage.is_none() =>
            {
                let owner = shared.clone();
                let reply =
                    Reply::new(move |outcome| complete(&owner, lane, ticket, bytes, outcome));
                // The module owner completes asynchronously; the I/O owner stays free.
                // Reply's drop path publishes an aborted outcome if the handoff panics.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hand(reply)));
                continue;
            }
            Some(OwnedWork::Now(work)) => Some(work),
            _ => None,
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let scoped = request.grants.as_deref().map(|scope| {
                exact_data::storage::scope(&grants, Some(scope))
                    .and_then(|s| {
                        ibex2::grant::GrantSet::parse(&exact_runner::io_grants(s))
                            .map_err(|e| e.to_string())
                    })
                    .map(|g| ibex2::host::Host::new().endow(g))
            });
            match scoped {
                Some(Err(message)) => failed(FailureKind::Refused, message),
                Some(Ok(ref scoped)) => execute(Ok(scoped), request, forced, work, &shared.abort),
                None => execute(
                    bindings.as_ref().ok_or(unbound.as_str()),
                    request,
                    forced,
                    work,
                    &shared.abort,
                ),
            }
        }))
        .unwrap_or_else(|_| failed(FailureKind::Aborted, "native work panicked"));
        complete(&shared, lane, ticket, bytes, outcome);
    }
}

fn bounded_outcome(outcome: Outcome, limit: usize) -> Outcome {
    let retained = match &outcome {
        Outcome::Response(r) => r
            .body
            .capacity()
            .saturating_add(
                r.headers
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(String, String)>()),
            )
            .saturating_add(
                r.headers
                    .iter()
                    .map(|(k, v)| k.capacity().saturating_add(v.capacity()))
                    .sum::<usize>(),
            ),
        Outcome::Storage(b) => b.capacity(),
        Outcome::Surface(exact_runner::SurfaceOutcome::Captured(b)) => b.capacity(),
        Outcome::Surface(exact_runner::SurfaceOutcome::Restored) => 0,
        Outcome::Failed { message, .. } => message.capacity(),
    };
    if retained > limit {
        failed(
            FailureKind::Refused,
            "native outcome exceeds retention limit",
        )
    } else {
        outcome
    }
}

fn execute(
    bindings: Result<&ibex2::host::Bindings, &str>,
    request: Request,
    forced: bool,
    work: Option<Work>,
    abort: &AbortController,
) -> Outcome {
    if abort.signal().aborted() {
        return failed(FailureKind::Aborted, "native executor retired");
    }
    if request.storage.is_some() {
        return failed(
            FailureKind::Unsupported,
            "storage requires an app storage adapter",
        );
    }
    if request.continuation.is_some() {
        return work.map_or_else(
            || {
                failed(
                    FailureKind::Unsupported,
                    "missing or consumed native continuation",
                )
            },
            |work| work(),
        );
    }
    let b = match bindings {
        Ok(b) => b,
        Err(unbound) => return failed(FailureKind::Refused, unbound),
    };
    let mut req = ibex2::stdlib::fetch::Request::get(&request.url);
    req.method = request.method;
    for (k, v) in &request.headers {
        req.headers.append(k, v);
    }
    if forced {
        req.headers.set("cache-control", "no-cache");
    }
    if !request.body.is_empty() {
        req.body = Some(request.body);
    }
    req.max_body = Some(match request.http {
        HttpScheduling::Ordered => MAX_BODY,
        HttpScheduling::Independent { max_response_bytes } => max_response_bytes as usize,
    });
    let result = b.fetch.stream(req, &abort.signal()).and_then(|r| {
        let headers = r.headers.entries();
        let bytes = headers.iter().try_fold(0usize, |n, (k, v)| {
            n.checked_add(k.len())?.checked_add(v.len())
        });
        if bytes.is_none_or(|n| n > MAX_HEADERS) || headers.len() > 1024 {
            return Err(ibex2::boundary::HostError::Failed(
                "HTTP headers exceed limit".into(),
            ));
        }
        r.collect()
    });
    match result {
        Ok(r) => Outcome::Response(Response {
            status: r.status,
            headers: r.headers.entries().to_vec(),
            body: r.body,
        }),
        Err(_) if abort.signal().aborted() => {
            failed(FailureKind::Aborted, "native executor retired")
        }
        Err(ibex2::boundary::HostError::Denied { capability }) => failed(
            FailureKind::Refused,
            format!("outside the app's grants ({capability})"),
        ),
        Err(e) => failed(
            FailureKind::Network,
            e.to_string().chars().take(2048).collect::<String>(),
        ),
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
