//! One admission for every call of a module's name, from every caller
//! (LLP 1133.002 D14), before anything answers it: the substitute, a ticket,
//! or a bound implementation, in production or under the agent.
//!
//! In order: the grant lines the name or its arguments list (LLP 1133.009
//! D10 item 9; the interface's own lines are the bake's, `grant-missing`),
//! and those an argument's value must be the target of (LLP 1133.011 D6,
//! `ceremony.rs`), which refuse with their own code;
//! the name's `origin` (`"app"`: a tool invocation or a wake is refused,
//! `person-required`, item 8; `{"whenAny", "is": "app"}`, only when the call
//! gives one of the options named, LLP 1133.012 D12 item 5); its
//! `foreground` (refused while the page is
//! hidden, item 3); each conditioned by its `when` where it has one; then the
//! permission. Every other refusal answers `NotAllowedError`, with the
//! message `person-required` for the origin's.
//!
//! **Routes.** A name is answered by its implementation as in production
//! where the host bound it so (outside the agent, or `--bind`), and the
//! platform decides its permission; by its implementation told it runs
//! under the agent where the interface is its own substitute
//! (`"substitute": "implementation"`, LLP 1133.006 D8, LLP 1133.008 D8 item
//! 9), behind the runner's permission and its tickets, the agent's
//! preferred value for the name laid over each answer; else by its
//! declared substitute (§6).
//!
//! **Origin** (D12). Each commit has one: a person's input or the app's own
//! code (`app`), a tool invocation, a wake. Each request carries its
//! commit's, an answer's commit carries its request's, and a re-ask the
//! device causes carries each reader's own last ask's. A data-code call
//! cannot be traced to its answer here, so it is admitted under the least
//! privileged origin in flight. A permission ticket is never raised for a
//! tool or a wake: the call fails `person-required` instead.
//!
//! **Invocations** (LLP 1133.023 D7). A wake's event and a tool call each
//! run as an invocation with an id, which its dispatch commit stamps on
//! every request, armed `then`, held call and queued send it makes; a
//! commit that answers a stamped request, or runs a stamped `then` or
//! queued send, is stamped too. A resource's re-ask by the device and a
//! model's handler (`invoke.rs`) carry none. What an invocation owns, and
//! what lets go of it, is read by id alone. The tables are part of a
//! commit's checkpoint, and an invocation is superseded only when the
//! commit that let go of its work stands (`let_go`).

use super::super::{CommitReceipt, DataSource, Runner, RunnerError, Target};
use super::{error_code, Decision, Json, ModulesRow, Waits};
use exact_plan::Value;
use serde_json::json;
use std::collections::BTreeMap;

/// Who a call comes from (LLP 1133.002 D12), least privileged first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallOrigin {
    /// The device: an event a package emitted (LLP 1133.004 D6).
    Device,
    /// A background wake (LLP 1133.017 D5).
    Wake,
    /// A tool invocation (LLP 1111).
    Tool,
    /// A person's input, or the app's own code in a foreground session.
    #[default]
    App,
}

impl CallOrigin {
    /// Its word in the journal, a ticket and `state.extensions`.
    pub fn word(self) -> &'static str {
        match self {
            CallOrigin::Device => "device",
            CallOrigin::Wake => "wake",
            CallOrigin::Tool => "tool",
            CallOrigin::App => "app",
        }
    }
}

/// The origins the runner keeps. Only those other than `app` are kept, so
/// an app with no tool or wake keeps nothing.
#[derive(Default)]
pub(in crate::runner) struct Origins {
    /// The commit being made.
    now: CallOrigin,
    /// Each request in flight, by ticket.
    tickets: BTreeMap<u64, CallOrigin>,
    /// Each resource's last ask, by resource.
    asked: BTreeMap<usize, CallOrigin>,
    /// A re-ask the device causes is being made: each resource asks under
    /// its own last ask's origin.
    reasking: bool,
    /// Each armed `then`, by mutation, whose answer's commit was not the
    /// app's: the `then` runs under it (LLP 1133.002 D12).
    thens: BTreeMap<usize, CallOrigin>,
    /// The invocation the commit being made is part of; 0 for none.
    id: u64,
    /// The last invocation id given.
    last_id: u64,
    /// Each stamped request in flight, by ticket.
    ids: BTreeMap<u64, u64>,
    /// Each stamped armed `then`, by mutation.
    then_ids: BTreeMap<usize, u64>,
    /// The invocations whose work a commit let go of, each with whether
    /// that work was a send; taken when the commit stands.
    let_go: Vec<(u64, bool)>,
}

/// The origin tables as a commit found them (`commit.rs`'s checkpoint).
pub(in crate::runner) struct OriginsSaved {
    tickets: BTreeMap<u64, CallOrigin>,
    asked: BTreeMap<usize, CallOrigin>,
    thens: BTreeMap<usize, CallOrigin>,
    ids: BTreeMap<u64, u64>,
    then_ids: BTreeMap<usize, u64>,
    let_go: usize,
}

/// What answers a module's name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::runner) enum Route {
    /// The host's implementation, as in production.
    Real,
    /// The host's implementation, told it runs under the agent.
    Agent,
    /// The declared substitute.
    Substitute,
}

impl<D: DataSource> Runner<D> {
    /// [`Runner::act`] as a commit of `origin`: a tool invocation's door
    /// (LLP 1111) or a wake's (LLP 1114.001) runs its action through this.
    pub fn act_from(
        &mut self,
        origin: CallOrigin,
        name: &str,
        args: Vec<Value>,
    ) -> Result<CommitReceipt, RunnerError> {
        let before = std::mem::replace(&mut self.extensions.origins.now, origin);
        let out = self.act(name, args);
        self.extensions.origins.now = before;
        out
    }

    /// The origin of the commit being made.
    pub fn call_origin(&self) -> CallOrigin {
        self.extensions.origins.now
    }

    /// Run `f` as a commit of `origin`, part of invocation `id` (0: none).
    pub(in crate::runner) fn as_invocation<T>(
        &mut self,
        origin: CallOrigin,
        id: u64,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let before = std::mem::replace(&mut self.extensions.origins.id, id);
        let out = self.as_origin(origin, f);
        self.extensions.origins.id = before;
        out
    }

    /// The invocation the commit being made is part of; 0 for none.
    pub(in crate::runner) fn invocation_id(&self) -> u64 {
        self.extensions.origins.id
    }

    /// A new invocation's id.
    pub(in crate::runner) fn next_invocation(&mut self) -> u64 {
        self.extensions.origins.last_id += 1;
        self.extensions.origins.last_id
    }

    /// The invocation request `ticket` is stamped with; 0 for none.
    pub(in crate::runner) fn ticket_id(&self, ticket: u64) -> u64 {
        self.extensions
            .origins
            .ids
            .get(&ticket)
            .copied()
            .unwrap_or(0)
    }

    /// Whether invocation `id` owns work in flight: a request, an armed
    /// `then`, a held call or a queued send stamped with it.
    pub(in crate::runner) fn invocation_owns(&self, id: u64) -> bool {
        !self.invocation_tickets(id).is_empty()
            || (self.extensions.origins.then_ids.iter())
                .any(|(m, i)| *i == id && self.then_due[*m].is_finite())
            || (self.extensions.held.iter()).any(|h| h.id == id)
            || self.queued_of(id)
    }

    /// The requests in flight stamped with invocation `id`, by ticket.
    pub(in crate::runner) fn invocation_tickets(&self, id: u64) -> Vec<u64> {
        let o = &self.extensions.origins;
        (self.pending.iter())
            .filter(|p| o.ids.get(&p.ticket) == Some(&id))
            .map(|p| p.ticket)
            .collect()
    }

    /// Drop the `then`s stamped with invocation `id`.
    pub(in crate::runner) fn drop_invocation_thens(&mut self, id: u64) {
        let thens: Vec<usize> = (self.extensions.origins.then_ids.iter())
            .filter(|(_, i)| **i == id)
            .map(|(m, _)| *m)
            .collect();
        for m in thens {
            self.extensions.origins.then_ids.remove(&m);
            self.extensions.origins.thens.remove(&m);
            self.then_due[m] = f64::INFINITY;
        }
    }

    /// Request `ticket` of `target` is let go by the commit being made: an
    /// invocation's request, let go by a commit not its own (a device's
    /// re-ask is none's), supersedes it when that commit stands.
    pub(in crate::runner) fn note_let_go(&mut self, ticket: u64, target: Target) {
        let o = &self.extensions.origins;
        let owner = o.ids.get(&ticket).copied().unwrap_or(0);
        let by = match target {
            Target::Resource(_) if o.reasking => 0,
            _ => o.id,
        };
        if owner != 0 && owner != by {
            let send = matches!(target, Target::Mutation(_));
            self.extensions.origins.let_go.push((owner, send));
        }
    }

    /// The origin tables, for a commit's checkpoint.
    pub(in crate::runner) fn origins_saved(&self) -> OriginsSaved {
        let o = &self.extensions.origins;
        OriginsSaved {
            tickets: o.tickets.clone(),
            asked: o.asked.clone(),
            thens: o.thens.clone(),
            ids: o.ids.clone(),
            then_ids: o.then_ids.clone(),
            let_go: o.let_go.len(),
        }
    }

    /// A refused commit's: the tables as it found them.
    pub(in crate::runner) fn origins_restore(&mut self, saved: OriginsSaved) {
        let o = &mut self.extensions.origins;
        o.tickets = saved.tickets;
        o.asked = saved.asked;
        o.thens = saved.thens;
        o.ids = saved.ids;
        o.then_ids = saved.then_ids;
        o.let_go.truncate(saved.let_go);
    }

    /// The invocations superseded by commits that stood: each ends so.
    pub(in crate::runner) fn invocations_let_go(&mut self) {
        for (id, send) in std::mem::take(&mut self.extensions.origins.let_go) {
            self.wake_superseded(id);
            self.call_superseded(id, send);
        }
    }

    /// Run `f` as a commit of `origin`.
    pub(in crate::runner) fn as_origin<T>(
        &mut self,
        origin: CallOrigin,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let before = std::mem::replace(&mut self.extensions.origins.now, origin);
        let out = f(self);
        self.extensions.origins.now = before;
        out
    }

    /// Run `f` as the device's re-ask of its readers (D12).
    pub(in crate::runner) fn reasking<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let before = std::mem::replace(&mut self.extensions.origins.reasking, true);
        let out = f(self);
        self.extensions.origins.reasking = before;
        out
    }

    /// The origin a request of `target` asked now carries.
    pub(in crate::runner) fn origin_for(&self, target: Option<Target>) -> CallOrigin {
        let o = &self.extensions.origins;
        match target {
            Some(Target::Resource(i)) if o.reasking => o.asked.get(&i).copied().unwrap_or_default(),
            _ => o.now,
        }
    }

    /// The origin of resource `i`'s last ask, when it was not the app's.
    pub(in crate::runner) fn origin_asked(&self, i: usize) -> Option<CallOrigin> {
        self.extensions.origins.asked.get(&i).copied()
    }

    /// The origin of request `ticket`'s answer, the commit it lands in.
    pub(in crate::runner) fn ticket_origin(&self, ticket: u64) -> CallOrigin {
        let o = &self.extensions.origins;
        o.tickets.get(&ticket).copied().unwrap_or_default()
    }

    /// Request `ticket` of `target` goes out: it keeps its origin, and a
    /// resource's ask is its readers' origin for the device's re-asks.
    pub(in crate::runner) fn note_asked(&mut self, target: Target, ticket: u64) {
        let origin = self.origin_for(Some(target));
        let o = &mut self.extensions.origins;
        if origin != CallOrigin::App {
            o.tickets.insert(ticket, origin);
        }
        let id = match target {
            Target::Resource(_) if o.reasking => 0,
            _ => o.id,
        };
        if id != 0 {
            o.ids.insert(ticket, id);
        }
        if let Target::Resource(i) = target {
            match origin {
                CallOrigin::App => o.asked.remove(&i),
                other => o.asked.insert(i, other),
            };
        }
    }

    /// A data-code call's origin: the least privileged of the commit being
    /// made, every request in flight and every handler running, since such
    /// a call cannot be traced to the answer that made it.
    pub(in crate::runner) fn data_origin(&self) -> CallOrigin {
        let o = &self.extensions.origins;
        (self.pending.iter())
            .filter_map(|p| o.tickets.get(&p.ticket).copied())
            .chain(self.tool_origin())
            .fold(o.now, CallOrigin::min)
    }

    /// Mutation `m`'s `then` is armed by the commit being made.
    pub(in crate::runner) fn note_then(&mut self, m: usize) {
        let o = &mut self.extensions.origins;
        match o.now {
            CallOrigin::App => o.thens.remove(&m),
            other => o.thens.insert(m, other),
        };
        match o.id {
            0 => o.then_ids.remove(&m),
            id => o.then_ids.insert(m, id),
        };
    }

    /// The origin and invocation mutation `m`'s armed `then` runs under;
    /// spent here.
    pub(in crate::runner) fn then_origin(&mut self, m: usize) -> (CallOrigin, u64) {
        let o = &mut self.extensions.origins;
        let origin = o.thens.remove(&m).unwrap_or_default();
        (origin, o.then_ids.remove(&m).unwrap_or(0))
    }

    /// Forget the origins of requests no longer in flight.
    pub(in crate::runner) fn prune_origins(&mut self) {
        let pending = &self.pending;
        let o = &mut self.extensions.origins;
        (o.tickets).retain(|t, _| pending.iter().any(|p| p.ticket == *t));
        (o.ids).retain(|t, _| pending.iter().any(|p| p.ticket == *t));
    }

    /// What answers `row` now (module doc).
    pub(in crate::runner) fn route(&self, row: &ModulesRow) -> Route {
        let extension = self.plan.str(row.extension);
        if self.extensions.bound.contains(extension) {
            Route::Real
        } else if self.extensions.agent.contains(extension)
            && self.substitute(row)["implementation"] == true
        {
            Route::Agent
        } else {
            Route::Substitute
        }
    }

    /// Admission (module doc) and then what answers: a refusal or the
    /// substitute's answer, a ticket, or the implementation.
    pub(super) fn admitted(
        &mut self,
        row: &ModulesRow,
        args: &Json,
        origin: CallOrigin,
        send: bool,
        timeout: Option<f64>,
    ) -> Decision {
        // Presence is not delegable (LLP 1133.010 D12): a person's check
        // fails for a tool or a wake before anything is asked, in every state.
        if self.substitute(row)["origin"] == "person" && origin != CallOrigin::App {
            self.log(format!(
                "{}: refused, person-required ({} origin)",
                self.key(row),
                origin.word()
            ));
            self.call_person_required();
            return Decision::Now(
                json!({ "error": "PERMISSION_DENIED", "message": "person-required" }),
            );
        }
        // A grant matched by the argument's value fails with its own code
        // (`SecurityError` for an RP ID, LLP 1133.011 D6).
        if let Err((code, why)) = self.value_grants(row, args) {
            self.log(format!("{}: refused, {why}", self.key(row)));
            return Decision::Now(json!({ "error": code, "message": why }));
        }
        if let Err(why) = self.admit(row, args, origin) {
            self.log(format!("{}: refused, {why}", self.key(row)));
            if why.starts_with("person-required") {
                self.call_person_required();
                return Decision::Now(
                    json!({ "error": "NotAllowedError", "message": "person-required" }),
                );
            }
            return Decision::Now(json!({ "error": "NotAllowedError" }));
        }
        let decision = match self.route(row) {
            Route::Real => return Decision::Forward,
            // A target's own restriction holds under the agent too (D14 step 2).
            Route::Agent if self.unsupported_here(row) => {
                Decision::Now(json!({ "error": "NotSupportedError" }))
            }
            // No dialog: the implementation opens it as the app's own or fails.
            Route::Agent if self.owned_call(row) => Decision::Forward,
            // The permission's ticket, a sheet's, or the raise's on the
            // runner's own status (LLP 1133.018 D13 item 4); else the implementation.
            Route::Agent => (self.decide_permission(row, send, args))
                .or_else(|| self.decide_choose(row, send))
                .or_else(|| self.decide_raise(row, send))
                .unwrap_or(Decision::Forward),
            Route::Substitute
                if self.substitute(row)["shortcuts"] == "pin" && !self.unsupported_here(row) =>
            {
                Decision::Now(self.pin_shortcut(row, args))
            }
            Route::Substitute => self.decide_timed(row, send, timeout, args),
        };
        // A name whose grants are per argument asks permission only for an
        // argument it was given: `getUserMedia(audio=false)` asks for no
        // device, and its implementation refuses it at once (LLP 1133.009
        // D3).
        let asks = match &self.substitute(row)["nameGrants"] {
            Json::Object(by_argument) => (by_argument.keys()).any(|a| given(args, a)),
            _ => true,
        };
        let decision = match decision {
            Decision::Hold(Waits::Permission) if !asks => match self.route(row) {
                Route::Agent => Decision::Forward,
                _ => Decision::Now(json!({ "error": "TypeError" })),
            },
            decision => decision,
        };
        match decision {
            // Only a person answers a prompt (D12).
            Decision::Hold(Waits::Permission) if origin != CallOrigin::App => {
                self.call_person_required();
                let code = self.permission_of(row, args)["denied"].clone();
                self.log(format!(
                    "{}: refused, person-required ({} origin)",
                    self.key(row),
                    origin.word()
                ));
                Decision::Now(json!({ "error": code }))
            }
            decision => decision,
        }
    }

    /// The permission a call of `row` with `args` asks: its declared one,
    /// or, where its interface declares several and the name's grants are a
    /// line per argument (`permissionBy`), the one of the arguments it was
    /// given that settles the call now: a denied one, else one still to ask,
    /// else the first (LLP 1133.008 D6, LLP 1133.009 D3).
    pub(in crate::runner) fn permission_of(&self, row: &ModulesRow, args: &Json) -> Json {
        // A permission chosen by an argument's value, or the first still to
        // ask of those a request asks together (`keyed.rs`).
        if let Some(p) = self.permission_by_value(row, args) {
            return p;
        }
        if let Some(names) = self.each_asked(row, args) {
            let first =
                (names.iter().find(|n| self.permission_state(n) == "prompt")).or(names.first());
            return first.map_or(Json::Null, |n| self.declared_permission(n));
        }
        let sub = self.substitute(row);
        // The first of the name's permissions whose grant line the app
        // declares (`"narrowest"`, LLP 1133.019 D4).
        if let Some(any) = sub["permissionAny"].as_array() {
            let declares = |p: &&Json| {
                (p["grants"].as_array().into_iter().flatten())
                    .filter_map(Json::as_str)
                    .any(|l| {
                        (self.data.grants().lines()).any(|g| g.split_whitespace().next() == Some(l))
                    })
            };
            return any
                .iter()
                .find(declares)
                .or(any.first())
                .cloned()
                .unwrap_or_default();
        }
        let Some(by) = sub["permissionBy"].as_object() else {
            return self.unsettled(row, &sub);
        };
        let given: Vec<&Json> = (by.iter())
            .filter(|(a, _)| given(args, a))
            .map(|(_, p)| p)
            .collect();
        let state = |p: &&Json| self.permission_state(p["name"].as_str().unwrap_or_default());
        (given.iter().find(|p| state(p) == "denied"))
            .or_else(|| given.iter().find(|p| state(p) == "prompt"))
            .or(given.first())
            .map_or_else(|| sub["permission"].clone(), |p| (*p).clone())
    }

    /// The permission a call of `row` asks of its own and those it requires
    /// on this target (LLP 1133.030 D12 item 1): a denied one, its own
    /// first; else the first required one still `prompt`; else its own. A
    /// status source answers its own.
    fn unsettled(&self, row: &ModulesRow, sub: &Json) -> Json {
        let own = &sub["permission"];
        let required = self.required(own);
        if required.is_empty() || own["status"] == self.plan.str(row.declared) {
            return own.clone();
        }
        let name = own["name"].as_str().unwrap_or_default();
        if self.own_permission_state(name, own) == "denied" {
            return own.clone();
        }
        let states: Vec<(String, String)> = (required.into_iter())
            .map(|r| {
                let state = self.permission_state(&r);
                (r, state)
            })
            .collect();
        (states.iter().find(|(_, s)| s == "denied"))
            .or_else(|| states.iter().find(|(_, s)| s == "prompt"))
            .map_or_else(|| own.clone(), |(r, _)| self.permission_named(row, r))
    }

    /// `row`'s substitute with the permission a call with `args` asks.
    pub(in crate::runner) fn substitute_for(&self, row: &ModulesRow, args: &Json) -> Json {
        let mut sub = self.substitute(row);
        if sub["permissionBy"].is_object()
            || sub["permissionAny"].is_array()
            || sub["permissionValue"].is_array()
            || sub["permission"]["requires"].is_object()
        {
            sub["permission"] = self.permission_of(row, args);
        }
        sub
    }

    /// [`Self::permission_of`]'s name; empty for none.
    pub(in crate::runner) fn permission_name(&self, row: &ModulesRow, args: &Json) -> String {
        // A request asking several together holds them all, comma-joined.
        if let Some(names) = self.each_asked(row, args).filter(|_| row.send) {
            return names.join(",");
        }
        (self.permission_of(row, args)["name"].as_str())
            .unwrap_or_default()
            .to_owned()
    }

    /// The permission `name` as `row` declares it: its own, one chosen by
    /// argument, or one its own requires on this target.
    pub(in crate::runner) fn permission_named(&self, row: &ModulesRow, name: &str) -> Json {
        let sub = self.substitute(row);
        // A name that asks by its arguments' values asks any of its own.
        if sub["permissionValue"].is_array() || sub["permissionEach"].is_array() {
            let p = self.declared_permission(name);
            let own = (sub["permissionList"].as_array().into_iter().flatten())
                .filter_map(|d| d["name"].as_str())
                .any(|d| d == name || d.strip_suffix('*').is_some_and(|pre| name.starts_with(pre)));
            return if own { p } else { Json::Null };
        }
        let by = sub["permissionBy"].as_object().into_iter().flatten();
        let any = sub["permissionAny"].as_array().into_iter().flatten();
        let required = self.required(&sub["permission"]);
        let requires = (sub["permissionList"].as_array().into_iter().flatten())
            .filter(|p| required.iter().any(|r| p["name"] == r.as_str()));
        (std::iter::once(&sub["permission"]).chain(by.map(|(_, p)| p)))
            .chain(any)
            .chain(requires)
            .find(|p| p["name"] == name)
            .cloned()
            .unwrap_or_default()
    }

    /// Whether `row` asks permission `name` for some call.
    pub(in crate::runner) fn asks_permission(&self, row: &ModulesRow, name: &str) -> bool {
        !self.permission_named(row, name).is_null()
    }

    /// Permission `granted` was just granted to held call `token`: when the
    /// call needs another that is not yet settled, it is denied with it, or
    /// held under its ticket, and `true`; `false` when nothing more is asked.
    pub(in crate::runner) fn ask_next(
        &mut self,
        row: &ModulesRow,
        send: bool,
        token: u64,
        granted: &str,
        args: &Json,
    ) -> bool {
        let name = self.permission_name(row, args);
        if name.is_empty() || name == granted {
            return false;
        }
        match self.decide_permission(row, send, args) {
            Some(Decision::Now(answer)) => self.release(token, answer),
            Some(Decision::Hold(waits)) => {
                if let Some((held_by, origin)) = self.held_name(token, row) {
                    let hold = self.device_hold(row, held_by, waits, origin, &name, args);
                    if let Some(h) = self.extensions.held.iter_mut().find(|h| h.token == token) {
                        h.hold = hold;
                        h.permission = name;
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// What held call `token` is held for, as its ticket names it (a
    /// resource or mutation, or a data-code call's key), and its origin.
    pub(in crate::runner) fn held_name(
        &self,
        token: u64,
        row: &ModulesRow,
    ) -> Option<(String, CallOrigin)> {
        let target = (self.pending.iter())
            .find(|p| p.continuation == Some(token))
            .map(|p| p.target);
        let held = self.extensions.held.iter().find(|h| h.token == token)?;
        let name = match (target, held.reply.is_some()) {
            (Some(target), _) => self.target_name(target),
            (None, true) => self.key(row),
            (None, false) => return None,
        };
        Some((name, held.origin))
    }

    /// The checks before any decision: why the call is refused, if it is.
    pub(super) fn admit(
        &self,
        row: &ModulesRow,
        args: &Json,
        origin: CallOrigin,
    ) -> Result<(), String> {
        let sub = self.substitute(row);
        let lines: Vec<&str> = (self.data.grants().lines())
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        let granted = |v: &Json| match v {
            Json::String(line) => lines.contains(&line.as_str()),
            Json::Array(any) => any
                .iter()
                .any(|l| l.as_str().is_some_and(|l| lines.contains(&l))),
            _ => true,
        };
        let named = |v: &Json| match v {
            Json::Array(any) => (any.iter().filter_map(Json::as_str))
                .collect::<Vec<_>>()
                .join(" or "),
            other => other.as_str().unwrap_or_default().to_owned(),
        };
        match &sub["nameGrants"] {
            Json::Object(by_argument) => {
                for (argument, line) in by_argument {
                    if given(args, argument) && !granted(line) {
                        return Err(format!("grant-missing ({argument}= needs {})", named(line)));
                    }
                }
            }
            // An empty list needs no line (a picker's, LLP 1133.018 D4).
            line @ Json::Array(l) if !l.is_empty() && !granted(line) => {
                return Err(format!("grant-missing ({})", named(line)));
            }
            _ => {}
        }
        // A name any of several lines admits asks the permission of one the
        // app declares, or is let through as a read of the app's own item
        // (`owned`, LLP 1133.019 D11 item 4).
        if sub["nameGrants"].as_array().is_some_and(|l| l.len() > 1) && !self.owned_call(row) {
            let permission = self.permission_of(row, args);
            let lines: Vec<&Json> = permission["grants"]
                .as_array()
                .into_iter()
                .flatten()
                .collect();
            if !lines.is_empty() && !lines.iter().any(|l| granted(l)) {
                return Err(format!(
                    "grant-missing (permission {})",
                    permission["name"].as_str().unwrap_or_default()
                ));
            }
        }
        // A path option is the app's own file, within the calling source's
        // `fs.read` or `fs.write` lines (LLP 1133.009 D10 item 4).
        for (option, path) in sub["paths"].as_object().into_iter().flatten() {
            let Some(given) = args[option].as_str() else {
                continue;
            };
            let capability = format!("fs.{}", path["access"].as_str().unwrap_or("read"));
            if !crate::save_file::plain_app_path(given)
                || !crate::save_file::covered(self.data.grants(), &capability, given)
            {
                return Err(format!(
                    "grant-missing ({option}={given} needs {capability})"
                ));
            }
        }
        let applies = sub["when"]
            .as_object()
            .is_none_or(|when| when.iter().all(|(k, v)| args.get(k) == Some(v)));
        // `{"whenAny": [options], "is": "app"}`: only a call giving one of
        // them is held to `app` (LLP 1133.012 D12 item 5).
        let app_only = match &sub["origin"] {
            Json::Object(o) => (o["whenAny"].as_array().into_iter().flatten())
                .any(|a| a.as_str().is_some_and(|a| given(args, a))),
            o => o == "app",
        };
        // A sampled source's tool or wake reader joins a stream the app's own
        // consumers run, as it runs; one that would start it is refused
        // (LLP 1133.013 D4, D11).
        let joins = (sub["samples"]["stream"].as_str()).is_some_and(|s| self.app_rate(s).is_some());
        if applies && app_only && origin != CallOrigin::App && !joins {
            return Err(format!("person-required ({} origin)", origin.word()));
        }
        if applies && sub["foreground"] == true && self.page.hidden {
            return Err("not in the foreground".into());
        }
        Ok(())
    }

    /// The implementation's answer for `row` under the agent, the agent's
    /// preferred value laid over it field by field (LLP 1133.009 D10 item
    /// 11); any other answer as it is.
    pub(in crate::runner) fn overlaid(&self, row: &ModulesRow, mut answer: Json) -> Json {
        // An implementation that lays it itself is told it (LLP 1133.016 §9).
        let laid = self.substitute(row)["overlays"] == "implementation";
        if self.route(row) != Route::Agent || laid || error_code(&answer).is_some() {
            return answer;
        }
        if let Some(preferred) = self.extensions.preferred.get(&self.key(row)) {
            if error_code(preferred).is_some() {
                return preferred.clone();
            }
            overlay(&mut answer, preferred);
        }
        answer
    }
}

/// Whether a call gave option `argument`: set and not `false`. The grant
/// key `"*"` is given by every call.
fn given(args: &Json, argument: &str) -> bool {
    argument == "*" || (!args[argument].is_null() && args[argument] != false)
}

/// `over`'s fields laid over `value`'s, record by record.
pub(in crate::runner) fn overlay(value: &mut Json, over: &Json) {
    match (value, over) {
        (Json::Object(at), Json::Object(over)) => {
            for (k, v) in over {
                match at.get_mut(k) {
                    Some(existing) if existing.is_object() && v.is_object() => overlay(existing, v),
                    _ => {
                        at.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (value, over) => *value = over.clone(),
    }
}
