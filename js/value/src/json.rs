//! JSON as the seam carries it, without serde (LLP 1047 §6).
//!
//! The value, its accessors and its compact text are serde_json 1.0's
//! (`serde_json::Value` with its default `Map`): an object's keys are sorted
//! and a repeated key keeps its last value; a number is a non-negative
//! integer, a negative integer or a float, as its text was. Floats are
//! written as their shortest round-trip decimal, which JavaScript reads back
//! as the same value.

use crate::Shape;
use exact_plan::Value;
use std::fmt;
use std::ops::{Index, IndexMut};

pub use crate::parse::{parse, Error};

/// An executor's envelope: its fields other than `value`, and `value`
/// decoded directly by the declared shape (a shape error is a value).
pub struct Reply {
    /// The envelope's fields other than the answer value.
    pub fields: Json,
    /// The shape-checked answer, or its shape error.
    pub value: Result<Value, String>,
}

/// An executor reply from its UTF-8 bytes.
pub fn reply(bytes: &[u8], shape: &Shape) -> Result<Reply, Error> {
    Reply::decode(bytes, shape)
}

/// A runner value as the module sees it, by its declared shape (LLP 1027
/// D2): records are objects keyed by field name, lists arrays, `option`
/// `null` or the value, numbers finite.
pub fn encode(v: &Value, shape: &Shape) -> Result<Json, String> {
    Ok(match (shape, v) {
        (Shape::Number, Value::Number(n)) => {
            if !n.is_finite() {
                return Err("a non-finite number".into());
            }
            Json::Number(Number::Float(*n))
        }
        (Shape::Bool, Value::Bool(b)) => Json::Bool(*b),
        (Shape::String, Value::Str(s)) => Json::String(s.to_string()),
        (Shape::Unit, Value::Unit) => Json::Null,
        (Shape::Option(_), Value::Option(None)) => Json::Null,
        (Shape::Option(inner), Value::Option(Some(v))) => encode(v, inner)?,
        (Shape::List(inner), Value::List(items)) => Json::Array(
            items
                .iter()
                .map(|i| encode(i, inner))
                .collect::<Result<_, _>>()?,
        ),
        (Shape::Record(fields), Value::Record(values)) => {
            if fields.len() != values.len() {
                return Err(format!(
                    "a record with {} fields where the shape has {}",
                    values.len(),
                    fields.len()
                ));
            }
            let mut object = Object::new();
            for ((name, shape), value) in fields.iter().zip(values.iter()) {
                object.insert(name.clone(), encode(value, shape)?);
            }
            Json::Object(object)
        }
        _ => return Err("a value outside its declared shape".into()),
    })
}

/// A module's JSON as the runner needs it, or why it is not that shape.
pub fn decode(j: &Json, shape: &Shape) -> Result<Value, String> {
    crate::from_lean(j, shape)
}

/// A JSON value.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Json {
    /// `null`.
    #[default]
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number.
    Number(Number),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object.
    Object(Object),
}

/// A number, as serde_json keeps one: its text was an integer that fits a
/// `u64` or an `i64`, or it is a float.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    /// A non-negative integer.
    PosInt(u64),
    /// A negative integer.
    NegInt(i64),
    /// Anything else: a fraction, an exponent, or an integer past 64 bits.
    Float(f64),
}

/// An object: members sorted by key, each key once.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Object(Vec<(String, Json)>);

static NULL: Json = Json::Null;

impl Object {
    /// No members.
    pub const fn new() -> Self {
        Object(Vec::new())
    }

    fn find(&self, key: &str) -> Result<usize, usize> {
        self.0.binary_search_by(|(k, _)| k.as_str().cmp(key))
    }

    /// The value at `key`.
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.find(key).ok().map(|i| &self.0[i].1)
    }

    /// Set `key` to `value`; the value it replaced, if any.
    pub fn insert(&mut self, key: String, value: Json) -> Option<Json> {
        match self.find(&key) {
            Ok(i) => Some(std::mem::replace(&mut self.0[i].1, value)),
            Err(i) => {
                self.0.insert(i, (key, value));
                None
            }
        }
    }

    /// Remove `key`; its value, if it had one.
    pub fn remove(&mut self, key: &str) -> Option<Json> {
        self.find(key).ok().map(|i| self.0.remove(i).1)
    }

    /// How many members.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Members in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Json)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    /// Keys in order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(k, _)| k)
    }
}

impl FromIterator<(String, Json)> for Object {
    /// Of equal keys, the last value stays.
    fn from_iter<I: IntoIterator<Item = (String, Json)>>(iter: I) -> Self {
        let mut object = Object::new();
        for (key, value) in iter {
            object.insert(key, value);
        }
        object
    }
}

/// An object from `(key, value)` pairs, in any order.
pub fn object<'k>(members: impl IntoIterator<Item = (&'k str, Json)>) -> Json {
    Json::Object(
        members
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

impl Json {
    /// The value at `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(object) => object.get(key),
            _ => None,
        }
    }

    /// A non-negative integer that fits a `u64`.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Number(Number::PosInt(n)) => Some(*n),
            _ => None,
        }
    }

    /// An integer that fits an `i64`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Number(Number::PosInt(n)) => i64::try_from(*n).ok(),
            Json::Number(Number::NegInt(n)) => Some(*n),
            _ => None,
        }
    }

    /// Any number, as an `f64`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(Number::PosInt(n)) => Some(*n as f64),
            Json::Number(Number::NegInt(n)) => Some(*n as f64),
            Json::Number(Number::Float(n)) => Some(*n),
            _ => None,
        }
    }

    /// A string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// A bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// An array.
    pub fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// An object.
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Json::Object(object) => Some(object),
            _ => None,
        }
    }

    /// Whether this is `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    /// The compact text, as `JSON.stringify` would read it.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// Append the compact text to `out`.
    pub fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(n) => n.write(out),
            Json::String(s) => write_str(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(object) => {
                out.push('{');
                for (i, (key, value)) in object.0.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl Number {
    fn write(&self, out: &mut String) {
        match *self {
            Number::PosInt(n) => write_u64(n, out),
            Number::NegInt(n) => {
                out.push('-');
                write_u64(n.unsigned_abs(), out);
            }
            // serde_json writes a non-finite float as `null`, as
            // JSON.stringify does; none is ever made here.
            Number::Float(n) if !n.is_finite() => out.push_str("null"),
            Number::Float(n) => {
                use std::fmt::Write as _;
                let start = out.len();
                let _ = write!(out, "{n}");
                // A float stays a float read back, as serde_json writes one.
                if !out[start..].contains(['.', 'e']) {
                    out.push_str(".0");
                }
            }
        }
    }
}

fn write_u64(mut n: u64, out: &mut String) {
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    loop {
        at -= 1;
        digits[at] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    // ASCII digits.
    out.push_str(std::str::from_utf8(&digits[at..]).unwrap_or_default());
}

/// A string in quotes, escaped as serde_json and JSON.stringify escape it:
/// `"` and `\`, the five short control escapes, `\u00xx` for the rest below
/// 0x20, and everything else as itself.
fn write_str(s: &str, out: &mut String) {
    out.push('"');
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        let escape = match b {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x08 => "\\b",
            0x0c => "\\f",
            0..=0x1f => "",
            _ => continue,
        };
        out.push_str(&s[start..i]);
        if escape.is_empty() {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            out.push_str("\\u00");
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xf) as usize] as char);
        } else {
            out.push_str(escape);
        }
        start = i + 1;
    }
    out.push_str(&s[start..]);
    out.push('"');
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

impl Index<&str> for Json {
    type Output = Json;
    /// A member of an object; `null` for a missing key or a non-object.
    fn index(&self, key: &str) -> &Json {
        self.get(key).unwrap_or(&NULL)
    }
}

impl Index<usize> for Json {
    type Output = Json;
    /// An element of an array; `null` past its end or for a non-array.
    fn index(&self, i: usize) -> &Json {
        match self {
            Json::Array(items) => items.get(i).unwrap_or(&NULL),
            _ => &NULL,
        }
    }
}

impl IndexMut<&str> for Json {
    /// A member of an object, inserted as `null` when missing; `null`
    /// becomes an empty object first. Anything else has no members.
    fn index_mut(&mut self, key: &str) -> &mut Json {
        if self.is_null() {
            *self = Json::Object(Object::new());
        }
        let Json::Object(object) = self else {
            panic!("cannot set a member of a JSON value that is not an object");
        };
        let i = match object.find(key) {
            Ok(i) => i,
            Err(i) => {
                object.0.insert(i, (key.to_owned(), Json::Null));
                i
            }
        };
        &mut object.0[i].1
    }
}

impl PartialEq<bool> for Json {
    fn eq(&self, other: &bool) -> bool {
        self.as_bool() == Some(*other)
    }
}

impl PartialEq<i32> for Json {
    fn eq(&self, other: &i32) -> bool {
        self.as_i64() == Some(i64::from(*other))
    }
}

impl PartialEq<u64> for Json {
    fn eq(&self, other: &u64) -> bool {
        self.as_u64() == Some(*other)
    }
}

impl PartialEq<str> for Json {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == Some(other)
    }
}

impl PartialEq<&str> for Json {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == Some(*other)
    }
}

impl PartialEq<String> for Json {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == Some(other.as_str())
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Self {
        Json::Bool(b)
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Self {
        Json::String(s.to_owned())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Self {
        Json::String(s)
    }
}

impl From<u64> for Json {
    fn from(n: u64) -> Self {
        Json::Number(Number::PosInt(n))
    }
}

impl From<u32> for Json {
    fn from(n: u32) -> Self {
        Json::Number(Number::PosInt(u64::from(n)))
    }
}

impl From<u16> for Json {
    fn from(n: u16) -> Self {
        Json::Number(Number::PosInt(u64::from(n)))
    }
}

impl From<i64> for Json {
    fn from(n: i64) -> Self {
        Json::Number(match u64::try_from(n) {
            Ok(n) => Number::PosInt(n),
            Err(_) => Number::NegInt(n),
        })
    }
}

impl From<f64> for Json {
    /// A float, as serde_json's `Number::from_f64` makes one.
    fn from(n: f64) -> Self {
        Json::Number(Number::Float(n))
    }
}

impl From<Vec<Json>> for Json {
    fn from(items: Vec<Json>) -> Self {
        Json::Array(items)
    }
}

impl From<Object> for Json {
    fn from(object: Object) -> Self {
        Json::Object(object)
    }
}

/// The serde_json form, for native callers that hold one.
impl From<Json> for serde_json::Value {
    fn from(json: Json) -> Self {
        match json {
            Json::Null => serde_json::Value::Null,
            Json::Bool(b) => serde_json::Value::Bool(b),
            Json::Number(Number::PosInt(n)) => serde_json::Value::Number(n.into()),
            Json::Number(Number::NegInt(n)) => serde_json::Value::Number(n.into()),
            Json::Number(Number::Float(n)) => serde_json::Number::from_f64(n)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            Json::String(s) => serde_json::Value::String(s),
            Json::Array(items) => {
                serde_json::Value::Array(items.into_iter().map(serde_json::Value::from).collect())
            }
            Json::Object(object) => serde_json::Value::Object(
                object
                    .0
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
        }
    }
}

/// From serde_json's form, for native callers that hold one.
impl From<&serde_json::Value> for Json {
    fn from(json: &serde_json::Value) -> Self {
        match json {
            serde_json::Value::Null => Json::Null,
            serde_json::Value::Bool(b) => Json::Bool(*b),
            serde_json::Value::Number(n) => Json::Number(if let Some(n) = n.as_u64() {
                Number::PosInt(n)
            } else if let Some(n) = n.as_i64() {
                Number::NegInt(n)
            } else {
                Number::Float(n.as_f64().unwrap_or(f64::NAN))
            }),
            serde_json::Value::String(s) => Json::String(s.clone()),
            serde_json::Value::Array(items) => Json::Array(items.iter().map(Json::from).collect()),
            serde_json::Value::Object(map) => Json::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), Json::from(v)))
                    .collect(),
            ),
        }
    }
}
