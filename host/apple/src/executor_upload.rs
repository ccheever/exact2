//! An upload's place in its executor (LLP 1016.000 D10). Its body, read
//! from its file or given inline, and the platform's copy of it are held in
//! the executor's upload budget from before its file is read until its
//! exchange ends. An upload waits in a queue, holding no stream's place,
//! thread, count or bytes of its lane, until the budget has room, fewer
//! than [`RUNNING`] uploads go out and its lane can take its buffers and
//! its reply's ceiling; then it takes them all. So a burst of attachments
//! leaves the app's other streams their places and its other requests
//! their room; one upload alone always fits. The executor records each
//! upload that has begun to go out, for the runner to read when it lets
//! one go.
use super::super::{complete, failed, Core, Shared, StreamHost, BYTES, STREAMS};
use exact_runner::{FailureKind, Message, Outcome, Request};
use ibex2::stdlib::{abort::AbortController, fetch::Sent};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

/// What an executor's uploads may hold at once, the platform's copies
/// counted: 256 MiB, the four 64 MiB file bodies reads may hold at once
/// (`executor_body.rs` `MAX_READERS`). An upload's own lane, 64 MiB, could
/// not hold a 50 MB body beside Apple's copy of it.
const UPLOAD_BYTES: usize =
    super::super::body::MAX_READERS * exact_runner::MAX_BODY_FROM_BYTES as usize;

/// The most uploads an executor sends at once, as upstream T3 Code sends
/// three per environment: the 16 streams' other places stay the app's.
const RUNNING: usize = 3;

/// The copies of an upload's body in memory while it goes out: the
/// stream's, and on Apple URLSession's `NSData`. The portable transport
/// sends from the stream's.
pub(in super::super) const COPIES: usize = if cfg!(target_vendor = "apple") { 2 } else { 1 };

/// The uploads of one executor: the bytes those going out hold, how many go
/// out, those waiting, in the order they were admitted, and those that have
/// begun to go out, until their end is drained or the runner reads them.
pub(in super::super) struct Uploads(Mutex<Budget>);
struct Budget {
    bytes: usize,
    limit: usize,
    running: usize,
    queue: VecDeque<Queued>,
    started: HashSet<u64>,
}
impl Default for Uploads {
    fn default() -> Self {
        Uploads(Mutex::new(Budget {
            bytes: 0,
            limit: UPLOAD_BYTES,
            running: 0,
            queue: VecDeque::new(),
            started: HashSet::new(),
        }))
    }
}

/// An upload admitted and waiting for room: everything its thread needs.
struct Queued {
    ticket: u64,
    request: Request,
    forced: bool,
    grants: String,
    host: StreamHost,
    abort: AbortController,
    /// Its room: its body's length, or for a file not read yet the most a
    /// file body may be, which [`Held::keep`] lowers once it is read.
    need: usize,
}

/// An upload's place in the budget and among those going out, given back
/// when dropped, which may start the next one waiting.
pub(in super::super) struct Held {
    shared: Arc<Shared>,
    bytes: usize,
}
impl Held {
    /// Keep only `bytes` of the place, once the body's size is known.
    pub(super) fn keep(&mut self, bytes: usize) {
        if bytes < self.bytes {
            self.shared.uploads.0.lock().unwrap().bytes -= self.bytes - bytes;
            self.bytes = bytes;
            start(&self.shared);
        }
    }
}
impl Drop for Held {
    fn drop(&mut self) {
        {
            let mut budget = self.shared.uploads.0.lock().unwrap();
            budget.bytes -= self.bytes;
            budget.running -= 1;
        }
        start(&self.shared);
    }
}

impl Uploads {
    /// Uploads waiting now, and the bytes those going out hold.
    #[cfg(test)]
    pub(in super::super) fn waiting(&self) -> (usize, usize) {
        let budget = self.0.lock().unwrap();
        (budget.queue.len(), budget.bytes)
    }

    /// A smaller budget, for a test's small bodies.
    #[cfg(test)]
    pub(in super::super) fn limit(&self, bytes: usize) {
        self.0.lock().unwrap().limit = bytes;
    }
}

impl Core {
    /// The uploads let go (no longer `held`) that had begun to go out,
    /// each told once, for the host to hand the runner after it let their
    /// tickets go (`Runner::uploads_started`, LLP 1016.000 D10).
    pub(crate) fn let_go_started(&self, held: impl Fn(u64) -> bool) -> Vec<u64> {
        let mut budget = self.shared.uploads.0.lock().unwrap();
        let gone: Vec<u64> = budget
            .started
            .iter()
            .copied()
            .filter(|t| !held(*t))
            .collect();
        for ticket in &gone {
            budget.started.remove(ticket);
        }
        gone
    }
}

/// Admit upload `ticket` to wait for room. Its running entry, which the
/// core made, holds no stream's place, count or bytes until it starts.
pub(in super::super) fn queue(
    shared: &Arc<Shared>,
    ticket: u64,
    request: Request,
    forced: bool,
    grants: String,
    host: StreamHost,
    abort: AbortController,
) {
    let body = match request.body_from {
        Some(_) => exact_runner::MAX_BODY_FROM_BYTES as usize,
        None => request.body.len(),
    };
    let queued = Queued {
        ticket,
        request,
        forced,
        grants,
        host,
        abort,
        need: body.saturating_mul(COPIES),
    };
    shared.uploads.0.lock().unwrap().queue.push_back(queued);
    start(shared);
}

/// Start each upload waiting at the head that has room in the budget, a
/// stream's place, one of the [`RUNNING`], and room in its lane for its
/// buffers and its reply's ceiling (`Running::limit`, as the core charges
/// a stream at admission): that place, a thread, a count and those bytes
/// are its from now on. One the runner has let go meanwhile ends unsent.
pub(in super::super) fn start(shared: &Arc<Shared>) {
    let (mut starting, mut gone) = (Vec::new(), Vec::new());
    {
        let mut guard = shared.state.lock().unwrap();
        let state = &mut *guard;
        let mut budget = shared.uploads.0.lock().unwrap();
        while let Some(next) = budget.queue.front() {
            let room = budget.bytes == 0 || budget.bytes.saturating_add(next.need) <= budget.limit;
            if state.retired || !room || budget.running >= RUNNING || state.streams >= STREAMS {
                break;
            }
            let Some(run) = state.running.iter_mut().find(|r| r.ticket == next.ticket) else {
                budget.queue.pop_front();
                continue;
            };
            if run.forgotten {
                gone.push(next.ticket);
                budget.queue.pop_front();
                continue;
            }
            let (lane, limit) = (run.lane, run.limit);
            let parked = state.book.parked(lane, &state.jobs[lane]);
            if limit > BYTES[lane].saturating_sub(state.bytes[lane] + parked) {
                break;
            }
            let next = budget.queue.pop_front().expect("a head");
            (run.stream, run.waiting, run.charge) = (true, false, limit);
            state.counts[lane] += 1;
            state.bytes[lane] += limit;
            state.streams += 1;
            budget.bytes += next.need;
            budget.running += 1;
            starting.push(next);
        }
        if state.retired {
            budget.queue.clear();
        }
    }
    for ticket in gone {
        complete(
            shared,
            ticket,
            failed(FailureKind::Aborted, "native request aborted"),
        );
    }
    for q in starting {
        let held = Held {
            shared: shared.clone(),
            bytes: q.need,
        };
        let ticket = q.ticket;
        let spawned = std::thread::Builder::new()
            .name(format!("exact-stream-{ticket}"))
            .spawn({
                let shared = shared.clone();
                move || {
                    let Queued {
                        ticket,
                        request,
                        forced,
                        grants,
                        host,
                        abort,
                        ..
                    } = q;
                    let held = Some(held);
                    super::opened(
                        &shared, ticket, request, forced, &grants, &*host, abort, held,
                    );
                    start(&shared);
                }
            });
        if let Err(e) = spawned {
            complete(shared, ticket, failed(FailureKind::Network, e.to_string()));
        }
    }
}

/// A completion was drained (`room`: the end of a request of the
/// independent lane, whose bytes, and a stream's place if it was one, are
/// free now): an upload's record of its start goes with it, and with the
/// core unlocked an upload waiting may take the room.
pub(in super::super) fn freed(
    shared: &Arc<Shared>,
    state: std::sync::MutexGuard<'_, super::super::State>,
    ticket: u64,
    room: bool,
) {
    drop(state);
    if room {
        shared.uploads.0.lock().unwrap().started.remove(&ticket);
        start(shared);
    }
}

/// Whether an upload waits in the queue.
pub(in super::super) fn queued(shared: &Shared) -> bool {
    !shared.uploads.0.lock().unwrap().queue.is_empty()
}

/// Let go of the uploads waiting whose tickets the runner no longer
/// `held`: each ends unsent, as aborted, which gives back its admission;
/// the places that frees may start others.
pub(in super::super) fn forget(shared: &Arc<Shared>, held: &dyn Fn(u64) -> bool) {
    let gone: Vec<u64> = {
        let mut budget = shared.uploads.0.lock().unwrap();
        let (kept, gone): (VecDeque<Queued>, VecDeque<Queued>) = std::mem::take(&mut budget.queue)
            .into_iter()
            .partition(|q| held(q.ticket));
        budget.queue = kept;
        gone.into_iter().map(|q| q.ticket).collect()
    };
    for ticket in gone {
        complete(
            shared,
            ticket,
            failed(FailureKind::Aborted, "native request aborted"),
        );
    }
    start(shared);
}

/// An upload's end once its deadline may have passed: a failure after the
/// deadline is the deadline's; a reply that landed as it passed stands, as a
/// plain request's does (`execute`).
pub(super) fn timed_out(end: Outcome, passed: bool, ms: u32) -> Outcome {
    match end {
        Outcome::Failed { .. } if passed => failed(
            FailureKind::Timeout,
            format!("the request timed out after {ms} ms"),
        ),
        end => end,
    }
}

/// An upload's hooks: each count of the body's bytes sent that the
/// transport reports is a message, which a newer one replaces while the
/// runner has not taken it (D4); its start, when the hop loop has admitted
/// it (`exact:sending`), is recorded here and is a message no other
/// replaces.
pub(super) fn progress(shared: &Arc<Shared>, ticket: u64) -> Sent {
    let (sending, shared) = (shared.clone(), shared.clone());
    Sent(
        Arc::new(move |loaded, total| {
            super::message(&shared, ticket, Message::upload_progress(loaded, total));
        }),
        Some(Arc::new(move || {
            sending.uploads.0.lock().unwrap().started.insert(ticket);
            super::message(&sending, ticket, Message::upload_start());
        })),
    )
}
