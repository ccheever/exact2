//! Handles (LLP 1133.009 D6, D10 item 10): a native object a call makes and
//! names in its answer (a stream, a track, a recorder) is the request's
//! until the runner delivers that answer; only then is it the session's. A
//! name declares where its answer carries them, `"handles": {"id":
//! "recorder"}`, a field path (`tracks[].id` for each element's) and its
//! kind.
//!
//! The runner watches the implementation's reply to such a request as it
//! leaves the implementation, before any host's executor sees it (an
//! executor discards the reply of a request the runner let go). A reply the
//! runner then does not deliver, because its request was superseded or let
//! go or its commit was refused, makes the runner tell the implementation
//! to dispose of every handle it carries, `{"extension", "dispose": [id,
//! …]}`, so no device stays open with no id in the app. A data-code call's
//! reply reaches its answer outside the runner, which cannot see it
//! dropped, except where the runner answers it (a ceremony's substitute,
//! `ceremony.rs`) or the implementation does (a bound route, watched here):
//! a data-code call is tied to its request by its body. The session's end
//! (`destroy`) still ends what any call made.

use super::super::{DataSource, Runner};
use super::{Json, Release, TOKENS};
use crate::request::{Dispatch, Outcome, Reply, Response, Work};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// A request whose reply carries handles, until it is delivered or its
/// handles are disposed of.
pub(super) struct Provisional {
    source: String,
    body: Vec<u8>,
    dispatched: bool,
    /// A data-code call's request, which its admission claims
    /// ([`Runner::claim_request`]), never [`Runner::handled_work`].
    data: bool,
}

/// The requests let go and not yet run whose replies are still watched for.
const UNRUN: usize = 64;

/// Replies that carried handles, as they left the implementation: each
/// request's ticket, the extension and the ids.
pub(super) type Arrived = Arc<Mutex<Vec<(u64, String, Vec<String>)>>>;

impl<D: DataSource> Runner<D> {
    /// Request `ticket` of `source` went out with `body`: a reply to a name
    /// that declares handles is provisional until it lands.
    pub(in crate::runner) fn note_handles(&mut self, ticket: u64, source: &str, body: &[u8]) {
        let (row, data) = match self.plan.module_named(source) {
            Some(row) => (row.clone(), false),
            // Data code's call of a module's name: the body names it.
            None => match self.called(body) {
                Some(row) => (row, true),
                None => return,
            },
        };
        // A member's answer carries handles too (LLP 1133.014 D12).
        let members =
            (self.members_of(&row).iter()).any(|(_, m)| self.substitute(m)["handles"].is_object());
        if self.substitute(&row)["handles"].is_object() || members {
            let source = self.plan.str(self.plan.sources[row.source.0 as usize].name);
            let provisional = Provisional {
                source: source.to_owned(),
                body: body.to_vec(),
                dispatched: false,
                data,
            };
            self.extensions.provisional.insert(ticket, provisional);
        }
    }

    /// The module name a data-code call's body names, `{"extension",
    /// "send"|"source"}`.
    pub(super) fn called(&self, body: &[u8]) -> Option<super::ModulesRow> {
        let v: Json = serde_json::from_slice(body).ok()?;
        let extension = v.get("extension")?.as_str()?;
        let declared = v["send"].as_str().or(v["source"].as_str())?;
        (self.plan.modules.iter())
            .find(|m| {
                self.plan.str(m.extension) == extension && self.plan.str(m.declared) == declared
            })
            .cloned()
    }

    /// The request of the data-code call whose body is `body`, the oldest
    /// such not yet claimed: its reply is provisional (module doc).
    pub(in crate::runner) fn claim_request(&mut self, body: &[u8]) -> Option<u64> {
        let (&ticket, p) = (self.extensions.provisional.iter_mut())
            .find(|(_, p)| p.data && !p.dispatched && p.body == body)?;
        p.dispatched = true;
        Some(ticket)
    }

    /// Request `ticket`'s reply carried `ids` of `extension`: provisional
    /// until it lands, disposed of if it is dropped.
    pub(in crate::runner) fn arrive(&mut self, ticket: u64, extension: String, ids: Vec<String>) {
        let mut arrived = (self.extensions.arrived.lock()).unwrap_or_else(|e| e.into_inner());
        arrived.push((ticket, extension, ids));
    }

    /// The host runs a module request whose reply carries handles, the
    /// oldest such request with this body not yet run: its implementation's
    /// reply is watched, whether or not the request is still in flight (a
    /// host may run one the runner has let go). `None` for any other body.
    pub(in crate::runner) fn handled_work(&mut self, body: &[u8]) -> Option<Dispatch> {
        let (&ticket, p) = (self.extensions.provisional.iter_mut())
            .find(|(_, p)| !p.data && !p.dispatched && p.body == body)?;
        p.dispatched = true;
        let body = p.body.clone();
        Some(self.watched(body, ticket))
    }

    /// A held module request the agent's ticket allowed, for the host to
    /// run: its call of the implementation, watched when its reply carries
    /// handles.
    pub(super) fn forwarded(&mut self, token: u64, body: Vec<u8>) -> Dispatch {
        let ticket = (self.pending.iter())
            .find(|p| p.continuation == Some(token))
            .map(|p| p.ticket);
        match ticket.and_then(|t| self.extensions.provisional.get_mut(&t).map(|p| (t, p))) {
            Some((ticket, p)) => {
                p.dispatched = true;
                self.watched(body, ticket)
            }
            None => self.to_implementation(body),
        }
    }

    /// `body` for the implementation, its reply's handles noted as it
    /// leaves, and the host woken to look at them.
    pub(super) fn watched(&self, body: Vec<u8>, ticket: u64) -> Dispatch {
        let source = &self.extensions.provisional[&ticket].source;
        let row = self.module_row(source);
        let extension = self.plan.str(row.extension).to_owned();
        let paths: Vec<String> = (self.substitute(&row)["handles"].as_object())
            .map(|h| h.keys().cloned().collect())
            .unwrap_or_default();
        let (handler, arrived, native) = (
            self.native_slot().handler(),
            self.extensions.arrived.clone(),
            self.native_slot(),
        );
        // The handlers its call listed, for the handles it carries (`invoke.rs`).
        let sent: Json = serde_json::from_slice(&body).unwrap_or_default();
        let (grant, granted) = (self.handler_grant(&row, &sent), self.granted());
        Dispatch::Run(Work::Later(Box::new(move |reply| {
            let watched = Reply::new(move |outcome: Outcome| {
                let ids = ids_in(&outcome, &paths);
                if let Some(grant) = &grant {
                    super::invoke::grant_handles(&granted, &ids, grant);
                }
                if !ids.is_empty() {
                    let mut arrived = arrived.lock().unwrap_or_else(|e| e.into_inner());
                    arrived.push((ticket, extension, ids));
                    drop(arrived);
                    native.changed("");
                }
                reply.send(outcome);
            });
            match handler {
                Some(handler) => handler(body, watched),
                None => watched.send(super::response(&json!({ "error": "NotSupportedError" }))),
            }
        })))
    }

    /// The handles request `ticket`'s reply carried, as it lands: no longer
    /// provisional, the commit's to deliver.
    pub(in crate::runner) fn landed_handles(
        &mut self,
        ticket: u64,
    ) -> Option<(String, Vec<String>)> {
        self.extensions.provisional.remove(&ticket);
        let mut arrived = (self.extensions.arrived.lock()).unwrap_or_else(|e| e.into_inner());
        let at = arrived.iter().position(|(t, _, _)| *t == ticket)?;
        let (_, extension, ids) = arrived.remove(at);
        Some((extension, ids))
    }

    /// Replies with handles whose request is no longer in flight: dropped,
    /// their handles disposed of. Requests let go before they were sent are
    /// forgotten.
    pub(in crate::runner) fn dispose_dropped(&mut self) {
        let pending: Vec<u64> = self.pending.iter().map(|p| p.ticket).collect();
        let dropped: Vec<(u64, String, Vec<String>)> = {
            let mut arrived = (self.extensions.arrived.lock()).unwrap_or_else(|e| e.into_inner());
            let (gone, kept) = std::mem::take(&mut *arrived)
                .into_iter()
                .partition(|(t, _, _)| !pending.contains(t));
            *arrived = kept;
            gone
        };
        for (ticket, extension, ids) in dropped {
            self.extensions.provisional.remove(&ticket);
            self.dispose((extension, ids), "its reply was dropped");
        }
        // A request let go that a host never runs leaves its entry: the
        // oldest such go past a bound.
        let unrun: Vec<u64> = (self.extensions.provisional.iter())
            .filter(|(t, p)| !p.dispatched && !pending.contains(t))
            .map(|(t, _)| *t)
            .collect();
        for ticket in unrun.iter().rev().skip(UNRUN).copied().collect::<Vec<_>>() {
            self.extensions.provisional.remove(&ticket);
        }
    }

    /// A reply carrying `handles` was not delivered (`why`): its
    /// implementation disposes of them.
    pub(in crate::runner) fn dispose(&mut self, handles: (String, Vec<String>), why: &str) {
        let (extension, ids) = handles;
        self.log(format!("dispose {extension} {}: {why}", ids.join(", ")));
        if let Some(handler) = self.native_slot().handler() {
            let body = json!({ "extension": extension, "dispose": ids });
            handler(body.to_string().into_bytes(), Reply::new(|_| {}));
        }
    }

    /// The work behind a released module request: its answer, or its call
    /// of the implementation.
    pub(super) fn released(&mut self, token: u64, release: Release) -> Dispatch {
        debug_assert!(TOKENS.contains(&token));
        match release {
            Release::Answer(outcome) => Dispatch::Run(Work::Now(Box::new(move || outcome))),
            Release::Forward(body) => self.forwarded(token, body),
        }
    }
}

/// The handle ids in `outcome` at each of `paths`.
pub(super) fn ids_in(outcome: &Outcome, paths: &[String]) -> Vec<String> {
    let Outcome::Response(Response { body, .. }) = outcome else {
        return Vec::new();
    };
    let Ok(answer) = serde_json::from_slice::<Json>(body) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for path in paths {
        collect(&answer, path, &mut ids);
    }
    ids
}

/// The strings at `path` in `value`: `a.b`, and `a[].b` for each element of
/// the list `a`.
pub(super) fn collect(value: &Json, path: &str, out: &mut Vec<String>) {
    let (head, rest) = match path.split_once('.') {
        Some((head, rest)) => (head, Some(rest)),
        None => (path, None),
    };
    let (field, each) = match head.strip_suffix("[]") {
        Some(field) => (field, true),
        None => (head, false),
    };
    let at = &value[field];
    let items: Vec<&Json> = match (each, at) {
        (true, Json::Array(items)) => items.iter().collect(),
        (true, _) => Vec::new(),
        (false, at) => vec![at],
    };
    for item in items {
        match (rest, item) {
            (Some(rest), item) => collect(item, rest, out),
            (None, Json::String(id)) => out.push(id.clone()),
            _ => {}
        }
    }
}
