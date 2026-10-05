//! Storage turns and liveness: one storage turn at a time, as the browser's
//! worker runs them; an answer waiting on another's work is asked again
//! after each delivery (LLP 1027.003.000 §13); a call let go mid-turn runs
//! its steps to the end (ledger F12).
use super::{Key, Module, DEFERRED, WAITING};
use exact_runner::{
    Answer, DataError, Dispatch, InFlight, Outcome, Response, Store, Target, Value, Work,
};
use std::collections::HashMap;

pub(crate) struct Retired {
    token: u64,
    source: String,
    args: Vec<Value>,
    store: Store,
}

impl Module {
    pub(crate) fn defer_turn(&self, source: &str) -> bool {
        self.sigs.contains_key(source)
            && (self.turn_open()
                || (!self.draining_retired
                    && self.parked.iter().any(|(_, p)| p.ticket == DEFERRED)))
    }

    pub(crate) fn begin_deferred(
        &mut self,
        store: &mut Store,
        target: Option<Target>,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        // The token just removed was the oldest live turn. It must start
        // before a newer retired one, which begin would otherwise drain.
        let draining = self.draining_retired;
        self.draining_retired = true;
        let result = self.begin(Some(store), target, source, args);
        self.draining_retired = draining;
        result
    }

    /// Finish forgotten storage in a throwaway, grant-scoped Store. The
    /// external effects survive; the discarded reply's Store writes do not.
    pub(crate) fn retire_calls(&mut self, store: &Store, in_flight: &[InFlight<'_>]) {
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
            .partition(|(key, p)| {
                key.0.is_some()
                    && !matches!(keep.get(key), Some(None))
                    && keep.get(key) != Some(&Some(p.call))
            });
        self.parked = kept;
        let (ended, open): (Vec<_>, Vec<_>) = std::mem::take(&mut self.streams)
            .into_iter()
            .partition(|(key, _)| key.0.is_some() && !keep.contains_key(key));
        self.streams = open;
        let mut calls = Vec::new();
        for ((_, source, _), p) in gone.into_iter().chain(ended) {
            if p.ticket == DEFERRED {
                if let Some(args) = p.deferred_args {
                    self.retired.push_back(Retired {
                        token: p.call,
                        source,
                        args,
                        store: store.clone(),
                    });
                }
            } else {
                calls.push(p.call);
            }
        }
        self.retired.make_contiguous().sort_by_key(|r| r.token);
        self.forget_calls(calls);
        // A discard can have marked a call even if this sweep found no new one.
        let owed = self.engine.as_mut().is_some_and(|engine| {
            engine
                .call("__exact_let_go", ["", "", ""])
                .is_ok_and(|r| r == "storage")
        });
        if owed {
            let mut local = store.clone();
            let grants = self.grants.clone();
            local.with_grants(&grants, |store| {
                self.host.store = Some(store as *mut Store);
                self.finish_let_go();
                self.host.store = None;
            });
        }
        self.finish_retired();
    }

    /// A sent mutation is queued work even if a newer send wants its reply
    /// slot. Execute it before the next turn; queries remain replaceable.
    pub(crate) fn finish_retired(&mut self) {
        if self.draining_retired || self.turn_open() {
            return;
        }
        self.draining_retired = true;
        while !self.turn_open() {
            if self.retired.front().is_some_and(|r| {
                self.parked
                    .iter()
                    .any(|(_, p)| p.ticket == DEFERRED && p.call < r.token)
            }) {
                break;
            }
            let Some(mut retired) = self.retired.pop_front() else {
                break;
            };
            let grants = self.grants.clone();
            retired.store.with_grants(&grants, |store| {
                if let Err(error) = self.begin(Some(store), None, &retired.source, &retired.args) {
                    self.logs
                        .push(format!("forgotten mutation {}: {error:?}", retired.source));
                }
                let key = Module::key(None, &retired.source, &retired.args);
                let tokens: Vec<_> = self
                    .parked
                    .iter()
                    .chain(&self.streams)
                    .filter(|(k, _)| *k == key)
                    .map(|(_, p)| p.call)
                    .collect();
                // This retired reply has no recipient. Already-started storage
                // finishes; a fetch retains the existing cancellation policy.
                self.parked.retain(|(k, _)| *k != key);
                self.streams.retain(|(k, _)| *k != key);
                self.forget_calls(tokens);
                self.host.store = Some(store as *mut Store);
                self.finish_let_go();
                self.host.store = None;
            });
        }
        self.draining_retired = false;
    }

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
    pub(crate) fn outstanding(&self) -> bool {
        !self.streams.is_empty() || self.parked.iter().any(|(_, p)| p.ticket != WAITING)
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
        for call in calls {
            let _ = engine.call("__exact_forget", [&call.to_string(), "", ""]);
        }
        // Drain on the next answer/reply, with its Store installed. Draining
        // here runs queued steps outside that context and refuses them as bake.
    }

    /// Deliver the storage steps of calls let go mid-turn until none is left.
    pub(crate) fn finish_let_go(&mut self) {
        if self.host.store.is_none() {
            return;
        }
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
    }
}
