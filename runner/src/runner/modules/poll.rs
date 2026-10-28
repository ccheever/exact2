//! Polled live sources (LLP 1133.009 D1, D10 item 7): a name declared
//! `"poll": "<option>"` (or `{"option", "default"}`) is asked again each
//! period that option gives, on the session's clock, while a reader reads
//! it and its implementation answers it. A poll is no plan timer, so it
//! never stops a clock advance: the runner asks once after an advance that
//! crossed a period, however many it crossed, and the implementation's
//! answer covers the time since its last. At most one ask of a source is in
//! flight. On the live clock its next period is a wake (`timer_due_ms`);
//! under the agent a jump's reply waits for the ask it made
//! (`polls_asking`, the agent's `polls`).

use super::super::{CommitReceipt, DataSource, Runner, RunnerError, Target};
use super::{error_code, Json, ModulesRow, Route};

impl<D: DataSource> Runner<D> {
    /// `row`'s poll option and its default period, if it is polled.
    fn poll_of(&self, row: &ModulesRow) -> Option<(String, Option<f64>)> {
        match &self.substitute(row)["poll"] {
            Json::String(option) => Some((option.clone(), None)),
            Json::Object(poll) => Some((
                poll.get("option")?.as_str()?.to_owned(),
                poll.get("default").and_then(Json::as_f64),
            )),
            _ => None,
        }
    }

    /// A polled source's period now: the shortest its readers ask with, else
    /// its default; `None` when it is not polled now.
    fn poll_period(&self, row: &ModulesRow) -> Option<f64> {
        let (option, default) = self.poll_of(row)?;
        let readers: Vec<Json> = (self.extensions.readers.get(&self.key(row)).into_iter())
            .flat_map(|r| r.values())
            .filter_map(|args| serde_json::from_str::<Json>(args).ok())
            .collect();
        // A reader that names no handle (every handle option empty) reads
        // nothing a poll would change.
        let sub = self.substitute(row);
        let handles: Vec<&str> = (sub["handleOptions"].as_array().into_iter().flatten())
            .filter_map(Json::as_str)
            .collect();
        let names = |args: &Json| {
            handles.is_empty()
                || handles
                    .iter()
                    .any(|h| args[h].as_str().is_some_and(|id| !id.is_empty()))
        };
        let polling: Vec<&Json> = readers.iter().filter(|args| names(args)).collect();
        if !readers.is_empty() && polling.is_empty() {
            return None;
        }
        (polling.into_iter())
            .filter_map(|args| args[&option].as_f64())
            .filter(|ms| *ms > 0.0)
            .reduce(f64::min)
            .or(default.filter(|ms| *ms > 0.0))
    }

    /// The polled sources an implementation answers and a reader reads, each
    /// with when its next ask is due. The substitute's sampled sources are
    /// polled too: their batches are taken on the session clock (LLP
    /// 1133.013 §9).
    fn polled(&self) -> Vec<(String, f64)> {
        (self.plan.modules.iter())
            .filter(|m| m.live && !m.send)
            .filter(|m| self.route(m) != Route::Substitute || self.substitute_polls(m))
            .filter_map(|m| {
                let key = self.key(m);
                if self.extensions.failed.contains(&key) || self.poll_ended(&key) {
                    return None;
                }
                let last = *self.extensions.polls.get(&key)?;
                let period = self.poll_period(m)?;
                Some((key, last + period))
            })
            .collect()
    }

    /// A polled source was asked now.
    pub(in crate::runner) fn note_poll(&mut self, row: &ModulesRow) {
        if self.poll_of(row).is_some() {
            (self.extensions.polls).insert(self.key(row), self.now_ms);
        }
    }

    /// A polled source answered: an error (a reader of a recorder or a track
    /// the session does not hold) is not polled again until a reader asks
    /// anew or the implementation announces a change, so a reader waiting
    /// for an id asks nothing each period. Nor is a source whose every
    /// Contract reader's last answer is outside its `while` (LLP 1133.016
    /// D15 item 3: a response no longer generating changes no more).
    pub(in crate::runner) fn note_poll_answer(
        &mut self,
        row: &ModulesRow,
        target: Target,
        answer: &Json,
    ) {
        if self.poll_of(row).is_none() {
            return;
        }
        let key = self.key(row);
        match error_code(answer).is_some() {
            true => self.extensions.failed.insert(key.clone()),
            false => self.extensions.failed.remove(&key),
        };
        let sub = self.substitute(row);
        let (Some(within), Target::Resource(i)) = (sub["poll"]["while"].as_object(), target) else {
            return;
        };
        let inside = (within.iter()).all(|(field, values)| {
            values
                .as_array()
                .is_some_and(|v| v.contains(&answer[field]))
        });
        self.extensions
            .within
            .entry(key)
            .or_default()
            .insert(i, inside);
    }

    /// Whether every Contract reader of `key` last read an answer outside
    /// its `while`.
    fn poll_ended(&self, key: &str) -> bool {
        let (Some(readers), Some(within)) = (
            self.extensions.readers.get(key),
            self.extensions.within.get(key),
        ) else {
            return false;
        };
        !readers.is_empty() && readers.keys().all(|i| within.get(i) == Some(&false))
    }

    /// When the next poll is due, for a host on the live clock to wake at.
    pub(in crate::runner) fn poll_due_ms(&self) -> Option<f64> {
        (self.polled().into_iter())
            .filter(|(key, _)| !self.asking(key))
            .map(|(_, due)| due)
            .chain(self.stale_due_ms())
            .reduce(f64::min)
    }

    /// After an advance: each polled source whose period passed and whose
    /// last ask has landed is asked again, once.
    pub(in crate::runner) fn poll(&mut self) -> Result<Vec<CommitReceipt>, RunnerError> {
        let due: Vec<String> = (self.polled().into_iter())
            .filter(|(key, due)| *due <= self.now_ms && !self.asking(key))
            .map(|(key, _)| key)
            .collect();
        let mut receipts = Vec::new();
        for key in due {
            self.extensions.polls.insert(key.clone(), self.now_ms);
            self.log(format!("poll {key}"));
            receipts.extend(self.changed(&key)?);
        }
        receipts.extend(self.ask_stale()?);
        Ok(receipts)
    }

    /// Whether a polled source's ask is in flight, or a stale answer's
    /// (`stale.rs`): what an agent's jump waits for before it replies.
    pub fn polls_asking(&self) -> bool {
        self.polled().iter().any(|(key, _)| self.asking(key)) || self.stale_asking()
    }
}
