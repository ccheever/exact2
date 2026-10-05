//! The executor's half of background work (LLP 1097 D5, D6): storage an
//! answer started and did not await finishes after the answer, under one
//! continuation of the module's own ([`BACKGROUND`]) that the runner holds
//! as a request in flight. The prelude keeps the owner and the one queue;
//! this hands out a round only when the operation in flight is the
//! background's, so the round is the only waiter on the store's shared
//! completions (D3), and delivers it with the background current.
use super::Module;
use exact_runner::{BackgroundState, DataError, Dispatch, Outcome, Request, Work, BACKGROUND};
use serde_json::Value as Json;

/// What the prelude last said about its background work.
#[derive(Default)]
pub(crate) struct Background {
    /// The operation in flight is the background's.
    head: bool,
    /// A round is out: the host holds its ticket.
    out: bool,
    /// The last round failed in the executor (an unloaded store, a timed
    /// out wait): none goes out again until something else has run, or a
    /// host that refuses it would be asked forever.
    stalled: bool,
    pub(crate) state: BackgroundState,
}

impl Module {
    /// Read the prelude's background state; after every call into it.
    /// Never between a settle and the step that reads its reply: a call
    /// clears the reply's captured strings.
    pub(crate) fn refresh_background(&mut self) {
        self.background.stalled = false;
        self.read_background();
    }

    fn read_background(&mut self) {
        if self.storage.is_none() {
            return;
        }
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let Ok(text) = engine.call("__exact_background", ["", "", ""]) else {
            return;
        };
        let Ok(json) = serde_json::from_str::<Json>(&text) else {
            return;
        };
        let num = |k: &str| json.get(k).and_then(Json::as_u64).unwrap_or(0);
        self.background.head = json.get("head") == Some(&Json::Bool(true));
        self.background.state = BackgroundState {
            queued: num("queued"),
            in_flight: num("inFlight"),
            operations: num("operations"),
            done: num("done"),
            failed: num("failed"),
            last: json.get("last").and_then(Json::as_str).map(str::to_string),
        };
    }

    /// The next round, when the background's operation is the one in
    /// flight and no round is out.
    pub(crate) fn background_request(&mut self) -> Option<Request> {
        let b = &mut self.background;
        if b.out || b.stalled || !b.head || self.storage.is_none() {
            return None;
        }
        b.out = true;
        Some(Request::continuation(BACKGROUND))
    }

    /// A round's work: wait on the host's I/O worker for the completion of
    /// the operation in flight, as an answer's storage continuation does.
    pub(crate) fn background_dispatch(&mut self) -> Dispatch {
        match self.storage.as_ref() {
            Some(session) => Dispatch::Run(Work::Now(session.continuation())),
            None => Dispatch::Missing,
        }
    }

    /// A round came back: deliver its completion with the background
    /// current, run what it settles, and say whether another round is due.
    pub(crate) fn background_round(
        &mut self,
        outcome: Outcome,
    ) -> Result<Option<Request>, DataError> {
        self.background.out = false;
        if let Outcome::Failed { message, .. } = outcome {
            self.background.stalled = true;
            return Err(DataError::Unavailable(message));
        }
        let Some(engine) = self.engine.as_mut() else {
            return Ok(None);
        };
        self.host.between_answers = true;
        let delivered = (|| {
            engine.call("__exact_enter_background", ["", "", ""])?;
            let delivered = engine.deliver_storage_one()?;
            engine.drain()?;
            Ok::<_, String>(delivered)
        })();
        self.host.between_answers = false;
        // What landed may settle an answer waiting on the background's work.
        self.progress += 1;
        self.read_background();
        match delivered {
            Ok(true) => Ok(self.background_request()),
            Ok(false) => Ok(None),
            Err(e) => Err(DataError::Unavailable(format!(
                "background work threw: {e}"
            ))),
        }
    }

    /// The module's journal (LLP 1097 D8): the runtime's own lines as they
    /// are, its `console` marked as the app's.
    pub(crate) fn journal_lines(&mut self) -> Vec<String> {
        let mut lines = std::mem::take(&mut self.host.journal);
        lines.extend(
            self.take_logs()
                .into_iter()
                .map(|line| format!("console: {line}")),
        );
        lines
    }
}
