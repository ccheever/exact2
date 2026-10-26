//! A device a call names, a send answered after a span, and a list whose
//! changes are events: forms of the declared substitute (§6) that a
//! module with devices of its own declares (LLP 1133.022 D10 item 3, §9):
//!
//! - **`"of"`**: `{"source", "list", "key", "current", "missing", "lacks",
//!   "needs"}`. The call names an entry of a sibling source's list by its
//!   `key` option, or, leaving it out, the entry the source's `current`
//!   names when the call arrives, which is written into the call. A call
//!   naming no entry fails `missing` (`NotFoundError`); `"lacks": {option:
//!   field}` fails one whose option's value is not in the entry's list
//!   field, and `"needs": field` one whose entry's field is not true, both
//!   `NotSupportedError`.
//! - **`"answer"`, `"after"`, `"preempts"`**: a send answers `answer`,
//!   after the sum of the options `after` names, in milliseconds on the
//!   session's clock (`clock +N` crosses it under the agent). A send that
//!   declares `preempts` ends every earlier one of its extension still
//!   running on the same entry, which answers `preempts` instead.
//! - **`"played"`**: `{"state", "by"}`. Each call is kept in
//!   `state.extensions.<module>.<state>`, with its options, the entry it
//!   named, its origin and, once answered, its `result`; with `by`, one per
//!   value of that option, the latest.
//! - **`"changes"`** on a source: `{"list", "key", "added", "removed"}`.
//!   When the agent's preferred value changes the list, each entry that
//!   came raises `added` and each that went `removed`, with the entry, as
//!   the device would.

use super::super::{DataSource, Runner};
use super::{error_code, CallOrigin, Decision, Json, ModulesRow, Route, Waits};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Default)]
pub(in crate::runner) struct Spans {
    /// What `played` keeps, by extension and state name.
    played: BTreeMap<String, BTreeMap<String, Vec<Json>>>,
}

impl<D: DataSource> Runner<D> {
    /// The entry of `of`'s list that `args` names, or why it names none.
    fn entry_of(&self, row: &ModulesRow, of: &Json, args: &Json) -> Result<Json, Json> {
        let missing = json!({ "error": of["missing"].as_str().unwrap_or("NotFoundError") });
        let source = of["source"].as_str().and_then(|s| self.sibling(row, s));
        let value = source.and_then(|s| self.value_of(&s)).unwrap_or_default();
        let key = of["key"].as_str().unwrap_or("index");
        let wanted = &args[key];
        let list = value[of["list"].as_str().unwrap_or_default()].as_array();
        let entry =
            (list.into_iter().flatten()).find(|e| !wanted.is_null() && numeric_eq(&e[key], wanted));
        entry.cloned().ok_or(missing)
    }

    /// `of`'s key, written into a call that leaves it out: the source's
    /// `current` now. Only the substitute resolves it: an implementation
    /// reads its own current entry at receipt.
    pub(in crate::runner) fn resolve_of(&self, row: &ModulesRow, args: &mut Json) {
        let sub = self.substitute(row);
        let of = &sub["of"];
        if of.is_null() || self.route(row) != Route::Substitute {
            return;
        }
        let key = of["key"].as_str().unwrap_or("index");
        if !args[key].is_null() {
            return;
        }
        let source = of["source"].as_str().and_then(|s| self.sibling(row, s));
        let value = source.and_then(|s| self.value_of(&s)).unwrap_or_default();
        let current = &value[of["current"].as_str().unwrap_or("current")];
        if let (false, Some(object)) = (current.is_null(), args.as_object_mut()) {
            object.insert(key.to_owned(), current.clone());
        }
    }

    /// What `of` decides of a call: `None` when it names an entry that can
    /// do what the call asks.
    pub(in crate::runner) fn decide_of(&self, row: &ModulesRow, args: &Json) -> Option<Json> {
        let sub = self.substitute(row);
        let of = &sub["of"];
        if of.is_null() {
            return None;
        }
        let entry = match self.entry_of(row, of, args) {
            Ok(entry) => entry,
            Err(missing) => return Some(missing),
        };
        let unsupported = Some(json!({ "error": "NotSupportedError" }));
        for (option, field) in of["lacks"].as_object().into_iter().flatten() {
            let can = entry[field.as_str().unwrap_or_default()].as_array();
            if args[option].is_string() && !can.is_some_and(|c| c.contains(&args[option])) {
                return unsupported;
            }
        }
        if let Some(field) = of["needs"].as_str() {
            if entry[field] != true {
                return unsupported;
            }
        }
        None
    }

    /// What a call's `of` and a send's span decide, before its value.
    pub(super) fn decide_spans(
        &self,
        row: &ModulesRow,
        sub: &Json,
        send: bool,
        args: &Json,
    ) -> Option<Decision> {
        if let Some(refused) = self.decide_of(row, args) {
            return Some(Decision::Now(refused));
        }
        send.then(|| self.decide_span(sub, args)).flatten()
    }

    /// A send's declared `answer`, now or after its span (`None`: the
    /// send declares none).
    fn decide_span(&self, sub: &Json, args: &Json) -> Option<Decision> {
        if sub["answer"].is_null() {
            return None;
        }
        Some(match span_ms(sub, args) {
            ms if ms > 0.0 => Decision::Hold(Waits::Span),
            _ => Decision::Now(sub["answer"].clone()),
        })
    }

    /// The span a held call of `row` runs for, from now.
    pub(in crate::runner) fn span_deadline(&self, row: &ModulesRow, args: &Json) -> f64 {
        self.now_ms + span_ms(&self.substitute(row), args)
    }

    /// A call of `row` admitted with `decision`: it ends the earlier ones
    /// it preempts, and is kept where it is `played`.
    pub(super) fn span_admitted(
        &mut self,
        row: &ModulesRow,
        args: &Json,
        origin: CallOrigin,
        decision: &Decision,
    ) {
        let answered = match decision {
            Decision::Now(answer) if error_code(answer).is_none() => Some(answer.clone()),
            Decision::Hold(Waits::Span) => None,
            _ => return,
        };
        if self.route(row) != Route::Substitute {
            return;
        }
        let sub = self.substitute(row);
        if !sub["preempts"].is_null() {
            self.preempt(row, &sub, args);
        }
        let token = match decision {
            Decision::Hold(_) => Some(super::TOKENS.start + self.extensions.next),
            _ => None,
        };
        let result = answered.map(|a| a.get("result").cloned().unwrap_or(a));
        self.play(row, args, origin, token, result);
    }

    /// An effect (a send answering nothing) the substitute did: kept where
    /// it is `played`.
    pub(in crate::runner) fn effect_played(
        &mut self,
        row: &ModulesRow,
        args: &Json,
        origin: CallOrigin,
    ) {
        self.play(row, args, origin, None, None);
    }

    fn play(
        &mut self,
        row: &ModulesRow,
        args: &Json,
        origin: CallOrigin,
        token: Option<u64>,
        result: Option<Json>,
    ) {
        let sub = self.substitute(row);
        let Some(state) = sub["played"]["state"].as_str() else {
            return;
        };
        let mut entry = args.as_object().cloned().unwrap_or_default();
        entry.retain(|_, v| !v.is_null() || sub["played"]["by"].is_string());
        entry.insert("origin".into(), json!(origin.word()));
        if let Some(token) = token {
            entry.insert("ref".into(), json!(token));
        }
        if let Some(result) = result {
            entry.insert("result".into(), result);
        } else if token.is_some() {
            entry.insert("result".into(), Json::Null);
        }
        let extension = self.plan.str(row.extension).to_owned();
        let list = (self.extensions.spans.played.entry(extension))
            .or_default()
            .entry(state.to_owned())
            .or_default();
        if let Some(by) = sub["played"]["by"].as_str() {
            list.retain(|e| !numeric_eq(&e[by], &args[by]));
        }
        list.push(Json::Object(entry));
    }

    /// End the spans running on the entry `args` names that `row`'s send
    /// preempts: each answers `preempts`.
    fn preempt(&mut self, row: &ModulesRow, sub: &Json, args: &Json) {
        let key = sub["of"]["key"].as_str().unwrap_or("index");
        let extension = self.plan.str(row.extension).to_owned();
        let ended: Vec<u64> = (self.extensions.held.iter())
            .filter(|h| h.waits == Waits::Span && numeric_eq(&h.args[key], &args[key]))
            .filter(|h| self.plan.str(self.module_row(&h.source).extension) == extension)
            .map(|h| h.token)
            .collect();
        for token in ended {
            let answer = self.substitute(row)["preempts"].clone();
            self.span_end(token, answer);
        }
    }

    /// The spans due by `now_ms`: each answers its declared `answer`.
    pub(in crate::runner) fn spans_due(&mut self, now_ms: f64) {
        let mut due: Vec<(f64, u64)> = (self.extensions.held.iter())
            .filter(|h| h.waits == Waits::Span && h.deadline.is_some_and(|d| d <= now_ms))
            .map(|h| (h.deadline.unwrap_or_default(), h.token))
            .collect();
        due.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, token) in due {
            let Some(h) = self.extensions.held.iter().find(|h| h.token == token) else {
                continue;
            };
            let answer = self.substitute(&self.module_row(&h.source))["answer"].clone();
            self.span_end(token, answer);
        }
    }

    /// The soonest span's end, for the clock's next wake.
    pub(in crate::runner) fn span_due(&self) -> Option<f64> {
        (self.extensions.held.iter())
            .filter(|h| h.waits == Waits::Span)
            .filter_map(|h| h.deadline)
            .min_by(f64::total_cmp)
    }

    fn span_end(&mut self, token: u64, answer: Json) {
        let result = answer.get("result").cloned().unwrap_or(answer.clone());
        for list in self
            .extensions
            .spans
            .played
            .values_mut()
            .flat_map(|s| s.values_mut())
        {
            for e in list.iter_mut().filter(|e| e["ref"] == token) {
                e["result"] = result.clone();
            }
        }
        if let Some(h) = self.extensions.held.iter().find(|h| h.token == token) {
            let key = self.key(&self.module_row(&h.source));
            self.log(format!("{key}: {}", result.as_str().unwrap_or("answered")));
        }
        self.release(token, answer);
        self.native_slot().changed("");
    }

    /// What `played` keeps for `extension`, by state name.
    pub(in crate::runner) fn played_json(
        &self,
        extension: &str,
    ) -> Option<impl Iterator<Item = (String, Json)> + '_> {
        let played = self.extensions.spans.played.get(extension)?;
        Some(
            played
                .iter()
                .map(|(state, calls)| (state.clone(), json!(calls))),
        )
    }

    /// The agent's preferred value for source `row` went from `before` to
    /// `after`: its declared `changes` raise their events.
    pub(in crate::runner) fn preferred_changed(
        &mut self,
        row: &ModulesRow,
        before: Option<Json>,
        after: Option<Json>,
    ) {
        let changes = self.substitute(row)["changes"].clone();
        let (Some(list), Some(key)) = (changes["list"].as_str(), changes["key"].as_str()) else {
            return;
        };
        let entries = |v: &Option<Json>| -> Vec<Json> {
            (v.as_ref()
                .and_then(|v| v[list].as_array())
                .into_iter()
                .flatten())
            .cloned()
            .collect()
        };
        let (was, is) = (entries(&before), entries(&after));
        let absent = |from: &[Json], e: &Json| !from.iter().any(|f| numeric_eq(&f[key], &e[key]));
        let extension = self.plan.str(row.extension).to_owned();
        let raise = |r: &mut Self, event: &Json, e: &Json| {
            if let Some(event) = event.as_str() {
                let args = Json::Array(vec![e.clone()]).to_string();
                if let Err(why) = r.raise_module_event(&extension, event, &args, CallOrigin::Device)
                {
                    r.log(why);
                }
            }
        };
        for e in was.iter().filter(|e| absent(&is, e)) {
            raise(self, &changes["removed"], e);
        }
        for e in is.iter().filter(|e| absent(&was, e)) {
            raise(self, &changes["added"], e);
        }
    }

    /// `row`'s value, where its substitute declares `changes`.
    pub(in crate::runner) fn changes_value(&self, row: &ModulesRow) -> Option<Json> {
        (!self.substitute(row)["changes"].is_null())
            .then(|| self.value_of(row))
            .flatten()
    }
}

/// The sum of the options `after` names, ms.
fn span_ms(sub: &Json, args: &Json) -> f64 {
    (sub["after"].as_array().into_iter().flatten())
        .filter_map(|o| o.as_str())
        .filter_map(|o| args[o].as_f64())
        .sum()
}

/// Two keys alike, as numbers when both are (the plan holds 0 as `0.0`).
fn numeric_eq(a: &Json, b: &Json) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}
