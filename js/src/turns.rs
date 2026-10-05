//! Storage turns and liveness: one storage turn at a time, as the browser's
//! worker runs them; an answer waiting on another's work is asked again
//! after each delivery (LLP 1027.003.000 §13); a call let go mid-turn runs
//! its steps to the end (ledger F12).
use super::{Module, WAITING};
use exact_runner::{Dispatch, Outcome, Response, Work};

impl Module {
    /// Whether an answer is between storage steps: parked on its storage
    /// continuation rather than on a fetch the host runs.
    pub(crate) fn turn_open(&self) -> bool {
        self.parked.iter().any(|(_, p)| p.ticket == 0)
    }

    /// A deferred answer's work: nothing to run, only a turn to wait for.
    pub(crate) fn deferred_work() -> Dispatch {
        Dispatch::Run(Work::Now(Box::new(|| {
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: Vec::new(),
            })
        })))
    }

    /// Whether anything in the module may yet settle a waiting answer: a
    /// fetch, a storage step, a stream, an answer not yet begun.
    /// The module's storage queued or in flight counts, an answer's or the
    /// background's (LLP 1097 D2: an answer may await background work).
    pub(crate) fn outstanding(&self) -> bool {
        !self.streams.is_empty()
            || self.background.state.operations > 0
            || self.parked.iter().any(|(_, p)| p.ticket != WAITING)
    }

    /// Ask a waiting answer again when something landed since it parked,
    /// or, with nothing outstanding, for its last settle, which refuses it
    /// as pending on nothing if it still waits.
    pub(crate) fn wake(&mut self, token: u64) -> Option<Dispatch> {
        let outstanding = self.outstanding();
        let progress = self.progress;
        let (_, parked) = self
            .parked
            .iter_mut()
            .find(|(_, p)| p.call == token && p.ticket == WAITING)?;
        if parked.progress < progress || !outstanding {
            parked.last = !outstanding;
            return Some(Module::deferred_work());
        }
        None
    }

    /// Parked calls let go in the prelude too, with the fetches they wait on.
    /// One let go between storage steps cannot be stopped: the steps it began
    /// are running, and the app's promise chain (a serialized database, say)
    /// goes on behind them. A browser runs that turn to its end, and so does
    /// this, now, its answer discarded: otherwise the next answer chained
    /// behind it waits on steps nobody delivers and is refused as pending on
    /// nothing (ledger F12: a pre-mutation refresh the runner discarded).
    pub(crate) fn forget_calls(&mut self, calls: Vec<u64>) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let mut owed = false;
        for call in calls {
            owed |= engine
                .call("__exact_forget", [&call.to_string(), "", ""])
                .is_ok_and(|r| r == "storage");
        }
        if owed {
            self.finish_let_go();
        }
    }

    /// Deliver the storage steps of calls let go mid-turn until none is left.
    pub(crate) fn finish_let_go(&mut self) {
        let (Some(session), Some(engine)) = (self.storage.as_ref(), self.engine.as_mut()) else {
            return;
        };
        while engine
            .call("__exact_let_go", ["", "", ""])
            .is_ok_and(|r| r == "storage")
        {
            if let Outcome::Failed { message, .. } = session.continuation()() {
                let _ = engine.call("__exact_let_go", ["failed", &message, ""]);
                break;
            }
            if engine.deliver_storage_one().is_err() || engine.drain().is_err() {
                break;
            }
        }
        // What landed may settle an answer waiting on another's work.
        self.progress += 1;
        self.refresh_background();
    }
}
