//! Runtime values and the canonical value encoding used by the data pool.
//!
//! @ref LLP 1004 D3 (closed types, `Option`-only absence) / D4 (a resource's
//! value crosses the seam as bytes and is `shape`-validated there)
//!
//! A value is one of a closed set: number (f64), bool, string, unit, option,
//! list, record. Records carry fields by position; the plan's `types` table
//! names them. Absence is `Option` and nothing else.

use crate::bytes::{Reader, Writer};
use crate::{FieldsRange, Plan, PlanError, TypeKind, TypesId};
use std::rc::Rc;

/// A runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// IEEE 754 binary64.
    Number(f64),
    /// A boolean.
    Bool(bool),
    /// UTF-8 text.
    Str(Rc<str>),
    /// The unit value.
    Unit,
    /// `none` or `some(v)`.
    Option(Option<Rc<Value>>),
    /// An ordered list.
    List(Rc<Vec<Value>>),
    /// A record; fields by position per its type.
    Record(Rc<Vec<Value>>),
}

impl Default for Value {
    /// `Unit` — the value of a statement body.
    fn default() -> Value {
        Value::Unit
    }
}

impl Value {
    /// A string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    /// `some(v)`.
    pub fn some(v: Value) -> Value {
        Value::Option(Some(Rc::new(v)))
    }

    /// `none`.
    pub const NONE: Value = Value::Option(None);

    /// A record from its fields in order.
    pub fn record(fields: Vec<Value>) -> Value {
        Value::Record(Rc::new(fields))
    }

    /// A list.
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(items))
    }

    /// The number, if it is one.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The text, if it is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Whether the value conforms to `ty` in `plan` — the `shape` check at the
    /// data seam and the type check on compiled data.
    pub fn conforms(&self, plan: &Plan, ty: TypesId) -> bool {
        let row = plan.type_(ty);
        match (row.kind, self) {
            (TypeKind::Number, Value::Number(n)) => n.is_finite(),
            (TypeKind::Bool, Value::Bool(_)) => true,
            (TypeKind::String, Value::Str(_)) => true,
            (TypeKind::Unit, Value::Unit) => true,
            (TypeKind::Option, Value::Option(None)) => true,
            (TypeKind::Option, Value::Option(Some(v))) => {
                row.elem.is_some_and(|e| v.conforms(plan, e))
            }
            (TypeKind::List, Value::List(items)) => row
                .elem
                .is_some_and(|e| items.iter().all(|v| v.conforms(plan, e))),
            (TypeKind::Record, Value::Record(values)) => {
                let fields = row.fields;
                values.len() == fields.len as usize
                    && fields
                        .iter()
                        .zip(values.iter())
                        .all(|(f, v)| v.conforms(plan, plan.field(f).ty))
            }
            _ => false,
        }
    }

    /// Canonical encoding into `w`: a tag byte then the payload.
    pub fn encode(&self, w: &mut Writer) {
        match self {
            Value::Number(n) => {
                w.u8(0);
                w.f64(*n);
            }
            Value::Bool(b) => {
                w.u8(1);
                w.u8(*b as u8);
            }
            Value::Str(s) => {
                w.u8(2);
                w.string(s);
            }
            Value::Unit => w.u8(3),
            Value::Option(None) => w.u8(4),
            Value::Option(Some(v)) => {
                w.u8(5);
                v.encode(w);
            }
            Value::List(items) => {
                w.u8(6);
                w.u32(items.len() as u32);
                for v in items.iter() {
                    v.encode(w);
                }
            }
            Value::Record(fields) => {
                w.u8(7);
                w.u32(fields.len() as u32);
                for v in fields.iter() {
                    v.encode(w);
                }
            }
        }
    }

    /// The canonical bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decode one value from `r`, bounded in depth so a hostile payload cannot
    /// exhaust the stack.
    pub fn decode(r: &mut Reader<'_>) -> Result<Value, PlanError> {
        Self::decode_depth(r, 0)
    }

    /// Decode exactly one value from `bytes`; trailing bytes are a refusal.
    pub fn from_bytes(bytes: &[u8]) -> Result<Value, PlanError> {
        let mut r = Reader::new(bytes);
        let v = Value::decode(&mut r)?;
        if !r.is_empty() {
            return Err(PlanError::TrailingBytes(r.remaining()));
        }
        Ok(v)
    }

    fn decode_depth(r: &mut Reader<'_>, depth: u32) -> Result<Value, PlanError> {
        if depth > 64 {
            return Err(PlanError::ValueTooDeep);
        }
        Ok(match r.u8()? {
            0 => {
                let n = r.f64()?;
                if !n.is_finite() {
                    return Err(PlanError::NonFiniteValue);
                }
                Value::Number(n)
            }
            1 => Value::Bool(r.u8()? != 0),
            2 => Value::Str(Rc::from(r.string()?)),
            3 => Value::Unit,
            4 => Value::Option(None),
            5 => Value::Option(Some(Rc::new(Self::decode_depth(r, depth + 1)?))),
            6 => {
                let n = r.count()?;
                let mut items = Vec::with_capacity(n.min(crate::bytes::RESERVE));
                for _ in 0..n {
                    items.push(Self::decode_depth(r, depth + 1)?);
                }
                Value::List(Rc::new(items))
            }
            7 => {
                let n = r.count()?;
                let mut fields = Vec::with_capacity(n.min(crate::bytes::RESERVE));
                for _ in 0..n {
                    fields.push(Self::decode_depth(r, depth + 1)?);
                }
                Value::Record(Rc::new(fields))
            }
            tag => return Err(PlanError::UnknownValueTag(tag)),
        })
    }
}

impl Plan {
    /// Find a record field's position by name within a record type.
    pub fn field_index(&self, ty: TypesId, name: &str) -> Option<usize> {
        let fields: FieldsRange = self.type_(ty).fields;
        fields
            .iter()
            .position(|f| self.str(self.field(f).name) == name)
    }
}
