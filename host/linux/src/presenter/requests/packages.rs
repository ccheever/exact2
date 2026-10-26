//! The packages this binary composes (`crate::package`), and the native
//! slot's one handler that reaches them (LLP 1133.022 D10 item 5). The
//! handler queues every body with its reply, from any thread, and wakes the
//! loop; the loop's turn (`modules_in`) hands a body that names a composed
//! package's extension to that package's instance on the loop's thread, a
//! shell's (Android's reader's Kotlin) to the shell, and answers the rest
//! `NotSupportedError`. So a shell and the packages never replace each
//! other's handler.

use super::Inbox;
use crate::package::{Context, Entry, Json, Package, Reply};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The composed packages and their instances in this session.
#[derive(Default)]
pub(in crate::presenter) struct Packages {
    table: Vec<Entry>,
    made: Vec<(&'static str, Box<dyn Package>, Context)>,
    /// Extensions told they run under the agent (`--bind` names the others).
    agent: Vec<&'static str>,
    now: Rc<Cell<f64>>,
    lines: Rc<RefCell<Vec<String>>>,
}

/// The extension a body names.
pub(super) fn extension_of(body: &[u8]) -> Option<String> {
    let v: Json = serde_json::from_slice(body).ok()?;
    Some(v.get("extension")?.as_str()?.to_owned())
}

impl Packages {
    /// The table the entry handed over; which of them run told they are
    /// under the agent: under the agent, all but those `EXACT_AGENT_BIND`
    /// names (`--bind`).
    pub(super) fn set(&mut self, table: Vec<Entry>, agent: bool) {
        let bound = std::env::var("EXACT_AGENT_BIND").unwrap_or_default();
        self.agent = (table.iter())
            .map(|e| e.extension)
            .filter(|e| agent && !bound.split(',').any(|b| b == *e))
            .collect();
        self.table = table;
    }

    /// The extensions bound as in production, and those bound under the
    /// agent, for `Runner::bind_extensions`.
    pub(super) fn bound(&self) -> (Vec<&'static str>, Vec<&'static str>) {
        (self.table.iter().map(|e| e.extension)).partition(|e| !self.agent.contains(e))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub(super) fn serves(&self, extension: &str) -> bool {
        self.table.iter().any(|e| e.extension == extension)
    }

    /// The journal lines the instances wrote since the last call.
    pub(super) fn take_lines(&self) -> Vec<String> {
        std::mem::take(&mut *self.lines.borrow_mut())
    }

    /// Body `body` of a composed package's extension, at `now` on the
    /// session's clock: its instance (made now if this is the session's
    /// first call) is called. A call that the runner asks to admit first
    /// (`admit`: a tool's or a wake's call that would ask) reads the
    /// instance's own status, and comes back through `inbox` without
    /// `admit` unless that status reads `prompt`.
    pub(super) fn dispatch(
        &mut self,
        body: &[u8],
        reply: exact_runner::Reply,
        now: f64,
        native: &exact_runner::Native,
        inbox: &Inbox,
        wake: &std::sync::Arc<dyn Fn() + Send + Sync>,
    ) {
        self.now.set(now);
        let Ok(request) = serde_json::from_slice::<Json>(body) else {
            return Reply::new(reply).error("NotSupportedError", Some("not a module call"));
        };
        let extension = request["extension"].as_str().unwrap_or_default();
        let Some(at) = self.instance(extension, native) else {
            return Reply::new(reply).error("NotSupportedError", None);
        };
        let (_, package, context) = &mut self.made[at];
        let reply = Reply::new(reply);
        let args = request.get("args");
        let named = request.get("source").or(request.get("send"));
        if let Some(name) = request["release"].as_str() {
            package.release(name, args);
            reply.send(Json::Object(Default::default()));
        } else if let (Some(values), None) = (request.get("preferred"), named) {
            package.preferred(values, &request["permissions"]);
            reply.send(Json::Object(Default::default()));
        } else if let Some(ids) = request["dispose"].as_array() {
            let ids: Vec<String> = (ids.iter())
                .filter_map(|i| i.as_str().map(str::to_owned))
                .collect();
            package.dispose(&ids);
            reply.send(Json::Object(Default::default()));
        } else if let Some(n) = request["cancel"].as_u64() {
            package.cancel(n);
            reply.send(Json::Object(Default::default()));
        } else if let Some(shown) = request["foreground"].as_bool() {
            context.0.foreground.set(shown);
            package.foreground(shown);
            reply.send(Json::Object(Default::default()));
        } else if let Some(name) = named.and_then(Json::as_str) {
            let origin = request["origin"].as_str().unwrap_or("app");
            *context.0.origin.borrow_mut() = origin.to_owned();
            context.0.request.set(request["request"].as_u64());
            let args = args.cloned().unwrap_or(Json::Object(Default::default()));
            match admission(&request) {
                Some((status, field, denied)) => {
                    // The instance's own status first (LLP 1133.002 D12):
                    // only a person may answer its prompt.
                    let (inbox, wake) = (inbox.clone(), wake.clone());
                    let mut again = request.clone();
                    again.as_object_mut().map(|o| o.remove("admit"));
                    let asked = exact_runner::Reply::new(move |outcome| {
                        let state = match &outcome {
                            exact_runner::Outcome::Response(r) => {
                                serde_json::from_slice::<Json>(&r.body).unwrap_or_default()
                                    [field.as_str()]
                                .clone()
                            }
                            _ => Json::Null,
                        };
                        if state == "prompt" {
                            return reply.error(&denied, Some("person-required"));
                        }
                        (inbox.lock().unwrap_or_else(|e| e.into_inner()))
                            .push((again.to_string().into_bytes(), reply.0));
                        wake();
                    });
                    let empty = Json::Object(Default::default());
                    package.later(&status, &empty, None, Reply::new(asked));
                }
                None => package.later(name, &args, request.get("preferred"), reply),
            }
        } else {
            // A view's or a surface's report, a wake's outcome: nothing
            // these hosts' packages serve yet.
            reply.send(Json::Object(Default::default()));
        }
    }

    /// The index of `extension`'s instance in this session, made now if
    /// none is.
    fn instance(&mut self, extension: &str, native: &exact_runner::Native) -> Option<usize> {
        if let Some(at) = self.made.iter().position(|(e, ..)| *e == extension) {
            return Some(at);
        }
        let entry = *self.table.iter().find(|e| e.extension == extension)?;
        #[cfg(windows)]
        crate::com::ensure_sta();
        let context = Context::new(
            entry.extension,
            self.agent.contains(&entry.extension),
            native.clone(),
            self.now.clone(),
            self.lines.clone(),
        );
        let package = entry.make(context.clone());
        self.made.push((entry.extension, package, context));
        Some(self.made.len() - 1)
    }

    /// The session ends: each instance's `destroy`, in the order they were
    /// made, and none called again.
    pub(super) fn destroy(&mut self) {
        for (_, mut package, _) in std::mem::take(&mut self.made) {
            package.destroy();
        }
    }
}

/// What a call's `admit` asks: the status source's name, the field that
/// reads `prompt`, and the error to fail with.
fn admission(request: &Json) -> Option<(String, String, String)> {
    let admit = request.get("admit")?;
    Some((
        admit["status"].as_str()?.to_owned(),
        admit["field"].as_str()?.to_owned(),
        admit["denied"]
            .as_str()
            .unwrap_or("NotAllowedError")
            .to_owned(),
    ))
}
