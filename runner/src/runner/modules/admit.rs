//! One admission for every call of a module's name, from every caller
//! (LLP 1133.002 D14), before anything answers it: the substitute, a ticket,
//! or a bound implementation, in production or under the agent.
//!
//! In order: the grant lines the name or its arguments list (LLP 1133.009
//! D10 item 9; the interface's own lines are the bake's, `grant-missing`);
//! the name's `origin` (`"app"`: a tool invocation or a wake is refused,
//! `person-required`, item 8); its `foreground` (refused while the page is
//! hidden, item 3); each conditioned by its `when` where it has one; then the
//! permission. Every refusal answers `NotAllowedError`.
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

use super::super::{CommitReceipt, DataSource, Runner, RunnerError, Target};
use super::{error_code, Decision, Json, ModulesRow, Waits};
use exact_plan::Value;
use serde_json::json;
use std::collections::BTreeMap;

/// Who a call comes from (LLP 1133.002 D12), least privileged first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallOrigin {
    /// A background wake (LLP 1114.001).
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
        if let Target::Resource(i) = target {
            match origin {
                CallOrigin::App => o.asked.remove(&i),
                other => o.asked.insert(i, other),
            };
        }
    }

    /// A data-code call's origin: the least privileged of the commit being
    /// made and every request in flight, since such a call cannot be traced
    /// to the answer that made it.
    pub(in crate::runner) fn data_origin(&self) -> CallOrigin {
        let o = &self.extensions.origins;
        (self.pending.iter())
            .filter_map(|p| o.tickets.get(&p.ticket).copied())
            .fold(o.now, CallOrigin::min)
    }

    /// Forget the origins of requests no longer in flight.
    pub(in crate::runner) fn prune_origins(&mut self) {
        let pending = &self.pending;
        (self.extensions.origins.tickets).retain(|t, _| pending.iter().any(|p| p.ticket == *t));
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
        if let Err(why) = self.admit(row, args, origin) {
            self.log(format!("{}: refused, {why}", self.key(row)));
            return Decision::Now(json!({ "error": "NotAllowedError" }));
        }
        let decision = match self.route(row) {
            Route::Real => return Decision::Forward,
            Route::Agent => self
                .decide_permission(row, send)
                .unwrap_or(Decision::Forward),
            Route::Substitute => self.decide_timed(row, send, timeout),
        };
        // A name whose grants are per argument asks permission only for an
        // argument it was given: `getUserMedia(audio=false)` asks for no
        // device, and its implementation refuses it at once (LLP 1133.009
        // D3).
        let asks = match &self.substitute(row)["nameGrants"] {
            Json::Object(by_argument) => {
                (by_argument.keys()).any(|a| !args[a].is_null() && args[a] != false)
            }
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
                let code = self.substitute(row)["permission"]["denied"].clone();
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

    /// The checks before any decision: why the call is refused, if it is.
    fn admit(&self, row: &ModulesRow, args: &Json, origin: CallOrigin) -> Result<(), String> {
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
                    let given = &args[argument];
                    if !given.is_null() && *given != false && !granted(line) {
                        return Err(format!("grant-missing ({argument}= needs {})", named(line)));
                    }
                }
            }
            line @ Json::Array(_) if !granted(line) => {
                return Err(format!("grant-missing ({})", named(line)));
            }
            _ => {}
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
        if applies && sub["origin"] == "app" && origin != CallOrigin::App {
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
        if self.route(row) != Route::Agent || error_code(&answer).is_some() {
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
