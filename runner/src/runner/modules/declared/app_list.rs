//! A keyed list kept by the app, with a due time and the person's choices
//! (LLP 1133.030 D12 item 2): `"list": {"state", "key", "scope": "app",
//! "due", "tap": {"choices"}}`, and the `update` effect beside `list` and
//! `unlist`.
//!
//! - **`"scope": "app"`** keeps the list in the host's app-wide state, so
//!   every session of the app reads one list. Its entries' `ref`s come from
//!   one count the app's sessions share. A session hears another's change
//!   at its next clock step, and asks its readers again.
//! - **`"due": {"at", "after", "into", "as", "fires"}`**: an entry is due at
//!   the instant option `at`, or at the agent clock plus the length option
//!   `after`, stored in field `into` as an epoch instant; `as` names the
//!   fields each form lists with (`{"at": {"state": "scheduled"}}`). The
//!   first session whose agent clock passes the instant writes `fires`'
//!   fields and `delivered: true` into the entry, once.
//! - **`"tap": {"choices"}`**: `tap @ref <choice>` on a delivered entry
//!   does what the platform's button would: `{"retire": true}` takes it
//!   away; `{"after": <field>, "set": {…}}` makes it due again that field's
//!   length from now; `{"when", "set", "event", "args"}` (LLP 1133.033
//!   D21) is refused by name unless the entry matches `when`, sets `set`
//!   and leaves it listed, then raises `event` as an activation. Such an
//!   entry refuses a tap without a choice.
//! - **`"update": {"state", "key", "when", "else", "set", "pause" |
//!   "resume"}`** changes the entry with the call's key in place, refusing
//!   `else` when there is none or its fields do not match `when` (a field's
//!   value, or an array of the values it may hold). `set`'s values take
//!   `update.rs`'s forms. `pause`
//!   names the field that keeps the time left, and clears the due instant;
//!   `resume` makes it due that long from now and clears the field.

use super::super::super::{DataSource, Runner};
use super::super::{CallOrigin, Json, ModulesRow};
use super::Listed;
use serde_json::{json, Map};

impl Listed {
    fn to_json(&self) -> Json {
        json!({ "ref": self.reference, "source": self.source, "fields": self.fields })
    }

    fn from_json(v: &Json) -> Option<Self> {
        Some(Listed {
            reference: v["ref"].as_u64()?,
            source: v["source"].as_str()?.to_owned(),
            fields: v["fields"].as_object()?.clone(),
        })
    }
}

impl<D: DataSource> Runner<D> {
    fn epoch(&self) -> f64 {
        self.time.epoch_at_zero + self.now_ms
    }

    /// The `list` declaration of `extension` that lists into `state`.
    pub(super) fn list_decl(&self, extension: &str, state: &str) -> Json {
        (self.plan.modules.iter())
            .filter(|m| self.plan.str(m.extension) == extension)
            .map(|m| self.substitute(m)["list"].clone())
            .find(|l| l["state"] == state)
            .unwrap_or_default()
    }

    /// Where an app-scoped list is kept in the app's state; `None` for a
    /// list the session keeps.
    fn app_key(&self, extension: &str, state: &str) -> Option<String> {
        (self.list_decl(extension, state)["scope"] == "app")
            .then(|| format!("{}/{extension}/list/{state}", self.data.app_id()))
    }

    /// List `state` of `extension`'s entries now.
    pub(super) fn entries(&self, extension: &str, state: &str) -> Vec<Listed> {
        let Some(key) = self.app_key(extension, state) else {
            return self
                .extensions
                .declared
                .lists
                .get(state)
                .cloned()
                .unwrap_or_default();
        };
        let Some(app) = self.extensions.declared.app.as_ref() else {
            return Vec::new();
        };
        let app = app.lock().unwrap_or_else(|e| e.into_inner());
        (app.get(&key)
            .and_then(|l| l["entries"].as_array())
            .into_iter())
        .flatten()
        .filter_map(Listed::from_json)
        .collect()
    }

    /// Change list `state` of `extension` with `f`; an app-scoped list's
    /// revision moves when `f` says it changed it, and this session has
    /// then seen it.
    pub(super) fn with_list<T>(
        &mut self,
        extension: &str,
        state: &str,
        f: impl FnOnce(&mut Vec<Listed>) -> (T, bool),
    ) -> T {
        let Some(key) = self.app_key(extension, state) else {
            let list = (self.extensions.declared.lists)
                .entry(state.to_owned())
                .or_default();
            return f(list).0;
        };
        let app = self.app_state();
        let mut app = app.lock().unwrap_or_else(|e| e.into_inner());
        let stored = app.get(&key).cloned().unwrap_or_default();
        let mut list: Vec<Listed> = (stored["entries"].as_array().into_iter())
            .flatten()
            .filter_map(Listed::from_json)
            .collect();
        let (out, changed) = f(&mut list);
        let mut rev = stored["rev"].as_u64().unwrap_or(0);
        if changed {
            rev += 1;
            let entries: Vec<Json> = list.iter().map(Listed::to_json).collect();
            app.insert(key.clone(), json!({ "rev": rev, "entries": entries }));
        }
        drop(app);
        self.extensions.declared.seen.insert(key, rev);
        out
    }

    /// A `ref` for an entry of an app-scoped list: one count every session
    /// of the app shares, never below this session's own tickets.
    pub(super) fn app_reference(&mut self) -> u64 {
        let key = format!("{}/refs", self.data.app_id());
        let app = self.app_state();
        let mut app = app.lock().unwrap_or_else(|e| e.into_inner());
        let reference = (app.get(&key).and_then(Json::as_u64).unwrap_or(0)).max(self.next_ticket);
        app.insert(key, json!(reference + 1));
        self.next_ticket = reference + 1;
        reference
    }

    /// An entry's fields as listed: its due instant and the fields its form
    /// lists with, where its list declares `due`.
    pub(super) fn due_fields(&self, list: &Json, fields: &mut Map<String, Json>) {
        let due = &list["due"];
        let (Some(into), true) = (due["into"].as_str(), due.is_object()) else {
            return;
        };
        let given = |option: &Json| {
            option
                .as_str()
                .and_then(|o| fields.get(o))
                .and_then(Json::as_f64)
        };
        let (form, at) = match (given(&due["at"]), given(&due["after"])) {
            (Some(at), _) => ("at", at),
            (None, Some(after)) => ("after", self.epoch() + after),
            (None, None) => return,
        };
        fields.insert(into.into(), ms(at));
        for (field, value) in due["as"][form].as_object().into_iter().flatten() {
            fields.insert(field.clone(), value.clone());
        }
        fields.insert("delivered".into(), json!(false));
    }

    /// Whether `list` keeps a due time.
    pub(super) fn is_due_list(list: &Json) -> bool {
        list["due"]["into"].is_string()
    }

    /// The agent clock reached `now_ms`: each entry of a list with a due
    /// time that is now due fires, once; a list another session changed is
    /// announced to this one's readers.
    pub(in crate::runner) fn listed_due(&mut self, now_ms: f64) {
        let now = self.time.epoch_at_zero + now_ms;
        let mut lists: Vec<(String, Json)> = Vec::new();
        for row in &self.plan.modules {
            let list = self.substitute(row)["list"].clone();
            let extension = self.plan.str(row.extension).to_owned();
            if Self::is_due_list(&list)
                && !lists
                    .iter()
                    .any(|(e, l)| *e == extension && l["state"] == list["state"])
            {
                lists.push((extension, list));
            }
        }
        for (extension, list) in lists {
            let state = list["state"].as_str().unwrap_or_default().to_owned();
            let into = list["due"]["into"].as_str().unwrap_or_default().to_owned();
            let fires = list["due"]["fires"]
                .as_object()
                .cloned()
                .unwrap_or_default();
            let key = self.app_key(&extension, &state);
            let seen = key
                .as_ref()
                .and_then(|k| self.extensions.declared.seen.get(k).copied());
            let fired: Vec<u64> = self.with_list(&extension, &state, |entries| {
                let mut fired = Vec::new();
                for e in entries.iter_mut() {
                    let due = e.fields.get(&into).and_then(Json::as_f64);
                    if e.fields.get("delivered") == Some(&json!(false))
                        && due.is_some_and(|d| d <= now)
                    {
                        e.fields.extend(fires.clone());
                        e.fields.insert("delivered".into(), json!(true));
                        fired.push(e.reference);
                    }
                }
                let changed = !fired.is_empty();
                (fired, changed)
            });
            for reference in &fired {
                self.log(format!("{state} @{reference}: due"));
            }
            let moved = key.is_some_and(|k| self.extensions.declared.seen.get(&k).copied() != seen);
            if !fired.is_empty() || moved {
                self.announce_listed(&state);
            }
        }
    }

    /// `tap @ref <choice>` on an entry whose list declares choices.
    pub(super) fn tap_choice(
        &mut self,
        extension: &str,
        state: &str,
        reference: u64,
        choice: Option<&str>,
    ) -> Result<String, String> {
        let list = self.list_decl(extension, state);
        let choices = list["tap"]["choices"].clone();
        let names: Vec<&str> = choices
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k.as_str())
            .collect();
        let Some(choice) = choice else {
            return Err(format!(
                "@{reference} takes a choice: {}",
                names.join(" or ")
            ));
        };
        let Some(does) = choices.get(choice).cloned() else {
            return Err(format!(
                "@{reference}: no choice `{choice}`; it takes {}",
                names.join(" or ")
            ));
        };
        let into = list["due"]["into"].as_str().unwrap_or_default().to_owned();
        let now = self.epoch();
        let done = self.with_list(extension, state, |entries| {
            let Some(at) = entries.iter().position(|e| e.reference == reference) else {
                return (Err(format!("@{reference} is no listed entry")), false);
            };
            if entries[at].fields.get("delivered") == Some(&json!(false)) {
                return (
                    Err(format!(
                        "@{reference} is not delivered yet: advance the clock past its time"
                    )),
                    false,
                );
            }
            if does["retire"] == true {
                entries.remove(at);
                return (Ok(()), true);
            }
            if !does["when"].is_null() && !Self::matches(&entries[at].fields, &does["when"]) {
                let now: Vec<String> = (does["when"].as_object().into_iter().flatten())
                    .map(|(f, _)| {
                        format!(
                            "its {f} is {}",
                            entries[at].fields.get(f).unwrap_or(&Json::Null)
                        )
                    })
                    .collect();
                return (
                    Err(format!(
                        "@{reference}: `{choice}` is refused: {}",
                        now.join(", ")
                    )),
                    false,
                );
            }
            let e = &mut entries[at];
            if !does["after"].is_string() {
                Self::set_fields(&mut e.fields, &does["set"], &Map::new(), now);
                return (Ok(()), true);
            }
            let after = does["after"]
                .as_str()
                .and_then(|f| e.fields.get(f))
                .and_then(Json::as_f64);
            let Some(after) = after else {
                return (
                    Err(format!(
                        "@{reference} has no `{}` for `{choice}`",
                        does["after"].as_str().unwrap_or_default()
                    )),
                    false,
                );
            };
            e.fields.insert(into.clone(), ms(now + after));
            e.fields.insert("delivered".into(), json!(false));
            for (field, value) in does["set"].as_object().into_iter().flatten() {
                e.fields.insert(field.clone(), value.clone());
            }
            (Ok(()), true)
        });
        done?;
        self.log(format!("{state} @{reference}: {choice}"));
        self.announce_listed(state);
        let Some(event) = does["event"].as_str() else {
            return Ok(
                json!({ "ref": reference, "choice": choice, "delivery": "substituted" })
                    .to_string(),
            );
        };
        let entry = (self.entries(extension, state).into_iter())
            .find(|e| e.reference == reference)
            .map(|e| Json::Object(e.fields))
            .unwrap_or_default();
        let args = super::entry_args(&does, &entry);
        self.raise_module_event(
            extension,
            event,
            &Json::Array(args).to_string(),
            CallOrigin::App,
        )?;
        Ok(json!({ "ref": reference, "choice": choice, "event": event, "delivery": "substituted" }).to_string())
    }

    /// Why an `update` of `row` with `args` is refused now, if it is.
    pub(super) fn update_refusal(&self, row: &ModulesRow, args: &Json) -> Option<Json> {
        let update = self.substitute(row)["update"].clone();
        let state = update["state"].as_str()?;
        let key = update["key"].as_str().unwrap_or_default();
        let extension = self.plan.str(row.extension);
        let matches = (self.entries(extension, state).iter())
            .find(|e| e.fields.get(key) == args.get(key))
            .is_some_and(|e| update["when"].is_null() || Self::matches(&e.fields, &update["when"]));
        (!matches).then(|| json!({ "error": update["else"] }))
    }

    /// An `update` of `row` with `args`: the entry changed in place.
    pub(super) fn apply_update(&mut self, row: &ModulesRow, args: &Json) -> Result<(), Json> {
        if let Some(refusal) = self.update_refusal(row, args) {
            return Err(refusal);
        }
        let update = self.substitute(row)["update"].clone();
        let state = update["state"].as_str().unwrap_or_default().to_owned();
        let key = update["key"].as_str().unwrap_or_default().to_owned();
        let extension = self.plan.str(row.extension).to_owned();
        let into = self.list_decl(&extension, &state)["due"]["into"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let now = self.epoch();
        let given = args.as_object().cloned().unwrap_or_default();
        self.with_list(&extension, &state, |entries| {
            if let Some(e) = entries
                .iter_mut()
                .find(|e| e.fields.get(&key) == args.get(&key))
            {
                if let Some(left) = update["pause"].as_str() {
                    let due = e.fields.get(&into).and_then(Json::as_f64).unwrap_or(now);
                    e.fields.insert(left.into(), ms((due - now).max(0.0)));
                    e.fields.insert(into.clone(), Json::Null);
                }
                if let Some(left) = update["resume"].as_str() {
                    let remaining = e.fields.get(left).and_then(Json::as_f64).unwrap_or(0.0);
                    e.fields.insert(into.clone(), ms(now + remaining));
                    e.fields.insert(left.into(), Json::Null);
                }
                Self::set_fields(&mut e.fields, &update["set"], &given, now);
            }
            ((), true)
        });
        self.log(format!("{}: updated", self.plan.str(row.declared)));
        self.announce_listed(&state);
        Ok(())
    }
}

/// An instant or a length, in whole milliseconds as the app writes them.
fn ms(v: f64) -> Json {
    json!(v.round() as i64)
}
