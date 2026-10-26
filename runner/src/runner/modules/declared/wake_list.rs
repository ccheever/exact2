//! A keyed list whose entries are registrations a wake runs for (LLP
//! 1133.017 §10): `"list": {"state", "key", "kind", "tap": {"event", "as":
//! "wake", "due", "args"}}`. Under the agent no wake fires by itself; a
//! drive raises each.
//!
//! - **Entries** are keyed by their key and `kind`, so a one-shot and a
//!   periodic registration of one tag are two. Each shows `kind`,
//!   `lastRun` and `due` (epoch ms) beside its options. An entry whose tap
//!   names `due`, the option that is its interval, is periodic; any other
//!   is a one-shot.
//! - **`tap @ref`** raises the entry's event as a warm wake in a hidden
//!   session, when the platform would: refused `not due until <t>` before
//!   its due time, and, for a one-shot, `offline` while the page is.
//! - **`--wake '<entry>'`** hands a relaunched app one entry as
//!   `state.backgroundSync` printed it: listed again, and raised as a cold
//!   wake, due or not, since the platform chose to wake it.
//! - **After the invocation,** by the web's rules (D1): a periodic entry
//!   that completed, or was superseded, ran; a one-shot that completed is
//!   retired unless its handler registered it again, which waits out a
//!   backoff of 5 minutes doubling to an hour; one that expired or was
//!   superseded stays, as an attempt. The third attempt in a row is the
//!   last chance: after it the entry is retired, unless registered again,
//!   which keeps it at the longest backoff and starts a new count. An
//!   expiry before delivery is no attempt.
//! - **`"listed": {"state", "kind", "fields"}`** on a source answers the
//!   entries of that kind, those fields each, and its readers are asked
//!   again whenever the list changes.

use super::super::super::{DataSource, Runner};
use super::super::{Json, ModulesRow};
use super::Listed;
use crate::runner::modules::WakeHow;
use serde_json::{json, Map};

/// The first backoff, and the longest (D1).
const BACKOFF_MS: f64 = 5.0 * 60_000.0;
const LONGEST_MS: f64 = 60.0 * 60_000.0;

/// The attempts in a row a registration gets before its last chance.
const ATTEMPTS: u32 = 3;

/// What the substitute keeps of a registration beside its options.
#[derive(Clone, Default)]
pub(super) struct Meta {
    /// When it was first registered, epoch ms.
    registered: f64,
    last_run: Option<f64>,
    /// Attempts in a row that did not retire it.
    attempts: u32,
    /// A backoff's end, epoch ms.
    not_before: Option<f64>,
    /// Its handler registered it again during its own invocation.
    again: bool,
}

impl<D: DataSource> Runner<D> {
    fn epoch_now(&self) -> f64 {
        self.time.epoch_at_zero + self.now_ms
    }

    /// Entry `reference` was listed, replacing `replaced` (same key and
    /// kind) where there was one: it keeps that one's runs and attempts, and
    /// one registered again by its own running invocation is marked so.
    pub(super) fn registered(&mut self, reference: u64, replaced: Option<u64>) {
        let now = self.epoch_now();
        let running = self.running_entries();
        let meta = &mut self.extensions.declared.wake_meta;
        let mut m = replaced.and_then(|r| meta.remove(&r)).unwrap_or(Meta {
            registered: now,
            ..Meta::default()
        });
        if let Some(r) = replaced {
            if running.contains(&r) {
                m.again = true;
            }
            self.wake_entry_moved(r, reference);
        }
        self.extensions.declared.wake_meta.insert(reference, m);
    }

    /// A periodic entry's interval, the option its tap's `due` names; `None`
    /// for a one-shot.
    fn interval(&self, e: &Listed) -> Option<f64> {
        let sub = self.substitute(&self.module_row(&e.source));
        let option = sub["list"]["tap"]["due"].as_str()?;
        Some(e.fields.get(option).and_then(Json::as_f64).unwrap_or(0.0))
    }

    /// When entry `e` is next due, epoch ms: a periodic one its interval
    /// after its last run (its registration before any), a one-shot at its
    /// backoff's end, else at once.
    fn due(&self, e: &Listed) -> f64 {
        let Some(m) = self.extensions.declared.wake_meta.get(&e.reference) else {
            return 0.0;
        };
        match self.interval(e) {
            Some(i) => m.last_run.unwrap_or(m.registered) + i,
            None => m.not_before.unwrap_or(m.registered),
        }
    }

    /// The fields `state.<list>` shows of entry `e` beyond its options.
    pub(super) fn wake_fields(&self, e: &Listed, v: &mut Map<String, Json>) {
        if !e.fields.contains_key("kind") {
            return;
        }
        let m = self.extensions.declared.wake_meta.get(&e.reference);
        v.insert("lastRun".into(), json!(m.and_then(|m| m.last_run)));
        v.insert("due".into(), json!(self.due(e)));
    }

    /// `tap @ref` on an entry whose tap is a wake: raised when due.
    pub(super) fn tap_wake(&mut self, state: &str, at: usize) -> Result<String, String> {
        let entry = &self.extensions.declared.lists[state][at];
        let reference = entry.reference;
        let due = self.due(entry);
        if self.epoch_now() < due {
            return Err(format!("@{reference} is not due until {due}"));
        }
        if self.interval(entry).is_none() && !self.page.on_line {
            return Err(format!("@{reference}: offline"));
        }
        self.log(format!("{state} @{reference}: tapped"));
        self.raise_entry(state, at, false)
    }

    /// `--wake '<entry>'`: the entry listed again in the relaunched app, and
    /// raised as a cold wake.
    pub(super) fn launch_wake(&mut self, entry: &Json) -> Result<String, String> {
        let kind = entry["kind"].as_str().unwrap_or_default();
        let Some(row) = (self.plan.modules.iter())
            .find(|m| {
                let list = &self.substitute(m)["list"];
                list["kind"] == kind && list["tap"]["as"] == "wake"
            })
            .cloned()
        else {
            return Err(format!(
                "--wake: no module of this app lists registrations of kind `{kind}`"
            ));
        };
        let sub = self.substitute(&row);
        let state = sub["list"]["state"].as_str().unwrap_or("listed").to_owned();
        let key = sub["list"]["key"].as_str().unwrap_or_default();
        let mut fields: Map<String, Json> = Map::new();
        for (k, v) in entry.as_object().into_iter().flatten() {
            if !matches!(k.as_str(), "ref" | "delivered" | "due" | "lastRun") {
                fields.insert(k.clone(), v.clone());
            }
        }
        let reference = self.next_ticket;
        self.next_ticket += 1;
        let value = fields.get(key).cloned();
        let entries = self
            .extensions
            .declared
            .lists
            .entry(state.clone())
            .or_default();
        entries.retain(|e| {
            e.fields.get(key) != value.as_ref() || e.fields.get("kind") != Some(&json!(kind))
        });
        entries.push(Listed {
            reference,
            source: self
                .plan
                .str(self.plan.sources[row.source.0 as usize].name)
                .to_owned(),
            fields,
        });
        let at = entries.len() - 1;
        let now = self.epoch_now();
        let last_run = entry["lastRun"].as_f64();
        (self.extensions.declared.wake_meta).insert(
            reference,
            Meta {
                registered: now,
                last_run,
                ..Meta::default()
            },
        );
        self.log(format!("launch --wake ({kind} @{reference})"));
        self.announce_listed(&state);
        self.raise_entry(&state, at, true)
    }

    /// Raise the wake event entry `at` of list `state` declares, as a wake
    /// of its own (an agent's drive is one platform object a tap).
    fn raise_entry(&mut self, state: &str, at: usize, cold: bool) -> Result<String, String> {
        let entry = &self.extensions.declared.lists[state][at];
        let reference = entry.reference;
        let row = self.module_row(&entry.source);
        let tap = self.substitute(&row)["list"]["tap"].clone();
        let mut fields = entry.fields.clone();
        let attempts =
            (self.extensions.declared.wake_meta.get(&reference)).map_or(0, |m| m.attempts);
        fields.insert("lastChance".into(), json!(attempts + 1 >= ATTEMPTS));
        let args = super::entry_args(&tap, &Json::Object(fields));
        let Some(event) = tap["event"].as_str() else {
            return Err(format!("@{reference}: its tap names no event"));
        };
        let extension = self.plan.str(row.extension).to_owned();
        let found = (self.plan.module_events.iter()).position(|e| {
            self.plan.str(e.extension) == extension && self.plan.str(e.declared) == event
        });
        if found.is_none() {
            self.log(format!("{extension}.{event}: no handler hears it"));
        }
        let how = WakeHow {
            id: reference,
            cold,
            hidden: true,
        };
        self.wake_invocation(found, args, how, Some(reference));
        Ok(
            json!({ "ref": reference, "event": event, "as": "wake", "delivery": "substituted" })
                .to_string(),
        )
    }

    /// Entry `reference`'s invocation was delivered.
    pub(in crate::runner) fn wake_entry_running(&mut self, reference: u64) {
        if let Some(m) = self.extensions.declared.wake_meta.get_mut(&reference) {
            m.again = false;
        }
    }

    /// Entry `reference`'s invocation ended `state` (module doc).
    pub(in crate::runner) fn wake_entry_ended(
        &mut self,
        reference: u64,
        state: &str,
        delivered: bool,
    ) {
        let Some((list, at)) = (self.extensions.declared.lists.iter()).find_map(|(s, l)| {
            l.iter()
                .position(|e| e.reference == reference)
                .map(|i| (s.clone(), i))
        }) else {
            return;
        };
        let now = self.epoch_now();
        let periodic = self
            .interval(&self.extensions.declared.lists[&list][at])
            .is_some();
        let Some(m) = self.extensions.declared.wake_meta.get_mut(&reference) else {
            return;
        };
        let mut retire = false;
        match (periodic, state) {
            (true, "completed" | "superseded") => {
                m.last_run = Some(now);
                m.attempts = 0;
            }
            (false, "completed") if !m.again => retire = true,
            (_, "completed" | "expired" | "superseded") if delivered => {
                m.attempts += 1;
                if !periodic {
                    let backoff = (BACKOFF_MS * 2f64.powi(m.attempts as i32 - 1)).min(LONGEST_MS);
                    m.not_before = Some(now + backoff);
                }
            }
            _ => {}
        }
        if !periodic && m.attempts >= ATTEMPTS {
            if m.again {
                m.attempts = 0;
                m.not_before = Some(now + LONGEST_MS);
            } else {
                retire = true;
            }
        }
        m.again = false;
        if retire {
            self.extensions.declared.wake_meta.remove(&reference);
            let entries = self
                .extensions
                .declared
                .lists
                .get_mut(&list)
                .expect("found above");
            entries.remove(at);
            self.log(format!("{list} @{reference}: retired"));
        }
        self.announce_listed(&list);
    }

    /// A source declared `"listed"`: the entries of its kind, its fields each.
    pub(in crate::runner) fn listed_value(&self, row: &ModulesRow, sub: &Json) -> Option<Json> {
        if let Some(entries) = self.reach_value(sub) {
            return Some(entries);
        }
        let listed = sub["listed"].as_object()?;
        let state = listed.get("state")?.as_str()?;
        let extension = self.plan.str(row.extension);
        let kind = listed.get("kind");
        let fields: Vec<&str> = (listed
            .get("fields")
            .and_then(Json::as_array)
            .into_iter()
            .flatten())
        .filter_map(Json::as_str)
        .collect();
        let entries = (self.entries(extension, state).iter())
            .filter(|e| kind.is_none() || e.fields.get("kind") == kind)
            .map(|e| {
                let mut all = e.fields.clone();
                self.wake_fields(e, &mut all);
                let picked: Map<String, Json> = (fields.iter())
                    .map(|f| (f.to_string(), all.get(*f).cloned().unwrap_or(Json::Null)))
                    .collect();
                Json::Object(picked)
            })
            .collect();
        Some(Json::Array(entries))
    }

    /// List `state` changed: each source that answers from it is asked again.
    pub(super) fn announce_listed(&self, state: &str) {
        let keys: Vec<String> = (self.plan.modules.iter())
            .filter(|m| self.substitute(m)["listed"]["state"] == state)
            .map(|m: &ModulesRow| self.key(m))
            .collect();
        for key in keys {
            self.announce(&key);
        }
    }
}
