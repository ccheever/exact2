//! A send that answers nothing (LLP 1133.004 D5) and the forms of the
//! agent's substitute a module declares for it (§7), so a third party's
//! module is substituted as the standard library's is:
//!
//! - **Admission**, per name: `asks` holds a call under a `permission`
//!   ticket while `prompt` and refuses it while `denied`; `request` asks
//!   while `prompt` and answers the status in every other state; `none` is
//!   never held or refused (`modules.rs`, `decide`).
//! - **A keyed list** in the agent's state: `"list": {"state", "key", "at",
//!   "launch", "tap"}` on a send adds an entry (its options as written, a
//!   `ref`, and `delivered`), replacing the one with its key; `"unlist":
//!   key` takes it away. An entry whose `at` time the agent clock has not
//!   reached is not yet delivered. `tap @ref` on a delivered entry does what
//!   the platform's click would: it opens the entry's `navigate` field, or
//!   raises the declared event as an activation, and retires the entry.
//!   `launch` names the launch flag that hands a relaunched app one entry.
//! - **An app's list** (`"scope": "app"`), with a due time, the person's
//!   choices and `update` (LLP 1133.030 D12 item 2): `app_list.rs`.
//! - **Checks** every route holds a call to (`"checks"`, LLP 1133.030 D2):
//!   exactly `one` of some options, an instant option `after` now, options
//!   at least a `min`, options `nonEmpty`, each refused with its `error`.
//! - **A value** in the agent's state: `"set": {"state", "scope", "from",
//!   "absent", "zero"}` sets it from the option `from` (`absent` when the
//!   call leaves it out, `zero` for 0), or to `value`. `"scope": "app"`
//!   keeps it in the host's app-wide state ([`AppState`]), which every
//!   session of the app reads, as two documents of one web app share one
//!   badge.
//!
//! Outside the agent a bound implementation takes the call, and journals
//! its own outcome.

use super::super::{DataSource, Runner};
use super::{error_code, response, CallOrigin, Json, ModulesRow, Pending, Waits};
use crate::request::{Dispatch, Outcome, Reply, Work};
use crate::HoldAnswer;
use exact_plan::Value;
use serde_json::{json, Map};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

mod app_list;
mod reach;
mod wake_list;

/// The host's app-wide state: one per app, shared by its sessions.
pub type AppState = Arc<Mutex<BTreeMap<String, Json>>>;

#[derive(Default)]
pub(in crate::runner) struct Declared {
    /// Each keyed list, by its state name, oldest first.
    lists: BTreeMap<String, Vec<Listed>>,
    /// The session's values, by state name.
    values: BTreeMap<String, Json>,
    /// The app's values; a session the host shares none with keeps its own.
    app: Option<AppState>,
    /// Effects held on a permission ticket.
    held: Vec<HeldEffect>,
    /// What a bound implementation's refusals said, for the journal.
    lines: Arc<Mutex<Vec<String>>>,
    /// What a registration a wake runs for keeps beside its options, by `ref`.
    wake_meta: BTreeMap<u64, wake_list::Meta>,
    /// The shortcut list and the content index (`reach.rs`).
    reach: reach::Reach,
    /// Each app-scoped list's revision this session last heard, by its key.
    seen: BTreeMap<String, u64>,
}

/// One entry of a keyed list.
#[derive(Clone)]
struct Listed {
    reference: u64,
    /// The send that listed it, by its source's name in the plan.
    source: String,
    /// Its options as the call wrote them, `null` for one left out.
    fields: Map<String, Json>,
}

struct HeldEffect {
    hold: u64,
    source: String,
    args: Json,
    /// The permission its ticket asks: its own, or one its own requires.
    permission: String,
    origin: CallOrigin,
    /// A data-code call's reply, settled when the ticket is answered.
    reply: Option<Arc<Mutex<Pending>>>,
}

/// What a call of a send that answers nothing did now.
enum Effect {
    /// Done, or refused with `{"error": …}`.
    Now(Json),
    /// Held on the permission ticket.
    Held,
}

impl<D: DataSource> Runner<D> {
    /// Share the host's app-wide state with this session (§7: `"scope":
    /// "app"`), before anything sets it.
    pub fn share_app_state(&mut self, state: AppState) {
        self.extensions.declared.app = Some(state);
    }

    fn app_state(&mut self) -> AppState {
        self.extensions
            .declared
            .app
            .get_or_insert_with(Default::default)
            .clone()
    }

    /// Whether `row` is a send that answers nothing (`"answers": null`).
    pub(in crate::runner) fn is_effect(&self, row: &ModulesRow) -> bool {
        row.send && self.substitute(row)["effect"] == true
    }

    /// The commands of this commit from `first` that are a module's sends
    /// answering nothing: taken off, to run once the commit stands.
    pub(in crate::runner) fn take_module_effects(
        &mut self,
        first: usize,
    ) -> Vec<(ModulesRow, Vec<Value>)> {
        let mut out = Vec::new();
        let mut i = first;
        while i < self.commands.len() {
            let row = self.plan.module_named(&self.commands[i].name).cloned();
            match row.filter(|r| self.is_effect(r)) {
                Some(row) => out.push((row, self.commands.remove(i).args)),
                None => i += 1,
            }
        }
        out
    }

    /// Run the effects a commit stated, in order, once it stands.
    pub(in crate::runner) fn run_module_effects(&mut self, effects: Vec<(ModulesRow, Vec<Value>)>) {
        let origin = self.call_origin();
        for (row, args) in effects {
            let mut args = self.args_json(&row, &args);
            self.clamp_bounds(&row, &mut args);
            let _ = self.module_effect(&row, args, None, origin);
        }
    }

    /// One call of a send that answers nothing, from Contract (`reply`
    /// none) or data code.
    fn module_effect(
        &mut self,
        row: &ModulesRow,
        args: Json,
        reply: Option<Arc<Mutex<Pending>>>,
        origin: CallOrigin,
    ) -> Effect {
        let declared = self.plan.str(row.declared).to_owned();
        // The grants, origin and foreground every call is held to (LLP
        // 1133.002 D14), before anything else; a grant matched by value
        // fails with its own code.
        if let Err((code, why)) = self.value_grants(row, &args) {
            self.log(format!("{}: refused, {why}", self.key(row)));
            return Effect::Now(json!({ "error": code, "message": why }));
        }
        if let Err(why) = self.admit(row, &args, origin) {
            self.log(format!("{}: refused, {why}", self.key(row)));
            let message = why
                .starts_with("person-required")
                .then_some("person-required");
            return Effect::Now(json!({ "error": "NotAllowedError", "message": message }));
        }
        if let Some(refusal) = self.effect_refusal(row, &args) {
            self.log(format!(
                "{declared}: refused: {}",
                refusal["error"].as_str().unwrap_or_default()
            ));
            return Effect::Now(refusal);
        }
        if self.route(row) == super::Route::Real {
            let request = self.as_origin(origin, |r| r.bound_request(row, args, None));
            let lines = self.extensions.declared.lines.clone();
            let slot = self.native_slot();
            if let Some(handler) = slot.handler() {
                handler(
                    request.body,
                    Reply::new(move |outcome| {
                        let refused = match &outcome {
                            Outcome::Response(r) => serde_json::from_slice::<Json>(&r.body)
                                .ok()
                                .and_then(|v| error_code(&v).map(str::to_owned)),
                            Outcome::Failed { message, .. } => Some(message.clone()),
                            _ => None,
                        };
                        if let Some(why) = refused {
                            let mut lines = lines.lock().unwrap_or_else(|e| e.into_inner());
                            lines.push(format!("{declared}: refused: {why}"));
                            drop(lines);
                            slot.changed("");
                        }
                    }),
                );
            }
            return Effect::Now(Json::Null);
        }
        let said = self.say_person_required(row, true, origin);
        // The permission that settles the call: its own, or one it requires.
        let sub = self.substitute_for(row, &args);
        let permission = &sub["permission"];
        let state = (permission["name"].as_str())
            .filter(|_| sub["admission"] != "none")
            .map(|name| self.permission_state(name));
        let denied = json!({ "error": permission["denied"] });
        match state.as_deref() {
            Some("denied") => {
                self.log(format!(
                    "{declared}: refused: {}",
                    permission["denied"].as_str().unwrap_or_default()
                ));
                Effect::Now(denied)
            }
            Some("prompt") if self.asks(row, true) && said => Effect::Now(denied),
            Some("prompt") if self.asks(row, true) => {
                let source = self
                    .plan
                    .str(self.plan.sources[row.source.0 as usize].name)
                    .to_owned();
                let permission = self.permission_name(row, &args);
                let hold =
                    self.device_hold(row, declared, Waits::Permission, origin, &permission, &args);
                self.extensions.declared.held.push(HeldEffect {
                    hold,
                    source,
                    args,
                    permission,
                    origin,
                    reply,
                });
                Effect::Held
            }
            _ => match self.apply_effect(row, &args) {
                Ok(()) => Effect::Now(Json::Null),
                Err(refusal) => {
                    let code = refusal["error"].as_str().unwrap_or_default();
                    self.log(format!("{declared}: refused: {code}"));
                    Effect::Now(refusal)
                }
            },
        }
    }

    /// Why a call of `row` with `args` is refused before its permission:
    /// a target the name does not exist on (under the agent), a declared
    /// check, or an `update` with nothing in the state it changes.
    fn effect_refusal(&self, row: &ModulesRow, args: &Json) -> Option<Json> {
        let real = self.route(row) == super::Route::Real;
        if !real && self.unsupported_here(row) {
            return Some(json!({ "error": "NotSupportedError" }));
        }
        let now = self.time.epoch_at_zero + self.now_ms;
        let sub = self.substitute(row);
        for check in sub["checks"].as_array().into_iter().flatten() {
            let given = |o: &Json| o.as_str().map(|o| &args[o]).filter(|v| !v.is_null());
            let names = |k: &str| check[k].as_array().cloned().unwrap_or_default();
            let fails = if check["one"].is_array() {
                names("one").iter().filter(|o| given(o).is_some()).count() != 1
            } else if check["after"].is_string() {
                given(&check["after"])
                    .and_then(Json::as_f64)
                    .is_some_and(|t| t <= now)
            } else if let Some(min) = check["min"].as_object() {
                (min.iter()).any(|(o, m)| {
                    let v = args[o.as_str()].as_f64();
                    v.zip(m.as_f64()).is_some_and(|(v, m)| v < m)
                })
            } else {
                (names("nonEmpty").iter())
                    .any(|o| given(o).and_then(Json::as_str).is_none_or(str::is_empty))
            };
            if fails {
                return Some(json!({ "error": check["error"] }));
            }
        }
        if real {
            return None;
        }
        self.update_refusal(row, args)
    }

    /// A data-code call of a send that answers nothing, substituted: it
    /// settles once the effect is done or refused.
    pub(super) fn effect_call(
        &mut self,
        row: &ModulesRow,
        args: Json,
        origin: CallOrigin,
    ) -> Dispatch {
        let slot = Arc::new(Mutex::new(Pending::default()));
        if let Effect::Now(answer) = self.module_effect(row, args, Some(slot.clone()), origin) {
            return Dispatch::Run(Work::Now(Box::new(move || response(&answer))));
        }
        Dispatch::Run(Work::Later(Box::new(move |reply| {
            let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
            match s.outcome.take() {
                Some(outcome) => reply.send(outcome),
                None => s.reply = Some(reply),
            }
        })))
    }

    /// What `row`'s declaration says its call does to the agent's state: a
    /// refusal where an `update` finds nothing in the state it changes.
    fn apply_effect(&mut self, row: &ModulesRow, args: &Json) -> Result<(), Json> {
        if let Some(answer) = self.apply_reach(row, args) {
            return if answer.is_null() {
                Ok(())
            } else {
                Err(answer)
            };
        }
        let sub = self.substitute(row);
        let declared = self.plan.str(row.declared).to_owned();
        let extension = self.plan.str(row.extension).to_owned();
        let mut fields = args.as_object().cloned().unwrap_or_default();
        fields.values_mut().for_each(whole);
        if sub["update"].is_object() {
            self.apply_update(row, args)?;
        }
        if let Some(list) = sub["list"].as_object() {
            let state = list["state"].as_str().unwrap_or("listed").to_owned();
            let key = list["key"].as_str().unwrap_or_default().to_owned();
            let value = fields.get(&key).cloned().unwrap_or(Json::Null);
            // A registration's kind keys it beside its key (LLP 1133.017 §10).
            let kind = list.get("kind").cloned();
            if let Some(kind) = &kind {
                fields.insert("kind".into(), kind.clone());
            }
            self.due_fields(&sub["list"], &mut fields);
            let reference = if list.get("scope").and_then(Json::as_str) == Some("app") {
                self.app_reference()
            } else {
                self.next_ticket += 1;
                self.next_ticket - 1
            };
            let source = self
                .plan
                .str(self.plan.sources[row.source.0 as usize].name)
                .to_owned();
            let entry = Listed {
                reference,
                source,
                fields: fields.clone(),
            };
            let replaced = self.with_list(&extension, &state, |entries| {
                let same = |e: &Listed| {
                    e.fields.get(&key) == Some(&value) && e.fields.get("kind") == kind.as_ref()
                };
                let replaced = entries.iter().find(|e| same(e)).map(|e| e.reference);
                if !value.is_null() {
                    entries.retain(|e| !same(e));
                }
                entries.push(entry);
                (replaced, true)
            });
            let named = value
                .as_str()
                .map(|v| format!(" ({v})"))
                .unwrap_or_default();
            self.log(format!("{declared}: listed{named} @{reference}"));
            if kind.is_some() {
                self.registered(reference, replaced.filter(|_| !value.is_null()));
            }
            self.announce_listed(&state);
        }
        // `"unlist": key`, or `{"key", "kind"}` for the entries of one kind.
        let (unlist, unkind) = match &sub["unlist"] {
            Json::Object(u) => (u.get("key").and_then(Json::as_str), u.get("kind")),
            u => (u.as_str(), None),
        };
        if let Some(key) = unlist {
            let value = fields.get(key).cloned().unwrap_or(Json::Null);
            let lists = self.lists_of(row);
            for state in lists {
                self.with_list(&extension, &state, |entries| {
                    let before = entries.len();
                    entries.retain(|e| {
                        e.fields.get(key) != Some(&value)
                            || unkind.is_some_and(|k| e.fields.get("kind") != Some(k))
                    });
                    ((), entries.len() != before)
                });
                self.announce_listed(&state);
            }
            self.log(format!("{declared}: unlisted"));
        }
        if let Some(set) = sub["set"].as_object() {
            let state = set["state"].as_str().unwrap_or("value").to_owned();
            let value = match set.get("from").and_then(Json::as_str) {
                Some(from) => match fields.get(from) {
                    None | Some(Json::Null) => set.get("absent").cloned().unwrap_or(Json::Null),
                    Some(n) if n.as_f64() == Some(0.0) => {
                        set.get("zero").cloned().unwrap_or(Json::Null)
                    }
                    Some(n) => n.clone(),
                },
                None => set.get("value").cloned().unwrap_or(Json::Null),
            };
            let key = self.value_key(row, &state);
            if set.get("scope").and_then(Json::as_str) == Some("app") {
                let app = self.app_state();
                app.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(key, value.clone());
            } else {
                self.extensions.declared.values.insert(key, value.clone());
            }
            self.log(format!("{declared}: {state} {value}"));
        }
        Ok(())
    }

    /// Where a declared value is kept: by the app, the module and its state
    /// name, so two modules' `count`s, or two apps an embedder hosts, never
    /// share one.
    fn value_key(&self, row: &ModulesRow, state: &str) -> String {
        format!(
            "{}/{}/{state}",
            self.data.app_id(),
            self.plan.str(row.extension)
        )
    }

    /// The list states the sends of `row`'s extension list into.
    fn lists_of(&self, row: &ModulesRow) -> Vec<String> {
        let extension = self.plan.str(row.extension);
        (self.plan.modules.iter())
            .filter(|m| self.plan.str(m.extension) == extension)
            .filter_map(|m| {
                self.substitute(m)["list"]["state"]
                    .as_str()
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The agent answered `hold`: an effect held on it is done or refused.
    /// False when the hold is no effect's.
    pub(super) fn effect_answered(&mut self, hold: u64, answer: &HoldAnswer) -> bool {
        let Some(h) = self
            .extensions
            .declared
            .held
            .iter()
            .find(|h| h.hold == hold)
        else {
            return false;
        };
        let row = self.module_row(&h.source);
        let name = h.permission.clone();
        match answer {
            HoldAnswer::Choice(c) if c == "allow" => self.set_permission(&name, "granted"),
            HoldAnswer::Choice(c) if c == "deny" => self.set_permission(&name, "denied"),
            _ => {
                let at = self
                    .extensions
                    .declared
                    .held
                    .iter()
                    .position(|h| h.hold == hold);
                let h = self
                    .extensions
                    .declared
                    .held
                    .remove(at.expect("found above"));
                let code = self.permission_named(&row, &h.permission)["denied"].clone();
                self.finish_effect(h, json!({ "error": code }));
            }
        }
        true
    }

    /// Permission `name` became `state`: its held effects are done
    /// (`granted`) or refused (`denied`). One whose own permission requires
    /// another asks the next still `prompt` first (LLP 1133.030 D12 item 1).
    pub(super) fn settle_effects(&mut self, name: &str, state: &str) {
        if state == "prompt" {
            return;
        }
        let (mine, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.extensions.declared.held)
            .into_iter()
            .partition(|h| h.permission == name);
        self.extensions.declared.held = keep;
        for mut h in mine {
            self.retire_hold(h.hold);
            let row = self.module_row(&h.source);
            let next = self.substitute_for(&row, &h.args)["permission"].clone();
            let next_name = next["name"].as_str().unwrap_or_default().to_owned();
            let answer = match (state, self.permission_state(&next_name).as_str()) {
                ("granted", "prompt") if next_name != name => {
                    let declared = self.plan.str(row.declared).to_owned();
                    h.hold = self.device_hold(
                        &row,
                        declared,
                        Waits::Permission,
                        h.origin,
                        &next_name,
                        &h.args,
                    );
                    h.permission = next_name;
                    self.extensions.declared.held.push(h);
                    continue;
                }
                ("granted", "denied") => json!({ "error": next["denied"] }),
                ("granted", _) => match self.apply_effect(&row, &h.args) {
                    Ok(()) => Json::Null,
                    Err(refusal) => refusal,
                },
                _ => json!({ "error": self.permission_named(&row, name)["denied"] }),
            };
            self.finish_effect(h, answer);
        }
    }

    fn finish_effect(&mut self, h: HeldEffect, answer: Json) {
        if let Some(code) = error_code(&answer) {
            let row = self.module_row(&h.source);
            self.log(format!("{}: refused: {code}", self.plan.str(row.declared)));
        }
        if let Some(slot) = h.reply {
            Pending::complete(&slot, response(&answer));
        }
    }

    /// A bound implementation's refusals since the last call, journaled.
    pub(in crate::runner) fn journal_effect_lines(&mut self) {
        let lines = std::mem::take(
            &mut *self
                .extensions
                .declared
                .lines
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        for line in lines {
            self.log(line);
        }
    }

    /// Each list and value the plan's modules declare, by its state name,
    /// as the agent's state shows it: `"notifications": [{…, "ref": 7,
    /// "delivered": true}]`, `"badge": 3`. Empty for a plan that declares
    /// none.
    pub fn declared_state(&self) -> Vec<(String, Json)> {
        let mut out: BTreeMap<String, Json> = BTreeMap::new();
        let app = self.extensions.declared.app.clone();
        for row in &self.plan.modules {
            let sub = self.substitute(row);
            if let Some(state) = sub["list"]["state"].as_str() {
                let extension = self.plan.str(row.extension);
                let entries = (self.entries(extension, state).iter())
                    .map(|e| {
                        let mut v = e.fields.clone();
                        v.insert("delivered".into(), json!(self.delivered(e)));
                        v.insert("ref".into(), json!(e.reference));
                        self.wake_fields(e, &mut v);
                        Json::Object(v)
                    })
                    .collect();
                out.insert(state.to_owned(), Json::Array(entries));
            }
            if let Some(state) = sub["set"]["state"].as_str() {
                let key = self.value_key(row, state);
                let value = if sub["set"]["scope"] == "app" {
                    (app.as_ref()).and_then(|a| {
                        a.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .get(&key)
                            .cloned()
                    })
                } else {
                    self.extensions.declared.values.get(&key).cloned()
                };
                out.insert(state.to_owned(), value.unwrap_or(Json::Null));
            }
        }
        // The app's tools and the system's calls of them (LLP 1133.023 §10).
        if let Some(intents) = self.intents_state() {
            out.insert("intents".into(), intents);
        }
        out.into_iter().collect()
    }

    /// Whether the agent clock has reached an entry's declared `at` time;
    /// for a list with a due time, whether it has fired (`app_list.rs`).
    fn delivered(&self, e: &Listed) -> bool {
        if let Some(fired) = e.fields.get("delivered").and_then(Json::as_bool) {
            return fired;
        }
        let row = self.module_row(&e.source);
        let at = self.substitute(&row)["list"]["at"]
            .as_str()
            .map(str::to_owned);
        let now = self.time.epoch_at_zero + self.now_ms;
        match at.and_then(|f| e.fields.get(&f).and_then(Json::as_f64)) {
            Some(t) => t <= now,
            None => true,
        }
    }

    /// [`Self::declared_state`] as the agent's state writes it: `,"name":value` each.
    pub fn declared_state_json(&self, s: &mut String) {
        for (name, value) in self.declared_state() {
            s.push(',');
            s.push_str(&Json::String(name).to_string());
            s.push(':');
            s.push_str(&value.to_string());
        }
    }

    /// The agent's operations on listed entries (§7): `{"op":"tap","ticket":ref}`
    /// on one, or `{"op":"launchEntry","launch","entry"}`. `None` for any other.
    pub fn listed_op(&mut self, op: &str, request: &str) -> Option<Result<String, String>> {
        let v: Json = serde_json::from_str(request).ok()?;
        match op {
            "launchEntry" if v["launch"] == "wake" => {
                let entry = match &v["entry"] {
                    Json::String(text) => serde_json::from_str(text).unwrap_or(Json::Null),
                    entry => entry.clone(),
                };
                Some(if entry.is_object() {
                    self.launch_wake(&entry)
                } else {
                    Err("--wake: the entry is a JSON object, as `state` printed it".into())
                })
            }
            // A cold call is the calls' (`calls.rs`).
            "launchEntry" if v["launch"] == "call" => None,
            "launchEntry" => {
                let entry = v["entry"]
                    .as_str()
                    .map_or_else(|| v["entry"].to_string(), str::to_owned);
                Some(self.launch_listed(v["launch"].as_str().unwrap_or_default(), &entry))
            }
            "tap" => {
                let reference = v["ticket"].as_u64()?;
                // A ticket this session holds is its own (an app's list
                // counts its refs across sessions).
                if self.device_holds().iter().any(|h| h.ticket == reference) {
                    return None;
                }
                let choice = v["choice"].as_str();
                self.is_listed(reference)
                    .then(|| self.tap_listed_as(reference, choice))
            }
            _ => None,
        }
    }

    /// Whether `reference` is a listed entry's `ref`.
    pub fn is_listed(&self, reference: u64) -> bool {
        self.listed_at(reference).is_some()
    }

    /// The extension and list state of the entry `reference`, in this
    /// session's lists or the app's.
    fn listed_at(&self, reference: u64) -> Option<(String, String)> {
        let mut lists: Vec<(String, String)> = Vec::new();
        for row in &self.plan.modules {
            if let Some(state) = self.substitute(row)["list"]["state"].as_str() {
                let at = (self.plan.str(row.extension).to_owned(), state.to_owned());
                if !lists.contains(&at) {
                    lists.push(at);
                }
            }
        }
        (lists.into_iter()).find(|(extension, state)| {
            (self.entries(extension, state).iter()).any(|e| e.reference == reference)
        })
    }

    /// `tap @ref` on a listed entry (§7): what the platform's click would
    /// do. `Ok` is the reply: `{"navigate": location}` for the host to open,
    /// or the event raised; the entry is retired either way.
    pub fn tap_listed(&mut self, reference: u64) -> Result<String, String> {
        self.tap_listed_as(reference, None)
    }

    /// [`Self::tap_listed`] with the person's choice, for an entry whose
    /// list declares choices (LLP 1133.030 D12 item 2).
    pub fn tap_listed_as(
        &mut self,
        reference: u64,
        choice: Option<&str>,
    ) -> Result<String, String> {
        if let Some((extension, state)) = self.listed_at(reference) {
            if self.list_decl(&extension, &state)["tap"]["choices"].is_object() {
                return self.tap_choice(&extension, &state, reference, choice);
            }
        }
        let Some((state, at)) = (self.extensions.declared.lists.iter()).find_map(|(s, l)| {
            l.iter()
                .position(|e| e.reference == reference)
                .map(|i| (s.clone(), i))
        }) else {
            return Err(format!("@{reference} is no listed entry"));
        };
        let entry = &self.extensions.declared.lists[&state][at];
        if !self.delivered(entry) {
            return Err(format!(
                "@{reference} is not delivered yet: advance the clock past its time"
            ));
        }
        // A registration's tap is a wake, and leaves it listed.
        let row = self.module_row(&entry.source);
        if self.substitute(&row)["list"]["tap"]["as"] == "wake" {
            return self.tap_wake(&state, at);
        }
        let entry = self
            .extensions
            .declared
            .lists
            .get_mut(&state)
            .expect("found above")
            .remove(at);
        let row = self.module_row(&entry.source);
        self.log(format!("{state} @{reference}: tapped"));
        self.activate_entry(&row, &Json::Object(entry.fields), reference)
    }

    /// A relaunched app's launch flag (`--notification '<entry>'`): the
    /// entry as the agent's state printed it, delivered as its tap would be
    /// (LLP 1133.004 §7), held until the session can hear it.
    pub fn launch_listed(&mut self, launch: &str, entry: &str) -> Result<String, String> {
        let entry: Json =
            serde_json::from_str(entry).map_err(|e| format!("--{launch}: not JSON: {e}"))?;
        let Some(row) = (self.plan.modules.iter())
            .find(|m| self.substitute(m)["list"]["launch"].as_str() == Some(launch))
            .cloned()
        else {
            return Err(format!(
                "--{launch}: no module of this app lists entries a launch hands over"
            ));
        };
        self.log(format!("launch --{launch}"));
        self.activate_entry(&row, &entry, 0)
    }

    /// An entry's click: its `navigate` field opened, else its event raised.
    fn activate_entry(
        &mut self,
        row: &ModulesRow,
        entry: &Json,
        reference: u64,
    ) -> Result<String, String> {
        let tap = self.substitute(row)["list"]["tap"].clone();
        if let Some(location) = (tap["navigate"].as_str())
            .and_then(|f| entry.get(f))
            .and_then(Json::as_str)
        {
            return Ok(
                json!({ "ref": reference, "navigate": location, "delivery": "substituted" })
                    .to_string(),
            );
        }
        let Some(event) = tap["event"].as_str() else {
            return Ok(json!({ "ref": reference, "delivery": "substituted" }).to_string());
        };
        let args = entry_args(&tap, entry);
        let extension = self.plan.str(row.extension).to_owned();
        self.raise_module_event(
            &extension,
            event,
            &Json::Array(args).to_string(),
            CallOrigin::App,
        )?;
        Ok(json!({ "ref": reference, "event": event, "delivery": "substituted" }).to_string())
    }
}

/// The event's arguments a listed entry's tap declares: each a field of the
/// entry, an object of its fields, or a literal.
fn entry_args(tap: &Json, entry: &Json) -> Vec<Json> {
    (tap["args"].as_array().into_iter().flatten())
        .map(|a| match a {
            Json::String(field) => entry.get(field).cloned().unwrap_or(Json::Null),
            Json::Object(map) => Json::Object(
                map.iter()
                    .map(|(k, f)| {
                        let v = f.as_str().and_then(|f| entry.get(f)).cloned();
                        (k.clone(), v.unwrap_or(Json::Null))
                    })
                    .collect(),
            ),
            other => other.clone(),
        })
        .collect()
}

/// A number the plan holds as a float, written as JSON writes a whole
/// number: `3`, not `3.0`, as the app passed it.
fn whole(value: &mut Json) {
    match value {
        Json::Number(n) => {
            if let Some(f) = n.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 9.0e15) {
                *value = json!(f as i64);
            }
        }
        Json::Object(map) => map.values_mut().for_each(whole),
        Json::Array(items) => items.iter_mut().for_each(whole),
        _ => {}
    }
}
