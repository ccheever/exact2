//! Permissions keyed by an argument's values (LLP 1133.026 D19 items 1 and
//! 2): a permission per data type, as HealthKit and Health Connect keep
//! them.
//!
//! - **By value** (`permissionValue`): a call asks the permission its
//!   argument names, `healthStatistics(type="steps")` asking `health.steps`.
//!   A platform's word (`apple.quantityType`) beats the shared option, and a
//!   `<prefix>.*` permission stands for each identifier it covers.
//! - **By list** (`permissionEach`): a request asks the permission of every
//!   value of a list, and of every flag it sets, in one ticket, since the
//!   platform shows one sheet. `tap @t allow` grants each, `deny` denies
//!   each, `type @t steps,workouts` grants those and denies the rest,
//!   `steps@2026-10-01` grants with a window from that date, and `cancel`
//!   leaves them. The request then goes on to the implementation however it
//!   was answered: the platform's completion says only that it was handled.
//! - **A per-target word** (`answered`): on a target it names (`{"ios":
//!   "unknown"}`) an answered permission stores that word, never `granted`
//!   or `denied`, so admission neither refuses nor holds it. The answer is
//!   kept as facts beside it, `shared` and `since`, which a later state does
//!   not clear; the implementation is told them and applies them to reads.
//! - **Per target** (`targets`): a permission declared for some targets is
//!   no permission on the others, where its status reads its default.

use super::super::DataSource;
use super::{Json, ModulesRow, Runner};
use crate::HoldAnswer;
use serde_json::json;

impl<D: DataSource> Runner<D> {
    /// `sub` without the permissions declared for other targets.
    pub(super) fn this_target(&self, sub: &mut Json) {
        let target = self.host_target();
        let here =
            |p: &Json| (p["targets"].as_array()).is_none_or(|t| t.iter().any(|x| x == target));
        if sub["permission"].is_object() && !here(&sub["permission"]) {
            if let Some(map) = sub.as_object_mut() {
                map.remove("permission");
            }
        }
        // Kept by name, so no `<prefix>.*` permission stands for one there.
        let elsewhere: Vec<Json> = (sub["permissionList"].as_array().into_iter().flatten())
            .filter(|p| !here(p))
            .map(|p| p["name"].clone())
            .collect();
        if let Some(list) = sub["permissionList"].as_array_mut() {
            list.retain(here);
        }
        if !elsewhere.is_empty() {
            sub["permissionElsewhere"] = Json::Array(elsewhere);
        }
    }

    /// The permission named `name` as declared: by its name, or as a
    /// `<prefix>.*` permission stands for it. `Null` for none here.
    pub(in crate::runner) fn declared_permission(&self, name: &str) -> Json {
        let mut covering = Json::Null;
        for row in &self.plan.modules {
            let sub = self.substitute(row);
            if (sub["permissionElsewhere"].as_array().into_iter().flatten()).any(|n| n == name) {
                return Json::Null;
            }
            let list = sub["permissionList"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for p in std::iter::once(sub["permission"].clone()).chain(list) {
                let Some(declared) = p["name"].as_str() else {
                    continue;
                };
                if declared == name {
                    return p;
                }
                let covers = declared
                    .strip_suffix('*')
                    .is_some_and(|pre| name.starts_with(pre));
                if covers && covering.is_null() {
                    covering = p.clone();
                    covering["name"] = json!(name);
                }
            }
        }
        covering
    }

    /// The permission a call of `row` with `args` asks by its arguments'
    /// values, where its declaration chooses one so; `None` where it does not.
    pub(super) fn permission_by_value(&self, row: &ModulesRow, args: &Json) -> Option<Json> {
        let sub = self.substitute(row);
        let pairs = sub["permissionValue"].as_array()?;
        for pair in pairs {
            let (Some(option), Some(template)) = (pair[0].as_str(), pair[1].as_str()) else {
                continue;
            };
            if let Some(value) = at(args, option).and_then(Json::as_str) {
                return Some(self.declared_permission(&template.replace("{}", value)));
            }
        }
        Some(Json::Null)
    }

    /// Every permission a request of `row` with `args` asks together
    /// (`permissionEach`); `None` for a name that asks none so.
    pub(super) fn each_asked(&self, row: &ModulesRow, args: &Json) -> Option<Vec<String>> {
        let sub = self.substitute(row);
        let pairs = sub["permissionEach"].as_array()?;
        let mut names: Vec<String> = Vec::new();
        for pair in pairs {
            let (Some(option), Some(template)) = (pair[0].as_str(), pair[1].as_str()) else {
                continue;
            };
            let values: Vec<String> = match at(args, option) {
                Some(Json::Array(items)) => (items.iter().filter_map(Json::as_str))
                    .map(|v| template.replace("{}", v))
                    .collect(),
                Some(Json::String(v)) => vec![template.replace("{}", v)],
                Some(Json::Bool(true)) => vec![template.to_owned()],
                _ => Vec::new(),
            };
            for name in values {
                if !names.contains(&name) && !self.declared_permission(&name).is_null() {
                    names.push(name);
                }
            }
        }
        Some(names)
    }

    /// Admission of a request that asks several permissions together: held
    /// under one ticket while any is `prompt`, else on to the
    /// implementation, which answers the status. `None` for any other name.
    pub(super) fn decide_each(
        &self,
        row: &ModulesRow,
        send: bool,
        args: &Json,
    ) -> Option<Option<super::Decision>> {
        let names = self.each_asked(row, args).filter(|_| send)?;
        let asks = names.iter().any(|n| self.permission_state(n) == "prompt");
        Some(asks.then_some(super::Decision::Hold(super::Waits::Permission)))
    }

    /// The ticket a request waiting on permission `permission` shares: one
    /// already held for it, whichever interface's request asked first (D19
    /// item 2: two interfaces declaring one name share its decision, as the
    /// platform shows one prompt). Settling it releases every request held
    /// on the name (`permits.rs`).
    pub(super) fn shared_hold(&mut self, waits: super::Waits, permission: &str) -> Option<u64> {
        if waits != super::Waits::Permission || permission.is_empty() {
            return None;
        }
        let hold = (self.extensions.held.iter())
            .filter(|h| h.waits == waits && h.permission == permission)
            .map(|h| h.hold)
            .find(|hold| self.device_holds().iter().any(|d| d.ticket == *hold))?;
        self.log(format!("permission {permission}: joins ticket {hold}"));
        Some(hold)
    }

    /// Whether `row`'s interface declares permission `name` in its list, so
    /// its every name, the status the implementation answers included, is
    /// asked again when the permission changes.
    pub(super) fn lists_permission(&self, row: &ModulesRow, name: &str) -> bool {
        (self.substitute(row)["permissionList"]
            .as_array()
            .into_iter()
            .flatten())
        .filter_map(|p| p["name"].as_str())
        .any(|p| p == name || p.strip_suffix('*').is_some_and(|pre| name.starts_with(pre)))
    }

    /// Whether the held ticket of `row` is one that asks several together.
    pub(super) fn is_each(&self, row: &ModulesRow) -> bool {
        self.substitute(row)["permissionEach"].is_array()
    }

    /// A typed answer to an `each` ticket asking `asked` (comma-joined):
    /// each named permission with the window it was given, if any. A name is
    /// its permission's whole name or its last part (`steps`).
    pub(super) fn each_typed(
        &self,
        asked: &str,
        text: &str,
    ) -> Result<Vec<(String, Option<f64>)>, String> {
        let asked: Vec<&str> = asked.split(',').filter(|s| !s.is_empty()).collect();
        let mut out = Vec::new();
        for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (word, since) = match item.split_once('@') {
                Some((w, d)) => (
                    w,
                    Some(date_ms(d).ok_or_else(|| format!("`{d}` is a date, 2026-10-01"))?),
                ),
                None => (item, None),
            };
            let name = (asked.iter())
                .find(|a| **a == word || a.rsplit('.').next() == Some(word))
                .ok_or_else(|| {
                    format!(
                        "`{word}` is not asked; the ticket asks {}",
                        asked.join(", ")
                    )
                })?;
            out.push(((*name).to_owned(), since));
        }
        Ok(out)
    }

    /// The agent answered `each` ticket `hold`: each permission it asked set
    /// as the answer says, then the request goes on. `false` for a ticket
    /// that is no `each` ticket.
    pub(super) fn each_answered(&mut self, hold: u64, answer: &HoldAnswer) -> bool {
        let Some(h) = self.extensions.held.iter().find(|h| h.hold == hold) else {
            return false;
        };
        let (token, source, asked) = (h.token, h.source.clone(), h.permission.clone());
        let row = self.module_row(&source);
        if !self.is_each(&row) || h.waits != super::Waits::Permission {
            return false;
        }
        let names: Vec<String> = asked
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let decided: Vec<(String, &str, Option<f64>)> = match answer {
            HoldAnswer::Choice(c) if c == "allow" => {
                names.iter().map(|n| (n.clone(), "granted", None)).collect()
            }
            HoldAnswer::Choice(c) if c == "deny" => {
                names.iter().map(|n| (n.clone(), "denied", None)).collect()
            }
            HoldAnswer::Choice(_) => Vec::new(),
            HoldAnswer::Value(text) => {
                let Ok(listed) = self.each_typed(&asked, text) else {
                    return true;
                };
                (names.iter())
                    .map(|n| match listed.iter().find(|(l, _)| l == n) {
                        Some((_, since)) => (n.clone(), "granted", *since),
                        None => (n.clone(), "denied", None),
                    })
                    .collect()
            }
        };
        for (name, state, since) in decided {
            self.set_permission_answered(&name, state, since);
        }
        let forwards =
            (self.extensions.held.iter()).any(|h| h.token == token && h.forward.is_some());
        if forwards {
            self.release_forward(token);
        } else {
            self.release(token, json!({ "error": "NotSupportedError" }));
        }
        true
    }

    /// Set permission `name` as a person's answer would: on a target its
    /// `answered` names, `granted` and `denied` store that word, with
    /// `shared` and a window kept as its facts.
    pub(in crate::runner) fn set_permission_answered(
        &mut self,
        name: &str,
        state: &str,
        since: Option<f64>,
    ) {
        let word = self.answered_word(name);
        let state = match (word, state) {
            (Some(word), "granted" | "denied") => {
                let facts = self
                    .extensions
                    .facts
                    .entry(name.to_owned())
                    .or_insert(json!({}));
                facts["shared"] = json!(state == "granted");
                if let Some(since) = since {
                    facts["since"] = json!(since);
                }
                word
            }
            _ => state.to_owned(),
        };
        self.set_permission(name, &state);
    }

    /// The word permission `name` stores once answered on this target, where
    /// its declaration names one (`answered`).
    pub(super) fn answered_word(&self, name: &str) -> Option<String> {
        let target = self.host_target().to_owned();
        (self.declared_permission(name)["answered"][&target].as_str()).map(str::to_owned)
    }

    /// The agent's `prefer permission.<name> <value>`, where `name` may stand
    /// for every permission under it (`health`) and `value` is a state or a
    /// fact (`shared=false`, `since=2026-10-01`). `Err` names why it is refused.
    pub(super) fn prefer_permission(&mut self, name: &str, value: &str) -> Result<(), String> {
        let mut names: Vec<String> = Vec::new();
        for row in &self.plan.modules {
            let sub = self.substitute(row);
            let list = sub["permissionList"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for p in std::iter::once(sub["permission"].clone()).chain(list) {
                let Some(n) = p["name"].as_str().filter(|n| !n.ends_with('*')) else {
                    continue;
                };
                let under = n == name || n.starts_with(&format!("{name}."));
                if under && !names.iter().any(|m| m == n) {
                    names.push(n.to_owned());
                }
            }
        }
        if names.is_empty() {
            if (self.plan.modules.iter()).any(|m| self.asks_permission(m, name)) {
                names.push(name.to_owned());
            } else {
                return Err(format!(
                    "prefer permission: no module here asks for `{name}`"
                ));
            }
        }
        for n in &names {
            let word = self.answered_word(n);
            match (value.split_once('='), value, word) {
                (Some(("shared", v)), _, Some(word)) => {
                    let shared = v == "true";
                    (self.extensions.facts.entry(n.clone()).or_insert(json!({})))["shared"] = json!(shared);
                    self.set_permission(n, &word);
                }
                (Some(("since", d)), _, Some(word)) => {
                    let since = date_ms(d).ok_or_else(|| format!("prefer permission.{n}: `{d}` is a date, 2026-10-01"))?;
                    let facts = self.extensions.facts.entry(n.clone()).or_insert(json!({ "shared": true }));
                    facts["since"] = json!(since);
                    self.set_permission(n, &word);
                }
                (Some(_), _, _) => {
                    return Err(format!("prefer permission.{n}: `{value}`: shared= and since= are the facts of a permission whose answer the platform hides"))
                }
                (None, "denied", Some(word)) => {
                    return Err(format!("prefer permission.{n}: denied is not {}'s to say here, which stores `{word}`; prefer shared=false", self.host_target()))
                }
                (None, "granted" | "denied" | "prompt", _) => self.set_permission_answered(n, value, None),
                (None, other, _) => {
                    return Err(format!("prefer permission.{n}: granted, denied or prompt, or shared= or since=, not `{other}`"))
                }
            }
        }
        Ok(())
    }

    /// The facts kept beside `extension`'s permissions, by name: what the
    /// implementation is told and `state.extensions` shows.
    pub(in crate::runner) fn facts_of(&self, extension: &str) -> Option<Json> {
        let mine: serde_json::Map<String, Json> = (self.extensions.facts.iter())
            .filter(|(name, _)| {
                (self.plan.modules.iter()).any(|m| {
                    self.plan.str(m.extension) == extension
                        && (self.substitute(m)["permissionList"]
                            .as_array()
                            .into_iter()
                            .flatten())
                        .filter_map(|p| p["name"].as_str())
                        .any(|p| {
                            p == name.as_str()
                                || p.strip_suffix('*').is_some_and(|pre| name.starts_with(pre))
                        })
                })
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        (!mine.is_empty()).then_some(Json::Object(mine))
    }
}

/// The value at `path` (`apple.quantityType`) in a call's arguments.
fn at<'a>(args: &'a Json, path: &str) -> Option<&'a Json> {
    path.split('.')
        .try_fold(args, |v, part| v.get(part))
        .filter(|v| !v.is_null())
}

/// `2026-10-01` as ms since the epoch, at midnight UTC.
fn date_ms(text: &str) -> Option<f64> {
    let mut parts = text.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(((era * 146_097 + doe - 719_468) * 86_400_000) as f64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_date_is_its_utc_midnight() {
        assert_eq!(super::date_ms("1970-01-01"), Some(0.0));
        assert_eq!(super::date_ms("2026-10-01"), Some(1_790_812_800_000.0));
        assert_eq!(super::date_ms("2026-13-01"), None);
        assert_eq!(super::date_ms("2026-10"), None);
    }
}
