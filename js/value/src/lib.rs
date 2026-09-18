//! Values across the seam as JSON, directed by the plan's declared shapes
//! (LLP 1027 D2): records are objects keyed by their declared field names,
//! lists are arrays, `option` is `null` or the value, numbers are finite.
//! The plan's `sources` table says which shape each argument and each answer
//! has; the conversion never guesses.

use exact_plan::{Plan, TypeKind, TypesId, Value};
use serde_json::{Map, Value as Json};
use std::rc::Rc;

mod decode;
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

/// A value as the module sees it.
pub fn to_json(v: &Value, shape: &Shape) -> Result<Json, String> {
    Ok(match (shape, v) {
        (Shape::Number, Value::Number(n)) => {
            if !n.is_finite() {
                return Err("a non-finite number".into());
            }
            serde_json::Number::from_f64(*n)
                .map(Json::Number)
                .ok_or("a number JSON cannot carry")?
        }
        (Shape::Bool, Value::Bool(b)) => Json::Bool(*b),
        (Shape::String, Value::Str(s)) => Json::String(s.to_string()),
        (Shape::Unit, Value::Unit) => Json::Null,
        (Shape::Option(_), Value::Option(None)) => Json::Null,
        (Shape::Option(inner), Value::Option(Some(v))) => to_json(v, inner)?,
        (Shape::List(inner), Value::List(items)) => Json::Array(
            items
                .iter()
                .map(|i| to_json(i, inner))
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
            let mut map = Map::with_capacity(fields.len());
            for ((name, shape), value) in fields.iter().zip(values.iter()) {
                map.insert(name.clone(), to_json(value, shape)?);
            }
            Json::Object(map)
        }
        _ => return Err("a value outside its declared shape".into()),
    })
}

/// A value as the runner needs it, or why the module's answer is not one.
pub fn from_json(j: &Json, shape: &Shape) -> Result<Value, String> {
    Ok(match (shape, j) {
        (Shape::Number, Json::Number(n)) => {
            let n = n.as_f64().ok_or("a number out of range")?;
            if !n.is_finite() {
                return Err("a non-finite number".into());
            }
            Value::Number(n)
        }
        (Shape::Bool, Json::Bool(b)) => Value::Bool(*b),
        (Shape::String, Json::String(s)) => Value::str(s),
        (Shape::Unit, Json::Null) => Value::Unit,
        (Shape::Option(_), Json::Null) => Value::Option(None),
        (Shape::Option(inner), other) => Value::Option(Some(Rc::new(from_json(other, inner)?))),
        (Shape::List(inner), Json::Array(items)) => Value::list(
            items
                .iter()
                .map(|i| from_json(i, inner))
                .collect::<Result<_, _>>()?,
        ),
        (Shape::Record(fields), Json::Object(map)) => {
            let mut values = Vec::with_capacity(fields.len());
            for (name, shape) in fields {
                let v = map
                    .get(name)
                    .ok_or_else(|| format!("field `{name}` is missing"))?;
                values.push(from_json(v, shape).map_err(|e| format!("field `{name}`: {e}"))?);
            }
            if map.len() != fields.len() {
                let extra = map
                    .keys()
                    .find(|k| !fields.iter().any(|(n, _)| n == *k))
                    .cloned()
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
                    Json::Null => "null",
                    Json::Bool(_) => "a bool",
                    Json::Number(_) => "a number",
                    Json::String(_) => "a string",
                    Json::Array(_) => "an array",
                    Json::Object(_) => "an object",
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
