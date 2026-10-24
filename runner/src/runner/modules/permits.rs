//! Permission states and what setting one does (module doc, **Permission**).
//! An interface may declare several permissions (LLP 1133.019 D4: reading
//! the library and adding to it): one status source answers each one's
//! state in its own `field`, a permission another's grant includes
//! (`grantedBy`) reads `granted` while that one does, and a permission's
//! `choices` are answers besides `allow` and `deny` (`limited`, D11 item 3),
//! each setting the status fields it names. A name that shows the person a
//! choice (`choose`, §10) is held under its own ticket, whose `type @t`
//! answer becomes the agent word it names.

use super::super::DataSource;
use super::{Decision, Json, ModulesRow, Route, Runner, Waits};
use crate::HoldAnswer;
use serde_json::json;

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
    /// JSON array) becomes the agent word the ticket names, which the
    /// implementation is told; `cancel` leaves it. Either way the
    /// implementation then answers the call, where one is bound.
    pub(super) fn chosen(&mut self, row: &ModulesRow, token: u64, answer: &HoldAnswer) {
        if let HoldAnswer::Value(text) = answer {
            let sub = self.substitute(row);
            let word = sub["choose"]["word"].as_str().unwrap_or_default();
            let extension = self.plan.str(row.extension).to_owned();
            let key = format!("{extension}.{word}");
            let listed: Vec<Json> = (text.split(','))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| json!(s))
                .collect();
            let value = serde_json::from_str::<Json>(text.trim())
                .ok()
                .filter(Json::is_array)
                .unwrap_or(Json::Array(listed));
            self.extensions.preferred.insert(key.clone(), value);
            self.log(format!("{key}: chosen by the agent"));
            self.tell_preferred(&extension);
        }
        let forwards =
            (self.extensions.held.iter()).any(|h| h.token == token && h.forward.is_some());
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
    pub(super) fn permission_state(&self, name: &str) -> String {
        let declared = (self.plan.modules.iter())
            .flat_map(|m| {
                self.statuses_of(m)
                    .into_iter()
                    .chain([self.permission_named(m, name)])
            })
            .find(|p| p["name"] == name)
            .unwrap_or_default();
        if let Some(by) = declared["grantedBy"].as_str().filter(|by| *by != name) {
            if self.permission_state(by) == "granted" {
                return "granted".into();
            }
        }
        if let Some(state) = self.extensions.permission.get(name) {
            return state.clone();
        }
        (declared["initial"].as_str().map(str::to_owned)).unwrap_or_else(|| "prompt".into())
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
        self.extensions
            .permission
            .insert(name.to_owned(), state.to_owned());
        self.log(format!("permission {name}: {state}"));
        let rows: Vec<ModulesRow> = (self.plan.modules.iter())
            .filter(|m| {
                self.asks_permission(m, name)
                    || self.statuses_of(m).iter().any(|p| p["name"] == name)
            })
            .cloned()
            .collect();
        if state != "prompt" {
            self.status_fields(&rows, name, which);
        }
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
            let fails = self.denial_fails(&self.substitute_for(&row, &args), &args);
            match (state, waits) {
                ("granted", Waits::Permission) | ("denied", Waits::Permission)
                    if state == "granted" || !fails =>
                {
                    self.retire_hold(hold);
                    // A call that needs another permission too asks it next
                    // (`getUserMedia(audio=true, video=true)`, LLP 1133.009 D3).
                    if state == "granted" && self.ask_next(&row, send, token, name, &args) {
                        continue;
                    }
                    let forward = (self.extensions.held.iter())
                        .any(|h| h.token == token && h.forward.is_some());
                    if forward {
                        self.release_forward(token);
                        continue;
                    }
                    match self.decide_timed(&row, send, timeout, &args) {
                        Decision::Now(answer) => self.release(token, answer),
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
    /// `approximate`).
    pub(super) fn status_fields(&mut self, rows: &[ModulesRow], name: &str, which: &str) {
        let grants = self.data.grants().to_owned();
        for row in rows {
            let Some(permission) = self
                .statuses_of(row)
                .into_iter()
                .find(|p| p["name"] == name)
            else {
                continue;
            };
            let fields = match which {
                "allow" | "deny" => &permission[which],
                choice => &permission["choices"][choice],
            };
            if fields.is_null() {
                continue;
            }
            let mut status = self.value_of(row).unwrap_or(json!({}));
            // The states are the permissions', never a preferred value's.
            for p in self.statuses_of(row) {
                if let (Some(object), Some(field)) = (status.as_object_mut(), p["field"].as_str()) {
                    object.remove(field);
                }
            }
            for (field, value) in fields.as_object().into_iter().flatten() {
                let value = match value.as_object() {
                    // The longest line the app grants: the approximate one where both are.
                    Some(by_line) => (by_line.iter())
                        .filter(|(line, _)| {
                            grants
                                .lines()
                                .any(|g| g.split_whitespace().next() == Some(line.as_str()))
                        })
                        .max_by_key(|(line, _)| line.len())
                        .map(|(_, v)| v.clone()),
                    None => Some(value.clone()),
                };
                if let (Some(v), Some(object)) = (value, status.as_object_mut()) {
                    object.insert(field.clone(), v);
                }
            }
            let key = self.key(row);
            self.extensions.preferred.insert(key, status);
        }
    }
}
