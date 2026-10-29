//! Work that outlives its answer (LLP 1146 D2). A let-go call whose awaited
//! fetch has effects is set aside until the commit that let it go ends
//! (`set_aside`); the runner then names the requests it keeps
//! (`keep_sent_now`), the prelude adopts each one's fetch (`__exact_adopt`)
//! and forgets the call, and the fetch's reply reaches the let-go call's
//! continuation in a module turn: the store installed and no checkpoint
//! open, so what the turn writes stands on its own. A refused commit puts
//! such a call back instead, when an equal one had replaced it.
use super::{watch::Watched, wire, DataError, Key, Module, Parked};
use exact_runner::{InFlight, Outcome, Store};
use std::collections::HashMap;

/// Let-go calls set aside for the runner's `keep_sent`, and what it last said
/// is in flight.
#[derive(Default)]
pub(crate) struct Letting {
    /// Let-go calls whose awaited fetch has effects, set aside until the
    /// commit that let them go ends: the runner then names the ones it keeps,
    /// and the rest are forgotten.
    calls: Vec<(Key, Parked)>,
    /// What the runner said is in flight at its last `forgotten`, by key: a
    /// continuation's token, or `None` for a plain request.
    pub(crate) flight: HashMap<Key, Option<u64>>,
}

/// Start call `call`'s turn: the steps module work left for it run first
/// (LLP 1146 §2.3).
pub(crate) fn begin_turn(engine: &mut Watched, call: u64, source: &str) -> Result<(), DataError> {
    (engine
        .call("__exact_turn", [&call.to_string(), "", ""])
        .map(|_| ()))
    .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))
}

impl Module {
    /// The calls a new call on `key` replaces, to forget now: those parked on
    /// it that are not set aside (`set_aside`), and its streams.
    pub(crate) fn replace_on(&mut self, key: &Key) -> Vec<u64> {
        let (gone, kept): (Vec<_>, Vec<_>) =
            (std::mem::take(&mut self.parked).into_iter()).partition(|(k, _)| k == key);
        self.parked = kept;
        let mut replaced = self.set_aside(gone);
        replaced.extend(
            (self.streams.iter())
                .filter(|(k, _)| k == key)
                .map(|(_, p)| p.call),
        );
        self.streams.retain(|(k, _)| k != key);
        replaced
    }

    /// Calls let go now: one whose awaited fetch has effects is set aside for
    /// `keep_sent_now`, and the others' calls are returned, to forget now. A
    /// worker's instance sets none aside: no module turn reaches it, so each
    /// is cancelled as before.
    pub(crate) fn set_aside(&mut self, gone: Vec<(Key, Parked)>) -> Vec<u64> {
        let mut forget = Vec::new();
        for (key, parked) in gone {
            if key.0.is_some() && self.main_thread && parked.effects {
                self.work.calls.push((key, parked));
            } else {
                forget.push(parked.call);
            }
        }
        forget
    }

    /// The let-go requests the runner keeps: the call set aside on each
    /// one's key has its fetch adopted, and its token is that fetch's ticket.
    /// Every call set aside is then forgotten, its other fetches rejected,
    /// unless the commit was `refused` and put its request back in flight:
    /// then it is parked again, and the call that replaced it forgotten.
    pub(crate) fn keep_sent_now(
        &mut self,
        let_go: &[InFlight<'_>],
        refused: bool,
    ) -> Vec<Option<u64>> {
        if let_go.is_empty() && self.work.calls.is_empty() {
            return Vec::new();
        }
        let mut kept = Vec::with_capacity(let_go.len());
        let mut forget = Vec::new();
        for f in let_go {
            let key = Module::key(Some(f.target), f.source, f.args);
            let Some(at) = (self.work.calls.iter()).position(|(k, _)| *k == key) else {
                kept.push(None);
                continue;
            };
            let (_, parked) = self.work.calls.remove(at);
            let (call, ticket) = (parked.call.to_string(), parked.ticket.to_string());
            let adopted = self.engine.as_mut().is_some_and(|engine| {
                engine
                    .call("__exact_adopt", [&call, &ticket, ""])
                    .is_ok_and(|r| r == "adopted")
            });
            kept.push(adopted.then_some(parked.ticket));
            forget.push(parked.call);
        }
        // The oldest call set aside under a key is the one whose request a
        // refused commit put back: the first restored, any later forgotten.
        let mut restored = std::collections::HashSet::new();
        for (key, parked) in std::mem::take(&mut self.work.calls) {
            // Back in flight: its plain request, or its own continuation.
            let back = match self.work.flight.get(&key) {
                Some(None) => true,
                Some(Some(call)) => *call == parked.call,
                None => false,
            };
            if refused && back && restored.insert(key.clone()) {
                let (replacing, others) = (std::mem::take(&mut self.parked).into_iter())
                    .partition::<Vec<_>, _>(|(k, _)| *k == key);
                self.parked = others;
                forget.extend(replacing.into_iter().map(|(_, p)| p.call));
                self.parked.push((key, parked));
            } else {
                forget.push(parked.call);
            }
        }
        self.forget_calls(forget);
        kept
    }

    /// Deliver the kept request `token`'s reply in a module turn.
    pub(crate) fn module_landed_now(
        &mut self,
        store: &mut Store,
        token: u64,
        outcome: Outcome,
    ) -> Result<(), DataError> {
        let Some(engine) = self.engine.as_mut() else {
            return Err(DataError::Unavailable(
                "exact-js: the engine is not loaded".into(),
            ));
        };
        let text = wire::outcome_to_json(&outcome).to_string();
        self.host.store = Some(store as *mut Store);
        self.host.between_answers = false;
        let ran = engine
            .call("__exact_turn", ["module", "", ""])
            .and_then(|_| engine.call("__exact_fulfill", [&token.to_string(), &text, ""]))
            .and_then(|_| engine.drain());
        // What let-go answers had waiting for their own turns runs now, with
        // no store, as a let-go chain does; the next turn is no module turn.
        self.host.store = None;
        self.host.between_answers = true;
        let _ = engine
            .call("__exact_module_discarded", ["", "", ""])
            .and_then(|_| engine.drain());
        self.host.between_answers = false;
        let _ = engine.call("__exact_turn", ["", "", ""]);
        // What landed may settle an answer waiting on another's work.
        self.progress += 1;
        self.refresh_background();
        ran.map(|_| ())
            .map_err(|e| DataError::Unavailable(format!("module work threw: {e}")))
    }
}
