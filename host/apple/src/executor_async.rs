//! The executor without a thread per request (LLP 1120 D3–D5): each lane has
//! a starter, the only place its work starts, and a finisher, the only place
//! its outcomes are built and its releases applied; one timer keeps every
//! deadline and one disposer destroys work let go. HTTP goes to an
//! asynchronous transport (ibex2's `fetch_async`): on Apple, one
//! `NSURLSession` per lane. The admission rules, the queues, settlement and
//! the fence are `executor_core.rs`'s, unchanged.
//!
//! Releases, then wakes: a callback, the timer, a reader or a buffer's
//! release only posts to the lane's finisher, which applies what the event
//! makes due under the state lock, then wakes the starter, so a starter
//! never decides on a release not yet applied. Every event names its job's
//! run (ticket and generation): a parked upload's next run ignores the last
//! one's late events.
use super::{
    body, bounded_outcome, failed, fetch_failure, fetch_request, handoff, retained, safe,
    scoped_bindings, Completed, Job, Running, Shared, State, StreamHost, BYTES, MAX_BODY,
    MAX_HEADERS,
};
use exact_runner::{
    FailureKind, HttpScheduling, Outcome, Reply, Request, Response, Work as OwnedWork,
};
use ibex2::boundary::HostError;
use ibex2::stdlib::abort::{AbortController, AbortRegistration};
use ibex2::stdlib::fetch_async::{
    AsyncTransport, Collected, Exchange, FetchEvents, Flow, Head, HeaderCaps, HopMetrics,
    HopRequest, Sink, Upload,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

#[path = "executor_timer.rs"]
mod timer;
use timer::Timer;
#[path = "executor_async_file.rs"]
mod file;
#[cfg(test)]
pub(super) use file::ReaderPhase;
#[path = "executor_workers.rs"]
mod workers;

/// Resumed independent HTTP exchanges at once, the session's per-host
/// connection limit too (LLP 1120 D2, D5): HTTP/2 multiplexes them.
pub(super) const IN_FLIGHT: usize = super::build_number(option_env!("EXACT_HTTP_IN_FLIGHT"), 32);
/// HTTP/1.1 exchanges the core resumes per origin per lane until one there
/// reports `h2` or `h3` (D3): URLSession's own limit, so it never queues.
const PER_ORIGIN: usize = 6;
/// The base set: two starters, two finishers, the timer, the disposer (D4).
pub(super) const THREADS: usize = 6 + if cfg!(any(not(target_vendor = "apple"), test)) {
    1 + super::INDEPENDENT
} else {
    0
};

/// A job's run: its ticket and generation.
type Run = (u64, u64);

/// Who claimed a run's terminal transition, under the state lock when it
/// happened (D3): only that outcome settles it, whatever else is queued.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Claim {
    #[default]
    None,
    /// Its own outcome: a fetch's, a closure's, a handoff's, a refusal.
    Answered,
    /// Its deadline.
    Timeout,
    /// A forget that cancelled a safe read.
    Cancelled,
}

/// Claim `run`'s transition for `claim`, if nothing has: whether it won.
fn claim(state: &mut State, run: Run, claim: Claim) -> bool {
    match find(&mut state.running, run) {
        Some(r) if !r.terminal && r.claim == Claim::None => {
            r.claim = claim;
            true
        }
        _ => false,
    }
}

/// The async core's bookkeeping, in `State`.
#[derive(Default)]
pub(super) struct Book {
    /// The portable blocking adapter retains one additional upload copy.
    pub(super) portable: [bool; 2],
    /// Live independent HTTP exchanges, and places held for ones about to
    /// start: the in-flight cap's count.
    in_flight: usize,
    /// Live HTTP/1.1 exchanges per origin, per lane, with places held.
    origins: [HashMap<String, usize>; 2],
    /// Origins an exchange reported `h2` or `h3` for, per lane.
    pub(super) multiplexed: [HashSet<String>; 2],
    /// The ordered lane's one blocking job is in flight.
    blocking: bool,
    /// Exchanges waiting for the starter: a redirect's next hop, an
    /// upload's first, each with its origin when it is known now.
    hops: [VecDeque<Hop>; 2],
    /// Places held for a run's next hop, from the starter's choice until
    /// its transport starts it: the in-flight one, and the origin's when
    /// known.
    reserved: HashMap<Run, Option<String>>,
    /// Upload bytes charged per run, released once the lent buffer has
    /// gone and the run has physically ended (D5).
    uploads: HashMap<Run, Charged>,
    /// A file job's request and what its fetch needs, while a reader sizes
    /// or reads its file.
    files: HashMap<Run, FileJob>,
    /// What a sized `exactBodyFrom` job learned: its file's size, kept
    /// while it is parked at the head (D5).
    pub(super) sized: HashMap<u64, u64>,
    /// One sizing decision per lane: successors remain queued, so a park
    /// accounts for every request buffer that could otherwise crowd it out.
    sizing: [Option<Run>; 2],
    /// A sizing reader waiting for the finisher's decision.
    deciders: HashMap<Run, mpsc::Sender<Decision>>,
    /// The last run's generation.
    generation: u64,
    /// Work let go under the lock (an exchange whose run settled, with an
    /// upload's bytes): destroyed by the disposer, never under the lock.
    trash: Vec<Box<dyn Send>>,
}

impl Book {
    /// The bytes the parked uploads in `jobs` still need, which admission
    /// counts as taken (LLP 1120 §6 question 8, provisional), wherever in
    /// the queue they wait.
    pub(super) fn parked(&self, lane: usize, jobs: &VecDeque<Job>) -> usize {
        jobs.iter()
            .filter_map(|j| {
                self.sized.get(&j.ticket).map(|n| {
                    j.limit - j.charge
                        + self
                            .upload_bytes(lane, *n as usize)
                            .expect("admitted upload")
                })
            })
            .sum()
    }
    pub(super) fn reserve_inline_copy(
        &self,
        request: &Request,
        (lane, charge, limit): (usize, usize, usize),
    ) -> Result<(usize, usize, usize), &'static str> {
        let extra = if self.portable[lane] {
            request.body.len()
        } else {
            0
        };
        let limit = limit.checked_add(extra).ok_or("request size overflow")?;
        if limit > BYTES[lane] {
            return Err("native response ceiling exceeds the lane's byte budget");
        }
        Ok((lane, charge, limit))
    }
    fn upload_bytes(&self, lane: usize, size: usize) -> Option<usize> {
        size.checked_mul(if self.portable[lane] { 2 } else { 1 })
    }
}

/// An exchange for the starter to start.
struct Hop {
    run: Run,
    origin: Option<String>,
    start: Start,
}

/// An upload's charge and whether what holds it still does.
struct Charged {
    lane: usize,
    bytes: usize,
    /// The lent buffer was made, and whether it still exists: every hop
    /// that sends it holds it, the platform's copy included.
    made: bool,
    buffer: bool,
    /// Exchanges on the wire that send it.
    hops: usize,
    /// The run has physically ended.
    ended: bool,
}

impl Charged {
    fn new(lane: usize, bytes: usize) -> Charged {
        Charged {
            lane,
            bytes,
            made: false,
            buffer: false,
            hops: 0,
            ended: false,
        }
    }
}

/// What a file job's fetch needs once its file is read.
struct FileJob {
    request: Request,
    forced: bool,
    limit: usize,
    abort: AbortController,
    retiring: AbortRegistration,
    transport: Arc<dyn AsyncTransport>,
    fetch: ibex2::host::Fetch,
}

/// What the finisher tells a sizing reader.
enum Decision {
    /// The whole charge is taken: read the file.
    Read,
    /// The request is settled, or parked: stop.
    Exit,
}

/// How a selected job starts, off the lock.
type Start = Box<dyn FnOnce(&Arc<Machine>) + Send>;
/// What a finisher runs after an event, off the lock.
type After = Box<dyn FnOnce(&Arc<Machine>)>;
/// A lane's transport and the grants its owner holds.
type Lane = (Arc<dyn AsyncTransport>, ibex2::host::Fetch);

/// What a finisher applies.
enum Event {
    /// A run's outcome: a fetch's, a closure's, a handoff's, the timer's.
    Outcome(Run, Done),
    /// Something physical of a run ended: a hop (with the origin it
    /// counted, and whether it sent the upload), a reader, a start.
    Ended(Run, Option<String>, bool),
    /// A charged upload buffer's last release, including queued disposal.
    Released(Run),
    /// An origin's protocol, from a hop's metrics.
    Protocol(String, String),
    /// A sizing reader's answer.
    Sized(Run, Result<u64, Outcome>),
    /// A reader's file, or why not (`None`: it grew past its charge).
    Read(Run, Result<Vec<u8>, Option<Outcome>>),
    /// A redirect's next hop.
    Hop(Run, Box<dyn FnOnce() + Send>),
}

enum Done {
    /// A fetch's result, and its response ceiling.
    Fetch(Result<Collected, HostError>, usize),
    Now(Outcome),
    /// The deadline's, which claimed the transition.
    Timeout(u32),
    /// A forget's, which claimed it by cancelling a safe read.
    Cancelled,
}

/// The machine for one core: its lanes' transports, channels and threads.
pub(super) struct Machine {
    shared: Arc<Shared>,
    worker_budget: Arc<workers::Budget>,
    transports: [Option<Lane>; 2],
    /// Why a request is refused on a lane with no transport.
    unbound: String,
    finishers: [Mutex<Option<mpsc::Sender<Event>>>; 2],
    timer: Timer,
    grants: String,
    host: StreamHost,
    /// Retired: a starter starts nothing more.
    retired: AtomicBool,
    #[cfg(test)]
    pub(super) reader_hook: Mutex<Option<file::ReaderHook>>,
}

/// How a lane's transport is made from its owner's bindings.
pub(super) type Lanes = Arc<
    dyn Fn(usize, &ibex2::host::Bindings, Arc<workers::Budget>) -> (Arc<dyn AsyncTransport>, bool)
        + Send
        + Sync,
>;

impl Machine {
    pub(super) fn new(
        shared: Arc<Shared>,
        owners: Vec<Option<ibex2::host::Bindings>>,
        grants: &str,
        host: StreamHost,
        lanes: &Lanes,
    ) -> Arc<Machine> {
        let parsed = || ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants));
        let unbound = match (&owners.first().cloned().flatten(), parsed()) {
            (None, Err(e)) => format!("the app's grants did not parse: {e}"),
            _ => "the app declares no grants".to_string(),
        };
        let mut owners = owners.into_iter();
        let ordered = owners.next().flatten();
        let independent = owners
            .next()
            .flatten()
            .or_else(|| parsed().ok().map(|g| host().endow(g)));
        let worker_budget = Arc::new(workers::Budget::default());
        let transport = |lane: usize, b: Option<ibex2::host::Bindings>| {
            b.map(|b| {
                let (transport, portable) = lanes(lane, &b, worker_budget.clone());
                shared.state.lock().unwrap().book.portable[lane] = portable;
                (transport, b.fetch)
            })
        };
        let transports = [transport(0, ordered), transport(1, independent)];
        Arc::new(Machine {
            shared,
            transports,
            worker_budget,
            unbound,
            finishers: [Mutex::new(None), Mutex::new(None)],
            timer: Timer::default(),
            grants: grants.to_string(),
            host,
            retired: AtomicBool::new(false),
            #[cfg(test)]
            reader_hook: Mutex::new(None),
        })
    }

    /// Start the base set, each thread holding one of `slots` until it
    /// exits; false when a thread could not be made (the caller then
    /// retires the core, and the ones made exit).
    pub(super) fn start(self: &Arc<Self>, slots: Vec<super::WorkerSlot>) -> bool {
        let mut slots = slots.into_iter();
        let mut ok = true;
        let mut spawn = |name: String, body: Box<dyn FnOnce() + Send>| {
            let slot = slots.next();
            ok &= workers::spawn(
                name,
                Box::new(move || {
                    let _slot = slot;
                    body()
                }),
            )
            .is_ok();
        };
        for lane in 0..2 {
            let (send, receive) = mpsc::channel();
            *self.finishers[lane].lock().unwrap() = Some(send);
            let m = self.clone();
            spawn(
                format!("exact-finish-{lane}"),
                Box::new(move || m.finisher(lane, receive)),
            );
            let m = self.clone();
            spawn(
                format!("exact-start-{lane}"),
                Box::new(move || m.starter(lane)),
            );
        }
        let m = self.clone();
        spawn("exact-timer".into(), Box::new(move || m.timer()));
        let m = self.clone();
        spawn("exact-dispose".into(), Box::new(move || m.disposer()));
        if ok {
            *self.worker_budget.0.lock().unwrap() = slots.collect();
        }
        ok
    }

    /// Retirement: nothing more starts, the finishers' channels close, the
    /// timer stops, and readers waiting for a decision are let go.
    pub(super) fn retire(&self) {
        self.stop(&mut self.shared.state.lock().unwrap());
    }

    /// `retire`, with the state lock held.
    pub(super) fn stop(&self, state: &mut State) {
        self.retired.store(true, Ordering::Release);
        for f in &self.finishers {
            f.lock().unwrap().take();
        }
        self.timer.stop();
        state.book.deciders.clear();
    }

    /// `run`'s own outcome, claimed now and published by the finisher.
    fn answer(&self, lane: usize, run: Run, outcome: Outcome) {
        if claim(&mut self.shared.state.lock().unwrap(), run, Claim::Answered) {
            self.post(lane, Event::Outcome(run, Done::Now(outcome)));
        }
    }

    fn post(&self, lane: usize, event: Event) {
        let send = self.finishers[lane].lock().unwrap().clone();
        if let Some(send) = send {
            let _ = send.send(event);
        }
    }

    /// A forgotten safe read's cancellation, which claimed its transition
    /// under the state lock (`Core::forget`): published now, as Aborted.
    pub(super) fn cancelled(&self, lane: usize, run: Run) {
        self.post(lane, Event::Outcome(run, Done::Cancelled));
    }

    // ---- the starter ----

    fn starter(self: Arc<Self>, lane: usize) {
        loop {
            let starts = {
                let mut state = self.shared.state.lock().unwrap();
                loop {
                    if state.retired {
                        return;
                    }
                    let starts = self.select(&mut state, lane);
                    if !starts.is_empty() {
                        break starts;
                    }
                    state = self.shared.ready.wait(state).unwrap();
                }
            };
            for (run, start) in starts {
                let _ended = Ended(self.clone(), lane, run);
                // Started only if its run is still open: one settled meanwhile
                // (or retirement) lets it go unstarted. Either way the start's
                // own hold on the run ends with it.
                let open = !self.retired.load(Ordering::Acquire)
                    && find(&mut self.shared.state.lock().unwrap().running, run)
                        .is_some_and(|r| !r.terminal && r.claim == Claim::None);
                if open {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| start(&self)))
                        .is_err()
                    {
                        self.answer(
                            lane,
                            run,
                            failed(FailureKind::Aborted, "native start panicked"),
                        );
                    }
                } else {
                    drop(start);
                }
            }
        }
    }

    /// What `lane` starts now, decided under the lock (D3, D5). Every
    /// exchange chosen holds its in-flight and origin places from here.
    fn select(&self, state: &mut State, lane: usize) -> Vec<(Run, Start)> {
        let mut starts: Vec<(Run, Start)> = Vec::new();
        // Waiting exchanges first: their requests already run.
        let mut at = 0;
        while at < state.book.hops[lane].len() {
            if lane == 1 && state.book.in_flight >= IN_FLIGHT {
                break;
            }
            let run = state.book.hops[lane][at].run;
            if !state
                .running
                .iter()
                .any(|r| (r.ticket, r.generation) == run && !r.terminal && r.claim == Claim::None)
            {
                // Its run settled meanwhile: nothing to start.
                let hop = state.book.hops[lane].remove(at).expect("index");
                state.book.trash.push(Box::new(hop.start));
                continue;
            }
            let hop = &state.book.hops[lane][at];
            if hop
                .origin
                .as_deref()
                .is_some_and(|o| at_limit(state, lane, o))
            {
                at += 1;
                continue;
            }
            let hop = state.book.hops[lane].remove(at).expect("index");
            hold(&mut state.book, lane, hop.run, hop.origin);
            // The start holds its run until it has run (`starter`).
            find(&mut state.running, hop.run).expect("open").live += 1;
            starts.push((hop.run, hop.start));
        }
        let mut index = 0;
        while index < state.jobs[lane].len() {
            let job = &state.jobs[lane][index];
            let kind = Kind::of(job);
            if lane == 0 && (index > 0 || state.book.blocking) {
                // One blocking job; a handoff only while none runs; nothing
                // passes the head.
                break;
            }
            let exchange = kind == Kind::Http && job.request.body_from.is_none();
            if kind == Kind::Http
                && job.request.body_from.is_some()
                && !state.book.sized.contains_key(&job.ticket)
                && state.book.sizing[lane].is_some()
            {
                break;
            }
            if exchange && lane == 1 && state.book.in_flight >= IN_FLIGHT {
                break;
            }
            // A head short of bytes waits and is not passed over.
            if !fits(state, lane, job, kind) {
                break;
            }
            if exchange && at_limit(state, lane, &origin(&job.request.url)) {
                // The independent lane passes over a head at its origin's
                // HTTP/1.1 limit; the ordered one waits for it (D3).
                if lane == 0 {
                    break;
                }
                index += 1;
                continue;
            }
            let start = self.take(state, lane, index, kind);
            starts.push(start);
            if lane == 0 && kind != Kind::Handoff {
                break;
            }
            // From the head again: a head passed over may no longer fit.
            index = 0;
        }
        starts
    }

    /// Move job `index`, which fits, into `running`, charged, and say how
    /// it starts.
    fn take(&self, state: &mut State, lane: usize, index: usize, kind: Kind) -> (Run, Start) {
        let job = state.jobs[lane].remove(index).expect("index");
        let mut job = job;
        let (more, upload) = charge(&state.book, lane, &job, kind);
        job.charge += more;
        state.bytes[lane] += more + upload;
        state.book.generation += 1;
        let run = (job.ticket, state.book.generation);
        let sized = state.book.sized.remove(&job.ticket).is_some();
        if kind == Kind::Http && job.request.body_from.is_some() && !sized {
            state.book.sizing[lane] = Some(run);
        }
        if sized {
            state.book.uploads.insert(run, Charged::new(lane, upload));
        }
        if kind == Kind::Http && job.request.body_from.is_none() {
            hold(&mut state.book, lane, run, Some(origin(&job.request.url)));
        }
        let blocking = lane == 0 && kind != Kind::Handoff;
        state.book.blocking |= blocking;
        let abort = AbortController::new();
        state.running.push(Running {
            started: Instant::now(),
            ticket: job.ticket,
            lane,
            charge: job.charge,
            limit: job.limit,
            // Only an exchange is cancelled by a forget (a safe read's).
            abort: (kind == Kind::Http && safe(&job.request)).then(|| abort.clone()),
            exchange: (kind == Kind::Http).then(|| abort.clone()),
            forgotten: job.forgotten,
            stream: false,
            waiting: false,
            read: lane == 0 && super::read(&job.request, job.work.is_some()),
            terminal: false,
            // The start holds its run until it has run (`starter`).
            live: 1,
            drained: false,
            blocking,
            generation: run.1,
            claim: Claim::None,
        });
        let start: Start =
            Box::new(move |m: &Arc<Machine>| m.run(lane, run, kind, job, sized, abort));
        (run, start)
    }

    /// Start one job, off the lock.
    fn run(
        self: &Arc<Self>,
        lane: usize,
        run: Run,
        kind: Kind,
        job: Job,
        sized: bool,
        abort: AbortController,
    ) {
        let Job {
            request,
            forced,
            work,
            ..
        } = job;
        let refuse = |outcome| self.answer(lane, run, outcome);
        // A source's narrower scope, refused before any work runs, as on
        // the workers.
        let scoped = scoped_bindings(&self.grants, request.grants.as_deref(), &self.host);
        if let Some(Err(message)) = scoped {
            return refuse(failed(FailureKind::Refused, message));
        }
        // Retirement aborts every job; forgetting aborts a safe read.
        let retiring = {
            let abort = abort.clone();
            self.shared.abort.signal().register(move || abort.abort())
        };
        match kind {
            Kind::Handoff => {
                let Some(OwnedWork::Later(hand)) = work else {
                    unreachable!("a handoff has its hand")
                };
                let m = self.clone();
                let reply = Reply::new(move |outcome| {
                    m.answer(lane, run, outcome);
                    drop(retiring);
                });
                // The module owner completes asynchronously; the starter stays
                // free. Reply's drop publishes Aborted if the handoff panics.
                // The call is physical work until it returns, even when its
                // reply comes inside it: the start's hold covers it.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hand(reply)));
            }
            Kind::Closure => {
                // Run here (a driver fault's, a continuation, storage): the
                // lane's starts wait for it (a declared change, D3).
                let work = match work {
                    Some(OwnedWork::Now(work)) => Some(work),
                    _ => None,
                };
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    closure(request, work, &abort)
                }))
                .unwrap_or_else(|_| failed(FailureKind::Aborted, "native work panicked"));
                refuse(outcome);
            }
            Kind::Http => {
                let fetch = scoped.map(|s| s.expect("checked").fetch);
                self.http(lane, run, request, forced, sized, abort, retiring, fetch)
            }
        }
    }

    /// An HTTP job: its file sized or read, or its fetch started on the
    /// lane's transport.
    #[allow(clippy::too_many_arguments)]
    fn http(
        self: &Arc<Self>,
        lane: usize,
        run: Run,
        request: Request,
        forced: bool,
        sized: bool,
        abort: AbortController,
        retiring: AbortRegistration,
        scoped: Option<ibex2::host::Fetch>,
    ) {
        let refuse = |outcome| self.answer(lane, run, outcome);
        if abort.signal().aborted() {
            return refuse(failed(FailureKind::Aborted, "native request aborted"));
        }
        if let Some(why) = request.timeout_refusal().or(request.long_poll_refusal()) {
            return refuse(failed(FailureKind::Refused, why));
        }
        // Writing a body to a file is the workers' (`executor_body_to.rs`) until this core has it.
        if request.body_to.is_some() {
            let why = "exactBodyTo: not on the asynchronous executor (EXACT_HTTP_ASYNC) yet";
            return refuse(failed(FailureKind::Unsupported, why));
        }
        let Some((transport, fetch)) = &self.transports[lane] else {
            return refuse(failed(FailureKind::Refused, self.unbound.clone()));
        };
        let fetch =
            super::reach::gate_fetch(scoped.unwrap_or_else(|| fetch.clone()), request.reach);
        let transport = transport.clone();
        let limit = match request.http {
            HttpScheduling::Ordered => MAX_BODY,
            HttpScheduling::Independent { max_response_bytes } => max_response_bytes as usize,
        };
        if request.body_from.is_some() {
            let file = FileJob {
                request,
                forced,
                limit,
                abort,
                retiring,
                transport,
                fetch,
            };
            return self.file(lane, run, file, sized);
        }
        // The deadline arms when the task is resumed (D3), in `Tracked`.
        let deadline = request.timeout_ms.map(|ms| (ms, abort.clone()));
        self.fetch(
            lane, run, request, None, forced, abort, retiring, limit, transport, fetch, deadline,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn fetch(
        self: &Arc<Self>,
        lane: usize,
        run: Run,
        request: Request,
        upload: Option<Arc<Upload>>,
        forced: bool,
        abort: AbortController,
        retiring: AbortRegistration,
        limit: usize,
        transport: Arc<dyn AsyncTransport>,
        fetch: ibex2::host::Fetch,
        deadline: Option<(u32, AbortController)>,
    ) {
        let timeout = request.timeout_ms;
        let mut req = fetch_request(request, forced);
        req.max_body = Some(limit);
        // The platform's idle timeout follows the deadline a second later,
        // as on the workers.
        req.timeout = timeout.map(|ms| Duration::from_millis(u64::from(ms) + 1_000));
        let events = Arc::new(Events {
            machine: self.clone(),
            lane,
            run,
            limit,
            _retiring: Mutex::new(Some(retiring)),
        });
        let tracked = Arc::new(Tracked {
            inner: transport,
            machine: self.clone(),
            lane,
            run,
            deadline: Mutex::new(deadline),
        });
        fetch.start_async(
            tracked,
            req,
            upload,
            abort.signal(),
            HeaderCaps {
                bytes: MAX_HEADERS,
                entries: 1024,
            },
            events,
        );
    }

    // ---- the finisher ----

    fn finisher(self: Arc<Self>, lane: usize, events: mpsc::Receiver<Event>) {
        while let Ok(event) = events.recv() {
            let after = {
                let mut state = self.shared.state.lock().unwrap();
                if state.retired {
                    return;
                }
                self.apply(&mut state, lane, event)
            };
            if let Some(after) = after {
                after(&self);
            }
            // What it settled or ended may have freed the independent lane
            // room an upload waits for (LLP 1016.000 D10).
            if lane == 1 {
                super::stream::start_uploads(&self.shared);
            }
            #[cfg(test)]
            self.shared
                .worker_notifications
                .fetch_add(1, Ordering::Relaxed);
            self.shared.ready.notify_all();
        }
    }

    /// Apply one event under the lock; anything to run after it, unlocked.
    fn apply(self: &Arc<Self>, state: &mut State, lane: usize, event: Event) -> Option<After> {
        match event {
            Event::Outcome(run, done) => {
                // Only the claimant settles: a fetch's outcome claims when it
                // is posted, as the timer and a forget do.
                let claimant = match &done {
                    Done::Timeout(_) => Claim::Timeout,
                    Done::Cancelled => Claim::Cancelled,
                    Done::Fetch(..) | Done::Now(_) => Claim::Answered,
                };
                if !find(&mut state.running, run)
                    .is_some_and(|r| !r.terminal && r.claim == claimant)
                {
                    state.book.trash.push(Box::new(done));
                    return None;
                }
                let outcome = match done {
                    Done::Now(outcome) => outcome,
                    Done::Timeout(ms) => failed(
                        FailureKind::Timeout,
                        format!("the request timed out after {ms} ms"),
                    ),
                    Done::Cancelled => failed(FailureKind::Aborted, "native request aborted"),
                    Done::Fetch(result, limit) => {
                        classify(result, limit, self.shared.abort.signal().aborted())
                    }
                };
                Some(self.settled(state, run, outcome))
            }
            Event::Ended(run, origin, sent) => {
                if let Some(origin) = origin {
                    unreserve(&mut state.book, lane, Some(&origin));
                }
                if let Some(u) = state.book.uploads.get_mut(&run).filter(|_| sent) {
                    u.hops -= 1;
                }
                if let Some(r) = find(&mut state.running, run) {
                    r.live -= 1;
                }
                physical(state, run);
                None
            }
            Event::Released(run) => {
                if let Some(u) = state.book.uploads.get_mut(&run) {
                    u.buffer = false;
                }
                release_upload(state, run);
                None
            }
            Event::Protocol(origin, protocol) => {
                if matches!(protocol.as_str(), "h2" | "h3") {
                    state.book.multiplexed[lane].insert(origin);
                } else {
                    state.book.multiplexed[lane].remove(&origin);
                }
                None
            }
            Event::Sized(run, size) => self.sized(state, lane, run, size),
            Event::Read(run, read) => self.read(state, lane, run, read),
            Event::Hop(run, start) => {
                let open = find(&mut state.running, run)
                    .is_some_and(|r| !r.terminal && r.claim == Claim::None);
                if !open {
                    state.book.trash.push(Box::new(start));
                    return None;
                }
                state.book.hops[lane].push_back(Hop {
                    run,
                    origin: None,
                    start: Box::new(move |_| start()),
                });
                None
            }
        }
    }

    /// Settle `run`, its claimant's `outcome`; its deadline goes after the
    /// lock.
    fn settled(&self, state: &mut State, run: Run, outcome: Outcome) -> After {
        settle(state, run, outcome);
        if let Some(decider) = state.book.deciders.remove(&run) {
            let _ = decider.send(Decision::Exit);
        }
        Box::new(move |m: &Arc<Machine>| m.timer.remove(run))
    }

    /// A decision the finisher makes itself (a sizing or reading failure,
    /// a refusal): it claims the transition, if nothing has, and settles it.
    fn settle(&self, state: &mut State, run: Run, outcome: Outcome) -> After {
        if claim(state, run, Claim::Answered) {
            settle(state, run, outcome);
        } else {
            state.book.trash.push(Box::new(outcome));
        }
        if let Some(decider) = state.book.deciders.remove(&run) {
            let _ = decider.send(Decision::Exit);
        }
        Box::new(move |m: &Arc<Machine>| m.timer.remove(run))
    }

    // ---- the timer and the disposer ----

    fn timer(self: Arc<Self>) {
        while let Some(entry) = self.timer.next() {
            // The claim is made under the lock: from here only the timer's
            // outcome settles the run, whatever is already queued.
            let claimed = {
                let mut state = self.shared.state.lock().unwrap();
                claim(&mut state, entry.run, Claim::Timeout)
            };
            if !claimed {
                continue;
            }
            self.post(
                entry.lane,
                Event::Outcome(entry.run, Done::Timeout(entry.ms)),
            );
            entry.abort.abort();
        }
    }

    fn disposer(self: Arc<Self>) {
        let mut state = self.shared.state.lock().unwrap();
        loop {
            if state.retired {
                let discarded = std::mem::take(&mut state.discard);
                let abandoned = std::mem::take(&mut state.jobs);
                let fenced = std::mem::take(&mut state.fenced);
                let hops = std::mem::take(&mut state.book.hops);
                let files = std::mem::take(&mut state.book.files);
                let trash = std::mem::take(&mut state.book.trash);
                state.book.deciders.clear();
                drop(state);
                drop((discarded, abandoned, fenced, hops, files, trash));
                return;
            }
            if !state.discard.is_empty() || !state.book.trash.is_empty() {
                let discarded = std::mem::take(&mut state.discard);
                let trash = std::mem::take(&mut state.book.trash);
                drop(state);
                drop((discarded, trash));
                state = self.shared.state.lock().unwrap();
                continue;
            }
            state = self.shared.ready.wait(state).unwrap();
        }
    }
}

/// Whether `Core::start_switched` (Apple's executor) and a test core run
/// the asynchronous machine (LLP 1120): off until the gate (§5) decides;
/// `EXACT_HTTP_ASYNC=1` at build or run turns it on. Linux and the render
/// host keep their workers until stage 2.
pub(super) fn asynchronous() -> bool {
    option_env!("EXACT_HTTP_ASYNC") == Some("1")
        || std::env::var("EXACT_HTTP_ASYNC").is_ok_and(|v| v == "1")
}

/// Each lane's transport: Apple's session per lane, or, in tests, the
/// owner's blocking transport on bounded, reserved workers.
pub(super) fn asynch_lanes() -> Lanes {
    #[cfg(all(target_vendor = "apple", not(test)))]
    {
        Arc::new(|lane, _, _| {
            let per_host = if lane == 1 { IN_FLIGHT } else { 0 };
            (
                Arc::new(ibex2::transport::darwin_async::DarwinLane::new(per_host)),
                false,
            )
        })
    }
    #[cfg(any(not(target_vendor = "apple"), test))]
    {
        Arc::new(|lane, bindings: &ibex2::host::Bindings, budget| {
            let threads = if lane == 0 { 1 } else { super::INDEPENDENT };
            let workers = Arc::new(workers::Workers::new(threads, budget));
            (Arc::new(bindings.fetch.blocking_adapter(workers)), true)
        })
    }
}

/// What a job is to the starter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A module's handoff (`native.later`, a continuation): called, returns.
    Handoff,
    /// Work the starter runs: a closure, storage, a continuation's own.
    Closure,
    /// An HTTP exchange on the lane's transport.
    Http,
}

impl Kind {
    fn of(job: &Job) -> Kind {
        if handoff(&job.request) && matches!(job.work, Some(OwnedWork::Later(_))) {
            Kind::Handoff
        } else if job.work.is_some()
            || job.request.storage.is_some()
            || job.request.continuation.is_some()
        {
            Kind::Closure
        } else {
            Kind::Http
        }
    }
}

/// `scheme://host:port`, the per-origin limit's key.
fn origin(url: &str) -> String {
    ibex2::stdlib::fetch::origin_of(url).map_or_else(
        |_| url.to_string(),
        |o| format!("{}://{}:{}", o.scheme, o.host, o.port),
    )
}

/// `origin` is at its HTTP/1.1 limit on `lane` (D3).
fn at_limit(state: &State, lane: usize, origin: &str) -> bool {
    !state.book.multiplexed[lane].contains(origin)
        && state.book.origins[lane].get(origin).copied().unwrap_or(0) >= PER_ORIGIN
}

/// What starting `job` charges more: response bytes, and a sized upload's
/// file. A handoff charges no response bytes, a job sizing its file only
/// the request buffers it holds (D5).
fn charge(book: &Book, lane: usize, job: &Job, kind: Kind) -> (usize, usize) {
    let sized = book.sized.get(&job.ticket).map(|n| *n as usize);
    let more = match kind {
        Kind::Handoff => 0,
        Kind::Http if job.request.body_from.is_some() && sized.is_none() => 0,
        _ => job.limit - job.charge,
    };
    (
        more,
        book.upload_bytes(lane, sized.unwrap_or(0))
            .expect("admitted upload"),
    )
}

/// Whether `job` fits the lane's free bytes now.
fn fits(state: &State, lane: usize, job: &Job, kind: Kind) -> bool {
    let (more, upload) = charge(&state.book, lane, job, kind);
    more + upload <= BYTES[lane].saturating_sub(state.bytes[lane])
}

/// Hold `run`'s next exchange's places: the in-flight one, and its
/// origin's when known (a redirect's is known when it starts).
fn hold(book: &mut Book, lane: usize, run: Run, origin: Option<String>) {
    if let Some(origin) = &origin {
        *book.origins[lane].entry(origin.clone()).or_default() += 1;
    }
    if lane == 1 {
        book.in_flight += 1;
    }
    book.reserved.insert(run, origin);
}

/// Give back one exchange's in-flight place and its origin's, if counted.
fn unreserve(book: &mut Book, lane: usize, origin: Option<&str>) {
    if let Some(origin) = origin {
        if let Some(n) = book.origins[lane].get_mut(origin) {
            *n -= 1;
            if *n == 0 {
                book.origins[lane].remove(origin);
            }
        }
    }
    if lane == 1 {
        book.in_flight -= 1;
    }
}

fn find(running: &mut [Running], run: Run) -> Option<&mut Running> {
    running
        .iter_mut()
        .find(|r| r.ticket == run.0 && r.generation == run.1)
}

/// What a closure job answers: `execute`'s non-HTTP arms.
fn closure(request: Request, work: Option<super::Work>, abort: &AbortController) -> Outcome {
    if abort.signal().aborted() {
        return failed(FailureKind::Aborted, "native request aborted");
    }
    if request.storage.is_some() {
        return failed(
            FailureKind::Unsupported,
            "storage requires an app storage adapter",
        );
    }
    match work {
        Some(work) => work(),
        None => failed(
            FailureKind::Unsupported,
            "missing or consumed native continuation",
        ),
    }
}

/// A fetch's own result as the runner's outcome, as `execute` words it.
/// Only a claimed transition is a timeout or a forget's Aborted; a fetch
/// aborted otherwise was retired.
fn classify(result: Result<Collected, HostError>, limit: usize, retired: bool) -> Outcome {
    match result {
        Ok(r) => Outcome::Response(Response {
            status: r.status,
            headers: r.headers.entries().to_vec(),
            body: r.body,
        }),
        // @ref LLP 1109 D3 — over its size limit is the host refusing it.
        Err(e) if !retired && e == ibex2::stdlib::fetch::over_limit(limit) => {
            failed(FailureKind::Refused, "HTTP response exceeds limit")
        }
        Err(e) => {
            let abort = AbortController::new();
            if retired || e == ibex2::stdlib::fetch_async::aborted() {
                abort.abort();
            }
            fetch_failure(e, &abort)
        }
    }
}

/// The terminal transition of `run` (LLP 1120 D3): its outcome is
/// published, the ordered slot released; its charge stays until its
/// physical end too (D5's later-of rule).
fn settle(state: &mut State, run: Run, outcome: Outcome) {
    let Some(r) = find(&mut state.running, run) else {
        state.book.trash.push(Box::new(outcome));
        return;
    };
    if r.terminal {
        state.book.trash.push(Box::new(outcome));
        return;
    }
    r.terminal = true;
    let (lane, limit, read, forgotten) = (r.lane, r.limit, r.read, r.forgotten);
    let elapsed = r.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    if r.blocking {
        r.blocking = false;
        state.book.blocking = false;
    }
    let outcome = bounded_outcome(outcome, limit);
    if forgotten {
        state.book.trash.push(Box::new(outcome));
        state.counts[lane] -= 1;
        find(&mut state.running, run).expect("found").drained = true;
    } else {
        state.light += usize::from(read);
        state.completed[lane].push_back(Completed {
            elapsed_ms: elapsed,
            ticket: run.0,
            outcome,
            // Charged to the running job until its physical end.
            bytes: 0,
            message: false,
            stream: false,
            light: read,
            again: false,
            reply: false,
        });
    }
    physical(state, run);
    if super::has_ready(state) || (lane == 0 && state.counts[0] == 0) {
        super::wake(state);
    }
}

/// If `run` is terminal and nothing physical runs, its charge moves to its
/// undrained outcome (independent: the running charge, as `complete`;
/// ordered: what it retains), or goes if the outcome is gone (D5).
fn physical(state: &mut State, run: Run) {
    let Some(at) = state
        .running
        .iter()
        .position(|r| (r.ticket, r.generation) == run)
    else {
        return;
    };
    let r = &state.running[at];
    if !r.terminal || r.live > 0 {
        return;
    }
    let r = state.running.swap_remove(at);
    let lane = r.lane;
    if state.book.sizing[lane] == Some(run) {
        state.book.sizing[lane] = None;
    }
    // An exchange chosen that never started gives back its places.
    if let Some(origin) = state.book.reserved.remove(&run) {
        unreserve(&mut state.book, lane, origin.as_deref());
    }
    let (gone, kept): (VecDeque<Hop>, VecDeque<Hop>) = std::mem::take(&mut state.book.hops[lane])
        .into_iter()
        .partition(|h| h.run == run);
    state.book.hops[lane] = kept;
    state
        .book
        .trash
        .extend(gone.into_iter().map(|h| Box::new(h.start) as Box<dyn Send>));
    if let Some(file) = state.book.files.remove(&run) {
        state.book.trash.push(Box::new(file));
    }
    let kept = if r.drained {
        0
    } else {
        let done = state.completed[lane]
            .iter_mut()
            .find(|d| d.ticket == run.0 && !d.message);
        match done {
            Some(done) => {
                done.bytes = if lane == 0 {
                    retained(&done.outcome) + std::mem::size_of::<Completed>()
                } else {
                    r.charge
                };
                done.bytes
            }
            None => 0,
        }
    };
    state.bytes[lane] = state.bytes[lane] + kept - r.charge;
    if let Some(u) = state.book.uploads.get_mut(&run) {
        u.ended = true;
    }
    release_upload(state, run);
}

/// A run's upload charge goes once its buffer is gone and no exchange
/// sending it is still on the wire: the later of its last release and the
/// last sending hop's physical end (D5). A buffer never made (a failed
/// read, a timeout first) goes with the run's physical end.
fn release_upload(state: &mut State, run: Run) {
    if state.book.uploads.get(&run).is_some_and(|u| {
        if u.made {
            !u.buffer && u.hops == 0
        } else {
            u.ended
        }
    }) {
        let u = state.book.uploads.remove(&run).expect("checked");
        state.bytes[u.lane] -= u.bytes;
    }
}

/// The lane's transport, counting each hop it starts: the in-flight cap,
/// the per-origin limit, the run's live count (D3, D5). Its first hop arms
/// the deadline, when the task is resumed.
struct Tracked {
    inner: Arc<dyn AsyncTransport>,
    machine: Arc<Machine>,
    lane: usize,
    run: Run,
    deadline: Mutex<Option<(u32, AbortController)>>,
}

#[path = "executor_metrics.rs"]
mod metrics_log;
#[path = "executor_async_transport.rs"]
mod tracked_transport;

/// A fetch's events, to its lane's finisher.
struct Events {
    machine: Arc<Machine>,
    lane: usize,
    run: Run,
    /// The response ceiling, which names an over-limit failure.
    limit: usize,
    /// Retirement's abort registration, held for the fetch's life.
    _retiring: Mutex<Option<AbortRegistration>>,
}

impl FetchEvents for Events {
    fn outcome(&self, result: Result<Collected, HostError>) {
        let accepted = {
            let state = &mut self.machine.shared.state.lock().unwrap();
            claim(state, self.run, Claim::Answered)
        };
        if accepted {
            self.machine.post(
                self.lane,
                Event::Outcome(self.run, Done::Fetch(result, self.limit)),
            );
        }
    }
    fn metrics(&self, url: &str, metrics: HopMetrics) {
        metrics_log::note(
            "lane",
            url,
            metrics.protocol.as_deref(),
            metrics.reused,
            metrics.local.as_deref(),
            metrics.remote.as_deref(),
            metrics.queued_ms,
        );
        if let Some(protocol) = metrics.protocol {
            self.machine
                .post(self.lane, Event::Protocol(origin(url), protocol));
        }
    }
    fn next_hop(&self, start: Box<dyn FnOnce() + Send>) {
        self.machine.post(self.lane, Event::Hop(self.run, start));
    }
}

fn body_refused(message: String) -> Outcome {
    failed(FailureKind::Refused, message)
}

/// A drained outcome whose job still runs physically: its bytes go with it.
pub(super) fn drained(state: &mut State, ticket: u64) {
    if let Some(run) = state.running.iter_mut().find(|r| r.ticket == ticket) {
        run.drained = true;
    }
}

/// Posts a reader's end when it leaves, however it leaves.
struct Ended(Arc<Machine>, usize, Run);
impl Drop for Ended {
    fn drop(&mut self) {
        self.0.post(self.1, Event::Ended(self.2, None, false));
    }
}

/// A forget cancels `run` if it is an exchange still open (a safe read's,
/// or any method's the app aborted, `by_app`; never a stream's, nor an
/// upload's waiting for its start, which its queue ends): its terminal
/// transition is claimed now (D3).
pub(super) fn cancel(run: &mut Running, by_app: bool) -> bool {
    let open = !run.terminal && run.claim == Claim::None;
    let exchange = run.abort.is_some() || (by_app && run.exchange.is_some());
    let cancels = open && exchange && !run.stream && !run.waiting;
    if cancels {
        run.claim = Claim::Cancelled;
    }
    cancels
}
