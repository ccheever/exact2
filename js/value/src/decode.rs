//! Decode the answer directly into runner values. Shape errors are values here,
//! not parse errors: parsing must finish before reporting them, and a later
//! duplicate object key can replace an earlier wrong-shaped value.
use crate::json::{self, Json, Object};
use crate::parse::{Error, Items, Members, Parser, Tree, Visit};
use crate::{describe, from_lean, Shape};
use exact_plan::Value;
use std::rc::Rc;

type Answer = Result<Value, String>;

/// Decode JSON text without constructing an intermediate JSON value tree.
/// JSON syntax and recursion limits are serde_json's (`crate::parse`).
pub fn from_json_text(text: &str, shape: &Shape) -> Result<Value, String> {
    let mut parser = Parser::new(text.as_bytes());
    let value = Node(Some(shape), unique_records(shape, text.len()))
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

impl json::Reply {
    pub(crate) fn decode(bytes: &[u8], shape: &Shape) -> Result<json::Reply, Error> {
        let mut parser = Parser::new(bytes);
        let reply = parser.value(Envelope(shape, unique_records(shape, bytes.len())))?;
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
#[derive(Clone, Copy)]
struct Node<'a>(Option<&'a Shape>, bool);

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
        let mut value = f(Node(shape, self.1))?;
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
        let node = Node(inner, self.1);
        loop {
            let item = if node.needs_tree() {
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
                    let node = Node(Some(&fields[i].1), self.1);
                    slots[i] = Some(node_value(members, node)?);
                }
                None => {
                    let name = key.to_owned();
                    let _ = node_value(members, Node(None, self.1))?;
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

struct Envelope<'a>(&'a Shape, bool);

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
                value = Some(node_value(members, Node(Some(self.0), self.1))?);
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
