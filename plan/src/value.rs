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
#[derive(Clone, PartialEq)]
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
    List(Rc<[Value]>),
    /// A record; fields by position per its type.
    Record(Rc<[Value]>),
}

/// The derived text, with the number printed by exact-num (LLP 1047 §6).
impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(n) => f
                .debug_tuple("Number")
                .field(&exact_num::ShortestDebug(*n))
                .finish(),
            Value::Bool(b) => f.debug_tuple("Bool").field(b).finish(),
            Value::Str(s) => f.debug_tuple("Str").field(s).finish(),
            Value::Unit => f.write_str("Unit"),
            Value::Option(o) => f.debug_tuple("Option").field(o).finish(),
            Value::List(items) => f.debug_tuple("List").field(items).finish(),
            Value::Record(fields) => f.debug_tuple("Record").field(fields).finish(),
        }
    }
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
        Value::Record(Rc::from(fields))
    }

    /// A list.
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::from(items))
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
        Self::decode_depth(r, 0, &mut Strings::default())
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

    fn decode_depth(
        r: &mut Reader<'_>,
        depth: u32,
        strings: &mut Strings,
    ) -> Result<Value, PlanError> {
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
            2 => Value::Str(strings.get(r.str()?)),
            3 => Value::Unit,
            4 => Value::Option(None),
            5 => Value::Option(Some(Rc::new(Self::decode_depth(r, depth + 1, strings)?))),
            6 => {
                let n = r.count()?;
                let mut items = Vec::with_capacity(n.min(crate::bytes::RESERVE));
                for _ in 0..n {
                    items.push(Self::decode_depth(r, depth + 1, strings)?);
                }
                Value::List(Rc::from(items))
            }
            7 => {
                let n = r.count()?;
                let mut fields = Vec::with_capacity(n.min(crate::bytes::RESERVE));
                for _ in 0..n {
                    fields.push(Self::decode_depth(r, depth + 1, strings)?);
                }
                Value::Record(Rc::from(fields))
            }
            tag => return Err(PlanError::UnknownValueTag(tag)),
        })
    }
}

/// Identical short strings in one decoded value share one allocation: a
/// baked list repeats its authors, file names, style names and small
/// numbers as text, and a separate `Rc<str>` for each copy is most of a
/// string's cost. Sharing is sound where identity is compared
/// (`compare::same`): equal strings are equal values. The table is
/// direct-mapped and bounded; a collision only keeps the later string. A
/// small value never makes one.
#[derive(Default)]
struct Strings {
    slots: Vec<Option<Rc<str>>>,
    seen: u32,
}

impl Strings {
    /// Longest string shared, in bytes.
    const SHORT: usize = 24;
    const SLOTS: usize = 4096;
    /// Strings a value decodes before it gets a table.
    const AFTER: u32 = 64;

    fn get(&mut self, s: &str) -> Rc<str> {
        if s.len() > Self::SHORT {
            return Rc::from(s);
        }
        if self.slots.is_empty() {
            self.seen += 1;
            if self.seen < Self::AFTER {
                return Rc::from(s);
            }
            self.slots = vec![None; Self::SLOTS];
        }
        // FxHash over the bytes; the high bits pick the slot.
        let mut h: u64 = 0;
        for &b in s.as_bytes() {
            h = (h.rotate_left(5) ^ b as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
        let slot = &mut self.slots[(h >> 52) as usize % Self::SLOTS];
        match slot {
            Some(rc) if **rc == *s => rc.clone(),
            _ => {
                let rc: Rc<str> = Rc::from(s);
                *slot = Some(rc.clone());
                rc
            }
        }
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

#[cfg(test)]
mod debug_tests {
    use super::Value;
    use std::rc::Rc;

    /// `Value` as it was, with the derived `Debug` and core's float printer.
    #[derive(Debug)]
    #[allow(dead_code)] // read only by `Debug`
    enum Derived {
        Number(f64),
        Bool(bool),
        Str(Rc<str>),
        Unit,
        Option(Option<Rc<Derived>>),
        List(Rc<Vec<Derived>>),
        Record(Rc<Vec<Derived>>),
    }

    fn derived(v: &Value) -> Derived {
        let all = |items: &[Value]| Rc::new(items.iter().map(derived).collect());
        match v {
            Value::Number(n) => Derived::Number(*n),
            Value::Bool(b) => Derived::Bool(*b),
            Value::Str(s) => Derived::Str(Rc::clone(s)),
            Value::Unit => Derived::Unit,
            Value::Option(o) => Derived::Option(o.as_deref().map(|v| Rc::new(derived(v)))),
            Value::List(items) => Derived::List(all(items)),
            Value::Record(fields) => Derived::Record(all(fields)),
        }
    }

    #[test]
    fn debug_text_is_the_derived_text() {
        let numbers = [
            0.0,
            -0.0,
            1.0,
            -2.5,
            0.1,
            1e-5,
            123456.789,
            1e16,
            1e300,
            f64::NAN,
            f64::INFINITY,
        ];
        let mut values: Vec<Value> = numbers.iter().map(|n| Value::Number(*n)).collect();
        values.extend([
            Value::Bool(true),
            Value::str("a \"quoted\"\nline"),
            Value::Unit,
            Value::Option(None),
            Value::Option(Some(Rc::new(Value::Number(0.5)))),
        ]);
        let list = Value::List(Rc::from(values.clone()));
        values.push(Value::Record(Rc::from(vec![
            list.clone(),
            Value::Number(-1e-7),
        ])));
        values.push(list);
        for v in &values {
            assert_eq!(format!("{v:?}"), format!("{:?}", derived(v)));
            assert_eq!(format!("{v:#?}"), format!("{:#?}", derived(v)));
        }
    }
}

#[cfg(test)]
mod decode_tests {
    use super::Value;
    use std::rc::Rc;

    fn strs(v: &Value) -> Vec<Rc<str>> {
        let Value::List(items) = v else {
            panic!("a list")
        };
        items
            .iter()
            .map(|v| match v {
                Value::Str(s) => s.clone(),
                _ => panic!("a string"),
            })
            .collect()
    }

    /// A large value shares its repeated short strings; a small one and a
    /// long string never do, and every string decodes to what was encoded.
    #[test]
    fn repeated_short_strings_share_one_allocation() {
        let long = "x".repeat(25);
        let items: Vec<Value> = (0..200)
            .map(|i| Value::str(if i % 2 == 0 { "bold" } else { &long }))
            .collect();
        let big = Value::List(Rc::from(items));
        let decoded = Value::from_bytes(&big.to_bytes()).unwrap();
        assert_eq!(decoded, big);
        let s = strs(&decoded);
        assert!(Rc::ptr_eq(&s[198], &s[196]), "late repeats share");
        assert!(!Rc::ptr_eq(&s[199], &s[197]), "long strings stay separate");
        let small = Value::List(Rc::from(vec![Value::str("bold"), Value::str("bold")]));
        let s = strs(&Value::from_bytes(&small.to_bytes()).unwrap());
        assert!(!Rc::ptr_eq(&s[0], &s[1]), "a small value makes no table");
    }
}
