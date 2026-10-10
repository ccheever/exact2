//! Liveness: an answer waiting on another's work, or on the background's,
//! is asked again after each delivery (LLP 1027.003.000 §13; LLP 1097 D2);
//! a call let go mid-turn runs its steps to the end (ledger F12).
use super::{Key, Module, WAITING};
use exact_runner::{Dispatch, InFlight, Outcome};
use std::collections::HashMap;

impl Module {
    /// A waiting answer's work: nothing to run, only an answer to ask again
    /// (`Dispatch::Again`, LLP 1041 §8.4 amended 2026-10-09). A host
    /// settles it in its ordered place without executor work.
    pub(crate) fn ask_again() -> Dispatch {
        Dispatch::Again
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
        for call in &calls {
            // Its waiter, if one is still running, now gives up without
            // taking any compression's right (LLP 1069.002 A1.5).
            if let Some(retired) = self.retired.remove(call) {
                retired.store(true, std::sync::atomic::Ordering::Release);
            }
        }
        for call in calls {
            if let Ok(said) = engine.call("__exact_forget", [&call.to_string(), "", ""]) {
                owed |= said.starts_with("storage");
                rejected |= said.ends_with("rejected");
            }
        }
        // The rejected fetches' continuations run now, their answers
        // discarded, before any answer is current: what they set (a busy
        // flag) is cleared, never stranded, and never lands on a live
        // answer's store. Between answers, as a let-go's steps run: storage
        // they start is the background's, not refused as at bake.
        if rejected {
            self.host.between_answers = true;
            // An interrupt (or a job that throws) ends the job it stopped;
            // the jobs queued behind it still run here, not in the next
            // answer's drain. Each try pops at least the job it stopped; the
            // bound only ends an endless chain of failing jobs.
            let mut drained = engine.drain();
            for _ in 0..1024 {
                if drained.is_ok() {
                    break;
                }
                self.watch.take();
                drained = engine.drain();
            }
            self.host.between_answers = false;
            if drained.is_ok() {
                // A continuation another answer shared may have settled it:
                // a waiter counts this as progress (`wake`).
                self.progress += 1;
            } else {
                self.watch.take();
            }
            // Storage a continuation started is the background's: arm it.
            self.refresh_background();
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
                // A compression so failed is settled and what was queued
                // behind it issued (LLP 1069.002 A1.5): a progress, and the
                // chain's other storage is delivered on.
                if engine
                    .call("__exact_let_go", ["failed", &message, ""])
                    .is_ok_and(|r| r == "settled")
                    && engine.drain().is_ok()
                {
                    delivered = true;
                    continue;
                }
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
