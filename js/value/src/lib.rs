//! Values across the seam as JSON, directed by the plan's declared shapes
//! (LLP 1027 D2): records are objects keyed by their declared field names,
//! lists are arrays, `option` is `null` or the value, numbers are finite.
//! The plan's `sources` table says which shape each argument and each answer
//! has; the conversion never guesses.

use exact_plan::{Plan, TypeKind, TypesId, Value};
use std::rc::Rc;

mod decode;
pub mod json;
mod parse;
pub use decode::{from_json_text, reply_from_json_slice, reply_from_json_text, Reply};

/// A declared shape, owned: what one `TypesId` in a plan denotes.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `number`.
    Number,
    /// `bool`.
    Bool,
    /// `string`.
    String,
    /// `unit`.
    Unit,
    /// `option<T>`.
    Option(Box<Shape>),
    /// `list<T>`.
    List(Box<Shape>),
    /// A record: named fields in declared order.
    Record(Vec<(String, Shape)>),
}

const MAX_DEPTH: usize = 64;

impl Shape {
    /// The shape `ty` denotes in `plan`.
    pub fn from_plan(plan: &Plan, ty: TypesId) -> Result<Shape, String> {
        Shape::build(plan, ty, 0)
    }

    fn build(plan: &Plan, ty: TypesId, depth: usize) -> Result<Shape, String> {
        if depth > MAX_DEPTH {
            return Err("a shape nests deeper than 64".into());
        }
        let row = plan
            .types
            .get(ty.0 as usize)
            .ok_or_else(|| format!("no type row {}", ty.0))?;
        let elem = |plan: &Plan| -> Result<Shape, String> {
            let e = row
                .elem
                .ok_or("an option or list without an element type")?;
            Shape::build(plan, e, depth + 1)
        };
        Ok(match row.kind {
            TypeKind::Number => Shape::Number,
            TypeKind::Bool => Shape::Bool,
            TypeKind::String => Shape::String,
            TypeKind::Unit => Shape::Unit,
            TypeKind::Option => Shape::Option(Box::new(elem(plan)?)),
            TypeKind::List => Shape::List(Box::new(elem(plan)?)),
            TypeKind::Record => {
                let start = row.fields.start as usize;
                let end = start + row.fields.len as usize;
                let rows = plan
                    .fields
                    .get(start..end)
                    .ok_or("a record's fields run past the table")?;
                let mut fields = Vec::with_capacity(rows.len());
                for f in rows {
                    fields.push((
                        plan.str(f.name).to_string(),
                        Shape::build(plan, f.ty, depth + 1)?,
                    ));
                }
                Shape::Record(fields)
            }
        })
    }
}

/// A value as the module sees it, in serde_json's form (native callers).
pub fn to_json(v: &Value, shape: &Shape) -> Result<serde_json::Value, String> {
    json::encode(v, shape).map(Into::into)
}

/// A value as the runner needs it, from serde_json's form (native callers).
pub fn from_json(j: &serde_json::Value, shape: &Shape) -> Result<Value, String> {
    decode_tree(j, shape)
}

pub(crate) fn from_lean(j: &json::Json, shape: &Shape) -> Result<Value, String> {
    decode_tree(j, shape)
}

/// A JSON value tree, as `decode_tree` reads one: the rules are written
/// once, for the lean tree and for serde_json's.
pub(crate) trait Tree: Sized {
    fn kind(&self) -> Kind<'_, Self>;
    /// The member at `name` of an object.
    fn member(&self, name: &str) -> Option<&Self>;
    /// An object's member count.
    fn members(&self) -> usize;
    /// An object's first key (in key order) that `keep` refuses.
    fn first_key(&self, keep: &dyn Fn(&str) -> bool) -> Option<String>;
}

pub(crate) enum Kind<'a, T> {
    Null,
    Bool(bool),
    Number(Option<f64>),
    String(&'a str),
    Array(&'a [T]),
    Object,
}

impl Tree for json::Json {
    fn kind(&self) -> Kind<'_, Self> {
        match self {
            json::Json::Null => Kind::Null,
            json::Json::Bool(b) => Kind::Bool(*b),
            json::Json::Number(_) => Kind::Number(self.as_f64()),
            json::Json::String(s) => Kind::String(s),
            json::Json::Array(items) => Kind::Array(items),
            json::Json::Object(_) => Kind::Object,
        }
    }
    fn member(&self, name: &str) -> Option<&Self> {
        self.get(name)
    }
    fn members(&self) -> usize {
        self.as_object().map_or(0, json::Object::len)
    }
    fn first_key(&self, keep: &dyn Fn(&str) -> bool) -> Option<String> {
        self.as_object()?.keys().find(|k| !keep(k)).cloned()
    }
}

impl Tree for serde_json::Value {
    fn kind(&self) -> Kind<'_, Self> {
        match self {
            serde_json::Value::Null => Kind::Null,
            serde_json::Value::Bool(b) => Kind::Bool(*b),
            serde_json::Value::Number(n) => Kind::Number(n.as_f64()),
            serde_json::Value::String(s) => Kind::String(s),
            serde_json::Value::Array(items) => Kind::Array(items),
            serde_json::Value::Object(_) => Kind::Object,
        }
    }
    fn member(&self, name: &str) -> Option<&Self> {
        self.get(name)
    }
    fn members(&self) -> usize {
        self.as_object().map_or(0, serde_json::Map::len)
    }
    fn first_key(&self, keep: &dyn Fn(&str) -> bool) -> Option<String> {
        self.as_object()?.keys().find(|k| !keep(k)).cloned()
    }
}

fn decode_tree<T: Tree>(j: &T, shape: &Shape) -> Result<Value, String> {
    Ok(match (shape, j.kind()) {
        (Shape::Number, Kind::Number(n)) => {
            let n = n.ok_or("a number out of range")?;
            if !n.is_finite() {
                return Err("a non-finite number".into());
            }
            Value::Number(n)
        }
        (Shape::Bool, Kind::Bool(b)) => Value::Bool(b),
        (Shape::String, Kind::String(s)) => Value::str(s),
        (Shape::Unit, Kind::Null) => Value::Unit,
        (Shape::Option(_), Kind::Null) => Value::Option(None),
        (Shape::Option(inner), _) => Value::Option(Some(Rc::new(decode_tree(j, inner)?))),
        (Shape::List(inner), Kind::Array(items)) => Value::list(
            items
                .iter()
                .map(|i| decode_tree(i, inner))
                .collect::<Result<_, _>>()?,
        ),
        (Shape::Record(fields), Kind::Object) => {
            let mut values = Vec::with_capacity(fields.len());
            for (name, shape) in fields {
                let v = j
                    .member(name)
                    .ok_or_else(|| format!("field `{name}` is missing"))?;
                values.push(decode_tree(v, shape).map_err(|e| format!("field `{name}`: {e}"))?);
            }
            if j.members() != fields.len() {
                let extra = j
                    .first_key(&|k| fields.iter().any(|(n, _)| n == k))
                    .unwrap_or_default();
                return Err(format!("field `{extra}` is not in the shape"));
            }
            Value::record(values)
        }
        (shape, other) => {
            return Err(format!(
                "expected {}, got {}",
                describe(shape),
                match other {
                    Kind::Null => "null",
                    Kind::Bool(_) => "a bool",
                    Kind::Number(_) => "a number",
                    Kind::String(_) => "a string",
                    Kind::Array(_) => "an array",
                    Kind::Object => "an object",
                }
            ))
        }
    })
}

fn describe(shape: &Shape) -> String {
    match shape {
        Shape::Number => "a number".into(),
        Shape::Bool => "a bool".into(),
        Shape::String => "a string".into(),
        Shape::Unit => "null".into(),
        Shape::Option(inner) => format!("null or {}", describe(inner)),
        Shape::List(_) => "an array".into(),
        Shape::Record(_) => "an object".into(),
    }
}
