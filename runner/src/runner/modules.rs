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
use exact_plan::{ModulesRow, TypesId, Value};
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, BTreeSet};

mod admit;
mod agent;
mod answered;
mod arrival;
mod bound;
mod calls;
mod ceremony;
mod declared;
mod devices;
mod disclosed;
mod events;
mod fed;
mod handles;
mod invoke;
mod keyed;
mod letgo;
mod members;
mod network;
mod permits;
mod pictures;
mod poll;
mod presence;
mod processes;
mod samples;
mod services;
mod sheet;
mod spans;
mod stale;
#[cfg(test)]
mod substitute_tests;
mod values;
mod views;
mod wakes;
pub use admit::CallOrigin;
pub(super) use admit::OriginsSaved;
use admit::{Origins, Route};
pub use calls::CallHow;
pub use declared::AppState;
pub(super) use declared::CHOSEN;
pub(super) use disclosed::STATED;
pub(super) use events::EMITTED;
pub use presence::{PresenceAnswer, PresenceAsk};
pub(super) use services::CLAIMED;
use std::sync::{Arc, Mutex};
use values::{absent_is_none, fill, set_path};

/// The key of the answer `decide` gives a send that lists what it answers,
/// which its caller turns into the entry (`region_list.rs`, `listed_or`).
const LISTS: &str = "exact:lists";
pub use wakes::WakeHow;
pub(super) use wakes::{EXPIRED, TOLD};

/// The continuation tokens the runner keeps for its held module requests:
/// a data source's count up from zero, far below, and its reserved ones
/// (`BACKGROUND`) sit at the top, above.
const TOKENS: std::ops::Range<u64> = 1 << 62..1 << 63;

#[cfg(test)]
thread_local! {
    /// Tests: how many substitutes were parsed from the plan's text.
    pub(crate) static SUBSTITUTE_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The runner's module state for this session (LLP 1124 D7: until cleared
/// or the session ends).
#[derive(Default)]
pub(super) struct Extensions {
    /// `@exact/child-process`'s children and clock (LLP 1133.007).
    processes: processes::Processes,
    /// Each module's substitute as it reads on the host target, by the plan's
    /// string: parsed once, since the plan's strings never change under a
    /// runner (a new plan is a new runner); cleared when the target changes,
    /// so it holds at most one entry a module row.
    substitutes: std::cell::RefCell<std::collections::HashMap<u32, std::sync::Arc<Json>>>,
    /// The agent's preferred values, by `extension.source`.
    preferred: BTreeMap<String, Json>,
    /// Each declared permission's state, by its name: `prompt` when absent.
    permission: BTreeMap<String, String>,
    /// The facts a person's answer kept beside a state the platform hides
    /// (`keyed.rs`): `shared` and `since`, by permission.
    facts: BTreeMap<String, Json>,
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
    /// What the declared substitute keeps (LLP 1133.004 §7): keyed lists,
    /// values, and the effects held on a permission ticket.
    declared: declared::Declared,
    /// Module events not yet heard (D6).
    pub(super) events: events::Events,
    /// Wakes, running and to come (LLP 1133.017 D5).
    wakes: wakes::Wakes,
    /// Foreground-service claims under the agent (LLP 1133.033 D21).
    services: services::Services,
    /// What bound implementations disclose of their own objects (`disclosed.rs`).
    disclosed: disclosed::Disclosed,
    /// Tool calls, running and to come (LLP 1133.023 D7).
    calls: calls::Calls,
    /// Each bound live source's Contract readers, by resource, with the
    /// arguments each last asked with (`bound.rs`).
    readers: BTreeMap<String, BTreeMap<usize, String>>,
    /// The argument groups each bound live source's implementation was
    /// asked for and not yet told are gone.
    groups: BTreeMap<String, BTreeSet<String>>,
    /// Who each call comes from (`admit.rs`).
    origins: Origins,
    /// Requests whose reply carries handles, by ticket (`handles.rs`).
    provisional: BTreeMap<u64, handles::Provisional>,
    /// Their replies, as they left the implementation.
    arrived: handles::Arrived,
    /// Each polled source's last ask, on the session's clock (`poll.rs`).
    polls: BTreeMap<String, f64>,
    /// When each live source's last answer goes stale, on the session's
    /// clock; infinite once asked (`stale.rs`).
    stale: BTreeMap<String, f64>,
    /// What each package view's implementation was told (`views.rs`).
    views: views::Views,
    /// The polled sources whose last answer was an error, not asked again by
    /// a poll (`poll.rs`).
    failed: BTreeSet<String>,
    /// Presence secrets: the host's backend and the calls on it (`presence.rs`).
    presence: presence::Presence,
    /// Sampled sources' cursors, tracks and canvas inputs (`samples.rs`).
    samples: samples::Samples,
    /// The ceremonies and signals held or answered, by held token, with
    /// their extension (`ceremony.rs`).
    ceremonies: Vec<(u64, String, Json)>,
    /// The sends answered after a span, and the calls kept (`spans.rs`).
    spans: spans::Spans,
    /// The devices remembered and open, their readers and writes (`devices.rs`).
    devices: devices::Devices,
    /// The pictures admitted and refused, by extension (`pictures.rs`).
    pictures: pictures::Pictures,
    /// The canvases packages feed, and their leases (`fed.rs`).
    fed: fed::Fed,
    /// The handles whose work may call the app's handlers, and the handlers
    /// running (`invoke.rs`).
    invocations: invoke::Invocations,
    /// The payment sheets the substitute holds (`sheet.rs`).
    sheets: sheet::Sheets,
    /// The last number a forwarded send was given, and each send in flight
    /// by ticket: its `extension.send` and number, which a `cancel` names
    /// when it is let go (`bound.rs`).
    requests: u64,
    sent: BTreeMap<u64, (String, u64)>,
    /// Each polled source's Contract readers whose last answer is within
    /// its `while`, by resource (`poll.rs`).
    within: BTreeMap<String, BTreeMap<usize, bool>>,
    /// What arrivals keep (`arrival.rs`), and the data-code calls held, by
    /// the request that made them (`letgo.rs`).
    arrivals: arrival::Arrivals,
    letgo: letgo::LetGo,
}

/// A Contract read or send of a module's name, as admission saw it.
struct Called<'a> {
    target: Target,
    source: &'a str,
    ty: TypesId,
    origin: CallOrigin,
    timeout: Option<f64>,
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
    /// The invocation it is part of (`admit.rs`); 0 for none.
    id: u64,
    /// The call's arguments: a denial fails it only where they ask for what
    /// the permission gates (`permission.continues`), and which of an
    /// interface's permissions it asks (`permission_of`).
    args: Json,
    /// The permission it waits on or asked, by name; empty for none.
    permission: String,
    /// A data-code call's request, whose reply carries handles (`handles.rs`).
    request: Option<u64>,
    /// A ceremony waiting behind a modal one, with no ticket yet (`ceremony.rs`).
    queued: bool,
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
    /// A send that is a person's check (`check`: LLP 1133.010 §7), answered
    /// by a tap: `allow` passes it, each other choice fails it.
    Check,
    /// A sheet the person answers (`ceremony`: LLP 1133.011 §9), with a
    /// typed response or a tap that fails it.
    Ceremony,
    /// A send that shows the person a choice the implementation then
    /// answers from (`choose`: the limited selection, LLP 1133.019 §10): the
    /// agent's `type @t` sets the agent word it names; `cancel` leaves it.
    Choose,
    /// A send the substitute answers after its span (`spans.rs`).
    Span,
    /// A device module's chooser, or its permission dialog at an open on
    /// Android (`devices.rs`).
    Device,
}

impl Waits {
    fn word(self) -> &'static str {
        match self {
            Waits::Permission => "permission",
            Waits::Value => "value",
            Waits::Raise => "raise",
            Waits::Check => "check",
            Waits::Ceremony => "ceremony",
            Waits::Choose => "choose",
            Waits::Span => "span",
            Waits::Device => "device",
        }
    }
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
        let mut args = self.args_json(&row, args);
        self.clamp_bounds(&row, &mut args);
        self.resolve_of(&row, &mut args);
        let send = matches!(target, Target::Mutation(_));
        let timeout = send.then(|| self.timeout(&row, &args)).flatten();
        let origin = self.origin_for(Some(target));
        let reader = match target {
            Target::Resource(i) => Some(samples::Reader::Resource(i)),
            Target::Mutation(_) => None,
        };
        // A sampled source's reader is known to its answer (`samples.rs`).
        self.extensions.samples.asking = reader.map(|r| (r, origin));
        self.sampled_ask(&row, &args);
        let causes = self.causes_for(target);
        let decision = self.calling(causes, |r| r.admitted(&row, &args, origin, send, timeout));
        let called = Called {
            target,
            source,
            ty,
            origin,
            timeout,
        };
        let out = self.module_decided(called, &row, args, decision);
        self.extensions.samples.asking = None;
        out
    }

    /// [`Self::module_target`] once admission decided.
    fn module_decided(
        &mut self,
        called: Called,
        row: &ModulesRow,
        args: Json,
        decision: Decision,
    ) -> Result<Answer, DataError> {
        let Called {
            target,
            source,
            ty,
            origin,
            timeout,
        } = called;
        let (row, key, route) = (row.clone(), self.key(row), self.route(row));
        let what = match target {
            Target::Resource(i) => format!("query {}", self.plan.str(self.plan.resources[i].name)),
            Target::Mutation(m) => format!("send {}", self.plan.str(self.plan.mutations[m].name)),
        };
        let reader = match target {
            Target::Resource(i) => Some(i),
            Target::Mutation(_) => None,
        };
        let feeds = !self.substitute(&row)["feeds"].is_null();
        match decision {
            Decision::Forward => {
                self.log(format!("{what}: {key} (native)"));
                if feeds {
                    self.fed_send(&row, &args, origin);
                }
                Ok(Answer::Later(self.bound_request(&row, args, reader)))
            }
            Decision::Now(json) => {
                let json = self.listed_or(&row, &args, json);
                // The runner's own answer (an implementation's permission status
                // under the agent) is the substitute's, not a refusal.
                let how = if route == Route::Substitute || error_code(&json).is_none() {
                    "substituted"
                } else {
                    "refused"
                };
                self.log(format!("{what}: {key} ({how})"));
                if error_code(&json).is_none() && route == Route::Substitute {
                    self.sampled_answered(&row, &json);
                    self.sampled_reader(&row, &args);
                    if feeds {
                        self.fed_send(&row, &args, origin);
                        self.feed(&row, &args, origin);
                    }
                }
                self.answer_json(ty, &key, &json).map(Answer::Now)
            }
            Decision::Hold(waits) => {
                let held_args = args.clone();
                let forward =
                    (route == Route::Agent).then(|| self.forward_body(&row, args, reader));
                Ok(Answer::Later(self.hold_module(
                    target, source, waits, timeout, forward, origin, held_args,
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
    fn decide_timed(
        &self,
        row: &ModulesRow,
        send: bool,
        timeout: Option<f64>,
        args: &Json,
    ) -> Decision {
        match self.decide(row, send, args) {
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
        self.spans_due(now_ms);
        self.sheet_deadlines(now_ms);
        let due: Vec<(u64, u64)> = (self.extensions.held.iter())
            .filter(|h| h.waits != Waits::Span && h.deadline.is_some_and(|d| d <= now_ms))
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
        &mut self,
        target: Target,
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
        let json = self.presence_status(&row, json);
        self.note_poll_answer(&row, target, &json);
        if let (Target::Resource(i), None) = (target, error_code(&json)) {
            self.extensions.samples.asking = Some((samples::Reader::Resource(i), CallOrigin::App));
            self.sampled_answered(&row, &json);
            self.extensions.samples.asking = None;
        }
        self.answer_json(ty, &key, &json).map(Answer::Now)
    }

    fn module_row(&self, source: &str) -> ModulesRow {
        self.plan
            .module_named(source)
            .expect("a module's name")
            .clone()
    }

    /// `extension.source`, as `prefer` and the journal name it.
    pub(super) fn key(&self, row: &ModulesRow) -> String {
        format!(
            "{}.{}",
            self.plan.str(row.extension),
            self.plan.str(row.declared)
        )
    }

    fn substitute(&self, row: &ModulesRow) -> std::sync::Arc<Json> {
        let key = row.substitute.0;
        if let Some(sub) = self.extensions.substitutes.borrow().get(&key) {
            return sub.clone();
        }
        #[cfg(test)]
        SUBSTITUTE_PARSES.with(|n| n.set(n.get() + 1));
        let mut sub = serde_json::from_str(self.plan.str(row.substitute)).unwrap_or_default();
        self.this_target(&mut sub);
        let sub = std::sync::Arc::new(sub);
        (self.extensions.substitutes.borrow_mut()).insert(key, sub.clone());
        sub
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

    /// §6's transitions for a read (or a send) of `row` now.
    fn decide(&self, row: &ModulesRow, send: bool, args: &Json) -> Decision {
        if self.unavailable(row) || self.unsupported_here(row) {
            return Decision::Now(json!({ "error": "NotSupportedError" }));
        }
        if let Some(decision) = self.decide_permission(row, send, args) {
            return decision;
        }
        let sub = self.substitute(row);
        if send && sub["ceremony"].is_object() {
            return self.decide_ceremony(row, &sub, args);
        }
        // A signal the substitute records (LLP 1133.011 §9) answers nothing.
        if send && sub["records"] == true {
            return Decision::Now(Json::Null);
        }
        if let Some(decision) = self.decide_spans(row, &sub, send, args) {
            return decision;
        }
        self.decide_value(row, &sub, send, args)
    }

    /// What the runner's permission decides of `row` now: its status
    /// source's answer, a denial, or a ticket; `None` when it allows the call.
    fn decide_permission(&self, row: &ModulesRow, send: bool, args: &Json) -> Option<Decision> {
        if let Some(each) = self.decide_each(row, send, args) {
            return each;
        }
        let sub = self.substitute_for(row, args);
        let permission = &sub["permission"];
        let name = permission["name"].as_str()?;
        let state = self.permission_state(name);
        if !send && permission["status"].as_str() == Some(self.plan.str(row.declared)) {
            let mut status = self.value_of(row).unwrap_or(json!({}));
            // Each permission this source answers the state of, in its field:
            // a path into a record the status has (`android.notifications`).
            // A target whose restriction makes the module absent keeps its
            // own answer (`unknown`, LLP 1133.030 D7).
            let absent =
                self.substitute(row)["targets"][self.host_target()]["available"] == json!(false);
            for p in self.statuses_of(row).into_iter().filter(|_| !absent) {
                if let (Some(field), Some(name)) = (p["field"].as_str(), p["name"].as_str()) {
                    set_path(&mut status, field, json!(self.permission_state(name)));
                }
            }
            return Some(Decision::Now(status));
        }
        // A name admitted with no permission is never held or refused, and
        // a request answers the status in every state but `prompt` (LLP
        // 1133.004 §7).
        match sub["admission"].as_str() {
            Some("none") => return None,
            Some("request") if state != "prompt" => return None,
            _ => {}
        }
        // A denial that only gates part of what a call can ask fails a call
        // that asks for that part alone (`continues`: Face ID refused, the
        // passcode still works, LLP 1133.010 D12).
        // A request that answers the status answers it denied too (WebKit's
        // `requestPermission` resolves `denied`, LLP 1133.013 D11).
        let answers_denied = send && sub["deniedAnswers"] == true;
        if state == "denied" && self.denial_fails(&sub, args) && !answers_denied {
            return Some(Decision::Now(json!({ "error": permission["denied"] })));
        }
        let asks = self.asks(row, send);
        if network::asks_in(permission, &state) && asks && self.status_asks(row, &sub) {
            return Some(Decision::Hold(Waits::Permission));
        }
        // A name that is no request fails until the state is granted, where
        // the permission says so, naming the request that asks (D11).
        (state == "prompt" && !asks && permission["whilePrompt"].is_object())
            .then(|| Decision::Now(permission["whilePrompt"].clone()))
    }

    /// Whether a call of `row` may show the platform's prompt.
    fn asks(&self, row: &ModulesRow, send: bool) -> bool {
        match self.plan.str(row.request) {
            "always" => send,
            "when-prompt" => !send,
            _ => false,
        }
    }

    /// Whether a denial fails a call with `args`: always, unless the
    /// permission `continues` for a call that does not set that option.
    fn denial_fails(&self, sub: &Json, args: &Json) -> bool {
        match sub["permission"]["continues"].as_str() {
            Some(option) => args[option] == true,
            None => true,
        }
    }

    /// Whether the platform would ask now: the status fields the
    /// permission's `asks` names hold (Face ID asks; Touch ID never does).
    fn status_asks(&self, row: &ModulesRow, sub: &Json) -> bool {
        let Some(asks) = sub["permission"]["asks"].as_object() else {
            return true;
        };
        let status = (sub["permission"]["status"].as_str())
            .and_then(|s| self.sibling(row, s))
            .and_then(|s| self.value_of(&s))
            .unwrap_or_default();
        asks.iter().all(|(k, v)| status.get(k) == Some(v))
    }

    /// What answers `row` once its permission allows it.
    fn decide_value(&self, row: &ModulesRow, sub: &Json, send: bool, args: &Json) -> Decision {
        if let Some(arrival) = self.arrival_decide(row, sub, send, args) {
            return arrival;
        }
        if let Some(choose) = self.decide_choose(row, send) {
            return choose;
        }
        if !send && sub["samples"].is_object() {
            return Decision::Now(self.sampled(row, args));
        }
        // A send that feeds a canvas's input answers which (`samples.rs`).
        if send && sub["feeds"].is_object() {
            let field = |k: &str| (sub["feeds"][k].as_str()).map(|o| args[o].clone());
            return Decision::Now(
                json!({ "surface": field("surface"), "sensor": field("sensor") }),
            );
        }
        // A person's check, unless the status says the device has none.
        if send && sub["check"].is_object() {
            let status = (sub["permission"]["status"].as_str())
                .and_then(|s| self.sibling(row, s))
                .and_then(|s| self.value_of(&s))
                .unwrap_or_default();
            for (field, by_value) in sub["check"]["unless"].as_object().into_iter().flatten() {
                let said = status.get(field).map(Json::to_string).unwrap_or_default();
                if let Some(code) = by_value.get(&said) {
                    return Decision::Now(json!({ "error": code }));
                }
            }
            return Decision::Hold(Waits::Check);
        }
        if let Some(value) = (!send)
            .then(|| self.network_value(row, sub, args))
            .flatten()
        {
            return Decision::Now(value);
        }
        if !send && sub["counts"].is_object() {
            return Decision::Now(self.counted(row, sub, args));
        }
        if !send {
            return Decision::Now(
                self.value_of(row)
                    .unwrap_or_else(|| json!({ "error": "NotSupportedError" })),
            );
        }
        // A send that answers another source's value, raising it when it
        // matches `raise.when` (D10): held under the raise's ticket.
        if let Some(answers) = sub["answers"].as_str().and_then(|a| self.sibling(row, a)) {
            let Decision::Now(value) = self.decide(&answers, false, &json!({})) else {
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
        // A send that lists what it answers does so once admitted
        // (`region_list.rs`), where the caller can change the state.
        if sub["list"].is_object() {
            return Decision::Now(json!({ LISTS: true }));
        }
        // A send with nothing preferred answers its declared default (LLP
        // 1133.021 §12: a geocode answers `[]`).
        if !sub["default"].is_null() {
            return Decision::Now(sub["default"].clone());
        }
        Decision::Now(json!({ "error": "NotSupportedError" }))
    }

    /// A source's preferred value, completed by its `fill`; else its
    /// default. An interface that is its own substitute takes a partial
    /// preferred value (LLP 1133.009 D10 item 11), laid over the default
    /// where the runner answers itself (a status), as does a `partial`
    /// source's. A default has this target's restrictions laid over it
    /// (`targets`). Its `facts` are the runner's own, over either.
    fn value_of(&self, row: &ModulesRow) -> Option<Json> {
        let sub = self.substitute(row);
        if let Some(listed) = self.listed_value(row, &sub) {
            return Some(listed);
        }
        let default = (!sub["default"].is_null()).then(|| {
            let mut default = sub["default"].clone();
            if let Some(restricted) = sub["targets"].get(self.host_target()) {
                admit::overlay(&mut default, restricted);
            }
            default
        });
        let partial = sub["partial"] == true
            || (sub["implementation"] == true && default.as_ref().is_some_and(Json::is_object));
        let mut value = match self.preferred(row) {
            Some(p) if partial && error_code(&p).is_none() => {
                let mut value = default.unwrap_or(json!({}));
                admit::overlay(&mut value, &p);
                value
            }
            Some(p) => p,
            None => default?,
        };
        for (field, fact) in sub["facts"].as_object().into_iter().flatten() {
            let known = match fact.as_str() {
                // The host keeps locked secrets (LLP 1133.010 D7).
                Some("presence-secrets") => json!(self.presence_secrets()),
                _ => continue,
            };
            if let Some(object) = value.as_object_mut() {
                object.insert(field.clone(), known);
            }
        }
        // A login item's status with the app's registration (LLP 1133.025 §11).
        if sub["login"] == "status" {
            value = self.login_status(value);
        }
        Some(value)
    }

    fn preferred(&self, row: &ModulesRow) -> Option<Json> {
        let mut value = self.extensions.preferred.get(&self.key(row))?.clone();
        if error_code(&value).is_none() {
            self.fill_all(row, &mut value);
        }
        Some(value)
    }

    /// Hold a module request for the agent under a fresh device ticket: work
    /// under a continuation token of the runner's. `forward` is the body the
    /// implementation takes once the ticket allows the call.
    #[allow(clippy::too_many_arguments)]
    fn hold_module(
        &mut self,
        target: Target,
        source: &str,
        waits: Waits,
        timeout: Option<f64>,
        forward: Option<Vec<u8>>,
        origin: CallOrigin,
        args: Json,
    ) -> Request {
        let row = self.module_row(source);
        let token = TOKENS.start + self.extensions.next;
        self.extensions.next += 1;
        let name = self.target_name(target);
        let permission = self.permission_name(&row, &args);
        // One sheet at a time (`ceremony.rs`): behind a modal one, no ticket yet.
        let queued = waits == Waits::Ceremony && self.ceremony_begins(&row);
        let (hold, deadline) = match waits {
            Waits::Span => (u64::MAX, Some(self.span_deadline(&row, &args))),
            _ if queued => (u64::MAX, None),
            _ => (
                self.device_hold(&row, name, waits, origin, &permission, &args),
                timeout
                    .filter(|_| waits == Waits::Value)
                    .map(|t| self.now_ms + t),
            ),
        };
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
            id: self.invocation_id(),
            args,
            permission,
            request: None,
            queued,
        });
        self.arrival_claimed(&row);
        if waits == Waits::Ceremony && !queued {
            self.sheet_opened(token);
        }
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
        permission: &str,
        call: &Json,
    ) -> u64 {
        if let Some(hold) = self.shared_hold(waits, permission) {
            return hold;
        }
        let sub = self.substitute(row);
        let declared = self.plan.str(row.declared).to_owned();
        let checks = check_choices(&sub);
        let taps = self.ceremony_choices(row, &sub);
        let taps: Vec<&str> = taps.iter().map(String::as_str).collect();
        // A permission's further answers (`choices`), after allow and deny.
        let mut asked = vec!["allow".to_owned(), "deny".to_owned()];
        asked.extend(
            (self.permission_named(row, permission)["choices"].as_object())
                .into_iter()
                .flat_map(|c| c.keys().cloned()),
        );
        let asked: Vec<&str> = asked.iter().map(String::as_str).collect();
        // A sheet's taps besides `cancel` (LLP 1133.018 D13 item 4).
        let sheet = self.sheet_choices(row);
        let sheet: Vec<&str> = sheet.iter().map(String::as_str).collect();
        let (more, typed) = Self::ticket_taps(&sub);
        let more: Vec<&str> = more.iter().map(String::as_str).collect();
        let (capability, args, choices, value) = match waits {
            // A span holds no device ticket (`spans.rs`).
            Waits::Span => return u64::MAX,
            Waits::Permission => {
                // What `allow` grants this request (`asks`, D13 item 3).
                let line = self.asked_line(row);
                let grants = self.permission_named(row, permission);
                let asks = self.answer_fields(&grants, "allow", line.as_deref());
                let mut args = json!({ "capability": permission, "request": declared, "origin": origin.word() });
                if let (Some(asks), true) = (asks, line.is_some()) {
                    args["asks"] = Json::Object(asks);
                }
                // A request asking several together names each (`keyed.rs`).
                if self.is_each(row) {
                    args["permissions"] = json!(permission.split(',').collect::<Vec<_>>());
                }
                ("permission".to_owned(), args, &asked[..], self.is_each(row))
            }
            Waits::Choose => (
                sub["choose"]["ticket"]
                    .as_str()
                    .unwrap_or("choose")
                    .to_owned(),
                // The sheet shows what the call asked (the properties, the event).
                json!({ "request": declared, "origin": origin.word(), "options": call, "target": self.host_target() }),
                &sheet[..],
                sub["choose"]["typed"] != false,
            ),
            Waits::Value => (
                sub["ticket"].as_str().unwrap_or("value").to_owned(),
                json!({ "request": declared }),
                &more[..],
                typed,
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
            Waits::Check => (
                sub["check"]["ticket"]
                    .as_str()
                    .unwrap_or("check")
                    .to_owned(),
                json!({ "request": declared, "origin": origin.word() }),
                &checks[..],
                false,
            ),
            Waits::Ceremony => (
                (sub["ceremony"]["ticket"].as_str())
                    .unwrap_or("ceremony")
                    .to_owned(),
                self.ceremony_args(row, call, origin),
                &taps[..],
                true,
            ),
            Waits::Device => {
                let (capability, args, choices) = self.device_ticket(row, origin, call);
                let choices: Vec<&str> = choices.iter().map(String::as_str).collect();
                let ticket = self.hold(&capability, None, &args.to_string(), &choices, false);
                if let Some(h) = self.device_holds.iter_mut().find(|h| h.ticket == ticket) {
                    h.name = name;
                }
                return ticket;
            }
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
            return Some(self.released(token, release));
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
            .map(|(token, release, _)| (token, self.released(token, release)))
            .collect()
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
        let answer = self.ceremony_ended(&held, answer);
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
        let mut args = v.get("args").cloned().unwrap_or(json!({}));
        self.clamp_bounds(&row, &mut args);
        self.resolve_of(&row, &mut args);
        let origin = self.data_origin();
        let causes = self.data_causes();
        // A send that answers nothing (LLP 1133.004 D5) settles once done,
        // as does one that lists what it answers (`region_list.rs`).
        if self.is_effect(&row) || self.lists_answer(&row) {
            return Some(self.calling(causes, |r| r.effect_call(&row, args, origin)));
        }
        let held_args = args.clone();
        let timeout = self.timeout(&row, &args);
        let route = self.route(&row);
        let reader = self.data_reader(&row, &args);
        self.extensions.samples.asking = Some((reader.clone(), origin));
        self.sampled_ask(&row, &args);
        // The call's request, when its reply carries handles (`handles.rs`).
        let request = self.claim_request(body);
        let decision = self.calling(causes, |r| r.admitted(&row, &args, origin, send, timeout));
        let out = match decision {
            Decision::Forward => {
                if !send {
                    self.bound_read(&key);
                }
                let sampled = self.samples_of(&row);
                // The implementation hears the origin admission read, and a
                // Contract send keeps the number it went out with (`bound.rs`).
                let mut body = self.as_origin(origin, |r| r.forward_body(&row, args, None));
                if let Some(n) = v["request"].as_u64() {
                    body = numbered(&body, n);
                }
                let anchored = self.anchor_body(&row, &body, origin);
                // A reply carrying handles grants them the handlers its call
                // listed (`invoke.rs`); a watched one's `watched` grants them.
                let work = match request {
                    Some(ticket) => self.watched(body, ticket),
                    None => self.granting(&row, &body.clone(), self.to_implementation(body)),
                };
                let work = self.unanchoring(anchored, work);
                Some(match (sampled, reader) {
                    (Some(decl), samples::Reader::Data(id)) => {
                        self.cursored(work, id, decl["cursor"].as_str().unwrap_or("sequence"))
                    }
                    _ => work,
                })
            }
            Decision::Now(answer) => {
                let how = if route == Route::Substitute || error_code(&answer).is_none() {
                    "substituted"
                } else {
                    "refused"
                };
                self.log(format!("native {key} ({how})"));
                if error_code(&answer).is_none() {
                    self.sampled_answered(&row, &answer);
                }
                let sub = self.substitute(&row);
                if sub["ceremony"].is_object() || sub["records"] == true {
                    let token = TOKENS.start + self.extensions.next;
                    self.extensions.next += 1;
                    let outcome = error_code(&answer).unwrap_or("recorded").to_owned();
                    self.record_ceremony(token, &row, &args, &outcome);
                }
                Some(now(self.completed(&row, answer)))
            }
            Decision::Hold(waits) => {
                let token = TOKENS.start + self.extensions.next;
                self.extensions.next += 1;
                let permission = self.permission_name(&row, &args);
                // One sheet at a time (`ceremony.rs`): behind a modal one, no ticket yet.
                let queued = waits == Waits::Ceremony && self.ceremony_begins(&row);
                let hold = match queued || waits == Waits::Span {
                    true => u64::MAX,
                    false => self.device_hold(&row, key, waits, origin, &permission, &args),
                };
                let slot = Arc::new(Mutex::new(Pending::default()));
                let deadline = match waits {
                    Waits::Span => Some(self.span_deadline(&row, &args)),
                    _ => timeout
                        .filter(|_| waits == Waits::Value)
                        .map(|t| self.now_ms + t),
                };
                let forward = (route == Route::Agent).then(|| self.forward_body(&row, args, None));
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
                    id: self.invocation_id(),
                    args: held_args.clone(),
                    permission,
                    request,
                    queued,
                });
                self.note_data_hold(token, body);
                self.arrival_claimed(&row);
                if waits == Waits::Ceremony {
                    self.record_ceremony(
                        token,
                        &row,
                        &held_args,
                        if queued { "queued" } else { "held" },
                    );
                    if !queued {
                        self.sheet_opened(token);
                    }
                }
                Some(Dispatch::Run(Work::Later(Box::new(move |reply| {
                    let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
                    match s.outcome.take() {
                        Some(outcome) => reply.send(outcome),
                        None => s.reply = Some(reply),
                    }
                }))))
            }
        };
        self.extensions.samples.asking = None;
        out
    }

    /// `work`, a data-code read of a sampled source, whose reply sets its
    /// reader's cursor as it leaves the implementation (`samples.rs`).
    fn cursored(&self, work: Dispatch, id: String, field: &str) -> Dispatch {
        let Dispatch::Run(Work::Later(run)) = work else {
            return work;
        };
        let (cursors, field) = (self.data_cursors(), field.to_owned());
        Dispatch::Run(Work::Later(Box::new(move |reply| {
            run(Reply::new(move |outcome| {
                if let Outcome::Response(Response { body, .. }) = &outcome {
                    let at = serde_json::from_slice::<Json>(body)
                        .ok()
                        .and_then(|v| v[&field].as_f64());
                    if let (Some(at), Ok(mut cursors)) = (at, cursors.lock()) {
                        cursors.insert(id, at);
                    }
                }
                reply.send(outcome);
            }));
        })))
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
        if h.waits == Waits::Ceremony {
            let row = self.module_row(&h.source);
            let typed = self.ceremony_typed(&row, &h.args, h.token, text);
            return Some(typed.map(|_| ()).map_err(|e| format!("@{ticket}: {e}")));
        }
        if h.waits == Waits::Choose {
            let row = self.module_row(&h.source);
            let typed = self.choose_typed(&row, &h.args, text);
            return Some(typed.map_err(|e| format!("@{ticket}: {e}")));
        }
        if h.waits == Waits::Permission && self.is_each(&self.module_row(&h.source)) {
            let typed = self.each_typed(&h.permission, text);
            return Some(typed.map(|_| ()).map_err(|e| format!("@{ticket}: {e}")));
        }
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

    /// `text`, the agent's answer to a value ticket: JSON (a record, or a
    /// list for a source answering one, as `reverseGeocode`'s places), or
    /// the `short` form, numbers by comma into the declared paths; checked,
    /// filled, as the source's shape takes it.
    fn typed(&self, row: &ModulesRow, text: &str) -> Result<Json, String> {
        let key = self.key(row);
        let value = match serde_json::from_str::<Json>(text.trim()) {
            Ok(v) if v.is_object() || v.is_array() => v,
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
        self.release_sampled();
        self.dispose_dropped();
        self.cancel_let_go();
        self.let_go_data_calls();
        self.prune_origins();
        self.sheets_let_go();
        let gone: Vec<u64> = (self.extensions.held.iter())
            .filter(|h| h.reply.is_none() && !self.sheet_kept(h.token))
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
            let message = match json["message"].as_str() {
                Some(m) => format!("{key}: {code}: {m}"),
                None => format!("{key}: {code}"),
            };
            return Err(DataError::Failed(
                FailureCode::Error,
                message,
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

/// The code of an error answer, `{"error": "POSITION_UNAVAILABLE"}`, which
/// may say more in a `message`.
/// Each platform's own code may travel beside it (`apple`, `android`; LLP
/// 1133.011 D1).
fn error_code(json: &Json) -> Option<&str> {
    let object = json.as_object()?;
    let only =
        (object.keys()).all(|k| matches!(k.as_str(), "error" | "message" | "apple" | "android"));
    only.then(|| object.get("error")?.as_str())?
}

/// A forwarded body with the send's number `n`.
fn numbered(body: &[u8], n: u64) -> Vec<u8> {
    let mut v: Json = serde_json::from_slice(body).unwrap_or_default();
    v["request"] = json!(n);
    v.to_string().into_bytes()
}

/// The choices a `check` ticket's tap takes besides `cancel`: `allow`, and
/// each failure it declares.
fn check_choices(sub: &Json) -> Vec<&'static str> {
    let mut out = vec!["allow"];
    for choice in ["fail", "lockout"] {
        if !sub["check"]["choices"][choice].is_null() {
            out.push(choice);
        }
    }
    out
}
