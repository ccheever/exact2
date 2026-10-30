//! A keyed list whose entries are watched regions (LLP 1133.021 §12): what
//! `wake_list.rs` does for a registration a wake runs for, where the entry
//! is neither a one-shot nor periodic but a state the platform reports
//! crossings of.
//!
//! - **The tap chooses its event by a field** (§9 item 9): `"tap": {"as":
//!   "wake", "event": {"by": "inside", "false": "geofenceenter", "true":
//!   "geofenceleave"}, "stays": true}` raises the event named under the
//!   field's value (`false` for none) and flips the field. A `--wake` entry
//!   that names its `event` raises that one and sets the field as it.
//! - **It stays.** A tap is refused neither before a due time nor offline,
//!   and the entry is listed whatever the invocation's outcome. One that
//!   expires or is superseded keeps the transition, `pending` on the entry,
//!   as the host's store keeps an undelivered one (D14 step 3); the next
//!   delivery of the entry clears it.
//! - **Each delivery carries a `timestamp`:** the agent clock's date for a
//!   tap, the entry's own for a `--wake` that gives one (a transition kept
//!   from before, D14).
//! - **A send that answers a value may list** (`addGeofence` answers its
//!   `Geofence`): the entry is listed as an answerless send's is, and the
//!   call answers the entry's fields.
//! - **`"most"`** bounds the entries of the list's kind, a call that would
//!   list one more failing with `"over"` (`QuotaExceededError`, D11); a
//!   call that replaces an entry of its key adds none.

use super::super::super::{DataSource, Runner};
use super::super::LISTS;
use super::super::{Json, ModulesRow};
use super::Listed;
use serde_json::{json, Map};

impl<D: DataSource> Runner<D> {
    /// Whether `list` has room for one more entry keyed `value`.
    pub(super) fn list_room(
        &self,
        extension: &str,
        list: &Map<String, Json>,
        value: &Json,
    ) -> Result<(), Json> {
        let Some(most) = list.get("most").and_then(Json::as_u64) else {
            return Ok(());
        };
        let state = list["state"].as_str().unwrap_or("listed");
        let key = list["key"].as_str().unwrap_or_default();
        let kind = list.get("kind");
        let others = (self.entries(extension, state).iter())
            .filter(|e| e.fields.get("kind") == kind && e.fields.get(key) != Some(value))
            .count();
        if others as u64 >= most {
            let over = list
                .get("over")
                .cloned()
                .unwrap_or(json!("QuotaExceededError"));
            return Err(json!({ "error": over }));
        }
        Ok(())
    }

    /// `answer`, or where it is [`LISTS`], the entry `row`'s call with
    /// `args` lists: refused as an answerless send is, else listed.
    pub(in crate::runner) fn listed_or(
        &mut self,
        row: &ModulesRow,
        args: &Json,
        answer: Json,
    ) -> Json {
        if answer.get(LISTS) != Some(&Json::Bool(true)) {
            return answer;
        }
        let refused = match self.effect_refusal(row, args) {
            Some(refusal) => Err(refusal),
            None => self.apply_effect(row, args),
        };
        match refused {
            Ok(()) => self.effect_answer(row, args),
            Err(refusal) => {
                let code = refusal["error"].as_str().unwrap_or_default().to_owned();
                let declared = self.plan.str(row.declared).to_owned();
                self.log(format!("{declared}: refused: {code}"));
                refusal
            }
        }
    }

    /// Whether `row` is a send that answers a value and lists (module doc):
    /// data code's call of it is held as an answerless send's is.
    pub(in crate::runner) fn lists_answer(&self, row: &ModulesRow) -> bool {
        row.send && !self.is_effect(row) && self.substitute(row)["list"].is_object()
    }

    /// What an applied effect answers: nothing, or for a send that lists
    /// what it answers, the entry as listed.
    pub(super) fn effect_answer(&self, row: &ModulesRow, args: &Json) -> Json {
        if !self.lists_answer(row) {
            return Json::Null;
        }
        let sub = self.substitute(row);
        let list = &sub["list"];
        let (state, key) = (
            list["state"].as_str().unwrap_or("listed"),
            list["key"].as_str(),
        );
        let value = key.map(|k| &args[k]);
        let extension = self.plan.str(row.extension).to_owned();
        let entry = (self.entries(&extension, state).into_iter())
            .rfind(|e| key.is_none() || e.fields.get(key.unwrap_or_default()) == value);
        let mut answer = entry.map(|e| e.fields).unwrap_or_default();
        // The entry keeps every option the call passed, a platform's record
        // (`android`) among them, and the list's own fields (`kind`); the
        // answer is the fields its shape declares.
        let ty = self.plan.sources[row.source.0 as usize].ty;
        let mut shape = exact_js_value::Shape::from_plan(&self.plan, ty).ok();
        while let Some(exact_js_value::Shape::Option(inner)) = shape {
            shape = Some(*inner);
        }
        if let Some(exact_js_value::Shape::Record(fields)) = shape {
            answer.retain(|k, _| fields.iter().any(|(f, _)| f == k));
        }
        Json::Object(answer)
    }

    /// Whether entry `e`'s tap leaves it listed whatever happens (module doc).
    pub(super) fn stays(&self, e: &Listed) -> bool {
        self.substitute(&self.module_row(&e.source))["list"]["tap"]["stays"] == true
    }

    /// The event entry `fields` raises by its tap, and what the field the
    /// tap chooses by becomes: the `event` a `--wake` entry names, else the
    /// one under the field's value. `None` for a tap that names no event.
    pub(super) fn chosen_event(
        tap: &Json,
        fields: &Map<String, Json>,
    ) -> Option<(String, Option<(String, Json)>)> {
        let by = match &tap["event"] {
            Json::String(event) => return Some((event.clone(), None)),
            Json::Object(by) => by,
            _ => return None,
        };
        let field = by.get("by")?.as_str()?;
        let word = |v: &Json| match v {
            Json::Bool(b) => b.to_string(),
            Json::String(s) => s.clone(),
            _ => "false".into(),
        };
        let now = word(fields.get(field).unwrap_or(&Json::Null));
        let named = fields.get("event").and_then(Json::as_str);
        let (was, event) = match named {
            Some(event) => {
                let was = (by.iter()).find(|(k, v)| *k != "by" && v.as_str() == Some(event));
                (was?.0.clone(), event.to_owned())
            }
            None => (now.clone(), by.get(&now)?.as_str()?.to_owned()),
        };
        // A two-valued field flips; the new value is the one that is not `was`.
        let next = match was.as_str() {
            "true" => json!(false),
            "false" => json!(true),
            other => json!(other),
        };
        Some((event, Some((field.to_owned(), next))))
    }

    /// Entry `reference` of a list that stays ended its invocation `state`:
    /// a completed delivery clears its kept transition; an expired or
    /// superseded one keeps it (module doc).
    pub(super) fn region_ended(&mut self, list: &str, reference: u64, state: &str, event: &str) {
        let Some(entries) = self.extensions.declared.lists.get_mut(list) else {
            return;
        };
        let Some(e) = entries.iter_mut().find(|e| e.reference == reference) else {
            return;
        };
        match state {
            "completed" => {
                e.fields.remove("pending");
            }
            "expired" | "superseded" => {
                e.fields.insert("pending".into(), json!(event));
                self.log(format!("{list} @{reference}: {event} kept, undelivered"));
            }
            _ => {}
        }
        self.announce_listed(list);
    }
}
