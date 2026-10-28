//! Permission states and what setting one does (module doc, **Permission**).
//! An interface may declare several permissions (LLP 1133.019 D4: reading
//! the library and adding to it): one status source answers each one's
//! state in its own `field`, a permission another's grant includes
//! (`grantedBy`) reads `granted` while that one does, and a permission's
//! `choices` are answers besides `allow` and `deny` (`limited`, D11 item 3),
//! each setting the status fields it names. A name that shows the person a
//! choice (`choose`, §10) is held under its own ticket, whose `type @t`
//! answer becomes the agent word it names.
//!
//! A sheet (`choose`, LLP 1133.018 D13 item 4) may also take taps besides
//! `cancel` (`choices`), refuse a typed answer (`typed: false`), and hold
//! several typed ids only where an option allows them (`many`); its answer
//! reaches the implementation as the call's `preferred`, `{"tap": choice}`
//! or `{"typed": [ids]}`. A request that names what it asks (`asks`, D13
//! item 3: the grant lines, narrowest first, that satisfy the call) is
//! granted what it asked: `allow` sets the status's fields to the values of
//! the first of those lines the app declares, where otherwise the longest
//! line the app grants wins.

use super::super::DataSource;
use super::{Decision, Json, ModulesRow, Route, Runner, Waits};
use crate::HoldAnswer;
use serde_json::{json, Map};

impl<D: DataSource> Runner<D> {
    /// The permissions whose state `row`'s source answers (its `status`):
    /// every one of its interface's that names it.
    pub(super) fn statuses_of(&self, row: &ModulesRow) -> Vec<Json> {
        let sub = self.substitute(row);
        let declared = self.plan.str(row.declared);
        let mut out: Vec<Json> = (sub["permissionList"].as_array().into_iter().flatten())
            .filter(|p| p["status"] == declared)
            .cloned()
            .collect();
        if out.is_empty() && sub["permission"]["status"] == declared {
            out.push(sub["permission"].clone());
        }
        out
    }

    /// A `choose` ticket answered: a `type @t` answer (a comma list, or a
    /// JSON array) becomes the agent word the ticket names, if it names one,
    /// which the implementation is told; a tap leaves it. Either way the
    /// implementation then answers the call, where one is bound, told the
    /// answer as the call's `preferred`: `{"typed": [ids]}` or `{"tap": c}`.
    pub(super) fn chosen(&mut self, row: &ModulesRow, token: u64, answer: &HoldAnswer) {
        let sheet = match answer {
            HoldAnswer::Value(text) => json!({ "typed": typed_list(text) }),
            HoldAnswer::Choice(c) => json!({ "tap": c }),
        };
        let sub = self.substitute(row);
        if let (HoldAnswer::Value(_), Some(word)) = (answer, sub["choose"]["word"].as_str()) {
            let extension = self.plan.str(row.extension).to_owned();
            let key = format!("{extension}.{word}");
            self.extensions
                .preferred
                .insert(key.clone(), sheet["typed"].clone());
            self.log(format!("{key}: chosen by the agent"));
            self.tell_preferred(&extension);
        }
        let held = (self.extensions.held.iter_mut()).find(|h| h.token == token);
        let forward = held.and_then(|h| h.forward.as_mut());
        let forwards = forward.is_some();
        if let Some(body) = forward {
            if let Ok(mut v) = serde_json::from_slice::<Json>(body) {
                v["preferred"] = sheet;
                *body = v.to_string().into_bytes();
            }
        }
        if forwards {
            self.release_forward(token);
        } else {
            // No implementation: the substitute's send answers nothing.
            self.release(token, json!({}));
        }
    }

    /// Permission `name`'s state: what the agent or a ticket set, else the
    /// state its declaration starts in (`initial`: `granted` where every
    /// native platform grants without asking, LLP 1133.013 §9), else
    /// `prompt`.
    /// A permission that another's grant includes (`grantedBy`: Apple's
    /// read-write access includes adding, LLP 1133.019 D5) reads `granted`
    /// while that one does.
    ///
    /// A permission that requires others on this target (`requires`, LLP
    /// 1133.030 D12 item 1) reads the least of its own state and theirs:
    /// `denied`, then `prompt`, then `unknown`, then `granted`.
    pub(super) fn permission_state(&self, name: &str) -> String {
        let declared = self.permission_or_status(name);
        let own = self.own_permission_state(name, &declared);
        (self.required(&declared).iter())
            .filter(|r| *r != name)
            .fold(own, |state, r| least(state, self.permission_state(r)))
    }

    /// Permission `name` as declared, or as a status source names it: a
    /// permission another one requires may be declared only there (LLP
    /// 1133.030 D12 item 1).
    fn permission_or_status(&self, name: &str) -> Json {
        let declared = self.declared_permission(name);
        if !declared.is_null() {
            return declared;
        }
        (self.plan.modules.iter())
            .flat_map(|m| self.statuses_of(m))
            .find(|p| p["name"] == name)
            .unwrap_or_default()
    }

    /// The permissions `permission` requires on this target, in the order
    /// they are asked.
    pub(in crate::runner) fn required(&self, permission: &Json) -> Vec<String> {
        (permission["requires"][self.host_target()]
            .as_array()
            .into_iter())
        .flatten()
        .filter_map(Json::as_str)
        .map(str::to_owned)
        .collect()
    }

    /// A permission that adds to another (`base`, LLP 1133.021 §12:
    /// background location to location) granted grants its base too; its
    /// base set to `prompt` or `denied` sets it the same. Each direction
    /// sets states only toward its own, so the two never loop.
    fn couple_base(&mut self, name: &str, state: &str) {
        if state == "granted" {
            let base = self.declared_permission(name)["base"]
                .as_str()
                .map(str::to_owned);
            if let Some(base) = base.filter(|b| self.permission_state(b) != "granted") {
                self.set_permission(&base, "granted");
            }
            return;
        }
        if !matches!(state, "prompt" | "denied") {
            return;
        }
        let mut dependents: Vec<String> = Vec::new();
        for row in &self.plan.modules {
            let sub = self.substitute(row);
            let list = sub["permissionList"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for p in std::iter::once(sub["permission"].clone()).chain(list) {
                let dependent = p["name"].as_str().filter(|_| p["base"] == name);
                if let Some(d) = dependent.filter(|d| !dependents.iter().any(|x| x == d)) {
                    dependents.push(d.to_owned());
                }
            }
        }
        for d in dependents {
            if self.extensions.permission.get(&d).map(String::as_str) != Some(state) {
                self.set_permission(&d, state);
            }
        }
    }

    /// Permission `name`'s own state, before what it requires.
    pub(super) fn own_permission_state(&self, name: &str, declared: &Json) -> String {
        if let Some(by) = declared["grantedBy"].as_str().filter(|by| *by != name) {
            if self.permission_state(by) == "granted" {
                return "granted".into();
            }
        }
        if let Some(state) = self.extensions.permission.get(name) {
            return state.clone();
        }
        // By target where it differs (`{"android": "prompt", "*": "granted"}`, LLP 1133.037 D17).
        let initial = &declared["initial"];
        (initial.as_str())
            .or(initial[self.host_target()].as_str())
            .or(initial["*"].as_str())
            .map_or_else(|| "prompt".into(), str::to_owned)
    }

    /// A `choose` send's ticket, where its status reads what `while` names
    /// (the selection's sheet opens only under limited access).
    pub(super) fn decide_choose(&self, row: &ModulesRow, send: bool) -> Option<Decision> {
        let sub = self.substitute(row);
        let choose = sub["choose"].as_object().filter(|_| send)?;
        let status = (choose.get("status").and_then(Json::as_str))
            .and_then(|s| self.sibling(row, s))
            .and_then(|s| match self.decide(&s, false, &json!({})) {
                Decision::Now(v) => Some(v),
                _ => None,
            })
            .unwrap_or_default();
        let applies = (choose.get("while").and_then(Json::as_object))
            .is_none_or(|w| w.iter().all(|(k, v)| status.get(k) == Some(v)));
        applies.then_some(Decision::Hold(Waits::Choose))
    }

    /// Set permission `name`'s state: the permission tickets pending for it
    /// settle as `allow` or `deny` would, and every name of its module is
    /// asked again.
    pub(super) fn set_permission(&mut self, name: &str, state: &str) {
        let which = if state == "denied" { "deny" } else { "allow" };
        self.set_permission_as(name, state, which)
    }

    /// [`Self::set_permission`], with the status fields of `which`: `allow`,
    /// `deny`, or one of the permission's `choices`.
    pub(super) fn set_permission_as(&mut self, name: &str, state: &str, which: &str) {
        self.set_permission_asked(name, state, which, None)
    }

    /// [`Self::set_permission_as`], for a request that asked grant line
    /// `asked` (`asks`): the status takes that line's values.
    pub(super) fn set_permission_asked(
        &mut self,
        name: &str,
        state: &str,
        which: &str,
        asked: Option<&str>,
    ) {
        self.extensions
            .permission
            .insert(name.to_owned(), state.to_owned());
        self.log(format!("permission {name}: {state}"));
        self.couple_base(name, state);
        let rows: Vec<ModulesRow> = (self.plan.modules.iter())
            .filter(|m| {
                self.asks_permission(m, name)
                    || self.statuses_of(m).iter().any(|p| p["name"] == name)
                    || self.lists_permission(m, name)
            })
            .cloned()
            .collect();
        if state != "prompt" {
            self.status_fields(&rows, name, which, asked);
        }
        // The answerless sends held on it are done or refused (LLP 1133.004 §7).
        self.settle_effects(name, state);
        // An implementation bound under the agent hears the states and the
        // status it now answers beside (LLP 1133.019 §10: a picture of a
        // photo the person took back is revoked).
        let told: std::collections::BTreeSet<String> = (rows.iter())
            .filter(|r| self.route(r) == Route::Agent)
            .map(|r| self.plan.str(r.extension).to_owned())
            .collect();
        for extension in &told {
            self.tell_preferred(extension);
        }
        type Waiting = (u64, u64, String, bool, Waits, Option<f64>, Json);
        let held: Vec<Waiting> = (self.extensions.held.iter())
            .filter(|h| h.permission == name)
            .map(|h| {
                (
                    h.token,
                    h.hold,
                    h.source.clone(),
                    h.send,
                    h.waits,
                    h.timeout,
                    h.args.clone(),
                )
            })
            .collect();
        for (token, hold, source, send, waits, timeout, args) in held {
            let row = self.module_row(&source);
            if !rows.iter().any(|r| r.source == row.source) {
                continue;
            }
            let sub = self.substitute_for(&row, &args);
            // A request that answers the status answers a denial with it.
            let fails = self.denial_fails(&sub, &args) && !(send && sub["deniedAnswers"] == true);
            // `unknown` (an answer the platform hides, `keyed.rs`) admits as
            // `granted` does, except where the host keeps the fact and
            // `unknown` means a reach may ask (`network.rs`).
            let host_fact = self.permission_named(&row, name)["fact"] == "host";
            let admits = state == "granted" || state == "unknown" && !host_fact;
            match (state, waits) {
                ("granted" | "unknown" | "denied", Waits::Permission) if admits || !fails => {
                    self.retire_hold(hold);
                    // A call that needs another permission too asks it next
                    // (`getUserMedia(audio=true, video=true)`, LLP 1133.009 D3).
                    if admits && self.ask_next(&row, send, token, name, &args) {
                        continue;
                    }
                    let forward = (self.extensions.held.iter())
                        .any(|h| h.token == token && h.forward.is_some());
                    if forward {
                        self.release_forward(token);
                        continue;
                    }
                    match self.decide_timed(&row, send, timeout, &args) {
                        Decision::Now(answer) => {
                            let answer = self.listed_or(&row, &args, answer);
                            self.release(token, answer)
                        }
                        Decision::Forward => self.release_forward(token),
                        Decision::Hold(waits) => {
                            if let Some((held_by, origin)) = self.held_name(token, &row) {
                                let hold =
                                    self.device_hold(&row, held_by, waits, origin, name, &args);
                                let deadline = timeout
                                    .filter(|_| waits == Waits::Value)
                                    .map(|t| self.now_ms + t);
                                if let Some(h) =
                                    self.extensions.held.iter_mut().find(|h| h.token == token)
                                {
                                    h.hold = hold;
                                    h.waits = waits;
                                    h.deadline = deadline;
                                }
                            }
                        }
                    }
                }
                ("denied", _) if fails => {
                    self.retire_hold(hold);
                    let code = self.permission_named(&row, name)["denied"].clone();
                    self.release(token, json!({ "error": code }));
                }
                _ => {}
            }
        }
        for row in &rows {
            let key = self.key(row);
            self.announce(&key);
        }
        self.pump_presence();
    }

    /// Permission `name`'s answer `which` sets the status fields it names,
    /// as a preferred status: `allow` (granted), a choice of its `choices`
    /// (`limited`, LLP 1133.019 D11 item 3), or `deny`. A field's value may
    /// be by the app's grant line (its approximate grant reads
    /// `approximate`); `asked` names the line a request asked for.
    pub(super) fn status_fields(
        &mut self,
        rows: &[ModulesRow],
        name: &str,
        which: &str,
        asked: Option<&str>,
    ) {
        for row in rows {
            let Some(permission) = self
                .statuses_of(row)
                .into_iter()
                .find(|p| p["name"] == name)
            else {
                continue;
            };
            let Some(fields) = self.answer_fields(&permission, which, asked) else {
                continue;
            };
            let mut status = self.value_of(row).unwrap_or(json!({}));
            // The states are the permissions', never a preferred value's.
            for p in self.statuses_of(row) {
                if let (Some(object), Some(field)) = (status.as_object_mut(), p["field"].as_str()) {
                    object.remove(field);
                }
            }
            if let Some(object) = status.as_object_mut() {
                object.extend(fields);
            }
            let key = self.key(row);
            self.extensions.preferred.insert(key, status);
        }
    }

    /// The status fields permission answer `which` sets, each by the app's
    /// grant line where it is given per line: the line `asked` where a
    /// request asked one; else, for a permission whose names say what they
    /// ask, the widest of the permission's own lines the app declares (the
    /// last, as they are listed narrowest first: a grant given at launch, or
    /// by `prefer permission`, is the most the app could have asked); else
    /// the longest (the approximate one where both are). A target's own
    /// `allowOn` replaces `allow` (Android grants a calendar ask of either
    /// level in full, LLP 1133.018 D11), and `denyOn` `deny` (the platform's
    /// own word for a denial, LLP 1133.030 D5). `None` where it sets none.
    pub(super) fn answer_fields(
        &self,
        permission: &Json,
        which: &str,
        asked: Option<&str>,
    ) -> Option<Map<String, Json>> {
        let fields = match which {
            "allow" => match &permission["allowOn"][self.host_target()] {
                Json::Null => &permission["allow"],
                on => on,
            },
            "deny" => match &permission["denyOn"][self.host_target()] {
                Json::Null => &permission["deny"],
                on => on,
            },
            choice => &permission["choices"][choice],
        };
        let fields = fields.as_object()?;
        let name = permission["name"].as_str().unwrap_or_default();
        let widest = match self.asks_declared(name) {
            true => (permission["grants"].as_array().into_iter().flatten())
                .filter_map(Json::as_str)
                .filter(|l| self.declares(l))
                .last(),
            false => None,
        };
        let mut out = Map::new();
        for (field, value) in fields {
            let value = match value.as_object() {
                Some(by_line) => (asked.or(widest))
                    .and_then(|line| by_line.get(line))
                    .cloned()
                    .or_else(|| {
                        (by_line.iter())
                            .filter(|(line, _)| self.declares(line))
                            .max_by_key(|(line, _)| line.len())
                            .map(|(_, v)| v.clone())
                    }),
                None => Some(value.clone()),
            };
            if let Some(v) = value {
                out.insert(field.clone(), v);
            }
        }
        Some(out)
    }

    /// Whether the app declares grant line `line`.
    pub(super) fn declares(&self, line: &str) -> bool {
        (self.data.grants().lines()).any(|g| g.split_whitespace().next() == Some(line))
    }

    /// The grant line a request of `row` asks (`asks`, LLP 1133.018 D13 item
    /// 3): the first of the lines it names that the app declares.
    pub(super) fn asked_line(&self, row: &ModulesRow) -> Option<String> {
        let sub = self.substitute(row);
        (sub["asks"].as_array().into_iter().flatten())
            .filter_map(Json::as_str)
            .find(|l| self.declares(l))
            .map(str::to_owned)
    }

    /// Whether any name that asks permission `name` says what it asks.
    fn asks_declared(&self, name: &str) -> bool {
        (self.plan.modules.iter())
            .any(|m| self.substitute(m)["asks"].is_array() && self.asks_permission(m, name))
    }

    /// A send that answers another source's value under the agent, raising
    /// it while that value matches `raise.when` (LLP 1133.018 D5: write-only
    /// raised to full): held under the raise's ticket, else that value now.
    /// `None` for a send that declares no raise.
    pub(super) fn decide_raise(&self, row: &ModulesRow, send: bool) -> Option<Decision> {
        let sub = self.substitute(row);
        if !send || !sub["raise"].is_object() {
            return None;
        }
        let answers = sub["answers"].as_str().and_then(|a| self.sibling(row, a))?;
        let Decision::Now(value) = self.decide(&answers, false, &json!({})) else {
            return Some(Decision::Now(json!({ "error": "NotSupportedError" })));
        };
        let when = sub["raise"]["when"].as_object();
        let matches = when.is_some_and(|w| w.iter().all(|(k, v)| value.get(k) == Some(v)));
        Some(match matches {
            true => Decision::Hold(Waits::Raise),
            false => Decision::Now(value),
        })
    }

    /// A typed answer to a `choose` sheet of `row`, whose call had `args`,
    /// checked before it is spent: refused where the sheet takes taps alone
    /// (`typed: false`), and more than one id where the call did not set the
    /// option `many` names.
    pub(super) fn choose_typed(
        &self,
        row: &ModulesRow,
        args: &Json,
        text: &str,
    ) -> Result<(), String> {
        let choose = &self.substitute(row)["choose"];
        if choose["typed"] == false {
            return Err("answered by tap, not type".into());
        }
        let n = typed_list(text).as_array().map_or(0, Vec::len);
        if let Some(many) = choose["many"].as_str() {
            if n > 1 && args[many] != true {
                return Err(format!("one id, not {n}: the call did not set `{many}`"));
            }
        }
        Ok(())
    }

    /// A sheet's taps besides `cancel` (LLP 1133.018 D13 item 4), less those
    /// its `unless` rules out while a field of its status holds a value
    /// (LLP 1133.028 §7: Apple's age sheet has no decline where the region
    /// requires the signal), `{"decline": {"status", "field", "has"}}`, the
    /// field a dotted path, a list holding the value or the value itself.
    pub(super) fn sheet_choices(&self, row: &ModulesRow) -> Vec<String> {
        let choose = &self.substitute(row)["choose"];
        let ruled_out = |choice: &str| {
            let rule = &choose["unless"][choice];
            let Some(field) = rule["field"].as_str() else {
                return false;
            };
            let status = (rule["status"].as_str())
                .and_then(|s| self.sibling(row, s))
                .and_then(|s| self.value_of(&s))
                .unwrap_or_default();
            let at = field.split('.').fold(&status, |v, k| &v[k]);
            at == &rule["has"] || at.as_array().is_some_and(|a| a.contains(&rule["has"]))
        };
        (choose["choices"].as_array().into_iter().flatten())
            .filter_map(Json::as_str)
            .filter(|c| !ruled_out(c))
            .map(str::to_owned)
            .collect()
    }

    /// Whether this target is one the name's declaration says it does not
    /// exist on (`unsupported`): it fails `NotSupportedError` there, under
    /// the agent too (LLP 1133.002 D14 step 2).
    pub(super) fn unsupported_here(&self, row: &ModulesRow) -> bool {
        let target = self.host_target();
        (self.substitute(row)["unsupported"]
            .as_array()
            .into_iter()
            .flatten())
        .any(|t| t == target)
    }
}

/// The lesser of two permission states: `denied`, then `prompt`, then
/// `unknown`, then `granted` (and any other word).
fn least(a: String, b: String) -> String {
    let rank = |s: &str| match s {
        "denied" => 0,
        "prompt" => 1,
        "unknown" => 2,
        _ => 3,
    };
    if rank(&b) < rank(&a) {
        b
    } else {
        a
    }
}

/// A typed sheet answer: a JSON array, or a comma list (`c2,c5`).
fn typed_list(text: &str) -> Json {
    let listed: Vec<Json> = (text.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| json!(s))
        .collect();
    serde_json::from_str::<Json>(text.trim())
        .ok()
        .filter(Json::is_array)
        .unwrap_or(Json::Array(listed))
}
