//! The data module's own work, run for it (LLP 1016 D2, LLP 1027.002 D3).
//!
//! A source that answers `Later` with a continuation (the LLP reader reading
//! its corpus) hands the host work to run off the runner's thread. After
//! every commit the host takes the requests out, asks the runner for each
//! one's work on this thread, and runs it on a thread of its own; the
//! outcome comes back through the same wake the module's announcements use,
//! and [`Host::announced`] fulfills it — one commit.
//!
//! The terminal host has no network executor: a request that is not the
//! module's own work (a `fetch`, a surface's) is refused with a reason.

use crate::host::Host;
use exact_runner::{DataSource, Dispatch, Outcome, Reply, RequestOut, Work};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

type Waker = Arc<dyn Fn() + Send + Sync>;

/// Work in flight and what came back.
pub(crate) struct Requests {
    done: Sender<(u64, Outcome)>,
    landed: Receiver<(u64, Outcome)>,
    /// Requests whose work the source holds until a later commit.
    parked: HashMap<u64, RequestOut>,
    /// The loop's wake, once it listens; work started before that reads it
    /// when it finishes.
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Default for Requests {
    fn default() -> Self {
        let (done, landed) = channel();
        Requests {
            done,
            landed,
            parked: HashMap::new(),
            waker: Arc::new(Mutex::new(None)),
        }
    }
}

impl Requests {
    pub(crate) fn listen(&mut self, waker: Waker) {
        *self.waker.lock().expect("waker") = Some(waker);
    }

    /// Where an outcome goes: the channel, then the loop is woken.
    fn reply(&self, ticket: u64) -> impl FnOnce(Outcome) + Send + 'static {
        let (done, waker) = (self.done.clone(), self.waker.clone());
        move |outcome| {
            let _ = done.send((ticket, outcome));
            if let Some(wake) = waker.lock().ok().and_then(|w| w.clone()) {
                wake();
            }
        }
    }
}

impl<D: DataSource> Host<D> {
    /// Run what the last commit asked for, and what it released.
    pub(crate) fn run_requests(&mut self) {
        for r in self.runner.take_requests() {
            let dispatch = match r.request.continuation {
                Some(token) => self.runner.dispatch_work(token),
                None => Dispatch::Missing,
            };
            self.run_dispatch(r, dispatch);
        }
        for (token, dispatch) in self.runner.release_work() {
            if let Some(r) = self.requests.parked.remove(&token) {
                self.run_dispatch(r, dispatch);
            }
        }
    }

    fn run_dispatch(&mut self, r: RequestOut, dispatch: Dispatch) {
        let reply = self.requests.reply(r.ticket);
        match dispatch {
            Dispatch::Run(Work::Now(work)) => {
                std::thread::spawn(move || reply(work()));
            }
            Dispatch::Run(Work::Later(work)) => work(Reply::new(reply)),
            // A re-ask: no work, and no admission bound here to wait on.
            Dispatch::Again => reply(Dispatch::again_outcome()),
            Dispatch::Held => {
                if let Some(token) = r.request.continuation {
                    self.requests.parked.insert(token, r);
                }
            }
            Dispatch::Host(_) | Dispatch::Missing => {
                let ordered = r.request.is_ordered();
                self.runner.refuse_request(
                    r.ticket,
                    "the terminal host runs only its data module's own work",
                    ordered,
                );
            }
        }
    }

    /// Fulfill every outcome that has come back; whether any committed.
    pub(crate) fn land_requests(&mut self) -> bool {
        let mut committed = false;
        while let Ok((ticket, outcome)) = self.requests.landed.try_recv() {
            committed |= matches!(self.runner.fulfill(ticket, outcome), Ok(Some(_)));
        }
        committed
    }
}
