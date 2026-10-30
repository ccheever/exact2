//! The desktop's modules substituted (LLP 1133.025 §11): a login item and
//! global shortcuts. Under the agent no login item is written and no hot
//! key registered; the runner keeps what the platform would say.
//!
//! - **`"login": "status"`** on a source answers its value with the
//!   registration the app made laid over it, and `launchedAtLogin` for a
//!   launch the agent's `--login` made. **`"register"`** and
//!   **`"unregister"`** on a send record the registration (`state`
//!   `enabled` with its `hidden`, or `disabled`) for the app, keeping a
//!   preferred `requires-approval`, and answer the status.
//! - **`"shortcut": "register"`** on a live source is a registration while
//!   something reads it: an entry `{id, keys, registered, reason}` of the
//!   source's `list`, answered as the entry. A reader's new `keys` lists the
//!   new entry before the old one goes, and the last reader's release
//!   unlists it (`release_idle`). The preferred value of the source sets
//!   `registered` and `reason` for every registration and check, as another
//!   app holding the chord would. `tap @ref` on a registered entry raises
//!   the list's event with a person's origin, as the press would, and keeps
//!   the entry (`"keep": true`). **`"check"`** on a send answers whether
//!   `keys` could be registered, listing nothing.

use super::super::super::{DataSource, Runner};
use super::super::{Json, ModulesRow};
use super::Listed;
use serde_json::json;

/// ARIA's modifier names (`aria-keyshortcuts`).
const MODIFIERS: &[&str] = &["Alt", "Control", "Shift", "Meta"];

/// Why `keys` is not one `aria-keyshortcuts` chord, or `None` when it is:
/// modifiers from [`MODIFIERS`], each once, then one key, joined by `+`; or
/// a modifier pair (`Shift+Shift`), both of one modifier's keys held (LLP
/// 1133.025 D3). ARIA's space-separated alternatives are refused: one chord
/// per registration.
pub fn chord_refusal(keys: &str) -> Option<&'static str> {
    if keys.is_empty() || keys.contains(char::is_whitespace) {
        return Some("invalid");
    }
    let parts: Vec<&str> = keys.split('+').collect();
    if let [a, b] = parts[..] {
        if a == b && MODIFIERS.contains(&a) {
            return None;
        }
    }
    let Some((key, modifiers)) = parts.split_last() else {
        return Some("invalid");
    };
    let distinct = modifiers
        .iter()
        .enumerate()
        .all(|(i, m)| MODIFIERS.contains(m) && !modifiers[..i].contains(m));
    if key.is_empty() || MODIFIERS.contains(key) || !distinct {
        return Some("invalid");
    }
    None
}

impl<D: DataSource> Runner<D> {
    /// Where the app's login item registration is kept: in the app's
    /// state, which every session reads.
    fn login_key(&self) -> String {
        format!("{}/login-item/registration", self.data.app_id())
    }

    /// The status a `"login": "status"` source answers, from `value`, its
    /// default and preferred value.
    pub(in crate::runner) fn login_status(&self, mut value: Json) -> Json {
        if super::super::error_code(&value).is_some() || !value.is_object() {
            return value;
        }
        let app = self.extensions.declared.app.as_ref();
        let kept = app.and_then(|a| {
            let a = a.lock().unwrap_or_else(|e| e.into_inner());
            a.get(&self.login_key()).cloned()
        });
        if let Some(kept) = kept {
            if value["state"] != "requires-approval" {
                value["state"] = kept["state"].clone();
            }
            value["hidden"] = kept["hidden"].clone();
        }
        let launch = self.extensions.declared.login_launch;
        value["launchedAtLogin"] = json!(launch.is_some());
        if let (Some(hidden), Json::Null) = (launch, &value["hidden"]) {
            value["hidden"] = json!(hidden);
        }
        value
    }

    /// Whether the target places a `-exact-menu="status"` button outside
    /// the window: macOS and Windows (LLP 1133.025 D14).
    pub fn places_status_items(&self) -> bool {
        matches!(self.host_target(), "macos" | "windows")
    }

    /// Whether `row` is answered by this file's forms, here.
    pub(in crate::runner) fn desktop_form(&self, row: &ModulesRow) -> bool {
        let sub = self.substitute(row);
        let form = matches!(sub["login"].as_str(), Some("register" | "unregister"))
            || matches!(sub["shortcut"].as_str(), Some("register" | "check"));
        form && !self.unsupported_here(row)
    }

    /// A call of a desktop form: its answer, and what it changes.
    pub(in crate::runner) fn desktop_answer(&mut self, row: &ModulesRow, args: &Json) -> Json {
        let sub = self.substitute(row);
        match (sub["login"].as_str(), sub["shortcut"].as_str()) {
            (Some(op), _) => self.login_change(row, op == "register", args),
            (_, Some("register")) => self.shortcut_register(row, args),
            _ => self.shortcut_check(row, args),
        }
    }

    /// `registerLoginItem` or `unregisterLoginItem`: the registration kept
    /// for the app, the status answered and its readers asked again.
    fn login_change(&mut self, row: &ModulesRow, register: bool, args: &Json) -> Json {
        let Some(status) = (self.plan.modules.iter())
            .find(|m| {
                self.plan.str(m.extension) == self.plan.str(row.extension)
                    && self.substitute(m)["login"] == "status"
            })
            .cloned()
        else {
            return json!({ "error": "NotSupportedError" });
        };
        let kept = json!({
            "state": if register { "enabled" } else { "disabled" },
            "hidden": register && args["hidden"] == true,
        });
        let key = self.login_key();
        let app = self.app_state();
        app.lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, kept.clone());
        let answer = self.login_status(self.value_of(&status).unwrap_or(json!({})));
        let declared = self.plan.str(row.declared).to_owned();
        self.log(format!(
            "{declared}: {}",
            answer["state"].as_str().unwrap_or("")
        ));
        self.announce(&self.key(&status));
        answer
    }

    /// `--login`: this launch is the login item's (LLP 1133.025 §11), and
    /// `hidden` what its registration asked.
    pub fn launch_login(&mut self, hidden: bool) -> Result<String, String> {
        self.extensions.declared.login_launch = Some(hidden);
        self.log(format!(
            "launch --login{}",
            if hidden { " hidden" } else { "" }
        ));
        let keys: Vec<String> = (self.plan.modules.iter())
            .filter(|m| self.substitute(m)["login"] == "status")
            .map(|m| self.key(m))
            .collect();
        for key in keys {
            self.announce(&key);
        }
        Ok(json!({ "launch": "login", "hidden": hidden }).to_string())
    }

    /// The registered and reason a chord answers: invalid, else the
    /// preferred value's, else registered.
    fn chord_answer(&self, row: &ModulesRow, keys: &str) -> (bool, Json) {
        if let Some(reason) = chord_refusal(keys) {
            return (false, json!(reason));
        }
        let register = (self.plan.modules.iter())
            .find(|m| {
                self.plan.str(m.extension) == self.plan.str(row.extension)
                    && self.substitute(m)["shortcut"] == "register"
            })
            .cloned();
        let preferred = register.and_then(|r| self.preferred(&r));
        match preferred {
            Some(p) if p["registered"] == false => (false, p["reason"].clone()),
            _ => (true, Json::Null),
        }
    }

    /// A Contract read of a registration counts its reader with its
    /// arguments, as a bound live source's does (`bound.rs`), so a change of
    /// `keys` and the last reader's going can be told apart; a data-code
    /// read holds the registration until no reader of the name is left.
    pub(in crate::runner) fn note_registration_reader(&mut self, row: &ModulesRow, args: &Json) {
        let key = self.key(row);
        let mut group = args.clone();
        if let Some(map) = group.as_object_mut() {
            map.retain(|_, v| !v.is_null());
        }
        let group = group.to_string();
        if let Some((super::super::samples::Reader::Resource(i), _)) =
            &self.extensions.samples.asking
        {
            let i = *i;
            (self.extensions.readers.entry(key.clone()).or_default()).insert(i, group.clone());
            (self.extensions.groups.entry(key.clone()).or_default()).insert(group);
        }
        self.extensions.live.insert(key);
    }

    /// A read of a registration: listed (replacing the entry of its `id`
    /// with other `keys` only once that reader's old keys are released) and
    /// answered as the entry.
    fn shortcut_register(&mut self, row: &ModulesRow, args: &Json) -> Json {
        let id = args["id"].as_str().unwrap_or_default().to_owned();
        let keys = args["keys"].as_str().unwrap_or_default().to_owned();
        let (registered, reason) = self.chord_answer(row, &keys);
        let fields = json!({ "id": id, "keys": keys, "registered": registered, "reason": reason });
        let extension = self.plan.str(row.extension).to_owned();
        let state = self.substitute(row)["list"]["state"]
            .as_str()
            .unwrap_or("globalShortcuts")
            .to_owned();
        let source = self
            .plan
            .str(self.plan.sources[row.source.0 as usize].name)
            .to_owned();
        let same = |e: &Listed| {
            e.fields.get("id") == Some(&fields["id"])
                && e.fields.get("keys") == Some(&fields["keys"])
        };
        let listed = self
            .entries(&extension, &state)
            .iter()
            .find(|e| same(e))
            .map(|e| e.reference);
        let reference = match listed {
            Some(reference) => reference,
            None => {
                self.next_ticket += 1;
                self.next_ticket - 1
            }
        };
        let entry = Listed {
            reference,
            source,
            fields: fields.as_object().cloned().unwrap_or_default(),
        };
        self.with_list(&extension, &state, |entries| {
            entries.retain(|e| !same(e));
            entries.push(entry);
            ((), true)
        });
        if listed.is_none() {
            let how = if registered {
                "registered".to_owned()
            } else {
                format!("not registered ({})", reason.as_str().unwrap_or(""))
            };
            self.log(format!(
                "{}: {keys} for {id}: {how} @{reference}",
                self.key(row)
            ));
        }
        fields
    }

    /// `checkGlobalShortcut`: whether `keys` could be registered now.
    fn shortcut_check(&mut self, row: &ModulesRow, args: &Json) -> Json {
        let keys = args["keys"].as_str().unwrap_or_default().to_owned();
        let (registered, reason) = self.chord_answer(row, &keys);
        self.log(format!(
            "{}: {keys}: {}",
            self.key(row),
            if registered {
                "registered"
            } else {
                reason.as_str().unwrap_or("")
            }
        ));
        json!({ "keys": keys, "registered": registered, "reason": reason })
    }

    /// A registration source's readers no longer ask with `args`, or none
    /// is left (`None`): its entries go from the list.
    pub(in crate::runner) fn shortcut_released(&mut self, row: &ModulesRow, args: Option<&Json>) {
        let extension = self.plan.str(row.extension).to_owned();
        let Some(state) = self.substitute(row)["list"]["state"]
            .as_str()
            .map(str::to_owned)
        else {
            return;
        };
        let gone = |e: &Listed| {
            args.is_none_or(|a| {
                e.fields.get("id") == Some(&a["id"]) && e.fields.get("keys") == Some(&a["keys"])
            })
        };
        let removed: Vec<u64> = self.with_list(&extension, &state, |entries| {
            let removed: Vec<u64> = entries
                .iter()
                .filter(|e| gone(e))
                .map(|e| e.reference)
                .collect();
            entries.retain(|e| !gone(e));
            let changed = !removed.is_empty();
            (removed, changed)
        });
        for reference in removed {
            self.log(format!("{}: unregistered @{reference}", self.key(row)));
        }
    }

    /// `tap @ref` on a registration (`"keep": true`): its event raised, as
    /// a press of the chord would, the entry kept; refused on one that is
    /// not registered.
    pub(super) fn tap_registration(
        &mut self,
        row: &ModulesRow,
        entry: &Listed,
    ) -> Result<String, String> {
        if entry.fields.get("registered") != Some(&json!(true)) {
            return Err(format!(
                "@{} is not registered ({}): no press reaches it",
                entry.reference,
                entry
                    .fields
                    .get("reason")
                    .and_then(Json::as_str)
                    .unwrap_or("unknown")
            ));
        }
        self.activate_entry(row, &Json::Object(entry.fields.clone()), entry.reference)
    }
}

#[cfg(test)]
mod tests {
    use super::chord_refusal;

    #[test]
    fn a_chord_is_one_aria_shortcut_or_a_modifier_pair() {
        for ok in [
            "Meta+Shift+2",
            "Control+Alt+K",
            "F12",
            "Shift+Shift",
            "Meta+Meta",
        ] {
            assert_eq!(chord_refusal(ok), None, "{ok}");
        }
        for bad in [
            "",
            "Meta+K Control+K",
            "Meta+",
            "Shift",
            "Meta+Meta+K",
            "Shift+Control",
            "Hyper+K",
        ] {
            assert_eq!(chord_refusal(bad), Some("invalid"), "{bad}");
        }
    }
}
