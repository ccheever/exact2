//! A keyed list whose entries change (LLP 1133.033 D17 item 7, D21): a call
//! the app places is listed, then its state moves as the app reports it
//! (`update`, `app_list.rs`) and as the person acts on the system's screen
//! (`tap @ref <choice>`). Each form is declared, so a third party's module
//! drives as the standard library's does.
//!
//! - **On the list** (`"list"` of a send): `"with"` sets fields over the
//!   options as written; `"absent"` sets each of its fields the call left
//!   out (an option's default that is no number); `"stamp"` names a field
//!   set to the agent clock's date; `"refuse": code` refuses a key already
//!   listed instead of replacing its entry; `"ends": {field: [values]}`
//!   names the entries that are over, which stay listed until the list next
//!   lists one; `"inCall": {field: [values]}` names the entries that hold
//!   the app in a call (admission, D10).
//! - **A rule** (`when`, `ends`, `inCall`) is `{field: value}` or
//!   `{field: [values]}`: each field holds the value, or one of them.
//! - **A value in `set`** is a literal; `{"now": true}`, the agent clock's
//!   date; or `{"from": option, "map": {…}, "absent": value}`, the
//!   option's value (`absent` when the call left it out), or what `map`
//!   gives for it (a map that names no entry for it leaves the field as it
//!   was).

use super::super::super::{DataSource, Runner};
use super::super::Json;
use serde_json::{json, Map};

impl<D: DataSource> Runner<D> {
    /// Whether `fields` matches `rule` (module doc). A rule that is not an
    /// object matches nothing.
    pub(super) fn matches(fields: &Map<String, Json>, rule: &Json) -> bool {
        let Some(rule) = rule.as_object() else {
            return false;
        };
        rule.iter().all(|(field, want)| {
            let v = fields.get(field).unwrap_or(&Json::Null);
            match want {
                Json::Array(values) => values.contains(v),
                value => value == v,
            }
        })
    }

    /// The listing's own forms (module doc) over the call's `fields`,
    /// before it is listed: `Err` with the refusal's code.
    pub(super) fn listing(
        &mut self,
        extension: &str,
        list: &Map<String, Json>,
        fields: &mut Map<String, Json>,
    ) -> Result<(), String> {
        let state = list
            .get("state")
            .and_then(Json::as_str)
            .unwrap_or("listed")
            .to_owned();
        if let Some(ends) = list.get("ends").filter(|e| e.is_object()) {
            self.with_list(extension, &state, |entries| {
                let before = entries.len();
                entries.retain(|e| !Self::matches(&e.fields, ends));
                ((), entries.len() != before)
            });
        }
        if let (Some(code), Some(key)) = (
            list.get("refuse").and_then(Json::as_str),
            list.get("key").and_then(Json::as_str),
        ) {
            let value = fields.get(key).cloned().unwrap_or(Json::Null);
            if (self.entries(extension, &state).iter()).any(|e| e.fields.get(key) == Some(&value)) {
                return Err(code.to_owned());
            }
        }
        for (k, v) in list
            .get("absent")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            if fields.get(k).is_none_or(Json::is_null) {
                fields.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in list
            .get("with")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            fields.insert(k.clone(), v.clone());
        }
        if let Some(stamp) = list.get("stamp").and_then(Json::as_str) {
            fields.insert(stamp.into(), json!(self.epoch_now()));
        }
        Ok(())
    }

    /// `set`'s fields, with the call's `args` and the agent clock's date
    /// `now`, onto `fields`.
    pub(super) fn set_fields(
        fields: &mut Map<String, Json>,
        set: &Json,
        args: &Map<String, Json>,
        now: f64,
    ) {
        for (field, value) in set.as_object().into_iter().flatten() {
            if let Some(v) = Self::set_value(value, args, now) {
                fields.insert(field.clone(), v);
            }
        }
    }

    /// One value of `set`; `None` leaves the field as it was.
    fn set_value(value: &Json, args: &Map<String, Json>, now: f64) -> Option<Json> {
        let Some(form) = value.as_object() else {
            return Some(value.clone());
        };
        if form.get("now") == Some(&Json::Bool(true)) {
            return Some(json!(now.round() as i64));
        }
        let Some(from) = form.get("from").and_then(Json::as_str) else {
            return Some(value.clone());
        };
        let given = match args.get(from) {
            Some(v) if !v.is_null() => v.clone(),
            _ => form.get("absent").cloned().unwrap_or(Json::Null),
        };
        match form.get("map").and_then(Json::as_object) {
            None => Some(given),
            Some(map) => {
                let word = match &given {
                    Json::String(s) => s.clone(),
                    other => other.to_string(),
                };
                map.get(&word).and_then(|v| Self::set_value(v, args, now))
            }
        }
    }

    /// Whether an entry of a list that declares `inCall` holds the app in a
    /// call now (D10): a capture's start is then admitted while the app is
    /// hidden, on the targets whose platforms allow it.
    pub(in crate::runner) fn in_call(&self) -> bool {
        (self.plan.modules.iter()).any(|row| {
            let list = &self.substitute(row)["list"];
            let (Some(state), rule) = (list["state"].as_str(), &list["inCall"]) else {
                return false;
            };
            rule.is_object()
                && (self.entries(self.plan.str(row.extension), state).iter())
                    .any(|e| Self::matches(&e.fields, rule))
        })
    }
}
