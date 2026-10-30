//! Decode the answer directly into runner values. Shape errors are values here,
//! not parse errors: parsing must finish before reporting them, and a later
//! duplicate object key can replace an earlier wrong-shaped value.
use crate::json::{self, Json, Object};
use crate::parse::{Error, Items, Members, Parser, Tree, Visit};
use crate::{describe, from_lean, Shape};
use exact_plan::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Answer = Result<Value, String>;

/// Decode JSON text without constructing an intermediate JSON value tree.
/// JSON syntax and recursion limits are serde_json's (`crate::parse`).
pub fn from_json_text(text: &str, shape: &Shape) -> Result<Value, String> {
    let mut parser = Parser::new(text.as_bytes());
    let value = Node(Some(shape), unique_records(shape, text.len()), None)
        .decode(&mut parser)
        .map_err(|e| e.to_string())?;
    parser.end().map_err(|e| e.to_string())?;
    value
}

/// An executor's envelope, with its `value` decoded separately. Metadata keeps
/// its JSON representation; shape errors do not hide errors or effects
/// carried by the envelope. The metadata object omits `value`.
pub struct Reply {
    /// The envelope's fields other than the answer value.
    pub fields: serde_json::Value,
    /// The shape-checked answer, or its shape error (used only for tag 0).
    pub value: Answer,
}

/// Decode a native executor reply; syntax errors are serde_json's.
pub fn reply_from_json_text(text: &str, shape: &Shape) -> Result<Reply, Error> {
    reply_from_json_slice(text.as_bytes(), shape)
}

/// Decode an executor reply from UTF-8 bytes.
pub fn reply_from_json_slice(bytes: &[u8], shape: &Shape) -> Result<Reply, Error> {
    let json::Reply { fields, value } = json::reply(bytes, shape)?;
    Ok(Reply {
        fields: fields.into(),
        value: value.unwrap_or_else(|| from_lean(&Json::Null, shape)),
    })
}

/// [`reply_from_json_text`], reusing the values of list elements whose
/// bytes are the ones `rows` kept from this source's last reply: those are
/// the records a module answers again unchanged (a transcript's window
/// holds its last page's rows). The same bytes under the same shape decode
/// to the same value, so the answer is exactly a full decode's.
pub fn reply_reusing(text: &str, shape: &Shape, rows: &RefCell<Rows>) -> Result<Reply, Error> {
    let bytes = text.as_bytes();
    let mut parser = Parser::new(bytes);
    let decoded = parser
        .value(Envelope(
            shape,
            unique_records(shape, bytes.len()),
            Some(rows),
        ))
        .and_then(|reply| parser.end().map(|()| reply));
    // Only an envelope with a value replaces what was kept: an answer's other envelopes
    // (a call begun, a fetch asked for) pass between its replies and keep nothing.
    match &decoded {
        Ok(json::Reply { value: None, .. }) => rows.borrow_mut().next.clear(),
        _ => rows.borrow_mut().settle(decoded.is_ok()),
    }
    let json::Reply { fields, value } = decoded?;
    Ok(Reply {
        fields: fields.into(),
        value: value.unwrap_or_else(|| from_lean(&Json::Null, shape)),
    })
}

/// A source's list elements of a record shape from its last reply: each
/// one's bytes and decoded value, by a hash of its first bytes and its shape.
/// What one reply's value decodes replaces what the last one kept, so it
/// never holds more than the elements decoded from one reply (and while a
/// reply decodes, the last one's too). An element under 64 bytes isn't kept.
#[derive(Default)]
pub struct Rows {
    held: Kept,
    next: Kept,
}

/// Elements by their shape's address and their first bytes' hash: each one's bytes and value.
type Kept = HashMap<(usize, u64), Vec<(Box<[u8]>, Value)>>;

/// Elements kept under one key at most: elements alike in their first bytes
/// are looked through in turn, so a shape whose elements all begin alike
/// keeps a few rather than paying a long search per element.
const BUCKET: usize = 8;

impl Rows {
    /// The elements kept, for tests and memory reports.
    pub fn len(&self) -> usize {
        self.held.values().map(Vec::len).sum()
    }

    /// No element kept.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    fn key(shape: &Shape, bytes: &[u8]) -> (usize, u64) {
        // FNV-1a over the first 64 bytes: a record's leading field (an id) tells most apart.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for &b in &bytes[..bytes.len().min(64)] {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        #[cfg(test)]
        if COLLIDE.with(|c| c.get()) {
            hash = 0; // every element under one key: only the byte compare tells them apart
        }
        (shape as *const Shape as usize, hash)
    }

    /// The value of a kept element the input `rest` begins with, and its
    /// length. A kept element is a whole JSON value, so input that begins with
    /// its bytes holds exactly that value there.
    fn get(&mut self, shape: &Shape, rest: &[u8]) -> Option<(Value, usize)> {
        let key = Self::key(shape, rest);
        let bucket = self.held.get_mut(&key)?;
        let i = bucket.iter().position(|(kept, _)| rest.starts_with(kept))?;
        // Moved to this reply's, not copied: an element answered twice in one reply decodes again.
        let (kept, value) = bucket.swap_remove(i);
        let len = kept.len();
        self.put(key, kept, value.clone());
        Some((value, len))
    }

    fn keep(&mut self, shape: &Shape, bytes: &[u8], value: &Value) {
        // A lookup hashes the input's next 64 bytes, so an element shorter than that is never found:
        // it isn't kept (decoding one costs about what looking it up would).
        if bytes.len() >= 64 {
            self.put(Self::key(shape, bytes), bytes.into(), value.clone());
        }
    }

    fn put(&mut self, key: (usize, u64), bytes: Box<[u8]>, value: Value) {
        let bucket = self.next.entry(key).or_default();
        if bucket.len() < BUCKET {
            bucket.push((bytes, value));
        }
    }

    /// The reply decoded: what it kept replaces the last reply's (a failed
    /// decode keeps nothing of its own and drops the last reply's too).
    fn settle(&mut self, ok: bool) {
        self.held = std::mem::take(&mut self.next);
        if !ok {
            self.held.clear();
        }
    }
}

#[cfg(test)]
thread_local!(static COLLIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });

impl json::Reply {
    pub(crate) fn decode(bytes: &[u8], shape: &Shape) -> Result<json::Reply, Error> {
        let mut parser = Parser::new(bytes);
        let reply = parser.value(Envelope(shape, unique_records(shape, bytes.len()), None))?;
        parser.end()?;
        Ok(reply)
    }
}

// A shape is shared by every row in an answer. This optional preflight uses
// at most one node/name comparison per input byte, capped at 4,096. Tiny answers
// must not scan huge unused shapes. Exhaustion retains the tree decoder.
fn unique_records(shape: &Shape, bytes: usize) -> bool {
    fn visit(shape: &Shape, depth: usize, work: &mut usize) -> bool {
        if depth > crate::MAX_DEPTH || *work == 0 {
            return false;
        }
        *work -= 1;
        match shape {
            Shape::Record(fields) => fields.iter().enumerate().all(|(i, (name, inner))| {
                let Some(remaining) = work.checked_sub(i) else {
                    return false;
                };
                *work = remaining;
                !fields[..i].iter().any(|(other, _)| name == other) && visit(inner, depth + 1, work)
            }),
            Shape::Option(inner) | Shape::List(inner) => visit(inner, depth + 1, work),
            _ => true,
        }
    }
    visit(shape, 0, &mut bytes.min(4096))
}

// None validates and discards a subtree, including numeric overflow and depth.
// The third field is the source's kept rows, when its reply reuses them.
#[derive(Clone, Copy)]
struct Node<'a>(Option<&'a Shape>, bool, Option<&'a RefCell<Rows>>);

impl Node<'_> {
    /// Hand-built shapes may repeat a field name: those decode through the
    /// value tree, as they always have; plan record fields are distinct.
    fn needs_tree(&self) -> bool {
        match self.inner() {
            Some(Shape::Record(fields)) => {
                !self.1
                    && fields
                        .iter()
                        .enumerate()
                        .any(|(i, (name, _))| fields[..i].iter().any(|(other, _)| name == other))
            }
            _ => false,
        }
    }

    fn decode(self, parser: &mut Parser<'_>) -> Result<Answer, Error> {
        if self.needs_tree() {
            let tree = parser.value(Tree)?;
            return Ok(from_lean(&tree, self.0.unwrap_or(&Shape::Unit)));
        }
        parser.value(self)
    }

    fn nonnull(self, f: impl FnOnce(Node<'_>) -> Answer) -> Answer {
        let mut shape = self.0;
        let mut options = 0;
        while let Some(Shape::Option(inner)) = shape {
            shape = Some(inner);
            options += 1;
        }
        let mut value = f(Node(shape, self.1, self.2))?;
        for _ in 0..options {
            value = Value::Option(Some(Rc::new(value)));
        }
        Ok(value)
    }

    fn mismatch(&self, actual: &str) -> Answer {
        match self.0 {
            Some(shape) => Err(format!("expected {}, got {actual}", describe(shape))),
            None => Ok(Value::Unit),
        }
    }

    fn inner(&self) -> Option<&Shape> {
        let mut shape = self.0;
        while let Some(Shape::Option(inner)) = shape {
            shape = Some(inner);
        }
        shape
    }
}

impl Visit for Node<'_> {
    type Out = Answer;

    fn null(self) -> Answer {
        match self.0 {
            Some(Shape::Unit) | None => Ok(Value::Unit),
            Some(Shape::Option(_)) => Ok(Value::Option(None)),
            _ => self.mismatch("null"),
        }
    }
    fn bool(self, value: bool) -> Answer {
        self.nonnull(|node| match node.0 {
            Some(Shape::Bool) => Ok(Value::Bool(value)),
            _ => node.mismatch("a bool"),
        })
    }
    fn number(self, value: json::Number) -> Answer {
        let n = match value {
            json::Number::PosInt(n) => n as f64,
            json::Number::NegInt(n) => n as f64,
            json::Number::Float(n) => n,
        };
        self.nonnull(|node| match node.0 {
            Some(Shape::Number) => Ok(Value::Number(n)),
            _ => node.mismatch("a number"),
        })
    }
    fn string(self, value: &str) -> Answer {
        self.nonnull(|node| match node.0 {
            Some(Shape::String) => Ok(Value::str(value)),
            _ => node.mismatch("a string"),
        })
    }
    fn array(self, items: &mut Items<'_, '_>) -> Result<Answer, Error> {
        // Keep syntax errors outside the shape-result layer.
        let inner = match self.inner() {
            Some(Shape::List(inner)) => Some(&**inner),
            _ => None,
        };
        let mut values = Vec::new();
        let mut error = None;
        // An element of a record shape is looked up by its bytes; one decoded
        // is kept for the next reply, and decoded without the rows, so what it
        // holds isn't kept twice.
        let node = Node(inner, self.1, None);
        let rows = match (self.2, inner) {
            (Some(rows), Some(shape @ Shape::Record(_))) if !node.needs_tree() => {
                Some((rows, shape))
            }
            _ => None,
        };
        loop {
            let item = if let Some((rows, shape)) = rows {
                let Some(start) = items.next_start()? else {
                    break;
                };
                let hit = rows.borrow_mut().get(shape, items.rest(start));
                match hit {
                    Some((value, len)) => {
                        items.skip_to(start + len);
                        Ok(value)
                    }
                    None => {
                        let item = items.visit_at(start, node)?;
                        if let Ok(value) = &item {
                            let end = items.at();
                            rows.borrow_mut()
                                .keep(shape, &items.rest(start)[..end - start], value);
                        }
                        item
                    }
                }
            } else if node.needs_tree() {
                match items.next(Tree)? {
                    Some(tree) => from_lean(&tree, node.0.unwrap_or(&Shape::Unit)),
                    None => break,
                }
            } else {
                match items.next(node)? {
                    Some(item) => item,
                    None => break,
                }
            };
            if inner.is_some() && error.is_none() {
                match item {
                    Ok(value) => values.push(value),
                    Err(e) => {
                        values.clear();
                        error = Some(e);
                    }
                }
            }
        }
        Ok(self.nonnull(|node| match node.0 {
            Some(Shape::List(_)) => error.map_or_else(|| Ok(Value::list(values)), Err),
            _ => node.mismatch("an array"),
        }))
    }
    fn object(self, members: &mut Members<'_, '_>) -> Result<Answer, Error> {
        let fields = match self.inner() {
            Some(Shape::Record(fields)) => &fields[..],
            _ => &[],
        };
        let mut slots: Vec<Option<Answer>> = (0..fields.len()).map(|_| None).collect();
        let mut extra: Option<String> = None;
        while let Some(key) = members.key()? {
            match fields.iter().position(|(name, _)| name == key) {
                Some(i) => {
                    let node = Node(Some(&fields[i].1), self.1, self.2);
                    slots[i] = Some(node_value(members, node)?);
                }
                None => {
                    let name = key.to_owned();
                    let _ = node_value(members, Node(None, self.1, None))?;
                    if extra.as_ref().is_none_or(|old| name < *old) {
                        extra = Some(name);
                    }
                }
            }
        }
        Ok(self.nonnull(|node| {
            if !matches!(node.0, Some(Shape::Record(_))) {
                return node.mismatch("an object");
            }
            let mut values = Vec::with_capacity(fields.len());
            for (slot, (name, _)) in slots.into_iter().zip(fields) {
                let value = slot.ok_or_else(|| format!("field `{name}` is missing"))?;
                values.push(value.map_err(|e| format!("field `{name}`: {e}"))?);
            }
            if let Some(name) = extra {
                return Err(format!("field `{name}` is not in the shape"));
            }
            Ok(Value::record(values))
        }))
    }
}

/// A member's value through `Node::decode`'s rules (the tree fallback for
/// repeated field names applies to members as it does at the top).
fn node_value(members: &mut Members<'_, '_>, node: Node<'_>) -> Result<Answer, Error> {
    if node.needs_tree() {
        let tree = members.value(Tree)?;
        return Ok(from_lean(&tree, node.0.unwrap_or(&Shape::Unit)));
    }
    members.value(node)
}

struct Envelope<'a>(&'a Shape, bool, Option<&'a RefCell<Rows>>);

impl Envelope<'_> {
    fn other(self, fields: Json) -> json::Reply {
        json::Reply {
            fields,
            value: None,
        }
    }
}

impl Visit for Envelope<'_> {
    type Out = json::Reply;
    fn null(self) -> json::Reply {
        self.other(Json::Null)
    }
    fn bool(self, value: bool) -> json::Reply {
        self.other(Json::Bool(value))
    }
    fn number(self, value: json::Number) -> json::Reply {
        self.other(Json::Number(value))
    }
    fn string(self, value: &str) -> json::Reply {
        self.other(Json::String(value.to_owned()))
    }
    fn array(self, items: &mut Items<'_, '_>) -> Result<json::Reply, Error> {
        let mut values = Vec::new();
        while let Some(value) = items.next(Tree)? {
            values.push(value);
        }
        Ok(self.other(Json::Array(values)))
    }
    fn object(self, members: &mut Members<'_, '_>) -> Result<json::Reply, Error> {
        let mut fields = Object::new();
        let mut value = None;
        while let Some(key) = members.key()? {
            if key == "value" {
                value = Some(node_value(members, Node(Some(self.0), self.1, self.2))?);
            } else {
                let key = key.to_owned();
                let member = members.value(Tree)?;
                fields.insert(key, member);
            }
        }
        Ok(json::Reply {
            fields: Json::Object(fields),
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hash_collision_is_caught_by_comparing_the_bytes() {
        let row = Shape::Record(vec![
            ("id".into(), Shape::String),
            ("n".into(), Shape::Number),
        ]);
        let shape = Shape::List(Box::new(row));
        let rows = RefCell::new(Rows::default());
        let text = |n: u32| {
            format!(
                r#"{{"tag":0,"value":[{}]}}"#,
                (0..20)
                    .map(|i| format!(
                        r#"{{"id":"r{i} a row long enough to be kept, well past the 64 bytes","n":{}}}"#,
                        i + n
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        COLLIDE.with(|c| c.set(true));
        reply_reusing(&text(0), &shape, &rows).unwrap();
        assert_eq!(rows.borrow().len(), 8, "one bucket, its first 8");
        // Every row under one key, so the bucket keeps the first 8: each row of the next reply finds
        // the bucket by key, and the byte compare tells its own entry from the others.
        for n in [0, 100, 0] {
            let cached = reply_reusing(&text(n), &shape, &rows).unwrap();
            let full = reply_from_json_text(&text(n), &shape).unwrap();
            assert_eq!(
                cached.value.map(|v| v.to_bytes()),
                full.value.map(|v| v.to_bytes())
            );
        }
        COLLIDE.with(|c| c.set(false));
    }
}
