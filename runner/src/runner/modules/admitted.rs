//! A data-code send to a bound implementation, admitted (LLP 1133.006 D8
//! item 3, LLP 1133.002 D14). Before this, such a send passed the runner
//! unchecked. Now, before the implementation sees it:
//!
//! - **Origin** (D6): a name the interface declares `"origin": "person"` is
//!   refused `NotAllowedError`, reason `person-required`, when the commit
//!   running now is not a person's.
//! - **Paths:** each `{"$path": "app:/…"}` in its arguments must be covered
//!   by the calling source's effective `fs.read` grant (the app's grants,
//!   narrowed by the request's scope), and must be a plain `app:/` path. The
//!   host then maps it to the file (`ModulePaths.swift`; on the web
//!   `direct.js`), so the implementation never checks a grant.
//!
//! What the runner admits an extension's sends by comes with its binding
//! (`bind_extensions`: a line `name {"person": […], "substitute": bool}`),
//! from its interface's `exact.json`, as the build compiled it.
//!
//! The implementation's answer may carry `$state`, what `state.extensions`
//! shows of it (a substitute's last write, LLP 1133.006 §7); it is kept
//! here, and the rest goes to the caller. Each send is counted by name.

use super::super::{DataSource, Runner};
use super::{response, Json};
use crate::request::{Dispatch, Outcome, Reply, Request, Response, Work};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The longest path a call names, in bytes (as `exactBodyFrom`'s).
const MAX_PATH: usize = crate::MAX_BODY_FROM_PATH;

/// What the runner keeps of the bound implementations' sends.
#[derive(Default)]
pub(in crate::runner) struct Admitted {
    /// Per extension, its admission, as its binding line gave it.
    pub(super) declared: BTreeMap<String, Json>,
    /// Sends counted by `extension` then name.
    calls: BTreeMap<String, BTreeMap<String, u64>>,
    /// Each extension's last `$state`, set from the host's threads.
    reported: Arc<Mutex<BTreeMap<String, Json>>>,
}

impl Admitted {
    /// An extension's binding line, `name` or `name {json}`: its name.
    pub(super) fn bind(&mut self, line: &str) -> String {
        let (name, json) = line.split_once(' ').unwrap_or((line, ""));
        if let Ok(admission) = serde_json::from_str::<Json>(json.trim()) {
            self.declared.insert(name.to_owned(), admission);
        }
        name.to_owned()
    }

    /// What `state.extensions` adds for `extension`: its call counts and its
    /// last reported state (a substitute says so in it).
    pub(super) fn state(&self, extension: &str) -> Option<Json> {
        let calls = self.calls.get(extension);
        let reported = (self.reported.lock().unwrap_or_else(|e| e.into_inner()))
            .get(extension)
            .cloned();
        if calls.is_none() && reported.is_none() && !self.declared.contains_key(extension) {
            return None;
        }
        let mut out = json!({ "calls": calls.cloned().unwrap_or_default() });
        if let Some(Json::Object(state)) = reported {
            for (k, v) in state {
                out[k] = v;
            }
        }
        Some(out)
    }

    /// The extensions with something to show, in name order.
    pub(super) fn extensions(&self) -> Vec<String> {
        let mut out: Vec<String> = self.declared.keys().cloned().collect();
        out.extend(self.calls.keys().cloned());
        out.sort();
        out.dedup();
        out
    }
}

impl<D: DataSource> Runner<D> {
    /// A send from data code to a bound implementation: refused here, or
    /// handed to it with its `$state` taken from the answer.
    pub(super) fn admitted_send(
        &mut self,
        extension: &str,
        declared: &str,
        body: &Json,
        request: &Request,
    ) -> Dispatch {
        let key = format!("{extension}.{declared}");
        *(self.extensions.admitted.calls)
            .entry(extension.to_owned())
            .or_default()
            .entry(declared.to_owned())
            .or_default() += 1;
        let refuse = |answer: Json| Dispatch::Run(Work::Now(Box::new(move || response(&answer))));
        let admission = self.extensions.admitted.declared.get(extension);
        let person = admission
            .and_then(|a| a["person"].as_array())
            .is_some_and(|names| names.iter().any(|n| n == declared));
        if person && self.extensions.events.origin() != super::Origin::Person {
            self.log(format!(
                "{key}: refused: NotAllowedError, reason person-required"
            ));
            return refuse(
                json!({ "error": "NotAllowedError", "message": format!("{key}: person-required") }),
            );
        }
        let mut paths = Vec::new();
        collect_paths(&body["args"], &mut paths);
        for path in paths {
            if let Some(why) = self.path_refusal(&path, request.grants.as_deref()) {
                self.log(format!("{key}: refused: {why}"));
                let name = if why.contains("no grant") {
                    "NotAllowedError"
                } else {
                    "DataError"
                };
                return refuse(json!({ "error": name, "message": format!("{key}: {why}") }));
            }
        }
        let handler = self.native_slot().handler();
        let reported = self.extensions.admitted.reported.clone();
        let (extension, bytes) = (extension.to_owned(), body.to_string().into_bytes());
        Dispatch::Run(Work::Later(Box::new(move |reply: Reply| match handler {
            Some(handler) => handler(
                bytes,
                Reply::new(move |outcome| reply.send(take_state(outcome, &extension, &reported))),
            ),
            None => reply.send(Outcome::Failed {
                kind: crate::FailureKind::Unsupported,
                message: "no native module here takes long calls".into(),
            }),
        })))
    }

    /// Why `path` may not be read by this call, if it may not.
    fn path_refusal(&self, path: &str, scope: Option<&str>) -> Option<String> {
        if !path.starts_with("app:/") {
            return Some(format!("{path}: an app:/ path"));
        }
        if path.len() > MAX_PATH {
            return Some(format!("a path is at most {MAX_PATH} bytes"));
        }
        if path[5..]
            .split('/')
            .any(|c| c == "." || c == ".." || c.contains('\0'))
        {
            return Some(format!("{path}: an app:/ path has no . or .. segment"));
        }
        let denied =
            || format!("denied: fs.read {path}: no grant covers it; grant `fs.read {path}`");
        let spec = self.data.grants();
        let lines = |text: &str| -> Vec<String> {
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let app = lines(spec);
        let admitted = match scope {
            Some(scope) => {
                let scoped = lines(scope);
                if scoped.iter().any(|l| !app.contains(l)) {
                    return Some(denied());
                }
                scoped
            }
            None => app,
        };
        let set = exact_grants::GrantSet::parse(&crate::io_grants(&admitted.join("\n"))).ok()?;
        let operation = exact_grants::Operation::FsRead {
            path: path.to_owned(),
        };
        (!set.permits(&operation)).then(denied)
    }
}

/// Every `{"$path": …}` in `value`.
fn collect_paths(value: &Json, out: &mut Vec<String>) {
    match value {
        Json::Object(map) => match (map.len(), map.get("$path")) {
            (1, Some(Json::String(path))) => out.push(path.clone()),
            _ => map.values().for_each(|v| collect_paths(v, out)),
        },
        Json::Array(list) => list.iter().for_each(|v| collect_paths(v, out)),
        _ => {}
    }
}

/// `outcome` without the `$state` its answer carries, which is kept for
/// `extension`.
fn take_state(
    outcome: Outcome,
    extension: &str,
    reported: &Mutex<BTreeMap<String, Json>>,
) -> Outcome {
    let Outcome::Response(Response {
        status: 200,
        headers,
        body,
    }) = outcome
    else {
        return outcome;
    };
    let Ok(Json::Object(mut answer)) = serde_json::from_slice::<Json>(&body) else {
        return Outcome::Response(Response {
            status: 200,
            headers,
            body,
        });
    };
    let Some(state) = answer.remove("$state") else {
        return Outcome::Response(Response {
            status: 200,
            headers,
            body,
        });
    };
    reported
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(extension.to_owned(), state);
    Outcome::Response(Response {
        status: 200,
        headers,
        body: Json::Object(answer).to_string().into_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_found_at_any_depth_and_only_as_the_one_key() {
        let mut out = Vec::new();
        let args = json!({"items": [{"image/png": {"$path": "app:/tmp/a.png"}, "text/plain": "x"},
            {"other": {"$path": "app:/data/b", "extra": 1}}]});
        collect_paths(&args, &mut out);
        assert_eq!(out, vec!["app:/tmp/a.png".to_owned()]);
    }

    #[test]
    fn a_binding_line_carries_its_admission() {
        let mut a = Admitted::default();
        assert_eq!(a.bind("geolocation"), "geolocation");
        assert_eq!(
            a.bind(r#"clipboard {"person":["write"],"substitute":true}"#),
            "clipboard"
        );
        assert_eq!(a.declared["clipboard"]["substitute"], true);
        assert!(!a.declared.contains_key("geolocation"));
    }

    #[test]
    fn the_state_an_answer_carries_is_kept_and_taken_out() {
        let reported = Mutex::new(BTreeMap::new());
        let body = br#"{"$state":{"last":{"types":["text/plain"]}}}"#.to_vec();
        let outcome = take_state(
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body,
            }),
            "clipboard",
            &reported,
        );
        let Outcome::Response(r) = outcome else {
            panic!()
        };
        assert_eq!(r.body, b"{}");
        assert_eq!(
            reported.lock().unwrap()["clipboard"]["last"]["types"][0],
            "text/plain"
        );
    }
}
