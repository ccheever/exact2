//! Presence secrets in a session (LLP 1133.010 D5, D8, D12, §7): the
//! store's calls that wait on the host's backend, the person's check under
//! the agent, and the occupancy the store adopts from the backend.
//!
//! Data code's `store.unlock`, `protect` and `unprotect` are long native
//! calls whose body is `{"presence": <op>, "name": …, "reason": …}`. They are
//! answered here, never by a module, on the store's own path: the reply
//! carries an unlocked value to the waiting promise alone, it is journaled by
//! its size, and the value never enters the store's values (D13). A
//! committed write of a presence name goes to the backend's queue instead of
//! the host's plain store, so one queue orders every operation on a name.
//!
//! **The check.** Under the agent the person is a ticket: a `permission`
//! ticket first while Face ID's permission is `prompt` (the local-auth
//! module's own state), then a `presence` ticket, whose `allow` is the
//! check passing. A call from a tool or a wake fails `person-required`
//! before anything is asked, in every state: presence is not delegable
//! (D12).

use super::super::{CommitReceipt, DataSource, Runner, RunnerError};
use super::{response, CallOrigin, Json, ModulesRow, Pending};
use crate::presence::{Backend, Checked, Done, Env, Event, Faults, Occupancy, Op};
use crate::request::{Dispatch, Work};
use crate::{HoldAnswer, StoreWrite};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The session's presence state.
#[derive(Default)]
pub(in crate::runner) struct Presence {
    backend: Option<Backend>,
    /// The host keeps locked secrets: its boot snapshot said so.
    capable: bool,
    /// The data source is ready: a pending move may run (D4).
    moves: bool,
    /// Data code's presence requests not yet dispatched: ticket and body.
    requests: Vec<(u64, Vec<u8>)>,
    /// Calls the backend runs, by id.
    calls: BTreeMap<u64, Call>,
    next: u64,
    /// The agent's tickets for checks: hold, call, and whether it is the
    /// permission's.
    holds: Vec<(u64, u64, bool)>,
    /// Checks the host's platform makes, for the host to take.
    asks: Vec<PresenceAsk>,
    /// Where the platform's keystore keeps the items (Apple: the keychain
    /// service), when the vault is it rather than the agent's store: the
    /// host reads an item itself after its check.
    keystore: Option<String>,
    /// Whether a device credential is set, as the bound implementation's
    /// status last said; `None` before it said.
    credential: Option<bool>,
    /// Checks that wait for the implementation to be bound: outside the
    /// agent no ticket answers for the person, and the module loads after
    /// first pixel (D8).
    waiting: Vec<u64>,
}

/// A presence check the host's platform makes (LLP 1133.010 D13): the bound
/// implementation's authenticator shows the sheet, and the host's backend
/// reads the item under the context it evaluated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresenceAsk {
    /// The call, which [`Runner::presence_answer`] names.
    pub id: u64,
    /// The name.
    pub name: String,
    /// The newest locked item's generation, which the read names.
    pub generation: u64,
    /// The policy, as the line writes it (`user-presence optional apple.orWatch`).
    pub policy: String,
    /// The reason's text, translated: what the sheet shows.
    pub reason: String,
    /// Who made the call.
    pub origin: CallOrigin,
    /// Where the host reads the item after the check (Apple: the keychain
    /// service). `None` under the agent, whose own store holds the value:
    /// the host only checks (`--bind`).
    pub keystore: Option<String>,
}

impl PresenceAsk {
    /// What a host is handed: the call, the name, its item's account
    /// (`<name>#<generation>`), where the item is (`service`), the line's
    /// words, the reason's text and the origin. Never a value.
    pub fn json(&self) -> String {
        json!({ "call": self.id, "name": self.name, "account": format!("{}#{}", self.name, self.generation),
            "service": self.keystore, "policy": self.policy, "reason": self.reason, "origin": self.origin.word() })
        .to_string()
    }
}

/// How a check the host made ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresenceAnswer {
    /// Passed; the backend reads the item (the agent's store).
    Passed,
    /// Passed, and the host read the item: its value, or `None` when the
    /// platform has dropped it.
    Read(Option<Vec<u8>>),
    /// Failed: D1's code and the platform's message.
    Failed(String, String),
}

struct Call {
    slot: Arc<Mutex<Pending>>,
    ticket: Option<u64>,
    origin: CallOrigin,
    op: &'static str,
    name: String,
    reason: String,
}

fn answer(done: &Done) -> Json {
    match done {
        Done::Value(value) => {
            json!({ "value": value.as_ref().map(|v| String::from_utf8_lossy(v).into_owned()) })
        }
        Done::Ok => json!({}),
        Done::Failed(code, message) => json!({ "error": code, "message": message }),
    }
}

impl<D: DataSource> Runner<D> {
    /// The host's presence backend for this session, reconciled before boot
    /// (its occupancy is in the boot snapshot). The runner runs it from now
    /// on: its queue, the checks, and every committed write of its names.
    /// `keystore`: where the platform's keystore keeps the items, when the
    /// vault is it, so a check the host makes reads the item itself.
    pub fn install_presence(&mut self, backend: Backend, keystore: Option<String>) {
        let p = &mut self.extensions.presence;
        p.keystore = keystore;
        p.capable = self.store.presence_capable();
        p.moves = self.data.ready();
        p.backend = Some(backend);
        self.pump_presence();
    }

    /// Whether this session keeps locked secrets: a backend that can keep
    /// them (`localAuthStatus().presenceSecrets`, D7).
    pub(in crate::runner) fn presence_secrets(&self) -> bool {
        let p = &self.extensions.presence;
        p.backend.is_some() && p.capable
    }

    /// The local-auth module's status source: the store's checks read its
    /// permission and status (§7).
    fn local_auth(&self) -> Option<ModulesRow> {
        (self.plan.modules.iter())
            .find(|m| {
                let sub = self.substitute(m);
                sub["authenticator"].is_object()
                    && sub["permission"]["status"].as_str() == Some(self.plan.str(m.declared))
            })
            .cloned()
    }

    /// The device's status as the substitute answers it.
    fn local_auth_status(&self) -> Json {
        let row = self.local_auth();
        row.and_then(|r| self.value_of(&r)).unwrap_or(json!({}))
    }

    /// A locked item can be kept now: the host keeps them, and a device
    /// credential is set (D5).
    fn can_lock(&self) -> bool {
        let credential = match self.presence_bound() {
            true => self.extensions.presence.credential.unwrap_or(true),
            false => self.local_auth_status()["deviceCredential"] != false,
        };
        self.presence_secrets() && credential
    }

    /// The local-auth implementation answers for the person: the platform's
    /// sheet, not a ticket.
    fn presence_bound(&self) -> bool {
        self.local_auth()
            .is_some_and(|row| self.extensions.bound.contains(self.plan.str(row.extension)))
    }

    /// The bound implementation's status answered: whether a device
    /// credential is set, which a locked write needs, and its facts are the
    /// runner's, as the substitute's are.
    pub(in crate::runner) fn presence_status(
        &mut self,
        row: &ModulesRow,
        mut answer: Json,
    ) -> Json {
        if !self.local_auth().is_some_and(|r| r.source == row.source) {
            return answer;
        }
        if let Some(set) = answer["deviceCredential"].as_bool() {
            self.extensions.presence.credential = Some(set);
        }
        if let Some(object) = answer.as_object_mut().filter(|o| !o.contains_key("error")) {
            object.insert("presenceSecrets".into(), json!(self.presence_secrets()));
        }
        answer
    }

    /// The local-auth implementation was bound: the checks that waited for
    /// it are made now.
    pub(in crate::runner) fn presence_rebound(&mut self) {
        if !self.presence_bound() {
            return;
        }
        for id in std::mem::take(&mut self.extensions.presence.waiting) {
            let Some(call) = self.extensions.presence.calls.get(&id) else {
                continue;
            };
            let (name, op) = (call.name.clone(), call.op);
            if let Some(presence) = self.store.presence_of(&name).cloned() {
                self.presence_check(id, &name, &presence, op);
            }
        }
    }

    /// The checks the host's platform makes, since the last take.
    pub fn take_presence_asks(&mut self) -> Vec<PresenceAsk> {
        std::mem::take(&mut self.extensions.presence.asks)
    }

    /// How a check the host made ended. A call let go meanwhile is dropped.
    pub fn presence_answer(&mut self, id: u64, answer: PresenceAnswer) {
        let checked = match answer {
            PresenceAnswer::Passed => Checked::Passed,
            PresenceAnswer::Read(value) => Checked::Read(value),
            PresenceAnswer::Failed(code, message) => Checked::Failed(code, message),
        };
        self.presence_checked(id, checked);
    }

    /// How a check the host made ended, as a host says it:
    /// `{"call", "status": "passed"|"read"|"failed", "value"?, "code"?,
    /// "message"?}`, a `read` without a value being an item the platform
    /// dropped. The value goes to the waiting call alone; never journaled.
    pub fn presence_reply(&mut self, json: &[u8]) {
        let v: Json = serde_json::from_slice(json).unwrap_or_default();
        let text = |k: &str| v[k].as_str().unwrap_or_default().to_owned();
        let answer = match v["status"].as_str() {
            Some("passed") => PresenceAnswer::Passed,
            Some("read") => {
                PresenceAnswer::Read(v["value"].as_str().map(|s| s.as_bytes().to_vec()))
            }
            _ => PresenceAnswer::Failed(text("code"), text("message")),
        };
        if let Some(call) = v["call"].as_u64() {
            self.presence_answer(call, answer);
        }
    }

    /// `key`'s text in the strings table the app shows now, or the key.
    pub(in crate::runner) fn strings_text(&self, key: &str) -> String {
        (self.plan.localized(self.resolved_locale(), key))
            .unwrap_or(key)
            .to_owned()
    }

    /// A request goes out: a presence call's ticket is kept for its dispatch.
    pub(in crate::runner) fn presence_noted(&mut self, ticket: u64, request: &crate::Request) {
        if request.is_native() && presence_body(&request.body).is_some() {
            (self.extensions.presence.requests).push((ticket, request.body.clone()));
        }
    }

    /// Whether request `ticket` is a presence call that waits on the agent's
    /// ticket: no clock waits on it, as on any hold.
    pub(in crate::runner) fn presence_holds(&self, ticket: u64) -> bool {
        let p = &self.extensions.presence;
        (p.calls.values()).any(|c| c.ticket == Some(ticket))
            && p.backend.as_ref().is_some_and(Backend::checking)
    }

    /// A long native call that is a store's presence call: answered here.
    pub(in crate::runner) fn presence_call(&mut self, body: &[u8]) -> Option<Dispatch> {
        let (op, name, reason) = presence_body(body)?;
        let at = (self.extensions.presence.requests.iter()).position(|(_, b)| b == body);
        let ticket = at.map(|i| self.extensions.presence.requests.remove(i).0);
        let now = |a: Json| Some(Dispatch::Run(Work::Now(Box::new(move || response(&a)))));
        let fail = |code: &str, message: String| json!({ "error": code, "message": message });
        let Some(presence) = self.store.presence_of(&name).cloned() else {
            return now(fail(
                "NotSupportedError",
                format!("{name} is not a secret.keep name with a presence policy"),
            ));
        };
        let origin = self.data_origin();
        let occupancy = self.store.occupancy(&name).unwrap_or(Occupancy::Absent);
        // Presence is not delegable (D12): refused before anything is asked.
        if matches!(op, "unlock" | "unprotect") && occupancy.locked() && origin != CallOrigin::App {
            self.log(format!(
                "store {op} {name}: refused, person-required ({} origin)",
                origin.word()
            ));
            return now(fail("PERMISSION_DENIED", "person-required".into()));
        }
        if self.extensions.presence.backend.is_none() {
            return now(fail(
                "NotSupportedError",
                "this device keeps no locked secret".into(),
            ));
        }
        let id = self.extensions.presence.next;
        self.extensions.presence.next += 1;
        let op_kind = match op {
            "unlock" => Op::Unlock,
            "protect" => Op::Protect,
            _ => Op::Unprotect,
        };
        let backend = self.extensions.presence.backend.as_mut()?;
        backend.enqueue(&name, op_kind, Some(id));
        let slot = Arc::new(Mutex::new(Pending::default()));
        self.log(format!("store {op} {name} ({})", presence.policy.word()));
        self.extensions.presence.calls.insert(
            id,
            Call {
                slot: slot.clone(),
                ticket,
                origin,
                op,
                name,
                reason,
            },
        );
        self.pump_presence();
        Some(Dispatch::Run(Work::Later(Box::new(move |reply| {
            let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
            match s.outcome.take() {
                Some(outcome) => reply.send(outcome),
                None => s.reply = Some(reply),
            }
        }))))
    }

    /// Committed writes for the host: a presence name's go to the backend's
    /// queue instead, carrying its value there and nowhere else (D5).
    pub(in crate::runner) fn presence_writes(
        &mut self,
        writes: Vec<StoreWrite>,
    ) -> Vec<StoreWrite> {
        let Some(backend) = self.extensions.presence.backend.as_mut() else {
            return writes;
        };
        let mut rest = Vec::with_capacity(writes.len());
        let mut queued = false;
        for write in writes {
            if self.store.presence_of(&write.name).is_some() {
                backend.enqueue(&write.name, Op::Write(write.value), None);
                queued = true;
            } else {
                rest.push(write);
            }
        }
        if queued {
            self.pump_presence();
        }
        rest
    }

    /// The data source is ready: a pending move may run (D4).
    pub(in crate::runner) fn presence_ready(&mut self) {
        if !self.extensions.presence.moves {
            self.extensions.presence.moves = true;
            self.pump_presence();
        }
    }

    /// Presence retries use the host's existing deadline, even on an idle
    /// screen with no authored timers.
    pub(in crate::runner) fn presence_due_ms(&self) -> Option<f64> {
        self.extensions
            .presence
            .backend
            .as_ref()
            .and_then(Backend::retry_due_ms)
    }

    /// Retry at the clock the advance reached, coalescing missed attempts.
    /// Recovery without a waiting call still publishes the new occupancy.
    pub(in crate::runner) fn tick_presence(
        &mut self,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        self.pump_presence();
        let mut changed = false;
        let mut recovered_plain = false;
        if let Some(backend) = self.extensions.presence.backend.as_ref() {
            for (name, occupancy) in backend.occupancies() {
                let before = self.store.occupancy(&name);
                changed |= before != Some(occupancy);
                recovered_plain |=
                    before == Some(Occupancy::Pending) && occupancy == Occupancy::Plain;
            }
        }
        if changed {
            // Unlike an explicit unprotect, boot recovery has no caller's
            // reply to tell readers that the plain value is available.
            let readers = self
                .store_readers
                .iter()
                .enumerate()
                .filter_map(|(i, reads)| (recovered_plain && *reads).then_some(i))
                .collect();
            self.commit_again(readers, "presence").map(Some)
        } else {
            Ok(None)
        }
    }

    /// Run what the backend can run now, and act on what it asks.
    pub(in crate::runner) fn pump_presence(&mut self) {
        let can_lock = self.can_lock();
        let now_ms = self.now_ms;
        let (events, lines) = {
            let faults = &self.faults;
            let p = &mut self.extensions.presence;
            let Some(backend) = p.backend.as_mut() else {
                return;
            };
            let fail = |point: &str| faults.take(point);
            let hold = |point: &str| faults.holds(point);
            let events = backend.pump(&Env {
                can_lock,
                moves: p.moves,
                now_ms,
                faults: Faults {
                    fail: &fail,
                    hold: &hold,
                },
            });
            (events, backend.take_log())
        };
        for line in lines {
            self.log(line);
        }
        for event in events {
            match event {
                Event::Done { id, done } => self.presence_done(id, done),
                Event::Check {
                    id,
                    name,
                    presence,
                    op,
                } => self.presence_check(id, &name, &presence, op),
            }
        }
    }

    fn presence_done(&mut self, id: u64, done: Done) {
        let Some(call) = self.extensions.presence.calls.remove(&id) else {
            return;
        };
        let outcome = match &done {
            Done::Value(Some(_)) => "unlocked".to_owned(),
            Done::Value(None) => "nothing kept".to_owned(),
            Done::Ok => "done".to_owned(),
            Done::Failed(code, _) => code.clone(),
        };
        self.log(format!("store {} {}: {outcome}", call.op, call.name));
        self.extensions.presence.holds.retain(|(_, c, _)| *c != id);
        Pending::complete(&call.slot, response(&answer(&done)));
    }

    /// Call `id` waits on the person: under the agent, a ticket (§7).
    fn presence_check(
        &mut self,
        id: u64,
        name: &str,
        presence: &exact_grants::Presence,
        op: &'static str,
    ) {
        let Some(call) = self.extensions.presence.calls.get(&id) else {
            return;
        };
        let (origin, reason) = (call.origin, call.reason.clone());
        if origin != CallOrigin::App {
            let checked = Checked::Failed("PERMISSION_DENIED".into(), "person-required".into());
            return self.presence_checked(id, checked);
        }
        if self.extensions.presence.keystore.is_some() && !self.presence_bound() {
            self.log(format!(
                "store {op} {name}: waits for the local-auth module"
            ));
            self.extensions.presence.waiting.push(id);
            return;
        }
        // The platform's own sheet: its permission and its refusals are the
        // platform's, and the host reads the item (D6, D13).
        if self.presence_bound() {
            let p = &self.extensions.presence;
            let generation = p.backend.as_ref().map_or(0, |b| b.generation(name));
            let ask = PresenceAsk {
                id,
                name: name.to_owned(),
                generation,
                policy: presence.line_words(),
                reason: self.strings_text(&reason),
                origin,
                keystore: p.keystore.clone(),
            };
            self.log(format!("store {op} {name}: the platform's check"));
            self.extensions.presence.asks.push(ask);
            return;
        }
        let status = self.local_auth_status();
        if status["available"] == false {
            let checked = Checked::Failed(
                "NOT_AVAILABLE".into(),
                "local authentication is not available on this device".into(),
            );
            return self.presence_checked(id, checked);
        }
        let permission = self.local_auth().and_then(|row| {
            let sub = self.substitute(&row);
            let name = sub["permission"]["name"].as_str()?.to_owned();
            let asks = sub["permission"]["asks"].as_object().cloned();
            Some((name, asks))
        });
        let (state, asks) = match &permission {
            Some((name, asks)) => (self.permission_state(name), asks.clone()),
            None => ("granted".to_owned(), None),
        };
        let biometric_only = !matches!(
            presence.policy,
            exact_grants::Policy::UserPresence | exact_grants::Policy::DeviceCredential
        );
        let asks = presence.policy.may_use_biometry()
            && asks.is_none_or(|a| a.iter().all(|(k, v)| status.get(k) == Some(v)));
        let args = |kind: &str| {
            json!({ "capability": kind, "request": format!("store.{op}"), "name": name,
                "policy": presence.line_words(), "reason": reason, "origin": origin.word() })
        };
        if state == "prompt" && asks {
            let hold = self.hold(
                "permission",
                None,
                &args("local-auth").to_string(),
                &["allow", "deny"],
                false,
            );
            self.name_hold(hold, name);
            self.extensions.presence.holds.push((hold, id, true));
            return;
        }
        if state == "denied" && biometric_only {
            let checked = Checked::Failed(
                "PERMISSION_DENIED".into(),
                "Face ID is not allowed for this app".into(),
            );
            return self.presence_checked(id, checked);
        }
        let check = self.store_check();
        let mut choices = vec!["allow"];
        for (choice, fails) in check["choices"].as_object().into_iter().flatten() {
            // A choice declared for one operation (`invalidated`: `unlock`).
            if fails != "unlock" || op == "unlock" {
                choices.push(choice.as_str());
            }
        }
        let ticket = check["ticket"].as_str().unwrap_or("presence");
        let hold = self.hold(ticket, None, &args(ticket).to_string(), &choices, false);
        self.name_hold(hold, name);
        self.extensions.presence.holds.push((hold, id, false));
    }

    fn name_hold(&mut self, hold: u64, name: &str) {
        if let Some(h) = self.device_holds.iter_mut().find(|h| h.ticket == hold) {
            h.name = format!("store.{name}");
        }
    }

    fn presence_checked(&mut self, id: u64, checked: Checked) {
        if let Some(backend) = self.extensions.presence.backend.as_mut() {
            backend.checked(id, checked);
        }
        self.pump_presence();
    }

    /// The agent answered hold `hold`, already spent: a presence check's
    /// permission or the check itself.
    pub(in crate::runner) fn presence_answered(&mut self, hold: u64, answer: &HoldAnswer) {
        let p = &mut self.extensions.presence;
        let Some(at) = p.holds.iter().position(|(h, _, _)| *h == hold) else {
            return;
        };
        let (_, id, permission) = p.holds.remove(at);
        let HoldAnswer::Choice(choice) = answer else {
            return;
        };
        if permission {
            let name = self.local_auth().and_then(|row| {
                self.substitute(&row)["permission"]["name"]
                    .as_str()
                    .map(str::to_owned)
            });
            match (choice.as_str(), name) {
                ("allow", Some(name)) => self.set_permission(&name, "granted"),
                ("deny", Some(name)) => self.set_permission(&name, "denied"),
                _ => {
                    let checked = Checked::Failed(
                        "PERMISSION_DENIED".into(),
                        "the permission was dismissed".into(),
                    );
                    return self.presence_checked(id, checked);
                }
            }
            // Asked again under the state the answer set.
            let Some(call) = self.extensions.presence.calls.get(&id) else {
                return;
            };
            let name = call.name.clone();
            let op = call.op;
            if let Some(presence) = self.store.presence_of(&name).cloned() {
                self.presence_check(id, &name, &presence, op);
            }
            return;
        }
        let check = self.store_check();
        let checked = match (choice.as_str(), &check["choices"][choice.as_str()]) {
            ("allow", _) => Checked::Passed,
            (_, fails) if fails == "unlock" => Checked::Invalidated,
            ("cancel", _) => {
                let code = check["cancel"].as_str().unwrap_or("USER_CANCEL");
                Checked::Failed(code.into(), "the person cancelled".into())
            }
            (choice, fails) => Checked::Failed(
                fails.as_str().unwrap_or("UnknownError").into(),
                format!("the agent answered {choice}"),
            ),
        };
        self.presence_checked(id, checked);
    }

    /// The store's checks' substitute, declared under the local-auth
    /// interface's authenticator (§7): the ticket, what `cancel` fails with,
    /// and each other choice's code (`unlock`: the item invalidated).
    fn store_check(&self) -> Json {
        let declared = self
            .local_auth()
            .map(|row| self.substitute(&row)["authenticator"]["substitute"].clone());
        declared.filter(Json::is_object).unwrap_or_else(|| {
            json!({ "ticket": "presence", "cancel": "USER_CANCEL",
                "choices": { "fail": "AUTHENTICATION_FAILED", "lockout": "LOCKOUT", "invalidated": "unlock" } })
        })
    }

    /// The store adopts the backend's occupancy, at the start of a commit:
    /// a move that landed shows in this commit, and asks the store's readers
    /// again where `has` or `locked` changed.
    pub(in crate::runner) fn adopt_presence(&mut self) {
        let can_lock = self.can_lock();
        let Some(backend) = self.extensions.presence.backend.as_ref() else {
            return;
        };
        for (name, occupancy) in backend.occupancies() {
            self.store
                .adopt_occupancy(&name, occupancy, || backend.plain_value(&name));
        }
        self.store.set_presence_capable(can_lock);
    }

    /// The presence names, each with what the agent's `state` says of it:
    /// `locked, user-presence`, `pending`, `plain` or `absent`, never a value
    /// (§7, Disclosure).
    pub fn presence_list(&self) -> Vec<(String, String)> {
        (self.store.presence_names().into_iter())
            .map(|(name, presence)| {
                let said = match self.store.occupancy(name).unwrap_or(Occupancy::Absent) {
                    Occupancy::Locked => format!("locked, {}", presence.policy.word()),
                    other => other.word().to_owned(),
                };
                (name.to_owned(), said)
            })
            .collect()
    }
}

/// A presence call's `(op, name, reason)`.
fn presence_body(body: &[u8]) -> Option<(&'static str, String, String)> {
    let v: Json = serde_json::from_slice(body).ok()?;
    let op = match v.get("presence")?.as_str()? {
        "unlock" => "unlock",
        "protect" => "protect",
        "unprotect" => "unprotect",
        _ => return None,
    };
    let name = v.get("name")?.as_str()?.to_owned();
    let reason = v
        .get("reason")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_owned();
    Some((op, name, reason))
}

impl<D: DataSource> Runner<D> {
    /// A Rust source's answer that read a locked name with `get` fails, as
    /// data code's `store.get` throws (D5): its value is `unlock`'s alone.
    pub(in crate::runner) fn refuse_locked<T>(
        &self,
        answer: Result<T, super::DataError>,
    ) -> Result<T, super::DataError> {
        match self.store.take_locked_read() {
            Some(name) => Err(crate::StoreError::Locked(name).into()),
            None => answer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_is_handed_the_item_and_the_words_never_a_value() {
        let ask = PresenceAsk {
            id: 4,
            name: "token".into(),
            generation: 2,
            policy: "user-presence optional".into(),
            reason: "Unlock Excord".into(),
            origin: CallOrigin::App,
            keystore: Some("com.example.presence".into()),
        };
        let json: Json = serde_json::from_str(&ask.json()).unwrap();
        assert_eq!(
            json,
            json!({ "call": 4, "name": "token", "account": "token#2", "service": "com.example.presence",
                "policy": "user-presence optional", "reason": "Unlock Excord", "origin": "app" })
        );
    }
}
