//! A data-code call of a module's send is let go with the request that
//! made it (LLP 1133.035 D16 item 4). A Contract send forwarded to an
//! implementation carries a number that `cancel` names when it is let go
//! (LLP 1133.016 D15 item 2, `bound.rs`); a data-code call's body gets one
//! as the request it travels in is handed out, so the same bookkeeping
//! covers it, and a call the substitute or the agent holds for its value
//! is retired when that request is no longer in flight: the answer that made it is no
//! longer kept, and a write it started never settles against a later tag.

use super::super::{DataSource, Runner};
use super::{Json, Waits};
use crate::request::Request;
use serde_json::json;
use std::collections::BTreeMap;

/// The data-code calls held for the agent, each by its held token: the
/// request that made it.
#[derive(Default)]
pub(in crate::runner) struct LetGo {
    held: BTreeMap<u64, u64>,
}

impl<D: DataSource> Runner<D> {
    /// `request`, about to be handed out, numbered when it is data code's
    /// call of one of this plan's module sends that has no number yet.
    pub(in crate::runner) fn number_data_call(&mut self, request: &mut Request) {
        if !request.is_native() {
            return;
        }
        let Ok(mut v) = serde_json::from_slice::<Json>(&request.body) else {
            return;
        };
        if v["send"].as_str().is_none() || !v["request"].is_null() {
            return;
        }
        if self.called(&request.body).is_none() {
            return;
        }
        self.extensions.requests += 1;
        v["request"] = json!(self.extensions.requests);
        request.body = v.to_string().into_bytes();
    }

    /// Data-code call `body` is held under `token`: kept beside the request
    /// in flight that carries it.
    pub(super) fn note_data_hold(&mut self, token: u64, body: &[u8]) {
        let Ok(v) = serde_json::from_slice::<Json>(body) else {
            return;
        };
        let Some(n) = v["request"].as_u64() else {
            return;
        };
        let ticket = (self.extensions.sent.iter()).find(|(_, (_, m))| *m == n);
        if let Some((&ticket, _)) = ticket {
            self.extensions.letgo.held.insert(token, ticket);
        }
    }

    /// Each held data-code call whose request is no longer in flight:
    /// retired with its ticket, its reply answered with its `stopped`
    /// (a scan let go is stopped, LLP 1133.035 §6), else its `cancel`.
    pub(super) fn let_go_data_calls(&mut self) {
        let gone: Vec<u64> = (self.extensions.letgo.held.iter())
            .filter(|(_, t)| !self.pending.iter().any(|p| p.ticket == **t))
            .map(|(token, _)| *token)
            .collect();
        for token in gone {
            self.extensions.letgo.held.remove(&token);
            // Only a call waiting on its value: a ceremony runs to its end
            // and its reply is disposed of (LLP 1133.016 D11), and a
            // permission's ticket is the person's to answer.
            let Some(h) =
                (self.extensions.held.iter()).find(|h| h.token == token && h.waits == Waits::Value)
            else {
                continue;
            };
            let (hold, source) = (h.hold, h.source.clone());
            let row = self.module_row(&source);
            let substitute = self.substitute(&row);
            let cancel = match (&substitute["stopped"], &substitute["cancel"]) {
                (Json::Null, Json::Null) => json!({ "error": "AbortError" }),
                (Json::Null, cancel) | (cancel, _) => cancel.clone(),
            };
            self.log(format!("{}: its call was let go", self.key(&row)));
            self.retire_hold(hold);
            self.release(token, cancel);
        }
        let held = &self.extensions.held;
        (self.extensions.letgo.held).retain(|token, _| held.iter().any(|h| h.token == *token));
    }
}
