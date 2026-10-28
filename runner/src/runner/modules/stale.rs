//! An answer that names when it goes stale (LLP 1133.015 D7, D16 item 2):
//! an implementation's reply to a live source may carry `"exact:stale"`, a
//! Unix time in ms, beside its fields (a list answer as `{"exact:answer":
//! [...], "exact:stale": t}`). The runner takes it off before anything
//! reads the answer and, when the session's wall clock (`epochAtZero` plus
//! the session clock, LLP 1027.000.000) reaches it, asks the source's
//! readers again, once: a subscription's end, which no store reports. Each
//! answer names its own next time; one with none, or one already past,
//! clears it. The ask runs beside the polls (`poll.rs`), outside the timer
//! wheel: it never stops a clock advance, and an agent's jump past it waits
//! for the ask to land.

use super::super::{CommitReceipt, DataSource, Runner, RunnerError};
use super::Json;

/// The reply key that names the stale time.
const STALE: &str = "exact:stale";

impl<D: DataSource> Runner<D> {
    /// `body`, a reply to live source `key`, without its stale time, which
    /// is noted: a list answer's envelope is unwrapped when nothing else is
    /// left in it. A body that names none is returned as it came.
    pub(in crate::runner) fn take_stale(&mut self, key: &str, body: Vec<u8>) -> Vec<u8> {
        let Ok(Json::Object(mut map)) = serde_json::from_slice::<Json>(&body) else {
            return body;
        };
        let Some(at) = map.remove(STALE) else {
            // A fresh answer that names no time ends the one before it.
            if self.extensions.stale.remove(key).is_some() {
                self.log(format!("{key}: names no stale time now"));
            }
            return body;
        };
        let session = at.as_f64().map(|unix| unix - self.time.epoch_at_zero);
        match session.filter(|ms| *ms > self.now_ms) {
            Some(ms) => {
                if self.extensions.stale.get(key) != Some(&ms) {
                    self.log(format!("{key}: stale at {ms} ms"));
                }
                self.extensions.stale.insert(key.to_owned(), ms);
            }
            None => {
                self.extensions.stale.remove(key);
            }
        }
        let value = match map.len() {
            1 if map.contains_key("exact:answer") => map.remove("exact:answer").unwrap_or_default(),
            _ => Json::Object(map),
        };
        value.to_string().into_bytes()
    }

    /// The earliest stale time not yet asked, for a host on the live clock.
    pub(super) fn stale_due_ms(&self) -> Option<f64> {
        (self.extensions.stale.iter())
            .filter(|(key, ms)| ms.is_finite() && !self.asking(key))
            .map(|(_, ms)| *ms)
            .reduce(f64::min)
    }

    /// After an advance: each source whose stale time passed is asked again,
    /// once; it stays marked asked until its answer names its next time.
    pub(super) fn ask_stale(&mut self) -> Result<Vec<CommitReceipt>, RunnerError> {
        let due: Vec<String> = (self.extensions.stale.iter())
            .filter(|(key, ms)| ms.is_finite() && **ms <= self.now_ms && !self.asking(key))
            .map(|(key, _)| key.clone())
            .collect();
        let mut receipts = Vec::new();
        for key in due {
            self.extensions.stale.insert(key.clone(), f64::INFINITY);
            self.log(format!("{key}: stale, asked again"));
            receipts.extend(self.changed(&key)?);
        }
        Ok(receipts)
    }

    /// Whether a stale ask is in flight, which an agent's jump waits for.
    pub(super) fn stale_asking(&self) -> bool {
        (self.extensions.stale.iter()).any(|(key, ms)| ms.is_infinite() && self.asking(key))
    }
}
