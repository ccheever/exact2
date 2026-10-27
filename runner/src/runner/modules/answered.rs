//! The agent's answers to the module tickets it holds (LLP 1133.002 §6).

use super::super::{DataSource, Runner};
use super::{Decision, Route, Waits};
use crate::HoldAnswer;
use serde_json::json;

impl<D: DataSource> Runner<D> {
    /// The agent answered hold `hold`, already spent: §6's transitions.
    pub(in crate::runner) fn module_answered(&mut self, hold: u64, answer: &HoldAnswer) {
        if self.effect_answered(hold, answer)
            || self.each_answered(hold, answer)
            || self.sheet_answered_hold(hold, answer)
        {
            return;
        }
        let Some(h) = self.extensions.held.iter().find(|h| h.hold == hold) else {
            return;
        };
        let (token, source, waits) = (h.token, h.source.clone(), h.waits);
        let name = h.permission.clone();
        let held_args = h.args.clone();
        let row = self.module_row(&source);
        let sub = self.substitute(&row);
        match (waits, answer) {
            (Waits::Permission, HoldAnswer::Choice(c)) => {
                let chosen = self.permission_named(&row, &name)["choices"][c.as_str()].is_object();
                // The access the request asked (`asks`, LLP 1133.018 D13 item 3).
                let asked = self.asked_line(&row);
                match c.as_str() {
                    "allow" => {
                        self.set_permission_asked(&name, "granted", "allow", asked.as_deref())
                    }
                    "deny" => self.set_permission(&name, "denied"),
                    // A grant of less than all (`limited`, LLP 1133.019 D11 item 3).
                    choice if chosen => {
                        self.set_permission_asked(&name, "granted", choice, asked.as_deref())
                    }
                    // The dialog dismissed: the state stays, this request fails.
                    _ => {
                        let code = self.permission_named(&row, &name)["denied"].clone();
                        self.release(token, json!({ "error": code }))
                    }
                }
            }
            // `cancel`, or a further tap the ticket takes (`taps`: a scan's `timeout`).
            (Waits::Value, HoldAnswer::Choice(c)) => {
                let cancel = (sub["taps"].get(c.as_str()).or(sub.get("cancel")).cloned())
                    .unwrap_or(json!({ "error": "NotSupportedError" }));
                self.release(token, cancel);
            }
            (Waits::Value, HoldAnswer::Value(text)) => {
                let Some(from) = sub["from"].as_str().and_then(|f| self.sibling(&row, f)) else {
                    return;
                };
                if let Ok(value) = self.typed(&from, text) {
                    let key = self.key(&from);
                    self.extensions.preferred.insert(key.clone(), value);
                    self.log(format!("{key}: answered by the agent"));
                    self.settle_values();
                    self.announce(&key);
                }
            }
            (Waits::Raise, HoldAnswer::Choice(c)) => {
                let Some(answers) = sub["answers"].as_str().and_then(|a| self.sibling(&row, a))
                else {
                    return;
                };
                let key = self.key(&answers);
                if c == "allow" {
                    let mut value = self
                        .extensions
                        .preferred
                        .get(&key)
                        .cloned()
                        .unwrap_or(json!({}));
                    for (field, v) in sub["raise"]["allow"].as_object().into_iter().flatten() {
                        value[field] = v.clone();
                    }
                    self.extensions.preferred.insert(key.clone(), value);
                    self.log(format!("{key}: raised by the agent"));
                }
                // Declined or dismissed is no failure: the call answers the value as it is.
                let answer = match self.decide(&answers, false, &json!({})) {
                    Decision::Now(value) => value,
                    Decision::Hold(_) | Decision::Forward => {
                        json!({ "error": "NotSupportedError" })
                    }
                };
                self.release(token, answer);
                // An implementation answering beside the runner's status hears it.
                if c == "allow" && self.route(&row) == Route::Agent {
                    self.tell_preferred(self.plan.str(row.extension));
                }
                self.announce(&key);
            }
            (Waits::Check, HoldAnswer::Choice(c)) => {
                let check = &sub["check"];
                let answer = match c.as_str() {
                    "allow" => {
                        let mut value = check.get("allow").cloned().unwrap_or(json!({}));
                        self.fill_all(&row, &mut value);
                        value
                    }
                    "cancel" => json!({ "error": check["cancel"] }),
                    other => json!({ "error": check["choices"][other] }),
                };
                self.release(token, answer);
            }
            (Waits::Ceremony, HoldAnswer::Choice(c)) => {
                let answer = self.ceremony_tapped(&row, &held_args, &sub, c);
                self.release(token, answer);
            }
            (Waits::Ceremony, HoldAnswer::Value(text)) => {
                if let Ok(response) = self.ceremony_typed(&row, &held_args, token, text) {
                    self.release(token, response);
                }
            }
            (Waits::Choose, answer) => self.chosen(&row, token, answer),
            (Waits::Device, answer) => self.device_answered(&row, token, &held_args, answer),
            (Waits::Permission | Waits::Raise | Waits::Check, HoldAnswer::Value(_))
            | (Waits::Span, _) => {}
        }
    }

    /// Value tickets whose source now has a preferred value: each answers.
    pub(super) fn settle_values(&mut self) {
        let waiting: Vec<(u64, u64, String)> = (self.extensions.held.iter())
            .filter(|h| h.waits == Waits::Value)
            .map(|h| (h.token, h.hold, h.source.clone()))
            .collect();
        for (token, hold, source) in waiting {
            let row = self.module_row(&source);
            let sub = self.substitute(&row);
            let from = sub["from"].as_str().and_then(|f| self.sibling(&row, f));
            if let Some(value) = from.as_ref().and_then(|f| self.preferred(f)) {
                self.retire_hold(hold);
                self.release(token, value);
            }
        }
    }
}
