//! Liveness: an answer waiting on another's work, or on the background's,
//! is asked again after each delivery (LLP 1027.003.000 §13; LLP 1097 D2);
//! a call let go mid-turn runs its steps to the end (ledger F12).
use super::{Key, Module, WAITING};
use exact_runner::{Dispatch, InFlight, Outcome, Response, Work};
use std::collections::HashMap;

impl Module {
    /// A waiting answer's work: nothing to run, only an answer to ask again.
    pub(crate) fn ask_again() -> Dispatch {
        Dispatch::Run(Work::Now(Box::new(|| {
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: Vec::new(),
            })
        })))
    }

    /// Whether anything in the module may yet settle a waiting answer: a
    /// fetch, a storage step, a stream. The module's storage queued or in flight counts, an answer's or the
    /// background's (LLP 1097 D2: an answer may await background work).
    pub(crate) fn outstanding(&self) -> bool {
        !self.streams.is_empty()
            || self.background.state.queued + self.background.state.in_flight > 0
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
            return Some(Module::ask_again());
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
        let (mut owed, mut rejected) = (false, false);
        for call in calls {
            match engine
                .call("__exact_forget", [&call.to_string(), "", ""])
                .as_deref()
            {
                Ok("storage") => owed = true,
                Ok("rejected") => rejected = true,
                _ => {}
            }
        }
        // The rejected fetches' continuations run now, their answers
        // discarded: what they set (a busy flag) is cleared, never stranded.
        // Between answers, as a let-go's steps run: storage they start is
        // the background's, not refused as at bake.
        if rejected {
            self.host.between_answers = true;
            let drained = engine.drain();
            self.host.between_answers = false;
            // A continuation another answer shared may have settled it: a
            // waiter counts this as progress (`wake`).
            self.progress += 1;
            // An interrupted drain is over; its flag is not the next turn's.
            if drained.is_err() {
                self.watch.take();
            }
        }
        if owed {
            self.finish_let_go();
        }
    }

    /// Drop targeted calls the runner no longer has in flight. Storage a
    /// call already issued still runs ([`Module::forget_calls`]). LLP 1097
    /// deletes the answer-to-answer deferral, so a call that had not started
    /// is not replayed later in a cloned store.
    pub(crate) fn forget_in_flight(&mut self, in_flight: &[InFlight<'_>]) {
        let keep: HashMap<Key, Option<u64>> = in_flight
            .iter()
            .map(|f| {
                (
                    Module::key(Some(f.target), f.source, f.args),
                    f.continuation,
                )
            })
            .collect();
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.parked)
            .into_iter()
            .partition(|(key, parked)| {
                key.0.is_some()
                    && !matches!(keep.get(key), Some(None))
                    && keep.get(key) != Some(&Some(parked.call))
            });
        self.parked = kept;
        let (ended, open): (Vec<_>, Vec<_>) = std::mem::take(&mut self.streams)
            .into_iter()
            .partition(|(key, _)| key.0.is_some() && !keep.contains_key(key));
        self.streams = open;
        self.forget_calls(
            gone.into_iter()
                .chain(ended)
                .map(|(_, parked)| parked.call)
                .collect(),
        );
    }

    /// Deliver the storage steps of calls let go mid-turn until none is left.
    /// There is no store: a let-go answer's `store.set` does not land on the
    /// live answer's store, and storage is not refused as at bake
    /// (`HostState::between_answers`).
    pub(crate) fn finish_let_go(&mut self) {
        let (Some(session), Some(engine)) = (self.storage.as_ref(), self.engine.as_mut()) else {
            return;
        };
        // Each step the chain begins after the first must reach storage too:
        // a refusal there ends the chain with its write unmade (splitter
        // rough 7). `between_answers` is that window: no store, and storage
        // is not refused as at bake.
        self.host.between_answers = true;
        let mut delivered = false;
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
            delivered = true;
        }
        self.host.between_answers = false;
        if !delivered {
            return;
        }
        // What landed may settle an answer waiting on another's work.
        self.progress += 1;
        self.refresh_background();
    }
}
