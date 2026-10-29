//! An upload's place in its executor (LLP 1016.000 D10). Its body, read
//! from its file or given inline, and the platform's copy of it are held in
//! the executor's upload budget from before its file is read until its
//! exchange ends. An upload waits for room in a queue before it takes a
//! stream's place or a thread, so a burst of attachments leaves the app's
//! other streams their places; one upload alone always fits.
use super::super::{complete, failed, Shared, StreamHost, STREAMS};
use exact_runner::{FailureKind, Message, Outcome, Request};
use ibex2::stdlib::{abort::AbortController, fetch::Sent};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// What an executor's uploads may hold at once, the platform's copies
/// counted: 256 MiB, the four 64 MiB file bodies reads may hold at once
/// (`executor_body.rs` `MAX_READERS`). An upload's own lane, 64 MiB, could
/// not hold a 50 MB body beside Apple's copy of it.
const UPLOAD_BYTES: usize = if cfg!(test) {
    1 << 20
} else {
    super::super::body::MAX_READERS * exact_runner::MAX_BODY_FROM_BYTES as usize
};

/// The copies of an upload's body in memory while it goes out: the
/// stream's, and on Apple URLSession's `NSData`. The portable transport
/// sends from the stream's.
pub(in super::super) const COPIES: usize = if cfg!(target_vendor = "apple") { 2 } else { 1 };

/// The uploads of one executor: the bytes those going out hold, and those
/// waiting for room, in the order they were admitted.
#[derive(Default)]
pub(in super::super) struct Uploads(Mutex<Budget>);
#[derive(Default)]
struct Budget {
    bytes: usize,
    queue: VecDeque<Queued>,
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

/// An upload's place in the budget, given back when dropped, which may
/// start the next one waiting.
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
        self.shared.uploads.0.lock().unwrap().bytes -= self.bytes;
        start(&self.shared);
    }
}

impl Uploads {
    /// Uploads waiting for room now, and the bytes those going out hold.
    #[cfg(test)]
    pub(in super::super) fn waiting(&self) -> (usize, usize) {
        let budget = self.0.lock().unwrap();
        (budget.queue.len(), budget.bytes)
    }
}

/// Admit upload `ticket` to wait for room. Its running entry, which the
/// core made, holds no stream's place until it starts.
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

/// Start each upload waiting at the head that has room in the budget and a
/// stream's place: that place and a thread are its from now on.
pub(in super::super) fn start(shared: &Arc<Shared>) {
    let mut starting = Vec::new();
    {
        let mut state = shared.state.lock().unwrap();
        let mut budget = shared.uploads.0.lock().unwrap();
        while let Some(next) = budget.queue.front() {
            let room = budget.bytes == 0 || budget.bytes.saturating_add(next.need) <= UPLOAD_BYTES;
            if state.retired || !room || state.streams >= STREAMS {
                break;
            }
            let next = budget.queue.pop_front().expect("a head");
            let Some(run) = state.running.iter_mut().find(|r| r.ticket == next.ticket) else {
                continue;
            };
            run.stream = true;
            state.streams += 1;
            budget.bytes += next.need;
            starting.push(next);
        }
        if state.retired {
            budget.queue.clear();
        }
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

/// A completion was drained (`place`: a stream's, whose place is free now):
/// with the core unlocked, an upload waiting may take the place.
pub(in super::super) fn freed(
    shared: &Arc<Shared>,
    state: std::sync::MutexGuard<'_, super::super::State>,
    place: bool,
) {
    drop(state);
    if place {
        start(shared);
    }
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

/// An upload's progress: each count of the body's bytes sent that the
/// transport reports is a message, which a newer one replaces while the
/// runner has not taken it (D4).
pub(super) fn progress(shared: &Arc<Shared>, ticket: u64) -> Sent {
    let shared = shared.clone();
    Sent(Arc::new(move |loaded, total| {
        super::message(&shared, ticket, Message::upload_progress(loaded, total));
    }))
}
