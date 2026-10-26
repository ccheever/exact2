//! Tool calls (LLP 1133.023 D7, LLP 1111 D4): a door hands the runner
//! `{tool, args, door}`, and the runner runs it as an invocation of origin
//! `tool`, as a wake's event is: one at a time with the wakes, as its own
//! commit, done when it owns nothing.
//!
//! - **Waits** while a wake or another call runs, up to [`WAIT`] on the
//!   session's clock, then is refused `busy`; and while the data source is
//!   not ready, as a cold call is delivered after the data module loads.
//! - **Validate:** the tool exists and is available, the door is one its
//!   face names (any door is the agent's), the face's digest matches, the
//!   arguments are its action's (`crate::tools`), and neither `returns` nor
//!   `fails when` reads a pending resource, nor is the module's background
//!   storage out (`busy`). A refusal here is certainty `none`.
//! - **Dispatch:** the action runs as one commit of origin `tool`, stamped
//!   with the call's invocation id (`admit.rs`), which every request,
//!   `then` and queued send it makes carries, and every commit that answers
//!   one of those. The commands D4a skips are not run and are listed.
//! - **Complete** when no work stamped with its id is in flight: `fails
//!   when` and `returns` are read once. Background storage out then is
//!   `failed`, `uncertain` (`background`). Background storage is the
//!   module's ticket for storage no answer awaits (`background.rs`, LLP
//!   1097 D5), which no invocation can own; an answer's own storage is in
//!   its answer, which the call owns or does not.
//! - **Superseded** when a commit not its own lets go of work it owns,
//!   taken when that commit stands: `uncertain` for a send, `applied` for a
//!   resource's ask. **Timeout** [`BOUND`] after dispatch. A name that needs
//!   a person refuses the call `person-required`, `uncertain`.

use super::super::{CommitReceipt, DataSource, Runner, RunnerError};
use super::{CallOrigin, Json};
use serde_json::json;
use std::collections::VecDeque;

/// How long a call waits for the invocation before it (D7).
pub(in crate::runner) const WAIT: f64 = 5_000.0;
/// A call's bound from its dispatch (LLP 1111 D4).
const BOUND: f64 = 10_000.0;
/// The calls `state.intents` keeps, newest last.
const KEPT: usize = 20;

/// The commands a call's commit does not run (LLP 1111 D4a): each moves
/// the person's view.
const SKIPPED: &[&str] = &[
    "focus",
    "blur",
    "scrollIntoView",
    "selectText",
    "setSelectionRange",
    "fastSeek",
    "load",
    "haptic",
];
/// The commands it runs as a press would; any other is refused.
const RUN: &[&str] = &[
    "postMessage",
    "stopSounds",
    "preventDefault",
    "stopPropagation",
];

/// How a door hands the runner a call.
#[derive(Clone, Debug, Default)]
pub struct CallHow {
    /// The door: `agent`, `apple`, `android`, `web`.
    pub door: String,
    /// The host booted the session for the call.
    pub cold: bool,
    /// No view of the session is visible: timers stop while it runs.
    pub hidden: bool,
    /// The face's digest the door was compiled with, where it has one.
    pub digest: Option<String>,
}

struct Call {
    id: u64,
    /// Its row in the plan's `tools`.
    tool: usize,
    args: Json,
    how: CallHow,
    arrived: f64,
    dispatched: Option<f64>,
    skipped: Vec<String>,
}

#[derive(Default)]
pub(in crate::runner) struct Calls {
    waiting: VecDeque<Call>,
    current: Option<Call>,
    /// The calls that ended, for `state.intents` and the door's `called`.
    ended: VecDeque<Json>,
}

impl<D: DataSource> Runner<D> {
    /// A door's call of `tool` with `args`, a JSON object: queued, and its
    /// invocation id answered. `Err` for a tool the plan does not name.
    pub fn call_tool(&mut self, tool: &str, args: &str, how: CallHow) -> Result<u64, String> {
        let Some(row) = (self.plan.tools.iter()).position(|t| self.plan.str(t.name) == tool) else {
            let names: Vec<&str> = (self.plan.tools.iter())
                .map(|t| self.plan.str(t.name))
                .collect();
            return Err(match names.is_empty() {
                true => format!("no tool `{tool}`: this app declares none"),
                false => format!("no tool `{tool}`; its tools: {}", names.join(", ")),
            });
        };
        let args = serde_json::from_str(args).unwrap_or(Json::Null);
        let id = self.next_invocation();
        self.log(format!("tool:system:{} {tool}: arrived", how.door));
        self.extensions.calls.waiting.push_back(Call {
            id,
            tool: row,
            args,
            how,
            arrived: self.now_ms,
            dispatched: None,
            skipped: Vec::new(),
        });
        self.native_slot().changed("");
        Ok(id)
    }

    /// Whether a call is between dispatch and its end.
    pub(in crate::runner) fn call_running(&self) -> bool {
        self.extensions.calls.current.is_some()
    }

    /// Run what is due of the calls: the next once nothing else runs, and
    /// the end of the one running once it owns nothing or its time is up.
    pub(in crate::runner) fn step_calls(&mut self) -> (Vec<CommitReceipt>, Option<RunnerError>) {
        let mut receipts = Vec::new();
        loop {
            if let Some(call) = self.extensions.calls.current.as_ref() {
                let (id, at) = (call.id, call.dispatched.unwrap_or(self.now_ms));
                if self.now_ms >= at + BOUND {
                    self.end_call("timeout", "uncertain", "", Json::Null);
                    continue;
                }
                if self.invocation_owns(id) || !self.complete_call() {
                    break;
                }
                continue;
            }
            let Some(next) = self.extensions.calls.waiting.front() else {
                break;
            };
            if self.wake_running() || !self.data.ready() {
                if self.now_ms >= next.arrived + WAIT {
                    let call = self.extensions.calls.waiting.pop_front().expect("front");
                    self.extensions.calls.current = Some(call);
                    self.end_call("refused", "none", "busy", Json::Null);
                    continue;
                }
                break;
            }
            let call = self.extensions.calls.waiting.pop_front().expect("front");
            self.extensions.calls.current = Some(call);
            match self.dispatch_call() {
                Ok(Some(receipt)) => receipts.push(receipt),
                Ok(None) => {}
                Err(e) => return (receipts, Some(e)),
            }
        }
        (receipts, None)
    }

    /// Validate the current call and run its action (module doc).
    fn dispatch_call(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        let call = self.extensions.calls.current.as_ref().expect("a call");
        let (id, row, door) = (call.id, call.tool, call.how.door.clone());
        let tool = self.plan.tools[row].clone();
        let derive = |d: Option<exact_plan::DerivesId>| d.map(|d| self.derive_at(d.0 as usize));
        if derive(tool.available) == Some(Some(&exact_plan::Value::Bool(false))) {
            return Ok(self.refuse_call("unavailable", "it is not available now"));
        }
        if let Some(why) = self.face_refused(row, &door, call.how.digest.as_deref()) {
            return Ok(self.refuse_call("unavailable", &why));
        }
        let action = self.plan.action(tool.action).clone();
        let mut fields = Vec::new();
        for p in action.params.iter() {
            let param = self.plan.param(p);
            let name = self.plan.str(param.name).to_owned();
            match exact_js_value::Shape::from_plan(&self.plan, param.ty) {
                Ok(shape) => fields.push((name, shape)),
                Err(e) => return Ok(self.refuse_call("arguments", &e)),
            }
        }
        let call_args = call.args.clone();
        let shape = exact_js_value::Shape::Record(fields);
        if let Err(why) = crate::tools::check(&call_args, &shape, "") {
            return Ok(self.refuse_call("arguments", &why));
        }
        let mut args = Vec::new();
        for p in action.params.iter() {
            let param = self.plan.param(p);
            let json = &call_args[self.plan.str(param.name)];
            match self.event_arg(param.ty, json) {
                Ok(v) => args.push(v),
                Err(why) => return Ok(self.refuse_call("arguments", &why)),
            }
        }
        let read = [tool.returns, tool.fails];
        if read.iter().any(|d| derive(*d) == Some(None)) {
            return Ok(self.refuse_call("busy", "what it reports is still being answered"));
        }
        if self.background_ticket().is_some() {
            return Ok(self.refuse_call("busy", "the app's storage is still writing"));
        }
        let name = self.plan.str(tool.name).to_owned();
        let call = self.extensions.calls.current.as_mut().expect("a call");
        call.dispatched = Some(self.now_ms);
        self.log(format!("tool:system:{door} {name} dispatched"));
        let result = self.as_invocation(CallOrigin::Tool, id, |r| {
            r.run_action(tool.action, args, &[])
        });
        match result {
            Ok(receipt) => Ok(Some(receipt)),
            Err(e) => {
                self.end_call("failed", "uncertain", "refused-commit", Json::Null);
                Err(e)
            }
        }
    }

    /// Why the face refuses the current call at `door`, if it does (LLP
    /// 1133.023 D8): a door it does not name, or a digest not the plan's.
    /// The agent's door takes every tool.
    fn face_refused(&self, row: usize, door: &str, digest: Option<&str>) -> Option<String> {
        if door == "agent" {
            return None;
        }
        let face: Json =
            serde_json::from_str(self.plan.str(self.plan.tools[row].face)).unwrap_or_default();
        if face.is_null() {
            return Some(format!(
                "it has no face, so the {door} door does not offer it"
            ));
        }
        let named = face["doors"]
            .as_array()
            .is_none_or(|d| d.iter().any(|x| x == door));
        if !named {
            return Some(format!("its face does not name the {door} door"));
        }
        let ours = face["digest"].as_str().unwrap_or_default();
        match digest {
            Some(given) if given != ours => Some(format!(
                "face: the door's digest {given} is not the plan's {ours}"
            )),
            _ => None,
        }
    }

    /// The current call is refused before dispatch.
    fn refuse_call(&mut self, reason: &str, message: &str) -> Option<CommitReceipt> {
        let name = self.current_tool_name();
        self.log(format!("tool {name}: refused, {reason} ({message})"));
        self.end_call("refused", "none", reason, json!(message));
        None
    }

    fn current_tool_name(&self) -> String {
        (self.extensions.calls.current.as_ref())
            .map(|c| self.plan.str(self.plan.tools[c.tool].name).to_owned())
            .unwrap_or_default()
    }

    /// The current call owns nothing: read what it reports, once. `false`
    /// while `fails when` or `returns` reads a pending resource.
    fn complete_call(&mut self) -> bool {
        let row = self.extensions.calls.current.as_ref().expect("a call").tool;
        let tool = self.plan.tools[row].clone();
        if self.background_ticket().is_some() {
            self.end_call("failed", "uncertain", "background", Json::Null);
            return true;
        }
        let fails = tool.fails.map(|d| self.derive_at(d.0 as usize).cloned());
        let returns = tool.returns.map(|d| self.derive_at(d.0 as usize).cloned());
        if matches!(fails, Some(None)) || matches!(returns, Some(None)) {
            return false;
        }
        let value = match (tool.returns, returns.flatten()) {
            (Some(d), Some(v)) => {
                let mut s = String::new();
                crate::agent::typed_json(
                    &self.plan,
                    self.plan.derives[d.0 as usize].ty,
                    &v,
                    &mut s,
                );
                serde_json::from_str(&s).unwrap_or(Json::Null)
            }
            _ => Json::Null,
        };
        match fails.flatten() {
            Some(exact_plan::Value::Bool(true)) => self.end_call("failed", "uncertain", "", value),
            _ => self.end_call("ok", "applied", "", value),
        }
        true
    }

    /// The current call ends `outcome`, with its certainty and reason.
    fn end_call(&mut self, outcome: &str, certainty: &str, reason: &str, value: Json) {
        let Some(call) = self.extensions.calls.current.take() else {
            return;
        };
        let name = self.plan.str(self.plan.tools[call.tool].name).to_owned();
        let message = match (outcome, value.as_str()) {
            ("refused", Some(m)) => m.to_owned(),
            (_, _) if certainty == "uncertain" => {
                "It may or may not have taken effect: read before calling again.".into()
            }
            _ => String::new(),
        };
        let value = if outcome == "refused" {
            Json::Null
        } else {
            value
        };
        self.log(format!("tool {name} {outcome} {certainty}"));
        let ended = json!({
            "id": call.id,
            "tool": name,
            "door": call.how.door,
            "origin": CallOrigin::Tool.word(),
            "cold": call.how.cold,
            "outcome": outcome,
            "certainty": certainty,
            "reason": reason,
            "stale": false,
            "skipped": call.skipped,
            "value": value,
            "message": message,
            "at": self.time.epoch_at_zero + self.now_ms,
        });
        let calls = &mut self.extensions.calls;
        calls.ended.push_back(ended);
        while calls.ended.len() > KEPT {
            calls.ended.pop_front();
        }
        self.native_slot().changed("");
    }

    /// Invocation `id` was let go of by a commit that stood: the running
    /// call ends `superseded`, `uncertain` when what was let go was a send.
    pub(in crate::runner) fn call_superseded(&mut self, id: u64, send: bool) {
        if (self.extensions.calls.current.as_ref()).is_some_and(|c| c.id == id) {
            let certainty = if send { "uncertain" } else { "applied" };
            self.end_call("superseded", certainty, "", Json::Null);
        }
    }

    /// A name that needs a person was called by the running call's work.
    pub(in crate::runner) fn call_person_required(&mut self) {
        let id = self.invocation_id();
        if (self.extensions.calls.current.as_ref()).is_some_and(|c| c.id == id && id != 0) {
            self.end_call("refused", "uncertain", "person-required", Json::Null);
        }
    }

    /// Take off this commit's commands from `first` that a call does not
    /// run, when the commit is the running call's (LLP 1111 D4a).
    pub(in crate::runner) fn skip_in_call(&mut self, first: usize) {
        let id = self.invocation_id();
        if id == 0 || !(self.extensions.calls.current.as_ref()).is_some_and(|c| c.id == id) {
            return;
        }
        let mut i = first;
        while i < self.commands.len() {
            let name = self.commands[i].name.clone();
            if RUN.contains(&name.as_str()) {
                i += 1;
                continue;
            }
            self.commands.remove(i);
            if SKIPPED.contains(&name.as_str()) {
                let call = self.extensions.calls.current.as_mut().expect("a call");
                call.skipped.push(name.clone());
                self.log(format!("{name}: skipped in a tool call"));
            } else {
                self.log(format!("{name}: refused: in a tool call"));
            }
        }
    }

    /// Whether a running call is in a session no one sees: timers stop.
    pub(in crate::runner) fn call_stops_timers(&self) -> bool {
        (self.extensions.calls.current.as_ref()).is_some_and(|c| c.how.hidden)
    }

    /// When the host should next advance for the calls.
    pub(in crate::runner) fn call_due(&self) -> Option<f64> {
        let calls = &self.extensions.calls;
        if let Some(call) = calls.current.as_ref() {
            let deadline = call.dispatched.unwrap_or(self.now_ms) + BOUND;
            return Some(match self.invocation_owns(call.id) {
                true => deadline,
                false => self.now_ms,
            });
        }
        let next = calls.waiting.front()?;
        Some(match self.wake_running() || !self.data.ready() {
            true => next.arrived + WAIT,
            false => self.now_ms,
        })
    }

    /// Call `id` as its door polls it: LLP 1111 D4's envelope once it has
    /// ended, else `{call, state}` with `waiting` or `running`.
    fn call_answer(&self, id: u64) -> Result<String, String> {
        let calls = &self.extensions.calls;
        if let Some(ended) = calls.ended.iter().find(|c| c["id"] == id) {
            return Ok(ended.to_string());
        }
        let state = match () {
            _ if calls.current.as_ref().is_some_and(|c| c.id == id) => "running",
            _ if calls.waiting.iter().any(|c| c.id == id) => "waiting",
            _ => return Err(format!("no call {id}")),
        };
        Ok(json!({ "call": id, "state": state }).to_string())
    }

    /// The agent's `call` (`{"op":"call","tool","args","door"}`, queued
    /// as the door would hand it), a launch's `--call '<tool> <json>'`
    /// (`{"op":"launchEntry","launch":"call","entry"}`: a cold call in a
    /// session no one sees, as a system's call boots one), a door's poll of
    /// its call (`{"op":"called","id"}`) and `entities`
    /// (`{"op":"entities","kind","matching"}`, what the content index's
    /// query answers a system). `None` for any other op.
    pub fn call_agent(&mut self, op: &str, request: &str) -> Option<Result<String, String>> {
        let v: Json = serde_json::from_str(request).ok()?;
        match op {
            "launchEntry" if v["launch"] == "call" => {
                let entry = v["entry"].as_str().unwrap_or_default().trim();
                let (tool, args) = entry
                    .split_once(char::is_whitespace)
                    .unwrap_or((entry, "{}"));
                let how = CallHow {
                    door: v["door"].as_str().unwrap_or("agent").to_owned(),
                    cold: true,
                    hidden: true,
                    digest: None,
                };
                self.log(format!("launch --call ({tool})"));
                let id = self.call_tool(tool, args.trim(), how);
                Some(id.map(|id| json!({ "call": id, "tool": tool, "cold": true }).to_string()))
            }
            "call" => {
                let tool = v["tool"].as_str().unwrap_or_default().to_owned();
                let args = match &v["args"] {
                    Json::String(text) => text.clone(),
                    args => args.to_string(),
                };
                let how = CallHow {
                    door: v["door"].as_str().unwrap_or("agent").to_owned(),
                    cold: v["cold"] == true,
                    hidden: v["hidden"] == true,
                    digest: v["digest"].as_str().map(str::to_owned),
                };
                let id = self.call_tool(&tool, &args, how);
                Some(
                    id.map(|id| {
                        json!({ "call": id, "tool": tool, "state": "waiting" }).to_string()
                    }),
                )
            }
            "called" => Some(match v["id"].as_u64() {
                Some(id) => self.call_answer(id),
                None => Err("called needs the call's id".into()),
            }),
            "entities" => {
                let Some(kind) = v["kind"].as_str() else {
                    return Some(Err("entities <kind> [<text>]".into()));
                };
                let found = self.index_entities(kind, v["matching"].as_str());
                Some(Ok(json!({ "entities": found }).to_string()))
            }
            _ => None,
        }
    }

    /// `state.intents`: each tool with its word, modifiers, availability
    /// and face, and the calls running, waiting and ended, never their
    /// arguments (LLP 1133.023 §10).
    pub fn intents_state(&self) -> Option<Json> {
        if self.plan.tools.is_empty() {
            return None;
        }
        let tools: Vec<Json> = (self.plan.tools.iter())
            .map(|t| {
                let available = t.available.map(|d| self.derive_at(d.0 as usize).cloned());
                let face: Json = serde_json::from_str(self.plan.str(t.face)).unwrap_or_default();
                json!({
                    "name": self.plan.str(t.name),
                    "consequence": self.plan.str(t.consequence),
                    "additive": t.additive,
                    "idempotent": t.idempotent,
                    "closed": t.closed,
                    "description": self.plan.str(t.description),
                    "action": self.plan.str(self.plan.action(t.action).name),
                    "available": !matches!(available, Some(Some(exact_plan::Value::Bool(false)))),
                    "face": face,
                })
            })
            .collect();
        let calls = &self.extensions.calls;
        let open = |c: &Call, state: &str| json!({ "id": c.id, "tool": self.plan.str(self.plan.tools[c.tool].name), "door": c.how.door, "origin": "tool", "state": state });
        let mut all: Vec<Json> = calls.ended.iter().cloned().collect();
        all.extend(calls.current.iter().map(|c| open(c, "running")));
        all.extend(calls.waiting.iter().map(|c| open(c, "waiting")));
        Some(json!({ "tools": tools, "calls": all }))
    }
}
