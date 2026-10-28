//! A module's events (LLP 1133.004 D6): a click on a notification, which
//! a value cannot carry, since two clicks are two openings. The plan names
//! each event the root hears and the action it runs (`module_events`).
//!
//! - **Raised** by the host, never during a commit: queued, the host woken,
//!   and delivered by [`Runner::apply_announced`], each as its own commit,
//!   in order, once. None is coalesced.
//! - **Held, and bounded:** up to the event's `hold`, the oldest dropped
//!   and journaled past it.
//! - **Origin** (LLP 1133.002 D12): an activation, a person's act on the
//!   platform's own UI that only the host raises, runs its action with a
//!   person's origin; an event a package emits runs with the device's, so a
//!   request it leads to while `prompt` fails, reason `person-required`.

use super::super::{CommitReceipt, DataSource, Runner, RunnerError};
use super::{CallOrigin, Json, ModulesRow};
use exact_plan::Value;
use std::collections::VecDeque;

/// The prefix of a `changed` topic that is an event a bound implementation
/// emitted, `{"extension", "event", "args", "seq"}` after it: a module's one
/// channel to the runner carries it, and `seq` keeps two alike from
/// coalescing as two announcements of one topic do.
pub(in crate::runner) const EMITTED: &str = "exact-event:";

#[derive(Default)]
pub(in crate::runner) struct Events {
    queued: VecDeque<Raised>,
}

struct Raised {
    row: usize,
    args: Vec<Json>,
    origin: CallOrigin,
}

impl<D: DataSource> Runner<D> {
    /// The host raises event `name` of `extension` with `args`, a JSON
    /// array: `origin` is [`CallOrigin::App`], a person's, only for an
    /// activation the host's own platform callback reported. Queued for
    /// [`Runner::apply_announced`]; `Err` for an event the plan does not
    /// know or arguments that are not an array.
    pub fn raise_module_event(
        &mut self,
        extension: &str,
        name: &str,
        args: &str,
        origin: CallOrigin,
    ) -> Result<(), String> {
        let key = format!("{extension}.{name}");
        let args: Vec<Json> = match serde_json::from_str(args) {
            Ok(Json::Array(args)) => args,
            _ => return Err(format!("{key}: the arguments are a JSON array")),
        };
        let Some(row) = (self.plan.module_events.iter()).position(|e| {
            self.plan.str(e.extension) == extension && self.plan.str(e.declared) == name
        }) else {
            self.log(format!("{key}: no handler hears it"));
            return Ok(());
        };
        let declared = &self.plan.module_events[row];
        if self.plan.str(declared.raised) == "wake" {
            return Err(format!(
                "{key}: a wake raises it, at the platform's call (`raise_wake`)"
            ));
        }
        // An activation is the host's to raise: a package's own raising of
        // one has the device's origin, the least privileged.
        let origin = match (self.plan.str(declared.raised), origin) {
            ("activation", origin) => origin,
            _ => CallOrigin::Device,
        };
        // Each event's own bound: past it, its own oldest is dropped.
        let hold = declared.hold.max(1) as usize;
        let queued = &mut self.extensions.events.queued;
        queued.push_back(Raised { row, args, origin });
        let mut dropped = Vec::new();
        while queued.iter().filter(|r| r.row == row).count() > hold {
            let Some(at) = queued.iter().position(|r| r.row == row) else {
                break;
            };
            let old = queued.remove(at).expect("found above");
            let first = old.args.first().cloned().unwrap_or(Json::Null);
            dropped.push(
                first
                    .as_str()
                    .map_or_else(|| first.to_string(), str::to_owned),
            );
        }
        for first in dropped {
            self.log(format!("{key}: dropped ({first})"));
        }
        self.log(format!("{key}: raised"));
        self.native_slot().changed("");
        Ok(())
    }

    /// An event a bound implementation emitted: the device's origin, whatever
    /// it claims.
    pub(in crate::runner) fn raise_emitted(&mut self, body: &str) {
        let v: Json = serde_json::from_str(body).unwrap_or_default();
        let (Some(extension), Some(event)) = (v["extension"].as_str(), v["event"].as_str()) else {
            self.log(format!("an emitted event is unreadable: {body}"));
            return;
        };
        if let Err(e) =
            self.raise_module_event(extension, event, &v["args"].to_string(), CallOrigin::Device)
        {
            self.log(e);
        }
    }

    /// Each queued event, in order, one commit each: its action run with as
    /// many of its arguments as the action takes, under its origin.
    pub(in crate::runner) fn deliver_module_events(
        &mut self,
    ) -> (Vec<CommitReceipt>, Option<RunnerError>) {
        self.journal_effect_lines();
        let mut receipts = Vec::new();
        while let Some(raised) = self.extensions.events.queued.pop_front() {
            let row = self.plan.module_events[raised.row].clone();
            let key = format!(
                "{}.{}",
                self.plan.str(row.extension),
                self.plan.str(row.declared)
            );
            let action = self.plan.action(row.action).clone();
            let mut args = Vec::new();
            let mut refused = None;
            for (i, p) in action.params.iter().enumerate() {
                let ty = self.plan.param(p).ty;
                let json = raised.args.get(i).cloned().unwrap_or(Json::Null);
                match self.event_arg(ty, &json) {
                    Ok(v) => args.push(v),
                    Err(why) => refused = Some(why),
                }
            }
            if let Some(why) = refused {
                self.log(format!("{key}: refused: {why}"));
                continue;
            }
            let action_name = self.plan.str(action.name).to_owned();
            self.log(format!("{key}: heard by {action_name}"));
            let result = self.as_origin(raised.origin, |r| r.run_action(row.action, args, &[]));
            match result {
                Ok(receipt) => receipts.push(receipt),
                Err(e) => return (receipts, Some(e)),
            }
        }
        // Then a wake's, one invocation at a time (LLP 1133.017 D5).
        let (woke, error) = self.step_wakes();
        receipts.extend(woke);
        (receipts, error)
    }

    /// One argument as the action's parameter takes it.
    pub(in crate::runner) fn event_arg(
        &self,
        ty: exact_plan::TypesId,
        json: &Json,
    ) -> Result<Value, String> {
        let shape = exact_js_value::Shape::from_plan(&self.plan, ty)?;
        let mut json = json.clone();
        super::absent_is_none(&mut json, &shape);
        let value = exact_js_value::from_json(&json, &shape)?;
        if !value.conforms(&self.plan, ty) {
            return Err(crate::conform::mismatch(&self.plan, &value, ty));
        }
        Ok(value)
    }

    /// Journal a request a commit not the app's made while `prompt`, which
    /// [`Self::decide`] refuses (LLP 1133.002 D12); whether it was one.
    pub(super) fn say_person_required(
        &mut self,
        row: &ModulesRow,
        send: bool,
        origin: CallOrigin,
    ) -> bool {
        if origin == CallOrigin::App {
            return false;
        }
        let sub = self.substitute(row);
        let Some(name) = sub["permission"]["name"].as_str() else {
            return false;
        };
        let refused = self.permission_state(name) == "prompt"
            && self.asks(row, send)
            && sub["admission"] != "none";
        if refused {
            self.log(format!(
                "{}: refused, person-required ({} origin)",
                self.key(row),
                origin.word()
            ));
        }
        refused
    }

    /// The session ends: each event still queued is journaled, unheard (D6).
    pub fn journal_unheard_events(&mut self) {
        while let Some(raised) = self.extensions.events.queued.pop_front() {
            let row = &self.plan.module_events[raised.row];
            let key = format!(
                "{}.{}",
                self.plan.str(row.extension),
                self.plan.str(row.declared)
            );
            self.log(format!("{key}: unheard at the session's end"));
        }
    }

    /// `state.extensions`'s queued events, by extension.
    pub(super) fn queued_events(&self) -> Vec<(String, Json)> {
        (self.extensions.events.queued.iter())
            .map(|r| {
                let row = &self.plan.module_events[r.row];
                (
                    self.plan.str(row.extension).to_owned(),
                    serde_json::json!({
                        "event": self.plan.str(row.declared),
                        "args": r.args,
                        "origin": r.origin.word(),
                    }),
                )
            })
            .collect()
    }
}
