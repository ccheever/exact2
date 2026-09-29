//! Ocho's data source. The app is a client of the `fleet` CLI, and everything
//! that reaches outside the process lives in the app's one native module (the
//! Swift `Ocho` on macOS; `modules/web/index.js` on the web): the `fleet
//! snapshot --watch` feed, the open tabs and their terminals, the one-shot
//! commands. This source is the portable glue: each answer is a long native
//! call (`Request::native`) whose JSON reply is read into the shape the
//! Contract declares. The `fleet` resource watches the module's `fleet`
//! topic, so each announcement asks it again.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};

/// A value's shape, mirroring the Contract's `shape` declarations field for
/// field. Records are read by name from the module's JSON and written by
/// position, the order the Contract declares.
enum Shape {
    Str,
    Num,
    Bool,
    List(&'static Shape),
    Record(&'static [(&'static str, Shape)]),
}

use Shape::{Bool, List, Num, Record, Str};

const MACHINE: Shape = Record(&[
    ("id", Str),
    ("name", Str),
    ("online", Bool),
    ("status", Str),
    ("sessions", Num),
    ("active", Num),
    ("detail", Str),
    ("local", Bool),
    ("home", Str),
    ("index", Num),
]);
const SESSION: Shape = Record(&[
    ("id", Str),
    ("machineId", Str),
    ("machine", Str),
    ("title", Str),
    ("provider", Str),
    ("account", Str),
    ("state", Str),
    ("cwd", Str),
    ("model", Str),
    ("summary", Str),
    ("updated", Str),
    ("pinned", Bool),
    ("live", Bool),
    ("index", Num),
]);
const ACCOUNT: Shape = Record(&[
    ("handle", Str),
    ("email", Str),
    ("provider", Str),
    ("status", Str),
    ("connected", Bool),
    ("usage", Str),
]);
const TAB: Shape = Record(&[
    ("id", Str),
    ("slot", Num),
    ("title", Str),
    ("subtitle", Str),
    ("provider", Str),
    ("machineId", Str),
    ("sessionId", Str),
    ("state", Str),
    ("status", Str),
    ("depth", Num),
    ("folder", Bool),
    ("collapsed", Bool),
]);
const CHOICE: Shape = Record(&[
    ("id", Str),
    ("code", Str),
    ("group", Str),
    ("label", Str),
    ("detail", Str),
    ("provider", Str),
]);
const FLEET: Shape = Record(&[
    ("ready", Bool),
    ("message", Str),
    ("machines", List(&MACHINE)),
    ("sessions", List(&SESSION)),
    ("accounts", List(&ACCOUNT)),
    ("tabs", List(&TAB)),
    ("accountChoices", List(&CHOICE)),
    ("machineChoices", List(&CHOICE)),
    ("recentDirs", List(&CHOICE)),
    ("liveCount", Num),
    ("revision", Num),
    ("terminalBackground", Str),
]);
const MODELS: Shape = Record(&[
    ("key", Str),
    ("ready", Bool),
    ("message", Str),
    ("models", List(&CHOICE)),
]);
const RECEIPT: Shape = Record(&[
    ("ok", Bool),
    ("message", Str),
    ("tabId", Str),
    ("version", Num),
]);

/// The shape each source answers with; `None` is an unknown source.
fn shape_of(source: &str) -> Option<&'static Shape> {
    Some(match source {
        "fleet" => &FLEET,
        "models" => &MODELS,
        "openTab" | "openShell" | "closeTab" | "toggleFolder" | "launch" | "sessionAction" => {
            &RECEIPT
        }
        _ => return None,
    })
}

/// The topic a source's answer watches, if any: the module announces it.
fn topic_of(source: &str) -> Option<&'static str> {
    match source {
        "fleet" => Some("fleet"),
        _ => None,
    }
}

/// `json` read as `shape`: a missing or mistyped field is the shape's empty
/// value, never a refusal, so a module that grows a field keeps working.
fn read(shape: &Shape, json: &serde_json::Value) -> Value {
    match shape {
        Str => match json {
            serde_json::Value::String(s) => Value::str(s),
            serde_json::Value::Number(n) => Value::str(&n.to_string()),
            serde_json::Value::Bool(b) => Value::str(if *b { "true" } else { "false" }),
            _ => Value::str(""),
        },
        Num => Value::Number(json.as_f64().unwrap_or(0.0)),
        Bool => Value::Bool(json.as_bool().unwrap_or(false)),
        List(inner) => Value::list(
            json.as_array()
                .map(|items| items.iter().map(|v| read(inner, v)).collect())
                .unwrap_or_default(),
        ),
        Record(fields) => Value::record(
            fields
                .iter()
                .map(|(name, s)| read(s, json.get(*name).unwrap_or(&serde_json::Value::Null)))
                .collect(),
        ),
    }
}

/// A Contract value as JSON, for the request's arguments.
fn json_of(value: &Value) -> serde_json::Value {
    match value {
        Value::Number(n) => serde_json::json!(n),
        Value::Bool(b) => serde_json::json!(b),
        Value::Unit => serde_json::Value::Null,
        Value::Option(None) => serde_json::Value::Null,
        Value::Option(Some(v)) => json_of(v),
        Value::List(items) => serde_json::Value::Array(items.iter().map(json_of).collect()),
        Value::Record(items) => serde_json::Value::Array(items.iter().map(json_of).collect()),
        other => serde_json::Value::String(other.text().to_string()),
    }
}

/// The app's data source. Every answer is a request the host runs: at the
/// bake that leaves the resource pending, so the first frame is the empty
/// picture and the host asks the module once it is up.
#[derive(Default)]
pub struct Ocho;

impl DataSource for Ocho {
    fn app_id(&self) -> &str {
        "dev.getfirewood.ocho"
    }

    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match shape_of(source) {
            Some(shape) => Ok(read(shape, &serde_json::Value::Null)),
            None => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let Some(_) = shape_of(source) else {
            return Err(DataError::UnknownSource(source.into()));
        };
        if let Some(topic) = topic_of(source) {
            store.observe_topic(topic);
        }
        let body = serde_json::json!({
            "op": source,
            "args": args.iter().map(json_of).collect::<Vec<_>>(),
        });
        Ok(Answer::Later(Request::native(
            body.to_string().into_bytes(),
        )))
    }

    fn parse(
        &mut self,
        _store: &mut Store,
        source: &str,
        _args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Some(shape) = shape_of(source) else {
            return Err(DataError::UnknownSource(source.into()));
        };
        // The answer keeps watching its topic, whatever came back: a request
        // the host could not run (the module not yet loaded, on a native
        // host's first frame) settles as the empty picture, and the module's
        // first announcement asks again.
        if let Some(topic) = topic_of(source) {
            _store.observe_topic(topic);
            if let Outcome::Failed { .. } = outcome {
                return Ok(Answer::Now(read(shape, &serde_json::Value::Null)));
            }
        }
        match outcome {
            Outcome::Response(response) if response.status == 200 => {
                let json: serde_json::Value = serde_json::from_slice(&response.body)
                    .map_err(|e| DataError::Unavailable(format!("{source}: {e}")))?;
                Ok(Answer::Now(read(shape, &json)))
            }
            Outcome::Response(response) => Err(DataError::Unavailable(format!(
                "{source}: {}",
                String::from_utf8_lossy(&response.body)
            ))),
            Outcome::Failed { message, .. } => {
                Err(DataError::Unavailable(format!("{source}: {message}")))
            }
            _ => Err(DataError::Unavailable(format!(
                "{source}: unexpected outcome"
            ))),
        }
    }
}
