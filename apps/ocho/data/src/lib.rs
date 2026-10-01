//! Ocho's data source: the workspace model, a port of the GPUI desktop's
//! (`fleet/desktop`, origin/main e577272), answering the view from memory.
//!
//! The seam is Elm-shaped. The contract keeps two counters (`ui.version`,
//! `ui.io`) and sends every event to `dispatch`; the model changes and the
//! counters come back. Resources `feed` and `desktop` bring the app's
//! module's I/O in (the `fleet snapshot --watch` stream, the `desktop.json`
//! mirror) and hand a version out; `io(n)` runs the jobs the model queued
//! (fleet commands) through the module and applies their replies; `view(…)`
//! is the whole view-model, answered now from the model whenever any of the
//! versions moves. Terminals stay native views (`ghostty-terminal`), keyed
//! by the tab the model names.

#![deny(missing_docs)]
// The port lands module by module; what the view does not reach yet is
// kept, not pruned, until the last phase wires it (then this line goes).
#![allow(dead_code)]

mod finders;
mod forms;
mod imessage_pair;
mod indicator;
mod keymap;
mod launch;
mod markdown;
mod markdown_doc;
mod model;
mod notifications;
mod palette;
mod permissions;
mod picker;
mod preferences;
mod quick;
pub mod recovery;
mod rows;
mod secret_requests;
mod session;
mod settings;
mod shapes;
mod tab_tree;
mod theme;
mod themes;
mod transcript;
mod types;
mod updater;
mod view;
mod whats_new;

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};

pub use model::{Event, Workspace};

/// The app's data source.
#[derive(Default)]
pub struct Ocho {
    model: Workspace,
}

/// A `dispatch` argument as text, whatever the contract passed.
fn text(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::Number(n)) => format!("{n}"),
        Some(Value::Bool(b)) => (if *b { "true" } else { "false" }).into(),
        Some(v) => v.as_str().unwrap_or_default().to_string(),
        None => String::new(),
    }
}

fn number(args: &[Value], i: usize) -> f64 {
    match args.get(i) {
        Some(Value::Number(n)) => *n,
        Some(v) => v.as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        None => 0.0,
    }
}

fn json_of(v: &Value) -> serde_json::Value {
    match v {
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::List(items) => serde_json::Value::Array(items.iter().map(json_of).collect()),
        Value::Option(inner) => inner
            .as_deref()
            .map(json_of)
            .unwrap_or(serde_json::Value::Null),
        v => match v.as_str() {
            Some(s) => serde_json::Value::String(s.to_string()),
            None => serde_json::Value::Null,
        },
    }
}

fn counters(model: &Workspace) -> Value {
    shapes::read(
        &shapes::UI,
        &serde_json::json!({ "version": model.version, "io": model.io }),
    )
}

fn version(n: u64) -> Value {
    shapes::read(&shapes::VERSION, &serde_json::json!({ "version": n }))
}

impl DataSource for Ocho {
    fn app_id(&self) -> &str {
        "dev.getfirewood.ocho"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "dispatch" => Ok(counters(&self.model)),
            "feed" | "desktop" | "io" => Ok(version(0)),
            "picture" => Ok(view::render(&self.model, &args_versions(args))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "dispatch" => {
                let event = Event::parse(
                    &text(args, 0),
                    &text(args, 1),
                    &text(args, 2),
                    number(args, 3),
                    number(args, 4),
                );
                self.model.dispatch(event);
                Ok(Answer::Now(counters(&self.model)))
            }
            "picture" => Ok(Answer::Now(view::render(&self.model, &args_versions(args)))),
            "feed" | "desktop" => {
                store.observe_topic(source);
                Ok(Answer::Later(native(source, args)))
            }
            "io" => {
                let jobs = self.model.take_jobs();
                if jobs.is_empty() {
                    return Ok(Answer::Now(version(self.model.io_version)));
                }
                let body = serde_json::json!({ "op": "io", "jobs": jobs });
                Ok(Answer::Later(Request::native(
                    body.to_string().into_bytes(),
                )))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        _args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if matches!(source, "feed" | "desktop") {
            // Keep watching, whatever came back: a request the module could
            // not run yet (its dylib after the first frame) settles as the
            // version so far, and its first announcement asks again.
            store.observe_topic(source);
        }
        let json = match outcome {
            Outcome::Response(response) if response.status == 200 => {
                serde_json::from_slice::<serde_json::Value>(&response.body)
                    .map_err(|e| DataError::Unavailable(format!("{source}: {e}")))?
            }
            Outcome::Response(response) => {
                self.model
                    .io_failed(&String::from_utf8_lossy(&response.body));
                serde_json::Value::Null
            }
            Outcome::Failed { message, .. } => {
                if source == "io" {
                    self.model.io_failed(&message);
                }
                serde_json::Value::Null
            }
            _ => serde_json::Value::Null,
        };
        match source {
            "feed" => {
                self.model.apply_feed(&json);
                Ok(Answer::Now(version(self.model.feed_version)))
            }
            "desktop" => {
                self.model.apply_desktop(&json);
                Ok(Answer::Now(version(self.model.desktop_version)))
            }
            "io" => {
                self.model.apply_io(&json);
                Ok(Answer::Now(version(self.model.io_version)))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn native(op: &str, args: &[Value]) -> Request {
    let body = serde_json::json!({
        "op": op,
        "args": args.iter().map(json_of).collect::<Vec<_>>(),
    });
    Request::native(body.to_string().into_bytes())
}

/// The version numbers `view(…)` was asked with, so an answer can say what
/// it saw (the contract never reads them; they exist to key the ask).
fn args_versions(args: &[Value]) -> Vec<f64> {
    (0..args.len()).map(|i| number(args, i)).collect()
}
