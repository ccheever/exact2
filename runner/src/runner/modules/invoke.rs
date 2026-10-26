//! A module calling the app back (LLP 1133.016 D9, D15 item 1): an
//! implementation's `invoke(handle, handler, args)` runs one of the data
//! module's `tools` for the work a handle names. Only a handler the call
//! that made the handle listed runs: that call's `handlers` path names them
//! (`tools[].name`), or its `handlersFrom` option names the handle whose
//! list it takes. The handler runs as a fresh answer that fills no slot,
//! under the less privileged of `tool` and the origin of the call that made
//! the handle, and each module call it makes is admitted under that origin
//! (`data_origin`). As the background round's (`background.rs`), the
//! requests it awaits go out under tickets of their own, no target's: no
//! commit, no `then`.

use super::super::{DataError, DataSource, Runner};
use super::handles::{collect, ids_in};
use super::{CallOrigin, Json, ModulesRow};
use crate::request::{Answer, Dispatch, Outcome, Reply, RequestOut, Work};
use exact_plan::Value;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// What `state.pending` and the journal call a handler's request.
const NAME: &str = "tool";

/// The handles kept granted at most; the oldest go first.
const KEPT: usize = 256;

/// A handle's grant: the extension whose call made it, the handlers that
/// call listed, and its origin.
#[derive(Clone)]
pub(super) struct Grant {
    extension: String,
    handlers: BTreeSet<String>,
    origin: CallOrigin,
}

/// The grants, by handle id, in the order they were made. Filled as a
/// reply leaves the implementation, from any thread.
pub(super) type Granted = Arc<Mutex<Vec<(String, Grant)>>>;

/// A handler awaiting a request of its own.
struct Running {
    at: String,
    source: String,
    args: Value,
    origin: CallOrigin,
    started: f64,
    reply: Option<Box<dyn FnOnce(String) + Send>>,
}

impl Running {
    fn answer(mut self, answer: &Json) {
        if let Some(reply) = self.reply.take() {
            reply(answer.to_string());
        }
    }
}

/// A handler the session drops unanswered (it ended, or was replaced) still
/// answers its implementation, which waits for it.
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            reply(json!({ "error": "AbortError", "message": "the session ended" }).to_string());
        }
    }
}

#[derive(Default)]
pub(in crate::runner) struct Invocations {
    granted: Granted,
    running: BTreeMap<u64, Running>,
    /// Requests the host refused at admission, settled in their turn.
    refused: Vec<(u64, &'static str)>,
}

impl<D: DataSource> Runner<D> {
    /// The grant a call of `row` with the implementation's `body` makes for
    /// the handles its reply carries, if its declaration lists handlers.
    pub(super) fn handler_grant(&self, row: &ModulesRow, body: &Json) -> Option<Grant> {
        let sub = self.substitute(row);
        let args = &body["args"];
        let handlers: BTreeSet<String> = if let Some(path) = sub["handlers"].as_str() {
            let mut names = Vec::new();
            collect(args, path, &mut names);
            names.into_iter().collect()
        } else {
            let id = args[sub["handlersFrom"].as_str()?].as_str()?;
            let granted = self.extensions.invocations.granted.lock();
            let granted = granted.unwrap_or_else(|e| e.into_inner());
            let (_, grant) = granted.iter().rev().find(|(h, _)| h == id)?;
            grant.handlers.clone()
        };
        let origin = match body["origin"].as_str() {
            Some("device") => CallOrigin::Device,
            Some("wake") => CallOrigin::Wake,
            Some("tool") => CallOrigin::Tool,
            _ => CallOrigin::App,
        };
        let extension = self.plan.str(row.extension).to_owned();
        Some(Grant {
            extension,
            handlers,
            origin,
        })
    }

    /// The grants, for a reply's watcher.
    pub(super) fn granted(&self) -> Granted {
        self.extensions.invocations.granted.clone()
    }

    /// `work`, a data-code call of `row` with `body`, whose reply grants
    /// the handles it carries as it leaves the implementation.
    pub(super) fn granting(&self, row: &ModulesRow, body: &[u8], work: Dispatch) -> Dispatch {
        let body: Json = serde_json::from_slice(body).unwrap_or_default();
        let paths: Vec<String> = (self.substitute(row)["handles"].as_object())
            .map(|h| h.keys().cloned().collect())
            .unwrap_or_default();
        let (Some(grant), false) = (self.handler_grant(row, &body), paths.is_empty()) else {
            return work;
        };
        let Dispatch::Run(Work::Later(run)) = work else {
            return work;
        };
        let granted = self.granted();
        Dispatch::Run(Work::Later(Box::new(move |reply| {
            run(Reply::new(move |outcome: Outcome| {
                grant_handles(&granted, &ids_in(&outcome, &paths), &grant);
                reply.send(outcome);
            }));
        })))
    }

    /// An implementation's call of the app's handler, `{"extension",
    /// "handle", "handler", "args"}`: refused unless the call that made
    /// `handle` listed `handler`, else run; `reply` is given `{"text"}`,
    /// the handler's value as JSON, or `{"error", "message"}`, once.
    pub fn invoke_module(&mut self, body: &str, reply: Box<dyn FnOnce(String) + Send>) {
        let v: Json = serde_json::from_str(body).unwrap_or_default();
        let text = |field: &str| v[field].as_str().unwrap_or_default().to_owned();
        let (extension, handle, handler) = (text("extension"), text("handle"), text("handler"));
        let at = format!("{extension}: {handler} for {handle}");
        let grant = {
            let granted = self.extensions.invocations.granted.lock();
            let granted = granted.unwrap_or_else(|e| e.into_inner());
            (granted.iter().rev())
                .find(|(h, g)| *h == handle && g.extension == extension)
                .map(|(_, g)| g.clone())
        };
        let Some(grant) = grant.filter(|g| g.handlers.contains(&handler)) else {
            self.log(format!(
                "{at}: refused, no call made for it lists the handler"
            ));
            let message = format!("no call that made {handle} lists {handler}");
            return reply(json!({ "error": "NotAllowedError", "message": message }).to_string());
        };
        let origin = grant.origin.min(CallOrigin::Tool);
        self.log(format!("{at}: run, {}:model:{handle}", origin.word()));
        let run = Running {
            at,
            source: format!("tool:{handler}"),
            args: Value::from(v["args"].to_string()),
            origin,
            started: self.now_ms,
            reply: Some(reply),
        };
        let answer = self.as_origin(origin, |r| {
            (r.data).answer(&mut r.store, &run.source, std::slice::from_ref(&run.args))
        });
        self.tool_step(run, answer);
    }

    /// A handler's turn ended: its answer to the implementation, or its
    /// next request out.
    fn tool_step(&mut self, run: Running, answer: Result<Answer, DataError>) {
        self.take_data_logs();
        let answer = match answer {
            Ok(Answer::Later(request)) => {
                let ticket = self.next_ticket;
                self.next_ticket += 1;
                self.requests.push(RequestOut {
                    ticket,
                    target: NAME.into(),
                    request,
                    forced: false,
                });
                self.extensions.invocations.running.insert(ticket, run);
                self.native_slot().changed("");
                return;
            }
            Ok(Answer::Now(value)) => {
                let text = value.as_str().unwrap_or("null").to_owned();
                let ms = self.now_ms - run.started;
                self.log(format!(
                    "{}: answered {} bytes in {ms} ms",
                    run.at,
                    text.len()
                ));
                json!({ "text": text })
            }
            Err(DataError::UnknownSource(message)) => {
                self.log(format!("{}: refused, {message}", run.at));
                json!({ "error": "NotFoundError", "message": message })
            }
            Err(e) => {
                let message = e.to_string();
                self.log(format!("{}: failed: {message}", run.at));
                json!({ "error": "OperationError", "message": message })
            }
        };
        run.answer(&answer);
    }

    /// Whether `ticket` is a handler's request.
    pub(in crate::runner) fn is_tool(&self, ticket: u64) -> bool {
        self.extensions.invocations.running.contains_key(&ticket)
    }

    /// The handlers' requests in flight, for `pending` and `in_flight`.
    pub(in crate::runner) fn tool_tickets(&self) -> Vec<(String, u64)> {
        (self.extensions.invocations.running.keys())
            .map(|t| (NAME.to_owned(), *t))
            .collect()
    }

    /// The least privileged origin of a handler running now, if one is.
    pub(in crate::runner) fn tool_origin(&self) -> Option<CallOrigin> {
        (self.extensions.invocations.running.values())
            .map(|r| r.origin)
            .min()
    }

    /// A handler's request came back: its turn resumes.
    pub(in crate::runner) fn tool_landed(&mut self, ticket: u64, outcome: Outcome) {
        let Some(run) = self.extensions.invocations.running.remove(&ticket) else {
            return;
        };
        let answer = self.as_invocation(run.origin, 0, |r| {
            let args = std::slice::from_ref(&run.args);
            (r.data).parse(&mut r.store, &run.source, args, outcome)
        });
        self.tool_step(run, answer);
    }

    /// The host refused a handler's request at admission: it settles in
    /// its turn, as a failure.
    pub(in crate::runner) fn refuse_tool(&mut self, ticket: u64, reason: &'static str) {
        self.extensions.invocations.refused.push((ticket, reason));
    }

    /// A handler's refused request, to settle.
    pub(in crate::runner) fn tool_refusal(&mut self) -> Option<(u64, &'static str)> {
        let refused = &mut self.extensions.invocations.refused;
        (!refused.is_empty()).then(|| refused.remove(0))
    }
}

/// The handles `ids` a reply carried are granted `grant`.
pub(super) fn grant_handles(granted: &Granted, ids: &[String], grant: &Grant) {
    let mut granted = granted.lock().unwrap_or_else(|e| e.into_inner());
    for id in ids {
        granted.retain(|(h, _)| h != id);
        granted.push((id.clone(), grant.clone()));
    }
    let over = granted.len().saturating_sub(KEPT);
    granted.drain(..over);
}
