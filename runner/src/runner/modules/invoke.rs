//! A module calling the app back (LLP 1133.016 D9, D15 item 1): an
//! implementation's `invoke(handle, handler, args)` runs one of the data
//! module's `tools` for the work a handle names. Only a handler the call
//! that made the handle listed runs: that call's `handlers` path names them
//! (`tools[].name`), or its `handlersFrom` option names the handle whose
//! list it takes.
//!
//! A pending call is an anchor too (LLP 1133.034 D16 item 1): the handle
//! `request:<n>` names the send numbered `n` while its implementation has
//! not answered it, and admits the handlers its `callbacks` options named.
//! One marked `final` holds the call while it runs: a let-go send is not
//! cancelled until it answers or its budget ends (`bound.rs`). The
//! substitute's payment sheet runs the app's handlers the same way
//! (`sheet.rs`), each answer going to the sheet.
//!
//! The handler runs as a fresh answer that fills no slot,
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

/// A pending send's grant: its extension, the handlers its `callbacks`
/// options named (each with whether it is `final`), its origin, and the
/// budget of a final one, ms.
pub(super) struct Anchor {
    extension: String,
    handlers: BTreeMap<String, bool>,
    origin: CallOrigin,
    budget: f64,
}

/// The anchors, by the send's number.
pub(super) type Anchors = Arc<Mutex<BTreeMap<u64, Anchor>>>;

/// Where a handler's answer goes.
pub(super) enum To {
    /// The implementation that invoked it, which waits for it.
    Implementation(Box<dyn FnOnce(String) + Send>),
    /// The substitute's sheet held under `token`, for its call `attempt`.
    Sheet { token: u64, attempt: u64 },
}

/// A handler awaiting a request of its own.
struct Running {
    at: String,
    source: String,
    args: Value,
    origin: CallOrigin,
    started: f64,
    to: Option<To>,
    /// The send it is the final handler of, by number.
    final_of: Option<u64>,
}

/// A handler the session drops unanswered (it ended, or was replaced) still
/// answers its implementation, which waits for it.
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(To::Implementation(reply)) = self.to.take() {
            reply(json!({ "error": "AbortError", "message": "the session ended" }).to_string());
        }
    }
}

#[derive(Default)]
pub(in crate::runner) struct Invocations {
    granted: Granted,
    running: BTreeMap<u64, Running>,
    /// Requests the host refused at admission, settled in their turn, and
    /// whether each was ordered.
    refused: Vec<(u64, &'static str, bool)>,
    anchors: Anchors,
    /// The sends whose final handler runs, with when its budget ends.
    finals: BTreeMap<u64, f64>,
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
        if let Some(n) = handle
            .strip_prefix("request:")
            .and_then(|n| n.parse::<u64>().ok())
        {
            return self.invoke_anchored(n, &v, at, reply);
        }
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
        let to = To::Implementation(reply);
        self.run_handler(at, &handler, &v["args"], origin, to, None);
    }

    /// [`Self::invoke_module`] for the send numbered `n`, while it is pending.
    fn invoke_anchored(
        &mut self,
        n: u64,
        v: &Json,
        at: String,
        reply: Box<dyn FnOnce(String) + Send>,
    ) {
        let (extension, handler) = (v["extension"].as_str(), v["handler"].as_str());
        let grant = {
            let anchors = self.extensions.invocations.anchors.lock();
            let anchors = anchors.unwrap_or_else(|e| e.into_inner());
            (anchors.get(&n))
                .filter(|a| Some(a.extension.as_str()) == extension)
                .and_then(|a| Some((*a.handlers.get(handler?)?, a.origin, a.budget)))
        };
        let Some((last, origin, budget)) = grant else {
            self.log(format!(
                "{at}: refused, no pending call numbered {n} names the handler"
            ));
            let message = format!(
                "no pending call numbered {n} names {}",
                handler.unwrap_or("")
            );
            return reply(json!({ "error": "NotAllowedError", "message": message }).to_string());
        };
        let origin = origin.min(CallOrigin::Tool);
        self.log(format!("{at}: run, {}", origin.word()));
        if last {
            let ends = self.now_ms + budget;
            self.extensions.invocations.finals.insert(n, ends);
        }
        let to = To::Implementation(reply);
        let handler = handler.unwrap_or_default().to_owned();
        self.run_handler(at, &handler, &v["args"], origin, to, last.then_some(n));
    }

    /// Run the data module's handler `handler` with `args` as a fresh answer
    /// of `origin`, its answer to `to`.
    pub(super) fn run_handler(
        &mut self,
        at: String,
        handler: &str,
        args: &Json,
        origin: CallOrigin,
        to: To,
        final_of: Option<u64>,
    ) {
        let run = Running {
            at,
            source: format!("tool:{handler}"),
            args: Value::from(args.to_string()),
            origin,
            started: self.now_ms,
            to: Some(to),
            final_of,
        };
        let answer = self.as_origin(origin, |r| {
            r.store.set_clock_ms(r.now_ms);
            (r.data).answer(&mut r.store, &run.source, std::slice::from_ref(&run.args))
        });
        self.tool_step(run, answer);
    }

    /// A send with `body` went to its implementation: its handlers are
    /// granted while it is pending, by its number, which is returned.
    pub(super) fn anchor_body(
        &self,
        row: &ModulesRow,
        body: &[u8],
        origin: CallOrigin,
    ) -> Option<u64> {
        let sub = self.substitute(row);
        let callbacks = sub["callbacks"].as_object()?;
        let body: Json = serde_json::from_slice(body).ok()?;
        let n = body["request"].as_u64()?;
        let handlers = (callbacks.iter())
            .filter_map(|(option, spec)| {
                let name = body["args"][option].as_str()?;
                Some((name.to_owned(), spec["final"] == true))
            })
            .collect();
        let anchor = Anchor {
            extension: self.plan.str(row.extension).to_owned(),
            handlers,
            origin,
            budget: sub["ceremony"]["budget"].as_f64().unwrap_or(25_000.0),
        };
        let anchors = self.extensions.invocations.anchors.lock();
        anchors.unwrap_or_else(|e| e.into_inner()).insert(n, anchor);
        Some(n)
    }

    /// The send numbered `n` answered or was cancelled: its handlers are no
    /// longer granted.
    pub(super) fn unanchor(&self, n: u64) {
        let anchors = self.extensions.invocations.anchors.lock();
        anchors.unwrap_or_else(|e| e.into_inner()).remove(&n);
    }

    /// `work`, a data-code send anchored as `n`, whose anchor goes with its reply.
    pub(super) fn unanchoring(&self, n: Option<u64>, work: Dispatch) -> Dispatch {
        let Some(n) = n else {
            return work;
        };
        let Dispatch::Run(Work::Later(run)) = work else {
            return work;
        };
        let anchors = self.extensions.invocations.anchors.clone();
        Dispatch::Run(Work::Later(Box::new(move |reply| {
            run(Reply::new(move |outcome: Outcome| {
                anchors.lock().unwrap_or_else(|e| e.into_inner()).remove(&n);
                reply.send(outcome);
            }));
        })))
    }

    /// Whether the send numbered `n` has a final handler running.
    pub(super) fn final_running(&self, n: u64) -> bool {
        self.extensions.invocations.finals.contains_key(&n)
    }

    /// Final handlers past their budget on the session's clock: their sends
    /// may be cancelled again.
    pub(super) fn finals_due(&mut self, now_ms: f64) {
        let due: Vec<u64> = (self.extensions.invocations.finals.iter())
            .filter(|(_, ends)| **ends <= now_ms)
            .map(|(n, _)| *n)
            .collect();
        for n in &due {
            self.extensions.invocations.finals.remove(n);
            self.log(format!("request {n}: its final handler is past its budget"));
        }
        if !due.is_empty() {
            self.cancel_let_go();
        }
    }

    /// A handler's turn ended: its answer to the implementation, or its
    /// next request out.
    fn tool_step(&mut self, run: Running, answer: Result<Answer, DataError>) {
        self.take_data_logs();
        let answer = match answer {
            Ok(Answer::Later(mut request)) => {
                let ticket = self.next_ticket;
                self.next_ticket += 1;
                request.reach = self.reach_of(run.origin);
                // A program starts only from the app's origin (LLP 1133.007
                // D13): a handler's is refused before it reaches the host,
                // so no ordered work waits on it.
                match self.process_refusal(&request) {
                    Some(why) => self.refuse_tool(ticket, why, false),
                    None => self.requests.push(RequestOut {
                        ticket,
                        target: NAME.into(),
                        request,
                        forced: false,
                    }),
                }
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
        let mut run = run;
        let final_of = run.final_of;
        match run.to.take() {
            Some(To::Implementation(reply)) => reply(answer.to_string()),
            Some(To::Sheet { token, attempt }) => self.sheet_handled(token, attempt, answer),
            None => {}
        }
        // A send held while its final handler ran is cancelled now if it was let go.
        if final_of.is_some_and(|n| self.extensions.invocations.finals.remove(&n).is_some()) {
            self.cancel_let_go();
        }
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
            r.store.set_clock_ms(r.now_ms);
            (r.data).parse(&mut r.store, &run.source, args, outcome)
        });
        self.tool_step(run, answer);
    }

    /// The host refused a handler's request at admission: it settles in
    /// its turn, as a failure, an ordered one only once earlier ordered work
    /// has.
    pub(in crate::runner) fn refuse_tool(
        &mut self,
        ticket: u64,
        reason: &'static str,
        ordered: bool,
    ) {
        self.extensions
            .invocations
            .refused
            .push((ticket, reason, ordered));
    }

    /// Whether a handler's refused request may settle now.
    pub(in crate::runner) fn has_tool_refusal(&self, allow_ordered: bool) -> bool {
        (self.extensions.invocations.refused.iter())
            .any(|(_, _, ordered)| !ordered || allow_ordered)
    }

    /// Whether an ordered handler request's refusal is waiting: it fences ordered work.
    pub(in crate::runner) fn has_ordered_tool_refusal(&self) -> bool {
        (self.extensions.invocations.refused.iter()).any(|(_, _, ordered)| *ordered)
    }

    /// A handler's refused request that may settle now.
    pub(in crate::runner) fn tool_refusal(
        &mut self,
        allow_ordered: bool,
    ) -> Option<(u64, &'static str)> {
        let refused = &mut self.extensions.invocations.refused;
        let at = refused
            .iter()
            .position(|(_, _, ordered)| !ordered || allow_ordered)?;
        let (ticket, reason, _) = refused.remove(at);
        Some((ticket, reason))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataError, Store};
    use exact_kernel::{Kernel, NodeType};
    use exact_plan::builder::PlanBuilder;

    /// A source whose handler fails with the outcome it was given.
    struct Source;

    impl DataSource for Source {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn parse(
            &mut self,
            _: &mut Store,
            _: &str,
            _: &[Value],
            outcome: Outcome,
        ) -> Result<Answer, DataError> {
            Err(DataError::Unavailable(outcome.summary()))
        }
    }

    /// A handler running ticket `ticket`'s request, whose replies `said` collects.
    fn run(r: &mut Runner<Source>, ticket: u64, said: &Arc<Mutex<Vec<(u64, String)>>>) {
        let said = said.clone();
        let reply: Box<dyn FnOnce(String) + Send> =
            Box::new(move |answer| said.lock().unwrap().push((ticket, answer)));
        let run = Running {
            at: format!("test: h for {ticket}"),
            source: "tool:h".into(),
            args: Value::str("{}"),
            origin: CallOrigin::Tool,
            started: 0.0,
            to: Some(To::Implementation(reply)),
            final_of: None,
        };
        r.extensions.invocations.running.insert(ticket, run);
    }

    /// Two handlers' requests refused together each settle once, the second
    /// in its own pump; an ordered one waits behind the ordered fence until
    /// earlier ordered work is done (LLP 1133.016 D9, LLP 1041 §8.4).
    #[test]
    fn refused_handler_requests_settle_once_each_in_order() {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        let plan = b.finish().unwrap();
        let mut r = Runner::boot(
            plan,
            Source,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let said = Arc::new(Mutex::new(Vec::new()));
        run(&mut r, 901, &said);
        run(&mut r, 902, &said);
        r.refuse_request(901, "native executor admission limit reached", false);
        r.refuse_request(902, "native executor admission limit reached", true);
        assert!(
            r.has_ordered_request_refusals(),
            "the ordered one fences ordered work"
        );
        let (ticket, outcome) = r.take_request_refusal(false).expect("the unordered one");
        assert_eq!(ticket, 901);
        r.fulfill(ticket, outcome).unwrap();
        assert!(
            !r.has_request_refusals(false),
            "the ordered one waits for ordered work"
        );
        assert!(r.take_request_refusal(false).is_none());
        assert!(
            r.has_request_refusals(true),
            "and asks for a pump once it is done"
        );
        let (ticket, outcome) = r.take_request_refusal(true).expect("the ordered one");
        assert_eq!(ticket, 902);
        r.fulfill(ticket, outcome).unwrap();
        assert!(!r.has_request_refusals(true) && !r.has_ordered_request_refusals());
        let said = said.lock().unwrap();
        assert_eq!(said.iter().map(|(t, _)| *t).collect::<Vec<_>>(), [901, 902]);
        assert!(
            said.iter().all(|(_, a)| a.contains("\"error\"")),
            "{said:?}"
        );
        assert!(r.extensions.invocations.running.is_empty());
    }
}
