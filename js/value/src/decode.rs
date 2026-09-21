//! Decode the answer directly into runner values. Shape errors are values here,
//! not serde errors: parsing must finish before reporting them, and a later
//! duplicate object key can replace an earlier wrong-shaped value.
use crate::{describe, from_json, Shape};
use exact_plan::Value;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::{Map, Value as Json};
use std::{fmt, rc::Rc};

type Answer = Result<Value, String>;

/// Decode JSON text without constructing an intermediate JSON value tree.
/// JSON syntax, number parsing and recursion limits are serde_json's own.
pub fn from_json_text(text: &str, shape: &Shape) -> Result<Value, String> {
    let mut de = serde_json::Deserializer::from_str(text);
    let value = Node(Some(shape), unique_records(shape, text.len()))
        .deserialize(&mut de)
        .map_err(|e| e.to_string())?;
    de.end().map_err(|e| e.to_string())?;
    value
}

/// An executor's envelope, with its `value` decoded separately. Metadata keeps
/// its existing JSON representation; shape errors do not hide errors or effects
/// carried by the envelope. The metadata object omits `value`.
pub struct Reply {
    /// The envelope's fields other than the answer value.
    pub fields: Json,
    /// The shape-checked answer, or its shape error (used only for tag 0).
    pub value: Answer,
}

/// Decode a native executor reply while retaining serde_json syntax errors.
pub fn reply_from_json_text(text: &str, shape: &Shape) -> Result<Reply, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(text);
    let reply = Envelope(shape, unique_records(shape, text.len())).deserialize(&mut de)?;
    de.end()?;
    Ok(reply)
}

/// Decode a browser executor reply directly from its UTF-8 output buffer.
pub fn reply_from_json_slice(bytes: &[u8], shape: &Shape) -> Result<Reply, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let reply = Envelope(shape, unique_records(shape, bytes.len())).deserialize(&mut de)?;
    de.end()?;
    Ok(reply)
}

// A shape is shared by every row in an answer. This optional preflight uses
// at most one node/name comparison per input byte, capped at 4,096. Tiny answers
// must not scan huge unused shapes. Exhaustion retains the original decoder.
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
// IgnoredAny skips those checks, so it cannot preserve the old JSON parser here.
struct Node<'a>(Option<&'a Shape>, bool);

impl<'de> DeserializeSeed<'de> for Node<'_> {
    type Value = Answer;

    fn deserialize<D: Deserializer<'de>>(self, de: D) -> Result<Answer, D::Error> {
        let mut shape = self.0;
        while let Some(Shape::Option(inner)) = shape {
            shape = Some(inner);
        }
        // Hand-built shapes may repeat a field name. Preserve the old decoder's
        // behavior for those unusual shapes; plan record fields are distinct.
        if let Some(Shape::Record(fields)) = shape {
            if !self.1
                && fields
                    .iter()
                    .enumerate()
                    .any(|(i, (name, _))| fields[..i].iter().any(|(other, _)| name == other))
            {
                return Json::deserialize(de).map(|j| from_json(&j, self.0.unwrap()));
            }
        }
        de.deserialize_any(self)
    }
}

impl Node<'_> {
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

    fn number(self, n: f64) -> Answer {
        self.nonnull(|node| match node.0 {
            Some(Shape::Number) => Ok(Value::Number(n)),
            _ => node.mismatch("a number"),
        })
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = Answer;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Answer, E> {
        Ok(match self.0 {
            Some(Shape::Unit) | None => Ok(Value::Unit),
            Some(Shape::Option(_)) => Ok(Value::Option(None)),
            _ => self.mismatch("null"),
        })
    }
    fn visit_bool<E>(self, value: bool) -> Result<Answer, E> {
        Ok(self.nonnull(|node| match node.0 {
            Some(Shape::Bool) => Ok(Value::Bool(value)),
            _ => node.mismatch("a bool"),
        }))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Answer, E> {
        Ok(self.number(value as f64))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Answer, E> {
        Ok(self.number(value as f64))
    }
    fn visit_f64<E>(self, value: f64) -> Result<Answer, E> {
        Ok(self.number(value))
    }
    fn visit_str<E>(self, value: &str) -> Result<Answer, E> {
        Ok(self.nonnull(|node| match node.0 {
            Some(Shape::String) => Ok(Value::str(value)),
            _ => node.mismatch("a string"),
        }))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Answer, A::Error> {
        // Keep syntax errors outside the shape-result layer.
        let mut shape = self.0;
        while let Some(Shape::Option(inner)) = shape {
            shape = Some(inner);
        }
        let inner = match shape {
            Some(Shape::List(inner)) => Some(&**inner),
            _ => None,
        };
        let mut items = Vec::new();
        let mut error = None;
        while let Some(item) = seq.next_element_seed(Node(inner, self.1))? {
            if inner.is_some() && error.is_none() {
                match item {
                    Ok(value) => items.push(value),
                    Err(e) => {
                        items.clear();
                        error = Some(e);
                    }
                }
            }
        }
        Ok(self.nonnull(|node| match node.0 {
            Some(Shape::List(_)) => error.map_or_else(|| Ok(Value::list(items)), Err),
            _ => node.mismatch("an array"),
        }))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Answer, A::Error> {
        let mut shape = self.0;
        while let Some(Shape::Option(inner)) = shape {
            shape = Some(inner);
        }
        let fields = match shape {
            Some(Shape::Record(fields)) => &fields[..],
            _ => &[],
        };
        let mut slots: Vec<Option<Answer>> = (0..fields.len()).map(|_| None).collect();
        let mut extra: Option<String> = None;
        while let Some(key) = map.next_key_seed(Key(fields))? {
            match key {
                Field::Known(i) => {
                    slots[i] = Some(map.next_value_seed(Node(Some(&fields[i].1), self.1))?);
                }
                Field::Extra(name) => {
                    let _ = map.next_value_seed(Node(None, self.1))?;
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

enum Field {
    Known(usize),
    Extra(String),
}
struct Key<'a>(&'a [(String, Shape)]);
impl<'de> DeserializeSeed<'de> for Key<'_> {
    type Value = Field;
    fn deserialize<D: Deserializer<'de>>(self, de: D) -> Result<Field, D::Error> {
        de.deserialize_str(self)
    }
}
impl<'de> Visitor<'de> for Key<'_> {
    type Value = Field;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an object key")
    }
    fn visit_str<E>(self, key: &str) -> Result<Field, E> {
        Ok(self
            .0
            .iter()
            .position(|(name, _)| name == key)
            .map_or_else(|| Field::Extra(key.into()), Field::Known))
    }
}

struct Envelope<'a>(&'a Shape, bool);
impl<'de> DeserializeSeed<'de> for Envelope<'_> {
    type Value = Reply;
    fn deserialize<D: Deserializer<'de>>(self, de: D) -> Result<Reply, D::Error> {
        de.deserialize_any(self)
    }
}
impl Envelope<'_> {
    fn other(self, fields: Json) -> Reply {
        Reply {
            fields,
            value: from_json(&Json::Null, self.0),
        }
    }
}
impl<'de> Visitor<'de> for Envelope<'_> {
    type Value = Reply;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON reply")
    }
    fn visit_unit<E>(self) -> Result<Reply, E> {
        Ok(self.other(Json::Null))
    }
    fn visit_bool<E>(self, v: bool) -> Result<Reply, E> {
        Ok(self.other(v.into()))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Reply, E> {
        Ok(self.other(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Reply, E> {
        Ok(self.other(v.into()))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Reply, E> {
        Ok(self.other(v.into()))
    }
    fn visit_str<E>(self, v: &str) -> Result<Reply, E> {
        Ok(self.other(v.into()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Reply, A::Error> {
        let mut items = Vec::new();
        while let Some(value) = seq.next_element()? {
            items.push(value);
        }
        Ok(self.other(Json::Array(items)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Reply, A::Error> {
        let mut fields = Map::new();
        let mut value = from_json(&Json::Null, self.0);
        while let Some(key) = map.next_key::<String>()? {
            if key == "value" {
                value = map.next_value_seed(Node(Some(self.0), self.1))?;
            } else {
                fields.insert(key, map.next_value()?);
            }
        }
        Ok(Reply {
            fields: Json::Object(fields),
            value,
        })
    }
}
