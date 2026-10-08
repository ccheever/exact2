//! The view-model's shapes in the contract's field order: `read` turns the
//! model's JSON into the runner's positional records. Every shape here has a
//! twin `shape` in `app.contract`, kept in step by hand.

use exact_plan::Value;

/// A field's type.
pub enum Shape {
    /// `string`.
    Str,
    /// `number`.
    Num,
    /// `bool`.
    Bool,
    /// `list<…>`.
    List(&'static Shape),
    /// A `shape`, its fields in order.
    Record(&'static [(&'static str, Shape)]),
}

use Shape::{Bool, List, Num, Record, Str};

/// `Ui`: the counters every event answers.
pub const UI: Shape = Record(&[
    ("version", Num),
    ("poll", Num),
    ("transcript", Num),
    ("send", Num),
    ("probe", Num),
    ("pulls", Num),
    ("reads", Num),
    ("buzz", Num),
    ("reports", Num),
    ("models", Num),
    ("launches", Num),
    ("gotoMachine", Str),
    ("gotoSession", Str),
]);

/// `Version`.
pub const VERSION: Shape = Record(&[("version", Num)]);

const MARKDOWN_RUN: Shape = Record(&[
    ("id", Str),
    ("text", Str),
    ("bold", Bool),
    ("italic", Bool),
    ("code", Bool),
    ("strike", Bool),
    ("url", Str),
    ("subdued", Bool),
    ("math", Str),
]);

const MARKDOWN_CELL: Shape = Record(&[
    ("id", Str),
    ("runs", List(&MARKDOWN_RUN)),
    ("flow", Bool),
    ("align", Str),
]);

const MARKDOWN_TABLE_ROW: Shape = Record(&[
    ("id", Str),
    ("header", Bool),
    ("cells", List(&MARKDOWN_CELL)),
]);

const MARKDOWN_BLOCK: Shape = Record(&[
    ("id", Str),
    ("kind", Str),
    ("level", Num),
    ("depth", Num),
    ("marker", Str),
    ("quote", Num),
    ("language", Str),
    ("text", Str),
    ("subdued", Bool),
    ("flow", Bool),
    ("runs", List(&MARKDOWN_RUN)),
    ("rows", List(&MARKDOWN_TABLE_ROW)),
    ("align", List(&Str)),
]);

const WINDOW: Shape = Record(&[("id", Str), ("label", Str), ("selected", Bool)]);

const ROW: Shape = Record(&[
    ("id", Str),
    ("kind", Str),
    ("machine", Str),
    ("session", Str),
    ("title", Str),
    ("host", Str),
    ("status", Str),
    ("working", Bool),
    ("blocked", Bool),
    ("offline", Bool),
    ("current", Bool),
    ("unread", Bool),
    ("depth", Num),
    ("first", Bool),
    ("last", Bool),
]);

const HOME: Shape = Record(&[
    ("title", Str),
    ("subtitle", Str),
    ("detail", Str),
    ("warn", Bool),
    ("windows", List(&WINDOW)),
    ("rows", List(&ROW)),
    ("empty", Str),
    ("refreshing", Bool),
    ("build", Str),
]);

const ENTRY: Shape = Record(&[
    ("id", Str),
    ("kind", Str),
    ("subdued", Bool),
    ("pending", Bool),
    ("queued", Bool),
    ("blocks", List(&MARKDOWN_BLOCK)),
    ("summary", Str),
]);

const QUEUED: Shape = Record(&[("id", Str), ("text", Str), ("interrupting", Bool)]);

const SESSION: Shape = Record(&[
    ("open", Bool),
    ("title", Str),
    ("host", Str),
    ("working", Bool),
    ("blocked", Bool),
    ("offline", Bool),
    ("status", Str),
    ("entries", List(&ENTRY)),
    ("earlier", Str),
    ("empty", Str),
    ("error", Str),
    ("canSend", Bool),
    ("note", Str),
    ("failed", Str),
    ("scrollRevision", Num),
    ("composerHeight", Num),
    ("queue", List(&QUEUED)),
    ("uploadUrl", Str),
    ("auth", Str),
    ("canTalk", Bool),
    ("voiceUrl", Str),
    ("voiceAuth", Str),
    ("draft", Str),
]);

/// `Compose`: the new-session screen.
const COMPOSE: Shape = Record(&[
    ("machine", Str),
    ("machineMenu", Str),
    ("account", Str),
    ("accountMenu", Str),
    ("accountSymbol", Str),
    ("accountTint", Str),
    ("model", Str),
    ("modelMenu", Str),
    ("effort", Str),
    ("effortMenu", Str),
    ("prompt", Str),
    ("launching", Bool),
    ("error", Str),
    ("canSend", Bool),
    ("composerHeight", Num),
    ("uploadUrl", Str),
    ("auth", Str),
    ("machineWidth", Num),
    ("accountWidth", Num),
]);

const PAIR: Shape = Record(&[("draft", Str), ("error", Str)]);

/// `View`: the whole picture.
pub const VIEW: Shape = Record(&[
    ("paired", Bool),
    ("home", HOME),
    ("session", SESSION),
    ("pair", PAIR),
    ("compose", COMPOSE),
]);

/// The value of `json` in `shape`, missing fields as their zero.
pub fn read(shape: &Shape, json: &serde_json::Value) -> Value {
    match shape {
        Str => match json {
            serde_json::Value::String(s) => Value::str(s),
            serde_json::Value::Number(n) => Value::str(&n.to_string()),
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
