//! Handles (LLP 1133.009 D6, D10 item 10): a native object a call makes and
//! names in its answer (a stream, a track, a recorder) is the request's
//! until the runner delivers that answer; only then is it the session's. A
//! name declares where its answer carries them, `"handles": {"id":
//! "recorder"}`, a field path (`tracks[].id` for each element's) and its
//! kind. A reply the runner drops (its request superseded or let go, or
//! its commit refused) makes the runner tell the implementation to dispose
//! of every handle it carries, `{"extension", "dispose": [id, …]}`, so no
//! device stays open with no id in the app. A data-code call's reply is
//! delivered to its answer outside the runner, which cannot see it dropped.

use super::super::{DataSource, Runner};
use super::Json;
use crate::request::{Outcome, Reply, Response};
use serde_json::json;

impl<D: DataSource> Runner<D> {
    /// Request `ticket` of `source` went out: a reply to a name that
    /// declares handles is provisional until it lands.
    pub(in crate::runner) fn note_handles(&mut self, ticket: u64, source: &str) {
        let Some(row) = self.plan.module_named(source) else {
            return;
        };
        if self.substitute(row)["handles"].is_object() {
            (self.extensions.provisional).insert(ticket, source.to_owned());
        }
    }

    /// The handles `outcome`, the reply to `ticket`, carries, by its
    /// extension; `None` for a reply that is no such name's.
    pub(in crate::runner) fn reply_handles(
        &mut self,
        ticket: u64,
        outcome: &Outcome,
    ) -> Option<(String, Vec<String>)> {
        let source = self.extensions.provisional.remove(&ticket)?;
        let Outcome::Response(Response { body, .. }) = outcome else {
            return None;
        };
        let answer: Json = serde_json::from_slice(body).ok()?;
        let row = self.plan.module_named(&source)?.clone();
        let mut ids = Vec::new();
        for path in self.substitute(&row)["handles"].as_object()?.keys() {
            collect(&answer, path, &mut ids);
        }
        (!ids.is_empty()).then(|| (self.plan.str(row.extension).to_owned(), ids))
    }

    /// A reply carrying `handles` was dropped (`why`): its implementation
    /// disposes of them.
    pub(in crate::runner) fn dispose(&mut self, handles: (String, Vec<String>), why: &str) {
        let (extension, ids) = handles;
        self.log(format!("dispose {extension} {}: {why}", ids.join(", ")));
        if let Some(handler) = self.native_slot().handler() {
            let body = json!({ "extension": extension, "dispose": ids });
            handler(body.to_string().into_bytes(), Reply::new(|_| {}));
        }
    }
}

/// The strings at `path` in `value`: `a.b`, and `a[].b` for each element of
/// the list `a`.
fn collect(value: &Json, path: &str, out: &mut Vec<String>) {
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
