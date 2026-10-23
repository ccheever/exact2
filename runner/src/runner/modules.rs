//! A native module's sources and sends (LLP 1133.002 D13, §6; LLP 1124 D5,
//! D7, D9). The plan's `modules` table names the sources a module answers,
//! not the app's data. Until a host binds an implementation (LLP 1133.001
//! D4), the runner answers them itself, as each name's declared substitute
//! says (`exact.json`), so the standard library's modules and anyone's are
//! substituted alike (LLP 1133.001 D8):
//!
//! - **Permission.** An interface that declares `permission` has one state,
//!   `prompt` until `prefer permission.<name>` or a ticket sets it. Its
//!   status source answers with that state in its `field`. While `denied`,
//!   every other name fails with the declared code. While `prompt`, a send
//!   whose `request` is `always`, or a read of a source whose `request` is
//!   `when-prompt`, is held under a `permission` ticket: `tap @t allow` sets
//!   `granted` (and the status fields `allow` names, by the app's grant
//!   line), and the request continues; `deny` sets `denied`; `cancel` leaves
//!   `prompt`; both fail the request.
//! - **Values.** A source answers the agent's preferred value (`prefer
//!   extension`), else its `default`. A send with `from` answers that
//!   source's preferred value; with none, it is held under its `ticket`,
//!   whose `type @t` answer (JSON, or the `short` form, `48.85,2.35`)
//!   answers it and becomes the source's preferred value. `fill` completes
//!   a preferred value (`timestamp` "now" is the agent clock's date).
//!
//! A held request is work under a continuation token of the runner's own:
//! the host dispatches it and parks it as any held work, and
//! [`Runner::release_work`] hands it back with its outcome. What changes a
//! module's answers announces its sources' keys, so the host asks their
//! readers again through [`Runner::apply_announced`].

use super::{DataError, DataSource, Runner, RunnerError, Target};
use crate::failure::FailureCode;
use crate::request::{Answer, Dispatch, Outcome, Reply, Request, Response, Work};
use crate::HoldAnswer;
use exact_plan::{ModulesRow, TypesId, Value};
use serde_json::{json, Map, Value as Json};
use std::collections::{BTreeMap, BTreeSet};

mod admit;
mod agent;
mod bound;
pub use admit::CallOrigin;
use admit::{Origins, Route};
use std::sync::{Arc, Mutex};

/// The continuation tokens the runner keeps for its held module requests:
/// a data source's count up from zero, far below, and its reserved ones
/// (`BACKGROUND`) sit at the top, above.
const TOKENS: std::ops::Range<u64> = 1 << 62..1 << 63;

/// The runner's module state for this session (LLP 1124 D7: until cleared
/// or the session ends).
#[derive(Default)]
pub(super) struct Extensions {
    /// The agent's preferred values, by `extension.source`.
    preferred: BTreeMap<String, Json>,
    /// Each declared permission's state, by its name: `prompt` when absent.
    permission: BTreeMap<String, String>,
    /// Requests held for the agent.
    held: Vec<Held>,
    /// Held requests answered, with whether the host has dispatched them.
    released: Vec<(u64, Release, bool)>,
    next: u64,
    /// The extensions the host's own implementations answer (`bound.rs`).
    bound: BTreeSet<String>,
    /// The extensions whose implementations the host bound told they run
    /// under the agent: they answer an interface that is its own substitute
    /// (`admit.rs`).
    agent: BTreeSet<String>,
    /// The bound live sources that have had a reader, until released.
    live: BTreeSet<String>,
    /// Who each call comes from (`admit.rs`).
    origins: Origins,
}

/// A held request's outcome once the agent answered it.
enum Release {
    /// The substitute's answer.
    Answer(Outcome),
    /// Admitted: the call goes to the implementation, this body.
    Forward(Vec<u8>),
}

/// A module request the runner holds for the agent.
struct Held {
    /// Its continuation token; a data-code call's, its id.
    token: u64,
    /// A data-code call's reply, which the agent's answer completes; `None`
    /// for a Contract read or send, released through `release_work`.
    reply: Option<Arc<Mutex<Pending>>>,
    /// The device hold the agent answers, `@N`.
    hold: u64,
    /// Its source's name in the plan.
    source: String,
    send: bool,
    waits: Waits,
    dispatched: bool,
    /// The request's own timeout (its declared `timeout` option), ms.
    timeout: Option<f64>,
    /// When a value ticket fails `TIMEOUT` on the agent's clock: set when the
    /// value is first sought, after any permission ticket (D2).
    deadline: Option<f64>,
    /// The call's body for the implementation, once the ticket allows it.
    forward: Option<Vec<u8>>,
    origin: CallOrigin,
}

/// A held data-code call's two halves, whichever comes first: the host's
/// reply, or the answer to send through it.
#[derive(Default)]
struct Pending {
    reply: Option<Reply>,
    outcome: Option<Outcome>,
}

impl Pending {
    fn complete(slot: &Mutex<Pending>, outcome: Outcome) {
        let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
        match s.reply.take() {
            Some(reply) => reply.send(outcome),
            None => s.outcome = Some(outcome),
        }
    }
}

fn response(answer: &Json) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: answer.to_string().into_bytes(),
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Waits {
    Permission,
    Value,
    /// A send that raises what its `answers` source says (`raise`: precise
    /// location for a task, D10).
    Raise,
}

/// What a name's substitute decides now.
enum Decision {
    /// An answer, or `{"error": code}`.
    Now(Json),
    /// A ticket for the agent.
    Hold(Waits),
    /// The implementation answers.
    Forward,
}

impl Extensions {
    /// Whether `token` is a held module request (not I/O: no clock waits).
    pub(super) fn holds_token(&self, token: Option<u64>) -> bool {
        token.is_some_and(|t| self.held.iter().any(|h| h.token == t))
    }
}

impl<D: DataSource> Runner<D> {
    /// What answers resource `i`, whose source `source` is a module's name.
    pub(super) fn module_answer(
        &mut self,
        i: usize,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, RunnerError> {
        let resource = self.plan.str(self.plan.resources[i].name).to_string();
        let key = self.key(&self.module_row(source));
        // The readers of a module's source are asked again when what it
        // answers changes (`announce`, or the bound module's own topic).
        self.store.observe_topic(&key);
        let ty = self.plan.resources[i].ty;
        self.module_target(Target::Resource(i), source, args, ty)
            .map_err(|error| RunnerError::Data { resource, error })
    }

    /// What answers a send of mutation `m` to `source`, a module's name.
    pub(super) fn module_send(
        &mut self,
        m: usize,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let ty = self.plan.mutations[m].ty;
        self.module_target(Target::Mutation(m), source, args, ty)
    }

    /// A Contract read or send of a module's name: admitted (D14), then
    /// answered by its route: the implementation, the agent's ticket, or
    /// the substitute.
    fn module_target(
        &mut self,
        target: Target,
        source: &str,
        args: &[Value],
        ty: TypesId,
    ) -> Result<Answer, DataError> {
        let row = self.module_row(source);
        let key = self.key(&row);
        let args = self.args_json(&row, args);
        let send = matches!(target, Target::Mutation(_));
        let timeout = send.then(|| self.timeout(&row, &args)).flatten();
        let origin = self.origin_for(Some(target));
        let route = self.route(&row);
        let decision = self.admitted(&row, &args, origin, send, timeout);
        let what = match target {
            Target::Resource(i) => format!("query {}", self.plan.str(self.plan.resources[i].name)),
            Target::Mutation(m) => format!("send {}", self.plan.str(self.plan.mutations[m].name)),
        };
        match decision {
            Decision::Forward => {
                self.log(format!("{what}: {key} (native)"));
                Ok(Answer::Later(self.bound_request(&row, args)))
            }
            Decision::Now(json) => {
                let how = if route == Route::Substitute {
                    "substituted"
                } else {
                    "refused"
                };
                self.log(format!("{what}: {key} ({how})"));
                self.answer_json(ty, &key, &json).map(Answer::Now)
            }
            Decision::Hold(waits) => {
                let forward = (route == Route::Agent).then(|| self.forward_body(&row, args));
                Ok(Answer::Later(self.hold_module(
                    target, source, waits, timeout, forward, origin,
                )))
            }
        }
    }

    /// A call's arguments as JSON: its one options record, by field name.
    fn args_json(&self, row: &ModulesRow, args: &[Value]) -> Json {
        let source = &self.plan.sources[row.source.0 as usize];
        let (Some(arg), Some(param)) = (
            args.first(),
            self.plan.source_params.get(source.params.start as usize),
        ) else {
            return json!({});
        };
        (exact_js_value::Shape::from_plan(&self.plan, param.ty).ok())
            .and_then(|shape| exact_js_value::to_json(arg, &shape).ok())
            .unwrap_or(json!({}))
    }

    /// The request's timeout, ms: the option its declaration's `timeout` names.
    fn timeout(&self, row: &ModulesRow, args: &Json) -> Option<f64> {
        let option = self.substitute(row)["timeout"].as_str()?.to_owned();
        args.get(option)?.as_f64().filter(|t| *t >= 0.0)
    }

    /// [`Self::decide`], where a value ticket that would wait `timeout=0`
    /// fails `TIMEOUT` at once (D2: an eligible cached fix, or none).
    fn decide_timed(&self, row: &ModulesRow, send: bool, timeout: Option<f64>) -> Decision {
        match self.decide(row, send) {
            Decision::Hold(Waits::Value) if timeout == Some(0.0) => {
                Decision::Now(json!({ "error": "TIMEOUT" }))
            }
            decision => decision,
        }
    }

    /// Value tickets whose timeout has passed on the agent's clock (§6):
    /// each fails `TIMEOUT`, its ticket retired.
    pub(super) fn module_deadlines(&mut self, now_ms: f64) {
        if !self.native_slot().synced() {
            self.publish_sync();
        }
        let due: Vec<(u64, u64)> = (self.extensions.held.iter())
            .filter(|h| h.deadline.is_some_and(|d| d <= now_ms))
            .map(|h| (h.token, h.hold))
            .collect();
        for (token, hold) in due {
            self.log(format!("device position {hold}: timed out"));
            self.retire_hold(hold);
            self.release(token, json!({ "error": "TIMEOUT" }));
            self.native_slot().changed("");
        }
    }

    /// The reply to a held module request, as `fulfill` parses it.
    pub(super) fn module_parse(
        &self,
        ty: TypesId,
        source: &str,
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let row = self.module_row(source);
        let key = self.key(&row);
        let Outcome::Response(Response { body, .. }) = outcome else {
            return Err(DataError::Unavailable(format!("{key}: no answer")));
        };
        let json: Json = serde_json::from_slice(&body)
            .map_err(|e| DataError::Unavailable(format!("{key}: {e}")))?;
        let json = self.overlaid(&row, json);
        self.answer_json(ty, &key, &json).map(Answer::Now)
    }

    fn module_row(&self, source: &str) -> ModulesRow {
        self.plan
            .module_named(source)
            .expect("a module's name")
            .clone()
    }

    /// `extension.source`, as `prefer` and the journal name it.
    fn key(&self, row: &ModulesRow) -> String {
        format!(
            "{}.{}",
            self.plan.str(row.extension),
            self.plan.str(row.declared)
        )
    }

    fn substitute(&self, row: &ModulesRow) -> Json {
        serde_json::from_str(self.plan.str(row.substitute)).unwrap_or_default()
    }

    /// The row of the name `declared` that `row`'s extension declares.
    fn sibling(&self, row: &ModulesRow, declared: &str) -> Option<ModulesRow> {
        let extension = self.plan.str(row.extension);
        (self.plan.modules.iter())
            .find(|m| {
                self.plan.str(m.extension) == extension && self.plan.str(m.declared) == declared
            })
            .cloned()
    }

    fn permission_state(&self, name: &str) -> String {
        (self.extensions.permission.get(name).cloned()).unwrap_or_else(|| "prompt".into())
    }

    /// §6's transitions for a read (or a send) of `row` now.
    fn decide(&self, row: &ModulesRow, send: bool) -> Decision {
        self.decide_permission(row, send)
            .unwrap_or_else(|| self.decide_value(row, &self.substitute(row), send))
    }

    /// What the runner's permission decides of `row` now: its status
    /// source's answer, a denial, or a ticket; `None` when it allows the call.
    fn decide_permission(&self, row: &ModulesRow, send: bool) -> Option<Decision> {
        let sub = self.substitute(row);
        let permission = &sub["permission"];
        let name = permission["name"].as_str()?;
        let state = self.permission_state(name);
        if !send && permission["status"].as_str() == Some(self.plan.str(row.declared)) {
            let mut status = self.value_of(row).unwrap_or(json!({}));
            if let (Some(object), Some(field)) =
                (status.as_object_mut(), permission["field"].as_str())
            {
                object.insert(field.into(), json!(state));
            }
            return Some(Decision::Now(status));
        }
        if state == "denied" {
            return Some(Decision::Now(json!({ "error": permission["denied"] })));
        }
        let asks = match self.plan.str(row.request) {
            "always" => send,
            "when-prompt" => !send,
            _ => false,
        };
        (state == "prompt" && asks).then_some(Decision::Hold(Waits::Permission))
    }

    /// What answers `row` once its permission allows it.
    fn decide_value(&self, row: &ModulesRow, sub: &Json, send: bool) -> Decision {
        if !send {
            return Decision::Now(
                self.value_of(row)
                    .unwrap_or_else(|| json!({ "error": "NotSupportedError" })),
            );
        }
        // A send that answers another source's value, raising it when it
        // matches `raise.when` (D10): held under the raise's ticket.
        if let Some(answers) = sub["answers"].as_str().and_then(|a| self.sibling(row, a)) {
            let Decision::Now(value) = self.decide(&answers, false) else {
                return Decision::Now(json!({ "error": "NotSupportedError" }));
            };
            let when = sub["raise"]["when"].as_object();
            let matches = when.is_some_and(|w| w.iter().all(|(k, v)| value.get(k) == Some(v)));
            return if matches {
                Decision::Hold(Waits::Raise)
            } else {
                Decision::Now(value)
            };
        }
        let from = sub["from"].as_str().and_then(|f| self.sibling(row, f));
        if let Some(value) = from.as_ref().and_then(|f| self.preferred(f)) {
            return Decision::Now(value);
        }
        if sub["ticket"].is_string() && from.is_some() {
            return Decision::Hold(Waits::Value);
        }
        Decision::Now(json!({ "error": "NotSupportedError" }))
    }

    /// A source's preferred value, completed by its `fill`; else its default.
    fn value_of(&self, row: &ModulesRow) -> Option<Json> {
        self.preferred(row).or_else(|| {
            let default = &self.substitute(row)["default"];
            (!default.is_null()).then(|| default.clone())
        })
    }

    fn preferred(&self, row: &ModulesRow) -> Option<Json> {
        let mut value = self.extensions.preferred.get(&self.key(row))?.clone();
        if error_code(&value).is_none() {
            self.fill_all(row, &mut value);
        }
        Some(value)
    }

    /// `row`'s declared `fill`, where `value` leaves a path out.
    fn fill_all(&self, row: &ModulesRow, value: &mut Json) {
        let now = self.time.epoch_at_zero + self.now_ms;
        for (path, filler) in self.substitute(row)["fill"]
            .as_object()
            .into_iter()
            .flatten()
        {
            let filler = if filler == "now" {
                json!(now)
            } else {
                filler.clone()
            };
            fill(value, path, filler);
        }
    }

    /// Hold a module request for the agent under a fresh device ticket: work
    /// under a continuation token of the runner's. `forward` is the body the
    /// implementation takes once the ticket allows the call.
    fn hold_module(
        &mut self,
        target: Target,
        source: &str,
        waits: Waits,
        timeout: Option<f64>,
        forward: Option<Vec<u8>>,
        origin: CallOrigin,
    ) -> Request {
        let row = self.module_row(source);
        let token = TOKENS.start + self.extensions.next;
        self.extensions.next += 1;
        let name = self.target_name(target);
        let hold = self.device_hold(&row, name, waits, origin);
        let deadline = timeout
            .filter(|_| waits == Waits::Value)
            .map(|t| self.now_ms + t);
        self.extensions.held.push(Held {
            token,
            reply: None,
            hold,
            source: source.to_owned(),
            send: row.send,
            waits,
            dispatched: false,
            timeout,
            deadline,
            forward,
            origin,
        });
        let verb = if row.send { "send" } else { "source" };
        let body =
            json!({ "extension": self.plan.str(row.extension), verb: self.plan.str(row.declared) });
        // Ordered work a host runs, never transport: its body says what it holds.
        let mut request = Request::continuation(token);
        request.body = body.to_string().into_bytes();
        request
    }

    fn device_hold(
        &mut self,
        row: &ModulesRow,
        name: String,
        waits: Waits,
        origin: CallOrigin,
    ) -> u64 {
        let sub = self.substitute(row);
        let declared = self.plan.str(row.declared).to_owned();
        let (capability, args, choices, value) = match waits {
            Waits::Permission => (
                "permission".to_owned(),
                json!({ "capability": sub["permission"]["name"], "request": declared, "origin": origin.word() }),
                &["allow", "deny"][..],
                false,
            ),
            Waits::Value => (
                sub["ticket"].as_str().unwrap_or("value").to_owned(),
                json!({ "request": declared }),
                &[][..],
                true,
            ),
            Waits::Raise => (
                sub["raise"]["ticket"]
                    .as_str()
                    .unwrap_or("raise")
                    .to_owned(),
                json!({ "request": declared }),
                &["allow", "deny"][..],
                false,
            ),
        };
        let ticket = self.hold(&capability, None, &args.to_string(), choices, value);
        if let Some(h) = self.device_holds.iter_mut().find(|h| h.ticket == ticket) {
            h.name = name;
        }
        ticket
    }

    /// The host dispatches held module request `token`: answered already,
    /// or held until the agent answers. `None` for a data source's token.
    pub(super) fn module_dispatch(&mut self, token: u64) -> Option<Dispatch> {
        if !TOKENS.contains(&token) {
            return None;
        }
        if let Some(at) = self
            .extensions
            .released
            .iter()
            .position(|(t, _, _)| *t == token)
        {
            let (_, release, _) = self.extensions.released.remove(at);
            return Some(self.released(release));
        }
        match self.extensions.held.iter_mut().find(|h| h.token == token) {
            Some(h) => {
                h.dispatched = true;
                Some(Dispatch::Held)
            }
            None => Some(Dispatch::Missing),
        }
    }

    /// Held module requests the agent answered since the host dispatched them.
    pub(super) fn module_release(&mut self) -> Vec<(u64, Dispatch)> {
        let (out, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.extensions.released)
            .into_iter()
            .partition(|(_, _, dispatched)| *dispatched);
        self.extensions.released = keep;
        out.into_iter()
            .map(|(token, release, _)| (token, self.released(release)))
            .collect()
    }

    /// The work behind a released request: its answer, or its call of the
    /// implementation.
    fn released(&self, release: Release) -> Dispatch {
        match release {
            Release::Answer(outcome) => Dispatch::Run(Work::Now(Box::new(move || outcome))),
            Release::Forward(body) => self.to_implementation(body),
        }
    }

    /// `body` for the bound implementation, through the native slot.
    fn to_implementation(&self, body: Vec<u8>) -> Dispatch {
        let handler = self.native_slot().handler();
        Dispatch::Run(Work::Later(Box::new(move |reply| match handler {
            Some(handler) => handler(body, reply),
            None => reply.send(response(&json!({ "error": "NotSupportedError" }))),
        })))
    }

    fn release(&mut self, token: u64, answer: Json) {
        let Some(at) = self.extensions.held.iter().position(|h| h.token == token) else {
            return;
        };
        let held = self.extensions.held.remove(at);
        match held.reply {
            Some(slot) => {
                let row = self.module_row(&held.source);
                Pending::complete(&slot, response(&self.completed(&row, answer)));
            }
            None => self.extensions.released.push((
                token,
                Release::Answer(response(&answer)),
                held.dispatched,
            )),
        }
    }

    /// A held call the agent's ticket now allows: to the implementation.
    fn release_forward(&mut self, token: u64) {
        let Some(at) = self.extensions.held.iter().position(|h| h.token == token) else {
            return;
        };
        let held = self.extensions.held.remove(at);
        let Some(body) = held.forward else {
            return;
        };
        match held.reply {
            Some(slot) => {
                let Dispatch::Run(Work::Later(work)) = self.to_implementation(body) else {
                    return;
                };
                work(Reply::new(move |outcome| Pending::complete(&slot, outcome)));
            }
            None => {
                (self.extensions.released).push((token, Release::Forward(body), held.dispatched))
            }
        }
    }

    /// A call from data code (LLP 1133.002 D14): a native request whose body
    /// names a module's source or send, `{"extension": …, "source": …,
    /// "args": {…}}`, admitted here as a Contract read or send is, before any
    /// module. `None` for any other body.
    pub(super) fn module_call(&mut self, body: &[u8]) -> Option<Dispatch> {
        let v: Json = serde_json::from_slice(body).ok()?;
        let extension = v.get("extension")?.as_str()?;
        let (declared, send) = match (v["source"].as_str(), v["send"].as_str()) {
            (Some(name), None) => (name, false),
            (None, Some(name)) => (name, true),
            _ => return None,
        };
        let key = format!("{extension}.{declared}");
        let now = |answer: Json| Dispatch::Run(Work::Now(Box::new(move || response(&answer))));
        let Some(row) = self
            .plan
            .modules
            .iter()
            .find(|m| self.key(m) == key)
            .cloned()
        else {
            self.log(format!("{key}: not a module this app's plan names"));
            return Some(now(json!({ "error": "NotSupportedError" })));
        };
        if row.send != send {
            return Some(now(json!({ "error": "NotSupportedError" })));
        }
        let args = v.get("args").cloned().unwrap_or(json!({}));
        let timeout = self.timeout(&row, &args);
        let origin = self.data_origin();
        let route = self.route(&row);
        match self.admitted(&row, &args, origin, send, timeout) {
            Decision::Forward => {
                if !send {
                    self.bound_read(&key);
                }
                let body = self.forward_body(&row, args);
                Some(self.to_implementation(body))
            }
            Decision::Now(answer) => {
                let how = if route == Route::Substitute {
                    "substituted"
                } else {
                    "refused"
                };
                self.log(format!("native {key} ({how})"));
                Some(now(self.completed(&row, answer)))
            }
            Decision::Hold(waits) => {
                let token = TOKENS.start + self.extensions.next;
                self.extensions.next += 1;
                let hold = self.device_hold(&row, key, waits, origin);
                let slot = Arc::new(Mutex::new(Pending::default()));
                let deadline = timeout
                    .filter(|_| waits == Waits::Value)
                    .map(|t| self.now_ms + t);
                let forward = (route == Route::Agent).then(|| self.forward_body(&row, args));
                self.extensions.held.push(Held {
                    token,
                    reply: Some(slot.clone()),
                    hold,
                    source: self
                        .plan
                        .str(self.plan.sources[row.source.0 as usize].name)
                        .to_owned(),
                    send,
                    waits,
                    dispatched: true,
                    timeout,
                    deadline,
                    forward,
                    origin,
                });
                Some(Dispatch::Run(Work::Later(Box::new(move |reply| {
                    let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
                    match s.outcome.take() {
                        Some(outcome) => reply.send(outcome),
                        None => s.reply = Some(reply),
                    }
                }))))
            }
        }
    }

    /// The topic a data-code read of a module's source watches, its key.
    pub(super) fn module_topic(&self, body: &[u8]) -> Option<String> {
        let v: Json = serde_json::from_slice(body).ok()?;
        let key = format!(
            "{}.{}",
            v.get("extension")?.as_str()?,
            v.get("source")?.as_str()?
        );
        self.plan
            .modules
            .iter()
            .any(|m| !m.send && self.key(m) == key)
            .then_some(key)
    }

    /// `answer` as data code reads a value of `row`'s shape: what it leaves
    /// out of an option field is `null`, as an omitted word is.
    fn completed(&self, row: &ModulesRow, mut answer: Json) -> Json {
        if error_code(&answer).is_none() {
            let ty = self.plan.sources[row.source.0 as usize].ty;
            if let Ok(shape) = exact_js_value::Shape::from_plan(&self.plan, ty) {
                absent_is_none(&mut answer, &shape);
            }
        }
        answer
    }

    /// A typed answer to `ticket`, checked before the hold is spent, so it
    /// can be corrected. `None` when the hold is not a module's.
    pub(super) fn module_check(&self, ticket: u64, text: &str) -> Option<Result<(), String>> {
        let h = self.extensions.held.iter().find(|h| h.hold == ticket)?;
        if h.waits != Waits::Value {
            return Some(Err(format!("@{ticket}: answered by tap, not type")));
        }
        let row = self.module_row(&h.source);
        let from = self.sibling(&row, self.substitute(&row)["from"].as_str()?)?;
        Some(
            self.typed(&from, text)
                .map(|_| ())
                .map_err(|e| format!("@{ticket}: {e}")),
        )
    }

    /// `text`, the agent's answer to a value ticket: JSON, or the `short`
    /// form, numbers by comma into the declared paths; checked, filled, as
    /// the source's shape takes it.
    fn typed(&self, row: &ModulesRow, text: &str) -> Result<Json, String> {
        let key = self.key(row);
        let value = match serde_json::from_str::<Json>(text.trim()) {
            Ok(v) if v.is_object() => v,
            _ => {
                let short: Vec<String> = (self.substitute(row)["short"]
                    .as_array()
                    .into_iter()
                    .flatten())
                .filter_map(|p| p.as_str().map(str::to_owned))
                .collect();
                let numbers: Vec<f64> = text
                    .split(',')
                    .filter_map(|n| n.trim().parse().ok())
                    .collect();
                if short.is_empty()
                    || numbers.is_empty()
                    || numbers.len() > short.len()
                    || text.split(',').count() != numbers.len()
                {
                    let form = if short.is_empty() {
                        String::new()
                    } else {
                        format!(", or {}", short.join(","))
                    };
                    return Err(format!("{key}: type its JSON{form}"));
                }
                let mut v = json!({});
                for (path, n) in short.iter().zip(numbers) {
                    fill(&mut v, path, json!(n));
                }
                v
            }
        };
        let mut filled = value.clone();
        self.fill_all(row, &mut filled);
        let ty = self.plan.sources[row.source.0 as usize].ty;
        self.answer_json(ty, &key, &filled)
            .map_err(|e| e.to_string())?;
        Ok(value)
    }

    /// The agent answered hold `hold`, already spent: §6's transitions.
    pub(super) fn module_answered(&mut self, hold: u64, answer: &HoldAnswer) {
        let Some(h) = self.extensions.held.iter().find(|h| h.hold == hold) else {
            return;
        };
        let (token, source, waits) = (h.token, h.source.clone(), h.waits);
        let row = self.module_row(&source);
        let sub = self.substitute(&row);
        match (waits, answer) {
            (Waits::Permission, HoldAnswer::Choice(c)) => {
                let name = sub["permission"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                match c.as_str() {
                    "allow" => self.set_permission(&name, "granted"),
                    "deny" => self.set_permission(&name, "denied"),
                    // The dialog dismissed: the state stays, this request fails.
                    _ => self.release(token, json!({ "error": sub["permission"]["denied"] })),
                }
            }
            (Waits::Value, HoldAnswer::Choice(_)) => {
                let cancel =
                    (sub.get("cancel").cloned()).unwrap_or(json!({ "error": "NotSupportedError" }));
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
                let answer = match self.decide(&answers, false) {
                    Decision::Now(value) => value,
                    Decision::Hold(_) | Decision::Forward => {
                        json!({ "error": "NotSupportedError" })
                    }
                };
                self.release(token, answer);
                self.announce(&key);
            }
            (Waits::Permission | Waits::Raise, HoldAnswer::Value(_)) => {}
        }
    }

    /// Set permission `name`'s state: the permission tickets pending for it
    /// settle as `allow` or `deny` would, and every name of its module is
    /// asked again.
    fn set_permission(&mut self, name: &str, state: &str) {
        self.extensions
            .permission
            .insert(name.to_owned(), state.to_owned());
        self.log(format!("permission {name}: {state}"));
        let rows: Vec<ModulesRow> = (self.plan.modules.iter())
            .filter(|m| self.substitute(m)["permission"]["name"].as_str() == Some(name))
            .cloned()
            .collect();
        if state == "granted" {
            self.allow(&rows);
        }
        let held: Vec<(u64, u64, String, bool, Waits, Option<f64>)> = (self.extensions.held.iter())
            .map(|h| {
                (
                    h.token,
                    h.hold,
                    h.source.clone(),
                    h.send,
                    h.waits,
                    h.timeout,
                )
            })
            .collect();
        for (token, hold, source, send, waits, timeout) in held {
            let row = self.module_row(&source);
            if !rows.iter().any(|r| r.source == row.source) {
                continue;
            }
            match (state, waits) {
                ("granted", Waits::Permission) => {
                    self.retire_hold(hold);
                    let forward = (self.extensions.held.iter())
                        .any(|h| h.token == token && h.forward.is_some());
                    if forward {
                        self.release_forward(token);
                        continue;
                    }
                    match self.decide_timed(&row, send, timeout) {
                        Decision::Now(answer) => self.release(token, answer),
                        Decision::Forward => self.release_forward(token),
                        Decision::Hold(waits) => {
                            let target = (self.pending.iter())
                                .find(|p| p.continuation == Some(token))
                                .map(|p| p.target);
                            let reply = (self.extensions.held.iter())
                                .any(|h| h.token == token && h.reply.is_some());
                            let name = match (target, reply) {
                                (Some(target), _) => Some(self.target_name(target)),
                                (None, true) => Some(self.key(&row)),
                                (None, false) => None,
                            };
                            if let Some(name) = name {
                                let origin = (self.extensions.held.iter())
                                    .find(|h| h.token == token)
                                    .map_or(CallOrigin::App, |h| h.origin);
                                let hold = self.device_hold(&row, name, waits, origin);
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
                ("denied", _) => {
                    self.retire_hold(hold);
                    let code = self.substitute(&row)["permission"]["denied"].clone();
                    self.release(token, json!({ "error": code }));
                }
                _ => {}
            }
        }
        for row in &rows {
            let key = self.key(row);
            self.announce(&key);
        }
    }

    /// `allow` sets the status fields the app's grant line names (its
    /// approximate grant reads `approximate`), as a preferred status.
    fn allow(&mut self, rows: &[ModulesRow]) {
        let grants = self.data.grants().to_owned();
        for row in rows {
            let sub = self.substitute(row);
            let permission = &sub["permission"];
            if permission["status"].as_str() != Some(self.plan.str(row.declared)) {
                continue;
            }
            let mut status = self.value_of(row).unwrap_or(json!({}));
            // The state is the permission's, never a preferred value's.
            if let (Some(object), Some(field)) =
                (status.as_object_mut(), permission["field"].as_str())
            {
                object.remove(field);
            }
            for (field, by_line) in permission["allow"].as_object().into_iter().flatten() {
                // The longest line the app grants: the approximate one where both are.
                let line = (by_line.as_object().into_iter().flatten())
                    .filter(|(line, _)| {
                        grants
                            .lines()
                            .any(|g| g.split_whitespace().next() == Some(line.as_str()))
                    })
                    .max_by_key(|(line, _)| line.len());
                if let (Some((_, v)), Some(object)) = (line, status.as_object_mut()) {
                    object.insert(field.clone(), v.clone());
                }
            }
            let key = self.key(row);
            self.extensions.preferred.insert(key, status);
        }
    }

    /// Value tickets whose source now has a preferred value: each answers.
    fn settle_values(&mut self) {
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

    fn retire_hold(&mut self, ticket: u64) {
        if let Some(at) = self.device_holds.iter().position(|h| h.ticket == ticket) {
            let hold = self.device_holds.remove(at);
            self.log(format!("device {} {ticket} retired", hold.capability));
        }
    }

    /// Held module requests whose request is gone (superseded, its reader
    /// released, a refused commit): retired with their tickets (§6).
    pub(super) fn retire_modules(&mut self) {
        self.release_idle();
        self.prune_origins();
        let gone: Vec<u64> = (self.extensions.held.iter())
            .filter(|h| h.reply.is_none())
            .filter(|h| !self.pending.iter().any(|p| p.continuation == Some(h.token)))
            .map(|h| h.hold)
            .collect();
        for hold in gone {
            self.extensions.held.retain(|h| h.hold != hold);
            self.retire_hold(hold);
        }
    }

    /// `key` changed: its readers are asked again, through the host's
    /// announcement path, which also wakes the host for released work.
    fn announce(&self, key: &str) {
        let mut queue = self.announced.lock().unwrap_or_else(|e| e.into_inner());
        if !queue.iter().any(|t| t == key) {
            queue.push(key.to_owned());
        }
        drop(queue);
        self.publish_sync();
        self.native_slot().changed("");
    }

    /// `json`, an answer of type `ty`: a value of its shape, or `{"error":
    /// code}`. An option field the answer leaves out is `none`.
    fn answer_json(&self, ty: TypesId, key: &str, json: &Json) -> Result<Value, DataError> {
        if let Some(code) = error_code(json) {
            return Err(DataError::Failed(
                FailureCode::Error,
                format!("{key}: {code}"),
                code.to_owned(),
            ));
        }
        let shape =
            exact_js_value::Shape::from_plan(&self.plan, ty).map_err(DataError::Unavailable)?;
        let mut json = json.clone();
        absent_is_none(&mut json, &shape);
        let shaped = |why: String| {
            DataError::Failed(FailureCode::Shape, format!("{key}: {why}"), String::new())
        };
        let value = exact_js_value::from_json(&json, &shape).map_err(shaped)?;
        // The JSON's kinds are its shape's; an enum's members are the plan's.
        if !value.conforms(&self.plan, ty) {
            return Err(shaped(crate::conform::mismatch(&self.plan, &value, ty)));
        }
        Ok(value)
    }
}

/// The code of an error answer, `{"error": "POSITION_UNAVAILABLE"}`.
fn error_code(json: &Json) -> Option<&str> {
    let object = json.as_object()?;
    (object.len() == 1).then(|| object.get("error")?.as_str())?
}

/// Set `path` (`coords.accuracy`) in `value` to `filler` where it is absent.
fn fill(value: &mut Json, path: &str, filler: Json) {
    let mut at = value;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        let Some(object) = at.as_object_mut() else {
            return;
        };
        if parts.peek().is_none() {
            object.entry(part).or_insert(filler);
            return;
        }
        at = object
            .entry(part)
            .or_insert_with(|| Json::Object(Map::new()));
    }
}

/// A module's answer leaves out an option field it has nothing for (a
/// platform's words where another platform answers, a declared default):
/// absent is `none`, as an omitted word is (LLP 1133.002 D2).
fn absent_is_none(json: &mut Json, shape: &exact_js_value::Shape) {
    use exact_js_value::Shape;
    match (json, shape) {
        (Json::Object(map), Shape::Record(fields)) => {
            for (name, field) in fields {
                match map.get_mut(name) {
                    Some(value) => absent_is_none(value, field),
                    None if matches!(field, Shape::Option(_)) => {
                        map.insert(name.clone(), Json::Null);
                    }
                    None => {}
                }
            }
        }
        (Json::Array(items), Shape::List(item)) => {
            items.iter_mut().for_each(|v| absent_is_none(v, item))
        }
        (value, Shape::Option(inner)) if !value.is_null() => absent_is_none(value, inner),
        _ => {}
    }
}
