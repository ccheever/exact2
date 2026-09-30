//! Native transport scheduling shared by Apple and Linux (LLP 1041 D1–D4).
//! Only explicitly independent HTTP leaves the ordered lane. Each worker has
//! its own bindings and transport, so held data cannot consume control leases.
use exact_runner::{
    FailureKind, HttpScheduling, Message, Outcome, Reply, Request, RequestOut, Response,
    Work as OwnedWork,
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
/// Open streams, bounded apart from the independent lane's count: a stream
/// holds a transport lease and a reader thread for as long as it is open,
/// and must neither starve nor be starved by a lane of held replies
/// (LLP 1016.000 D4; LLP 1069.004 As built). Its bytes stay on the lane.
const STREAMS: usize = 16;
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
    /// Charged now: the request's own buffers while an ordered job waits,
    /// the whole ceiling once it runs or while independent work is admitted.
    charge: usize,
    /// The ceiling: request buffers plus the response allowance.
    limit: usize,
    /// The runner no longer wants its reply (`Core::forget`).
    forgotten: bool,
}
/// A job a worker took, until its outcome is complete.
struct Running {
    ticket: u64,
    lane: usize,
    charge: usize,
    limit: usize,
    /// A safe read's own abort, which forgetting fires; other work has none.
    abort: Option<AbortController>,
    forgotten: bool,
    stream: bool,
}
struct Completed {
    ticket: u64,
    outcome: Outcome,
    bytes: usize,
    /// One message of a stream that is still open (LLP 1016.000): its
    /// reservation stays with the running stream until the stream ends.
    message: bool,
    /// A stream's last outcome: draining it frees a stream slot.
    stream: bool,
}
#[derive(Default)]
struct State {
    jobs: [VecDeque<Job>; 2],
    running: Vec<Running>,
    completed: [VecDeque<Completed>; 2],
    counts: [usize; 2],
    /// Admitted streams, also counted in `counts[1]`.
    streams: usize,
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
/// On both lanes a waiting job is charged its request buffers, a running one
/// its response ceiling, and a completed one what it retains; a worker waits
/// for bytes rather than refusing (LLP 1041 §8.4, LLP 1054.000 R3). Only the
/// count bounds the queue of waiting jobs.
pub(super) struct Core {
    shared: Arc<Shared>,
    disabled: bool,
    /// The app's grants, for a stream's own transport.
    grants: String,
    /// A stream's own transport (the platform's; a test's scripted one).
    stream_host: StreamHost,
}
/// Makes the host (and its transport) an owner, a scoped grant or a stream
/// fetches through: the platform's, unless the embedder names another.
pub(super) type StreamHost = Arc<dyn Fn() -> ibex2::host::Host + Send + Sync>;

impl Core {
    pub(super) fn start(bindings: Option<ibex2::host::Bindings>, grants: &str, wake: Wake) -> Self {
        // Additional transports are constructed on their own threads, not
        // while the presenter is trying to publish its first frame.
        Self::with_owners(vec![bindings, None, None], grants, wake)
    }

    /// [`Core::start`] with every transport the other owners, scoped grants
    /// and streams open made by `host` (the render host's, LLP 1048.000 D10).
    #[allow(dead_code)] // only the render host's executor calls it
    pub(super) fn start_on(
        bindings: Option<ibex2::host::Bindings>,
        grants: &str,
        wake: Wake,
        host: StreamHost,
    ) -> Self {
        Self::with_owners_on(vec![bindings, None, None], grants, wake, host)
    }

    fn with_owners(owners: Vec<Option<ibex2::host::Bindings>>, grants: &str, wake: Wake) -> Self {
        Self::with_owners_on(owners, grants, wake, Arc::new(ibex2::host::Host::new))
    }

    fn with_owners_on(
        owners: Vec<Option<ibex2::host::Bindings>>,
        grants: &str,
        wake: Wake,
        host: StreamHost,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                wake: Some(wake),
                ..State::default()
            }),
            ready: Condvar::new(),
            abort: AbortController::new(),
        });
        let reserve = || {
            LIVE_WORKERS
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                    (n + WORKERS <= MAX_WORKERS).then_some(n + WORKERS)
                })
                .is_ok()
        };
        #[cfg_attr(not(test), allow(unused_mut))]
        let mut reserved = reserve();
        // Every test in the binary shares the process's bound, one core per
        // test thread: a test waits for a slot rather than being refused.
        #[cfg(test)]
        for _ in 0..5000 {
            if reserved {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            reserved = reserve();
        }
        let mut core = Self {
            shared,
            disabled: !reserved,
            grants: grants.to_string(),
            stream_host: host,
        };
        if !reserved {
            return core;
        }
        for (index, bindings) in owners.into_iter().enumerate() {
            let shared = core.shared.clone();
            let grants = grants.to_string();
            let host = core.stream_host.clone();
            let guard = WorkerSlot;
            let spawned = std::thread::Builder::new()
                .name(format!("exact-io-{index}"))
                .spawn(move || {
                    let _guard = guard;
                    worker(shared, usize::from(index != 0), bindings, grants, host);
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
            let (lane, charge, limit) = reservation(&r.request)?;
            if self.disabled {
                return Err("native executor worker limit reached");
            }
            if state.retired {
                return Err("native executor retired");
            }
            if ordered && state.ordered_barrier {
                return Err("earlier ordered admission refusal must settle first");
            }
            // A stream starts at once, so it is charged its ceiling now.
            let (full, charge) = if r.request.stream {
                (state.streams >= STREAMS, limit)
            } else {
                let streams = if lane == 1 { state.streams } else { 0 };
                (state.counts[lane] - streams >= COUNTS[lane], charge)
            };
            if full || charge > BYTES[lane].saturating_sub(state.bytes[lane]) {
                return Err("native executor admission limit reached");
            }
            Ok((lane, charge, limit))
        })();
        let (lane, charge, limit) = match admitted {
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
        state.bytes[lane] += charge;
        if r.request.stream {
            // Never queued behind held replies: a stream opens on its own
            // thread with its own transport, and reads there (LLP 1067 D3).
            state.streams += 1;
            let abort = AbortController::new();
            state.running.push(Running {
                ticket: r.ticket,
                lane,
                charge,
                limit,
                abort: Some(abort.clone()),
                forgotten: false,
                stream: true,
            });
            drop(state);
            let (shared, grants) = (self.shared.clone(), self.grants.clone());
            let (ticket, request, forced) = (r.ticket, r.request, r.forced);
            let host = self.stream_host.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("exact-stream-{ticket}"))
                .spawn({
                    let shared = shared.clone();
                    move || stream::run(&shared, ticket, request, forced, &grants, &*host, abort)
                });
            if let Err(e) = spawned {
                complete(&shared, ticket, failed(FailureKind::Network, e.to_string()));
            }
            return Ok(());
        }
        state.jobs[lane].push_back(Job {
            ticket: r.ticket,
            request: r.request,
            forced: r.forced,
            work,
            charge,
            limit,
            forgotten: false,
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
        if !done.message {
            state.counts[lane] -= 1;
            state.streams -= usize::from(done.stream);
            state.bytes[lane] -= done.bytes;
        }
        // An ordered job may be waiting for these bytes.
        self.shared.ready.notify_all();
        if has_ready(&state) {
            wake(&mut state);
        }
        vec![(done.ticket, done.outcome)]
    }

    /// Let go of the work for every ticket the runner no longer `held`
    /// (LLP 1016 D5): its reply would only be dropped there. A completed
    /// outcome is dropped undrained and a queued safe HTTP read unrun; a
    /// running one is aborted. Other work (a write, a module turn) still runs,
    /// since a write that was sent, or a turn that has begun, is not undone.
    /// None of it holds a later ordered completion back, and each releases
    /// its count and bytes when it ends.
    pub(super) fn forget(&self, held: impl Fn(u64) -> bool) {
        let mut aborts = Vec::new();
        {
            let mut guard = self.shared.state.lock().unwrap();
            let state = &mut *guard;
            for lane in 0..2 {
                let (counts, bytes) = (&mut state.counts[lane], &mut state.bytes[lane]);
                let streams = &mut state.streams;
                state.jobs[lane].retain_mut(|job| {
                    if held(job.ticket) {
                        return true;
                    }
                    if job.work.is_none() && safe(&job.request) {
                        *counts -= 1;
                        *streams -= usize::from(job.request.stream);
                        *bytes -= job.charge;
                        return false;
                    }
                    job.forgotten = true;
                    true
                });
                state.completed[lane].retain(|done| {
                    if held(done.ticket) {
                        return true;
                    }
                    // A forgotten stream's message: the stream itself
                    // releases the reservation when its reader ends.
                    if !done.message {
                        *counts -= 1;
                        *streams -= usize::from(done.stream);
                        *bytes -= done.bytes;
                    }
                    false
                });
            }
            for run in &mut state.running {
                if !run.forgotten && !held(run.ticket) {
                    run.forgotten = true;
                    aborts.extend(run.abort.clone());
                }
            }
            state.ordered.retain(|ticket| held(*ticket));
            self.shared.ready.notify_all();
            if has_ready(state) {
                wake(state);
            }
        }
        // Transport callbacks run outside the lock, as retirement's do.
        for abort in aborts {
            abort.abort();
        }
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

    /// `notify`, as a handle another thread keeps (a native module's
    /// announcements, LLP 1016.002).
    pub(super) fn waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        let shared = self.shared.clone();
        std::sync::Arc::new(move || wake(&mut shared.state.lock().unwrap()))
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

/// The next job `lane`'s worker may start, with its abort, charging an
/// ordered one its response ceiling — or `None` until those bytes are free.
fn next_job(state: &mut State, lane: usize) -> Option<(Job, AbortController)> {
    let job = state.jobs[lane].front()?;
    // A module handoff's outcome is the owner's to make; it is charged
    // what it retains when it completes.
    let more = if handed_off(job) {
        0
    } else {
        job.limit - job.charge
    };
    if more > BYTES[lane].saturating_sub(state.bytes[lane]) {
        return None;
    }
    let mut job = state.jobs[lane].pop_front()?;
    job.charge += more;
    state.bytes[lane] += more;
    let abort = AbortController::new();
    state.running.push(Running {
        ticket: job.ticket,
        lane,
        charge: job.charge,
        limit: job.limit,
        abort: safe(&job.request).then(|| abort.clone()),
        forgotten: job.forgotten,
        stream: job.request.stream,
    });
    Some((job, abort))
}

fn complete(shared: &Shared, ticket: u64, outcome: Outcome) {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return;
    }
    let Some(at) = state.running.iter().position(|run| run.ticket == ticket) else {
        return;
    };
    let run = state.running.swap_remove(at);
    let outcome = bounded_outcome(outcome, run.limit);
    let lane = run.lane;
    // A completion frees ordered bytes a waiting job may need.
    shared.ready.notify_all();
    if run.forgotten {
        state.counts[lane] -= 1;
        state.streams -= usize::from(run.stream);
        state.bytes[lane] -= run.charge;
        return;
    }
    let bytes = if lane == 0 {
        retained(&outcome) + std::mem::size_of::<Completed>()
    } else {
        run.charge
    };
    state.bytes[lane] = state.bytes[lane] - run.charge + bytes;
    state.completed[lane].push_back(Completed {
        ticket,
        outcome,
        bytes,
        message: false,
        stream: run.stream,
    });
    if has_ready(&state) {
        wake(&mut state);
    }
}

/// One message of the open stream `ticket`, false once nobody wants more
/// (forgotten, retired, ended). The newest undelivered message per ticket is
/// kept and the ones it replaces are counted on it (LLP 1016.000 D4): display
/// data coalesces, and a log sees the gap and re-asks from its cursor.
fn message(shared: &Shared, ticket: u64, mut message: Message) -> bool {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return false;
    }
    let Some(lane) = state
        .running
        .iter()
        .find(|run| run.ticket == ticket && !run.forgotten)
        .map(|run| run.lane)
    else {
        return false;
    };
    let waiting = state.completed[lane]
        .iter_mut()
        .find(|done| done.ticket == ticket && done.message);
    match waiting {
        Some(done) => {
            if let Outcome::Message(older) = &done.outcome {
                message.coalesced = message
                    .coalesced
                    .saturating_add(older.coalesced)
                    .saturating_add(1);
            }
            done.outcome = Outcome::Message(message);
        }
        None => state.completed[lane].push_back(Completed {
            ticket,
            outcome: Outcome::Message(message),
            // Charged to the running stream: its ceiling covers one message.
            bytes: 0,
            message: true,
            stream: true,
        }),
    }
    if has_ready(&state) {
        wake(&mut state);
    }
    true
}

/// A continuation a worker hands to the module's owner instead of running,
/// and a long native call it hands to the app's native module.
fn handoff(request: &Request) -> bool {
    (request.continuation.is_some() && request.storage.is_none()) || request.is_native()
}

fn handed_off(job: &Job) -> bool {
    handoff(&job.request) && matches!(job.work, Some(OwnedWork::Later(_)))
}

/// An HTTP read with no effect to lose if it is never sent (RFC 9110 §9.2.1).
fn safe(request: &Request) -> bool {
    request.continuation.is_none()
        && request.storage.is_none()
        && ["GET", "HEAD"]
            .iter()
            .any(|m| request.method.eq_ignore_ascii_case(m))
}

fn wake(state: &mut State) {
    if !state.retired && !state.notified {
        state.notified = true;
        if let Some(wake) = &state.wake {
            wake();
        }
    }
}

/// The lane, the charge at admission and the ceiling a job may retain.
fn reservation(request: &Request) -> Result<(usize, usize, usize), &'static str> {
    if request.stream
        && (request.http == HttpScheduling::Ordered
            || request.storage.is_some()
            || request.continuation.is_some()
            || request.is_native())
    {
        return Err("a stream is independent HTTP (LLP 1016.000)");
    }
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
    let limit = bytes + body.saturating_mul(2).max(32 << 10) + MAX_HEADERS * 2;
    // Admission charges the request buffers; the worker charges the
    // ceiling when it starts the job, waiting for bytes (LLP 1054.000 R3).
    // A ceiling the lane can never hold is refused here, not left waiting.
    if limit > BYTES[lane] {
        return Err("native response ceiling exceeds the lane's byte budget");
    }
    Ok((lane, bytes, limit))
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
    host: StreamHost,
) {
    let parsed = || ibex2::grant::GrantSet::parse(&exact_runner::io_grants(&grants));
    let bindings = if lane == 1 && bindings.is_none() {
        parsed().ok().map(|g| host().endow(g))
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
        let (job, abort) = {
            let mut state = shared.state.lock().unwrap();
            loop {
                if state.retired {
                    let abandoned = std::mem::take(&mut state.jobs[lane]);
                    drop(state);
                    drop(abandoned);
                    return;
                }
                if let Some(next) = next_job(&mut state, lane) {
                    break next;
                }
                state = shared.ready.wait(state).unwrap();
            }
        };
        let Job {
            ticket,
            request,
            forced,
            work,
            ..
        } = job;
        let work = match work {
            Some(OwnedWork::Later(hand)) if handoff(&request) => {
                let owner = shared.clone();
                let reply = Reply::new(move |outcome| complete(&owner, ticket, outcome));
                // The module owner completes asynchronously; the I/O owner stays free.
                // Reply's drop path publishes an aborted outcome if the handoff panics.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hand(reply)));
                continue;
            }
            Some(OwnedWork::Now(work)) => Some(work),
            _ => None,
        };
        // Retirement aborts every job; forgetting aborts a safe read.
        let _retiring = {
            let abort = abort.clone();
            shared.abort.signal().register(move || abort.abort())
        };
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                match scoped_bindings(&grants, request.grants.as_deref(), &host) {
                    Some(Err(message)) => failed(FailureKind::Refused, message),
                    Some(Ok(ref scoped)) => execute(Ok(scoped), request, forced, work, &abort),
                    None => execute(
                        bindings.as_ref().ok_or(unbound.as_str()),
                        request,
                        forced,
                        work,
                        &abort,
                    ),
                }
            }))
            .unwrap_or_else(|_| failed(FailureKind::Aborted, "native work panicked"));
        complete(&shared, ticket, outcome);
    }
}

/// A source's narrower grant scope, as its own bindings; `None` unscoped.
fn scoped_bindings(
    grants: &str,
    scope: Option<&str>,
    host: &StreamHost,
) -> Option<Result<ibex2::host::Bindings, String>> {
    scope.map(|scope| {
        exact_data::storage::scope(grants, Some(scope))
            .and_then(|s| {
                ibex2::grant::GrantSet::parse(&exact_runner::io_grants(s))
                    .map_err(|e| e.to_string())
            })
            .map(|g| host().endow(g))
    })
}

#[cfg(test)]
impl Core {
    /// Streams open on `host`'s transport: a test's scripted one.
    fn streams_on(mut self, host: impl Fn() -> ibex2::host::Host + Send + Sync + 'static) -> Self {
        self.stream_host = Arc::new(host);
        self
    }
}

fn bounded_outcome(outcome: Outcome, limit: usize) -> Outcome {
    if retained(&outcome) > limit {
        failed(
            FailureKind::Refused,
            "native outcome exceeds retention limit",
        )
    } else {
        outcome
    }
}

/// The bytes an outcome keeps until the UI takes it.
fn retained(outcome: &Outcome) -> usize {
    match outcome {
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
        Outcome::Message(m) => m
            .data
            .capacity()
            .saturating_add(m.event.capacity())
            .saturating_add(m.id.capacity()),
        Outcome::Surface(exact_runner::SurfaceOutcome::Captured(b)) => b.capacity(),
        Outcome::Surface(exact_runner::SurfaceOutcome::Restored) => 0,
        Outcome::Failed { message, .. } => message.capacity(),
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
        return failed(FailureKind::Aborted, "native request aborted");
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
    let limit = match request.http {
        HttpScheduling::Ordered => MAX_BODY,
        HttpScheduling::Independent { max_response_bytes } => max_response_bytes as usize,
    };
    let mut req = fetch_request(request, forced);
    req.max_body = Some(limit);
    let result = b
        .fetch
        .stream(req, &abort.signal())
        .and_then(check_headers)
        .and_then(|r| r.collect());
    match result {
        Ok(r) => Outcome::Response(Response {
            status: r.status,
            headers: r.headers.entries().to_vec(),
            body: r.body,
        }),
        Err(e) => fetch_failure(e, abort),
    }
}

/// The transport's request for a runner's.
fn fetch_request(request: Request, forced: bool) -> ibex2::stdlib::fetch::Request {
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
    req
}

fn check_headers(
    r: ibex2::stdlib::fetch::StreamingResponse,
) -> Result<ibex2::stdlib::fetch::StreamingResponse, ibex2::boundary::HostError> {
    let headers = r.headers.entries();
    let bytes = headers.iter().try_fold(0usize, |n, (k, v)| {
        n.checked_add(k.len())?.checked_add(v.len())
    });
    if bytes.is_none_or(|n| n > MAX_HEADERS) || headers.len() > 1024 {
        return Err(ibex2::boundary::HostError::Failed(
            "HTTP headers exceed limit".into(),
        ));
    }
    Ok(r)
}

fn fetch_failure(e: ibex2::boundary::HostError, abort: &AbortController) -> Outcome {
    match e {
        _ if abort.signal().aborted() => failed(FailureKind::Aborted, "native request aborted"),
        ibex2::boundary::HostError::Denied { capability } => failed(
            FailureKind::Refused,
            format!("outside the app's grants ({capability})"),
        ),
        e => failed(
            FailureKind::Network,
            e.to_string().chars().take(2048).collect::<String>(),
        ),
    }
}

#[path = "executor_stream.rs"]
mod stream;

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
