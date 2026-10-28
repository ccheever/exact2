//! Ceremonies (LLP 1133.011 §9): a send whose substitute declares a
//! `ceremony` is a sheet the person answers, held under its `ticket`.
//!
//! - **Admission** first matches a grant by the argument's value: an
//!   argument whose grant is `{"line", "matches": "target"}` must be the
//!   target of one of the app's lines (`publicKey.rp.id` against
//!   `auth.passkey <rp id>`, D6), else the call fails the grant's `refuse`
//!   code before anything is asked. An omitted one stands for the page's
//!   host on the web (`"omitted": "page"`) and refuses natively, where
//!   there is no page; `localhost` refuses natively too, since no
//!   association names it.
//! - **Unless.** The ceremony's `status` source (its value on this target,
//!   `targets` laid over its default) fails a call at once where a rule's
//!   `field` `is` the value it names, for the calls its `when` matches: no
//!   credential manager, or no conditional `get` on macOS.
//! - **One at a time** (D9): a modal ceremony, or a second conditional one,
//!   ends a held conditional one with `AbortError`; a ceremony that arrives
//!   while a modal one is held waits behind it, with no ticket until the
//!   modal one ends.
//! - **The ticket's `args`** name the ceremony, its mediation, the RP ID,
//!   the origin this target's platform would write into `clientDataJSON`
//!   (`clientOrigin`, by target), the options as written and the call's
//!   origin: what `packages/passkeys/test/authenticator.mjs` signs over.
//!   `type @t <JSON>` answers it with a response, checked against the
//!   declared `answer` fields; `tap @t cancel` and each declared choice
//!   fail it with their error.
//! - **Withheld.** The answer's `withhold` paths (a PRF output, D10) are
//!   removed before it leaves the runner.
//! - **Dropped** (D11): a data-code call's reply that carries handles
//!   (`create`'s credential id) is provisional until its request lands. One
//!   answered after its request was let go (its mutation superseded) is
//!   disposed of at once and the journal says so.
//! - **Signals** (`records`) answer at once and are listed with the
//!   ceremonies in `state.extensions`, never their arguments.

use super::super::{DataSource, Runner};
use super::{CallOrigin, Held, Json, ModulesRow, Waits};
use serde_json::json;

/// The ceremonies and signals `state.extensions` lists, the newest last.
const KEPT: usize = 16;

/// The target this runner is built for, as a substitute's `targets` and a
/// ceremony's `clientOrigin` name it.
pub(in crate::runner) fn target() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "web"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "tvos") {
        "tvos"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// The value at `path` (`publicKey.rp.id`) in `value`.
pub(super) fn at<'a>(value: &'a Json, path: &str) -> Option<&'a Json> {
    path.split('.')
        .try_fold(value, |v, part| v.get(part))
        .filter(|v| !v.is_null())
}

/// The value at `path` in `value`, reading a string on the way as the JSON
/// it holds (a payment method's `data`, LLP 1133.034 D10).
pub(super) fn within(value: &Json, path: &str) -> Option<Json> {
    let mut at = value.clone();
    for part in path.split('.') {
        if let Some(text) = at.as_str() {
            at = serde_json::from_str(text).ok()?;
        }
        at = at.get(part)?.clone();
    }
    // A `json<…>` value is JSON text: the object or list it holds.
    if let Some(parsed) = at
        .as_str()
        .and_then(|t| serde_json::from_str::<Json>(t).ok())
    {
        if parsed.is_object() || parsed.is_array() {
            at = parsed;
        }
    }
    (!at.is_null()).then_some(at)
}

/// Remove `path` from `value`, where it is.
fn remove(value: &mut Json, path: &str) {
    let (parent, last) = match path.rsplit_once('.') {
        Some((parent, last)) => (parent, last),
        None => ("", path),
    };
    let mut at = Some(value);
    for part in parent.split('.').filter(|p| !p.is_empty()) {
        at = at.and_then(|v| v.get_mut(part));
    }
    if let Some(object) = at.and_then(Json::as_object_mut) {
        object.remove(last);
    }
}

impl<D: DataSource> Runner<D> {
    /// The grants `row` matches by an argument's value: the code and why a
    /// call with `args` is refused, if it is.
    pub(super) fn value_grants(&self, row: &ModulesRow, args: &Json) -> Result<(), (Json, String)> {
        let sub = self.substitute(row);
        // A computed colour that is none fails the call (LLP 1133.022 D10 item 2).
        for option in sub["colorOptions"].as_array().into_iter().flatten() {
            let given = option
                .as_str()
                .and_then(|o| args.get(o))
                .and_then(Json::as_str);
            if given.is_some_and(|c| exact_kernel::Color::parse(c).is_none()) {
                return Err((json!("TypeError"), format!("{option} is no CSS colour")));
            }
        }
        self.methods_admitted(row, args)?;
        for (path, spec) in sub["nameGrants"].as_object().into_iter().flatten() {
            let Some(line) = spec["line"].as_str() else {
                continue;
            };
            let code = json!(spec["refuse"].as_str().unwrap_or("NotAllowedError"));
            // A call without the argument the path is in asks nothing of it
            // (a sign-in alone, with no `publicKey`, LLP 1133.014 D3).
            let root = path.split(['.', '[']).next().unwrap_or(path);
            if path.contains('.') && args.get(root).is_none_or(Json::is_null) {
                continue;
            }
            // A path through a list names each element `when` matches (a
            // merchant ID per Apple Pay method, LLP 1133.034 D13).
            let values = match path.split_once("[].") {
                Some((list, rest)) => {
                    let matches = |e: &Json| {
                        (spec["when"].as_object())
                            .is_none_or(|w| w.iter().all(|(k, v)| e.get(k) == Some(v)))
                    };
                    let elements = args[list].as_array().into_iter().flatten();
                    elements
                        .filter(|e| matches(e))
                        .map(|e| within(e, rest).and_then(|v| v.as_str().map(str::to_owned)))
                        .collect::<Vec<_>>()
                }
                None => vec![at(args, path).and_then(Json::as_str).map(str::to_owned)],
            };
            for value in values {
                let Some(value) = value else {
                    if spec["omitted"] == "page" && target() == "web" {
                        continue;
                    }
                    return Err((
                        code,
                        format!("{path} is omitted, and here no page's host stands for it"),
                    ));
                };
                let granted = self.data.grants().lines().any(|l| {
                    let mut words = l.split_whitespace();
                    words.next() == Some(line) && words.next() == Some(value.as_str())
                });
                if !granted {
                    return Err((
                        code,
                        format!("grant-missing ({path}={value} needs `{line} {value}`)"),
                    ));
                }
                if value == "localhost" && target() != "web" {
                    return Err((
                        code,
                        format!("{path}=localhost: no association names it on {}", target()),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The RP ID a call of `row` with `args` names: the argument its grant
    /// matches by value.
    fn rp_id(&self, row: &ModulesRow, args: &Json) -> Json {
        let sub = self.substitute(row);
        (sub["nameGrants"].as_object().into_iter().flatten())
            .find(|(_, spec)| spec["matches"] == "target")
            .and_then(|(path, _)| at(args, path).cloned())
            .unwrap_or(Json::Null)
    }

    /// What `sub`'s ceremony decides of a call with `args` now: a failure
    /// its status rules out, else a ticket.
    pub(super) fn decide_ceremony(
        &self,
        row: &ModulesRow,
        sub: &Json,
        args: &Json,
    ) -> super::Decision {
        let ceremony = &sub["ceremony"];
        let status = (ceremony["status"].as_str())
            .and_then(|s| self.sibling(row, s))
            .and_then(|s| self.value_of(&s))
            .unwrap_or_default();
        // The owner's status speaks for its own ceremony alone: a sign-in
        // with no passkey asks nothing of passkeys' (LLP 1133.014 D3).
        let owned = self.owns_ceremony(sub, args);
        for rule in (ceremony["unless"].as_array().into_iter().flatten()).filter(|_| owned) {
            let applies = (rule["when"].as_object())
                .is_none_or(|when| when.iter().all(|(k, v)| args.get(k) == Some(v)));
            let field = rule["field"].as_str().unwrap_or_default();
            if applies && status.get(field) == Some(&rule["is"]) {
                let message = format!("`{field}` is {} on {}", rule["is"], target());
                return super::Decision::Now(json!({ "error": rule["error"], "message": message }));
            }
        }
        super::Decision::Hold(Waits::Ceremony)
    }

    /// A ceremony's ticket `args` (module doc).
    pub(super) fn ceremony_args(&self, row: &ModulesRow, args: &Json, origin: CallOrigin) -> Json {
        let sub = self.substitute(row);
        let ceremony = &sub["ceremony"];
        if ceremony["asks"].is_object() {
            return self.sheet_args_of(row, args, origin);
        }
        let rp_id = self.rp_id(row, args);
        let client = match (ceremony["clientOrigin"][target()].as_str(), rp_id.as_str()) {
            (Some("page") | None, _) | (_, None) => Json::Null,
            (Some(template), Some(rp)) => json!(template.replace("{rpId}", rp)),
        };
        let options = (ceremony["options"].as_str()).map_or(Json::Null, |o| args[o].clone());
        let mut out = json!({
            "request": self.plan.str(row.declared),
            "ceremony": self.plan.str(row.declared),
            "mediation": args["mediation"].as_str().unwrap_or("optional"),
            "rpId": rp_id,
            "clientOrigin": client,
            "options": options,
            "origin": origin.word(),
        });
        for (option, described) in self.members_args(row, args) {
            out[option] = described;
        }
        out
    }

    /// The choices a ceremony's ticket takes by tap besides `cancel`.
    pub(super) fn ceremony_choices(&self, row: &ModulesRow, sub: &Json) -> Vec<String> {
        let asked = (sub["ceremony"]["asks"].as_object().into_iter().flatten())
            .filter(|(_, ask)| ask["choice"] == true);
        let mut out: Vec<String> = (sub["ceremony"]["choices"].as_object().into_iter().flatten())
            .chain(asked)
            .map(|(c, _)| c.clone())
            .collect();
        if sub["ceremony"].is_object() {
            for c in self.members_choices(row) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// A typed response to a ceremony of `row`: a JSON object with the
    /// declared `answer` fields.
    pub(super) fn ceremony_typed(
        &self,
        row: &ModulesRow,
        args: &Json,
        token: u64,
        text: &str,
    ) -> Result<Json, String> {
        let sub = self.substitute(row);
        if sub["ceremony"]["asks"].is_object() {
            return self.sheet_typed(token, text);
        }
        let wants = sub["ceremony"]["answer"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let value: Json = serde_json::from_str(text.trim()).unwrap_or_default();
        // A member's answer (a sign-in's credential) answers a call that carries the member.
        if let Some(checked) = self.member_typed(row, args, &value) {
            return checked.map(|_| value);
        }
        let owned = self.owns_ceremony(&sub, args);
        if !value.is_object() || !owned || wants.iter().any(|(k, v)| value.get(k) != Some(v)) {
            return Err(format!(
                "{}: type the response's JSON, an object with {} (packages/passkeys/test/authenticator.mjs makes one from the ticket's args)",
                self.key(row),
                Json::Object(wants)
            ));
        }
        Ok(value)
    }

    /// The answer a ceremony's tap gives: its `cancel`, or a choice's error.
    pub(super) fn ceremony_tapped(
        &self,
        row: &ModulesRow,
        args: &Json,
        sub: &Json,
        choice: &str,
    ) -> Json {
        if let Some(answer) = self.member_tapped(row, args, choice) {
            return answer;
        }
        let ceremony = &sub["ceremony"];
        let answer = match choice {
            "cancel" => &ceremony["cancel"],
            other => &ceremony["choices"][other],
        };
        match answer.is_null() {
            true => json!({ "error": "NotAllowedError" }),
            false => answer.clone(),
        }
    }

    /// A ceremony of `row` is about to be held (D9): a held
    /// conditional one, or a passive sign-in, ends with `AbortError`; `true`
    /// when a modal one is held, behind which it waits.
    pub(super) fn ceremony_begins(&mut self, row: &ModulesRow) -> bool {
        let extension = self.plan.str(row.extension).to_owned();
        let ours = |r: &Self, h: &Held| {
            h.waits == Waits::Ceremony
                && !h.queued
                && r.plan.str(r.module_row(&h.source).extension) == extension
        };
        // A passive sign-in (LLP 1133.014 D5) ends as a conditional get does.
        let yields = |h: &Held| {
            h.args["mediation"] == "conditional"
                || h.args
                    .get("identity")
                    .is_some_and(|i| !i.is_null() && i["mode"] != "active")
        };
        let conditional: Vec<(u64, u64)> = (self.extensions.held.iter())
            .filter(|h| ours(self, h) && yields(h))
            .map(|h| (h.token, h.hold))
            .collect();
        for (token, hold) in conditional {
            self.retire_hold(hold);
            let ended = json!({ "error": "AbortError", "message": "a newer ceremony replaced this conditional or passive one" });
            self.release(token, ended);
        }
        (self.extensions.held.iter()).any(|h| ours(self, h))
    }

    /// A ceremony waiting behind a modal one gets its ticket once none is held.
    pub(super) fn promote_ceremony(&mut self) {
        let modal = (self.extensions.held.iter()).any(|h| h.waits == Waits::Ceremony && !h.queued);
        if modal {
            return;
        }
        let Some(at) = (self.extensions.held.iter()).position(|h| h.queued) else {
            return;
        };
        let (source, args, origin) = {
            let h = &self.extensions.held[at];
            (h.source.clone(), h.args.clone(), h.origin)
        };
        let row = self.module_row(&source);
        let name = self.key(&row);
        let hold = self.device_hold(&row, name, Waits::Ceremony, origin, "", &args);
        let h = &mut self.extensions.held[at];
        h.hold = hold;
        h.queued = false;
        let token = h.token;
        self.sheet_opened(token);
    }

    /// A ceremony or signal of `row` is listed in `state.extensions`.
    pub(super) fn record_ceremony(
        &mut self,
        token: u64,
        row: &ModulesRow,
        args: &Json,
        outcome: &str,
    ) {
        let mut entry = self.sheet_entry(token, outcome).unwrap_or_else(|| {
            json!({
                "name": self.plan.str(row.declared),
                "mediation": args["mediation"],
                "rpId": self.rp_id(row, args),
                "origin": self.ceremony_args(row, args, CallOrigin::App)["clientOrigin"],
                "outcome": outcome,
            })
        });
        // A member's request by its provider and mode, never its client or nonce.
        for (option, described) in self.members_args(row, args) {
            let providers: Vec<Json> = (described["providers"].as_array().into_iter().flatten())
                .map(|p| p["provider"].clone())
                .collect();
            entry[option] = json!({ "mode": described["mode"], "providers": providers });
        }
        let extension = self.plan.str(row.extension).to_owned();
        let list = &mut self.extensions.ceremonies;
        match list.iter_mut().find(|(t, _, _)| *t == token) {
            Some(existing) => existing.2 = entry,
            None => list.push((token, extension, entry)),
        }
        if list.len() > KEPT {
            list.remove(0);
        }
    }

    /// The ceremonies `extension` held or answered, for `state.extensions`.
    pub(super) fn ceremonies_of(&self, extension: &str) -> Vec<Json> {
        (self.extensions.ceremonies.iter())
            .filter(|(_, e, _)| e == extension)
            .map(|(_, _, entry)| entry.clone())
            .collect()
    }

    /// `answer`, a data-code call's of held call `held`, as it leaves the
    /// runner: its withheld paths removed, its outcome listed, and, for a
    /// reply that carries handles, provisional until its request lands, or
    /// disposed of now when that request was let go (D11).
    pub(super) fn ceremony_ended(&mut self, held: &Held, mut answer: Json) -> Json {
        let row = self.module_row(&held.source);
        let sub = self.substitute(&row);
        for path in sub["withhold"].as_array().into_iter().flatten() {
            if let Some(path) = path.as_str() {
                remove(&mut answer, path);
            }
        }
        if held.waits != Waits::Ceremony {
            return answer;
        }
        // A member's answer carries the member's handles, its package's to dispose of.
        let (extension, handles) = match self.member_answering(&row, &answer) {
            Some((extension, member)) => (extension, member["handles"].clone()),
            None => (
                self.plan.str(row.extension).to_owned(),
                sub["handles"].clone(),
            ),
        };
        let ids: Vec<String> = (handles.as_object().into_iter().flatten())
            .filter_map(|(path, _)| at(&answer, path)?.as_str().map(str::to_owned))
            .collect();
        if let (Some(ticket), false) = (held.request, ids.is_empty()) {
            match self.pending.iter().any(|p| p.ticket == ticket) {
                true => self.arrive(ticket, extension, ids),
                // The call ends: whatever still awaits it never has the response.
                false => {
                    self.dispose((extension, ids), "its reply was dropped");
                    answer = json!({ "error": "AbortError", "message": "the call was let go" });
                }
            }
        }
        let outcome = match super::error_code(&answer) {
            Some(code) => code.to_owned(),
            None => "answered".to_owned(),
        };
        self.record_ceremony(held.token, &row, &held.args, &outcome);
        if !held.queued {
            self.promote_ceremony();
        }
        answer
    }
}
