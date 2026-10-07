//! Runtime values and the canonical value encoding used by the data pool.
//!
//! @ref LLP 1004 D3 (closed types, `Option`-only absence) / D4 (a resource's
//! value crosses the seam as bytes and is `shape`-validated there)
//!
//! A value is one of a closed set: number (f64), bool, string, unit, option,
//! list, record. Records carry fields by position; the plan's `types` table
//! names them. Absence is `Option` and nothing else.

use crate::bytes::{Reader, Writer};
use crate::{FieldsRange, HeapStr, InlineStr, Items, Plan, PlanError, Str, TypeKind, TypesId};
use std::rc::Rc;

/// The pattern for text, whichever variant holds it: match text as
/// `v @ str_value!()` and read it with [`Value::text`] or
/// [`Value::as_str`], so no caller names one variant alone.
#[macro_export]
macro_rules! str_value {
    () => {
        $crate::Value::HeapStr(_) | $crate::Value::InlineStr(_)
    };
}

/// A runtime value.
///
/// Text is two variants, one meaning: [`Value::InlineStr`] holds up to
/// [`InlineStr::CAP`] bytes in the value itself and [`Value::HeapStr`] the
/// rest, shared by count. Construction picks ([`Value::str`], `From`); both
/// payloads are opaque, so text is read through [`Value::as_str`] and
/// nothing outside this crate matches either variant for its text. A
/// wildcard arm that meant "not text" must name both, or ask `is_str`
/// (Charlie, 2026-09-28; LLP 1017.003 §"The value's text").
#[derive(Clone)]
pub enum Value {
    /// IEEE 754 binary64.
    Number(f64),
    /// A boolean.
    Bool(bool),
    /// UTF-8 text longer than [`InlineStr::CAP`] bytes.
    HeapStr(HeapStr),
    /// UTF-8 text of up to [`InlineStr::CAP`] bytes, held inline.
    InlineStr(InlineStr),
    /// The unit value.
    Unit,
    /// `none` or `some(v)`.
    Option(Option<Rc<Value>>),
    /// An ordered list.
    List(Items),
    /// A record; fields by position per its type.
    Record(Items),
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
            Value::HeapStr(_) | Value::InlineStr(_) => f
                .debug_tuple("Str")
                .field(&self.as_str().unwrap_or_default())
                .finish(),
            Value::Unit => f.write_str("Unit"),
            Value::Option(o) => f.debug_tuple("Option").field(o).finish(),
            Value::List(items) => f.debug_tuple("List").field(items).finish(),
            Value::Record(fields) => f.debug_tuple("Record").field(fields).finish(),
        }
    }
}

/// Structural equality; text by its bytes, whichever variant holds it.
impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::Option(a), Value::Option(b)) => a == b,
            (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => a == b,
            (a, b) => matches!((a.as_str(), b.as_str()), (Some(x), Some(y)) if x == y),
        }
    }
}

impl From<&str> for Value {
    #[inline]
    fn from(s: &str) -> Value {
        Value::str(s)
    }
}

impl From<String> for Value {
    #[inline]
    fn from(s: String) -> Value {
        Value::str(&s)
    }
}

/// Shared text becomes a value; short text is held inline instead (the
/// allocation is released when `s` was its last holder).
impl From<Str> for Value {
    #[inline]
    fn from(s: Str) -> Value {
        match InlineStr::new(&s) {
            Some(inline) => Value::InlineStr(inline),
            None => Value::HeapStr(HeapStr(s)),
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
    /// A string value: inline when it fits, else shared.
    #[inline]
    pub fn str(s: &str) -> Value {
        match InlineStr::new(s) {
            Some(inline) => Value::InlineStr(inline),
            None => Value::HeapStr(HeapStr(Str::from(s))),
        }
    }

    /// The text, for an arm that matched `str_value!()`; empty for any
    /// other value.
    #[inline]
    pub fn text(&self) -> &str {
        self.as_str().unwrap_or_default()
    }

    /// Shared text whatever its length: what construction never makes for
    /// short text, so a consumer's tests can hold the two forms of one text
    /// side by side. Not for use outside tests.
    #[doc(hidden)]
    pub fn str_shared_for_tests(s: &str) -> Value {
        Value::HeapStr(HeapStr(Str::from(s)))
    }

    /// Whether the value is text.
    #[inline]
    pub fn is_str(&self) -> bool {
        matches!(self, Value::HeapStr(_) | Value::InlineStr(_))
    }

    /// The text as shared text: the value's own allocation, or a new one for
    /// inline text. For a caller that keeps text beyond the value.
    pub fn to_shared_str(&self) -> Option<Str> {
        match self {
            Value::HeapStr(s) => Some(s.0.clone()),
            Value::InlineStr(s) => Some(Str::from(s.as_str())),
            _ => None,
        }
    }

    /// Whether two texts are the same object: one allocation, or equal
    /// inline bytes (equal text, indistinguishable). `false` when either
    /// is not text.
    #[inline]
    pub fn same_str(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::HeapStr(a), Value::HeapStr(b)) => Str::ptr_eq(&a.0, &b.0),
            (Value::InlineStr(a), Value::InlineStr(b)) => a == b,
            _ => false,
        }
    }

    /// How many hold shared text's allocation; `None` for inline text,
    /// which has none, and for anything that is not text.
    pub fn str_strong_count(&self) -> Option<usize> {
        match self {
            Value::HeapStr(s) => Some(Str::strong_count(&s.0)),
            _ => None,
        }
    }

    /// An identity for hashing what [`Value::same_str`] compares: the
    /// allocation, or the inline text's bytes.
    pub fn str_identity(&self) -> Option<u64> {
        match self {
            Value::HeapStr(s) => Some(s.0.addr() as u64),
            Value::InlineStr(s) => Some(text_hash(s.as_str()) | 1),
            _ => None,
        }
    }

    /// `some(v)`.
    pub fn some(v: Value) -> Value {
        Value::Option(Some(Rc::new(v)))
    }

    /// `none`.
    pub const NONE: Value = Value::Option(None);

    /// A record from its fields in order.
    pub fn record(fields: Vec<Value>) -> Value {
        Value::Record(Items::from(fields))
    }

    /// A list.
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Items::from(items))
    }

    /// The number, if it is one.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The text, if it is one: the one way text is read (both variants).
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::HeapStr(s) => Some(&s.0),
            Value::InlineStr(s) => Some(s.as_str()),
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
    /// data seam and the type check on compiled data. A number conforms
    /// only when it is finite.
    pub fn conforms(&self, plan: &Plan, ty: TypesId) -> bool {
        self.conforms_finite(plan, ty, true)
    }

    /// Whether the value has type `ty`, its numbers any (infinities and NaN
    /// included): the check on an argument the compiler supplies rather
    /// than the host, a child's captured prop (`@capture:…`).
    pub fn typed(&self, plan: &Plan, ty: TypesId) -> bool {
        self.conforms_finite(plan, ty, false)
    }

    fn conforms_finite(&self, plan: &Plan, ty: TypesId, finite: bool) -> bool {
        let row = plan.type_(ty);
        match (row.kind, self) {
            (TypeKind::Number, Value::Number(n)) => !finite || n.is_finite(),
            (TypeKind::Bool, Value::Bool(_)) => true,
            (TypeKind::String, Value::HeapStr(_) | Value::InlineStr(_)) => true,
            (TypeKind::Unit, Value::Unit) => true,
            (TypeKind::Option, Value::Option(None)) => true,
            (TypeKind::Option, Value::Option(Some(v))) => {
                row.elem.is_some_and(|e| v.conforms_finite(plan, e, finite))
            }
            (TypeKind::List, Value::List(items)) => row.elem.is_some_and(|e| {
                // A list of scalars (a series' points) checks in one loop.
                match plan.type_(e).kind {
                    TypeKind::Number => items
                        .iter()
                        .all(|v| matches!(v, Value::Number(n) if !finite || n.is_finite())),
                    TypeKind::String => items.iter().all(Value::is_str),
                    _ => items.iter().all(|v| v.conforms_finite(plan, e, finite)),
                }
            }),
            (TypeKind::Record, Value::Record(values)) => {
                let fields = row.fields;
                values.len() == fields.len as usize
                    && fields
                        .iter()
                        .zip(values.iter())
                        .all(|(f, v)| v.conforms_finite(plan, plan.field(f).ty, finite))
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
            Value::HeapStr(_) | Value::InlineStr(_) => {
                w.u8(2);
                w.string(self.as_str().unwrap_or_default());
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
        Self::decode_depth(r, 0, &mut Pool::default())
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

    fn decode_depth(r: &mut Reader<'_>, depth: u32, pool: &mut Pool) -> Result<Value, PlanError> {
        if depth > 64 {
            return Err(PlanError::ValueTooDeep);
        }
        let value = match r.u8()? {
            0 => {
                let n = r.f64()?;
                if !n.is_finite() {
                    return Err(PlanError::NonFiniteValue);
                }
                Value::Number(n)
            }
            1 => Value::Bool(r.u8()? != 0),
            2 => {
                let s = r.str()?;
                pool.unique = s.len() > Strings::SHORT;
                return Ok(match InlineStr::new(s) {
                    Some(inline) => Value::InlineStr(inline),
                    None => Value::HeapStr(HeapStr(pool.strings.get(s))),
                });
            }
            3 => Value::Unit,
            4 => Value::Option(None),
            5 => {
                let inner = Rc::new(Self::decode_depth(r, depth + 1, pool)?);
                pool.unique = true;
                return Ok(Value::Option(Some(inner)));
            }
            tag @ (6 | 7) => {
                let n = r.count()?;
                // The items gather on the pool's stack, then move into
                // their object: no vector of their own (an allocation and
                // a free per object, most of a decode's cost).
                let start = pool.stack.len();
                pool.stack.reserve(n.min(crate::bytes::RESERVE));
                let mut unique = false;
                for _ in 0..n {
                    // A number item (a series' points) decodes here: what
                    // `decode_depth` does for one, without the call.
                    if r.peek() == Some(0) && depth < 64 {
                        r.u8()?;
                        let n = r.f64()?;
                        if !n.is_finite() {
                            return Err(PlanError::NonFiniteValue);
                        }
                        pool.stack.push(Value::Number(n));
                        continue;
                    }
                    let item = Self::decode_depth(r, depth + 1, pool)?;
                    pool.stack.push(item);
                    unique |= pool.unique;
                }
                let record = tag == 7;
                let items = pool.objects.get(record, &mut pool.stack, start, unique);
                pool.unique = unique || items.len() > Objects::LONGEST;
                return Ok(if record {
                    Value::Record(items)
                } else {
                    Value::List(items)
                });
            }
            tag => return Err(PlanError::UnknownValueTag(tag)),
        };
        pool.unique = false;
        Ok(value)
    }
}

/// What one decode shares: equal short strings, then equal small objects.
#[derive(Default)]
struct Pool {
    strings: Strings,
    objects: Objects,
    /// The items of the objects being decoded, innermost last.
    stack: Vec<Value>,
    /// The value just decoded is a part no other object can share by
    /// allocation (a long string, an option's box, or an object holding
    /// one), so an object holding it is not looked up: it cannot repeat.
    unique: bool,
}

/// One FxHash step.
fn mix(h: u64, x: u64) -> u64 {
    (h.rotate_left(5) ^ x).wrapping_mul(0x517c_c1b7_2722_0a95)
}

/// Identical short strings in one decoded value share one allocation (text
/// of [`InlineStr::CAP`] bytes or less has none; this is the rest): a
/// baked list repeats its authors, file names, style names and small
/// numbers as text, and a separate allocation for each copy is most of a
/// string's cost. Sharing is sound where identity is compared
/// (`compare::same`): equal strings are equal values. The table is
/// direct-mapped and bounded; a collision only keeps the later string. A
/// small value never makes one.
#[derive(Default)]
struct Strings {
    slots: Vec<Option<Str>>,
    seen: u32,
}

impl Strings {
    /// Longest string shared, in bytes.
    const SHORT: usize = 24;
    const SLOTS: usize = 4096;
    /// Strings a value decodes before it gets a table.
    const AFTER: u32 = 64;

    fn get(&mut self, s: &str) -> Str {
        if s.len() > Self::SHORT {
            return Str::from(s);
        }
        if self.slots.is_empty() {
            self.seen += 1;
            if self.seen < Self::AFTER {
                return Str::from(s);
            }
            self.slots = vec![None; Self::SLOTS];
        }
        // FxHash over the bytes; the high bits pick the slot.
        let slot = &mut self.slots[(text_hash(s) >> 52) as usize % Self::SLOTS];
        match slot {
            Some(rc) if **rc == *s => rc.clone(),
            _ => {
                let rc = Str::from(s);
                *slot = Some(rc.clone());
                rc
            }
        }
    }
}

fn text_hash(s: &str) -> u64 {
    s.as_bytes().iter().fold(0, |h, &b| mix(h, b as u64))
}

/// Equal records and short lists in one decoded value share one
/// allocation, as equal short strings do: a baked list repeats small
/// objects (a reaction, a plain styled run, an empty list), and each copy's
/// allocation is most of what it costs. Sound for the same reason: equal
/// values are indistinguishable, so `compare::same` may hold of them. The
/// key is shallow: scalars by content, a string or a nested object by its
/// allocation, so strings and children shared first let their parents
/// match; an object holding a part that is never shared is not looked up
/// (`Pool::unique`). Direct-mapped and bounded; a collision keeps the later
/// object. A small value never makes a table.
#[derive(Default)]
struct Objects {
    slots: Vec<Option<(u64, Items)>>,
    seen: u32,
}

impl Objects {
    const SLOTS: usize = 8192;
    /// Objects a value decodes before it gets a table.
    const AFTER: u32 = 64;
    /// Longest object shared: a long list is rarely repeated whole, and
    /// each of its items would cost a hash.
    const LONGEST: usize = 32;

    /// The object of `stack[start..]`, which it takes off the stack.
    fn get(&mut self, record: bool, stack: &mut Vec<Value>, start: usize, unique: bool) -> Items {
        let items = &stack[start..];
        if unique || items.len() > Self::LONGEST {
            return Items::from(stack.drain(start..));
        }
        if self.slots.is_empty() {
            self.seen += 1;
            if self.seen < Self::AFTER {
                return Items::from(stack.drain(start..));
            }
            self.slots = vec![None; Self::SLOTS];
        }
        // The kind and the length are in the hash, so a stored object
        // with the same hash is compared field by field, and only then.
        let h = items
            .iter()
            .fold(mix(record as u64, items.len() as u64), |h, v| {
                mix(h, shallow_hash(v))
            });
        let slot = &mut self.slots[(h >> 51) as usize % Self::SLOTS];
        match slot {
            Some((stored, rc))
                if *stored == h
                    && rc.len() == items.len()
                    && rc.iter().zip(items).all(|(a, b)| shallow_eq(a, b)) =>
            {
                let rc = rc.clone();
                stack.truncate(start);
                rc
            }
            _ => {
                let rc = Items::from(stack.drain(start..));
                *slot = Some((h, rc.clone()));
                rc
            }
        }
    }
}

/// A field's part of an object's key (see `Objects`).
fn shallow_hash(v: &Value) -> u64 {
    match v {
        Value::Number(n) => mix(1, n.to_bits()),
        Value::Bool(b) => 2 + *b as u64,
        Value::HeapStr(_) | Value::InlineStr(_) => mix(3, v.str_identity().unwrap_or(0)),
        Value::Unit => 4,
        Value::Option(None) => 5,
        Value::Option(Some(v)) => mix(6, Rc::as_ptr(v) as usize as u64),
        Value::List(items) => mix(7, items.addr() as u64),
        Value::Record(fields) => mix(8, fields.addr() as u64),
    }
}

/// Whether two fields are the same part of a key: what `shallow_hash`
/// reads, compared the way it reads it.
fn shallow_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::HeapStr(_) | Value::InlineStr(_), _) => Value::same_str(a, b),
        (Value::Unit, Value::Unit) | (Value::Option(None), Value::Option(None)) => true,
        (Value::Option(Some(a)), Value::Option(Some(b))) => Rc::ptr_eq(a, b),
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => {
            Items::ptr_eq(a, b)
        }
        _ => false,
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
    use crate::Items;
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
            Value::HeapStr(_) | Value::InlineStr(_) => Derived::Str(Rc::from(v.as_str().unwrap())),
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
        let list = Value::List(Items::from(values.clone()));
        values.push(Value::Record(Items::from(vec![
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
    use crate::{Items, Str};

    fn strs(v: &Value) -> Vec<Str> {
        let Value::List(items) = v else {
            panic!("a list")
        };
        items
            .iter()
            .map(|v| match v {
                Value::HeapStr(s) => s.0.clone(),
                _ => panic!("a shared string"),
            })
            .collect()
    }

    /// A large value shares its repeated short strings; a small one and a
    /// long string never do, and every string decodes to what was encoded.
    #[test]
    fn repeated_short_strings_share_one_allocation() {
        let long = "x".repeat(25);
        let items: Vec<Value> = (0..200)
            .map(|i| {
                Value::str(if i % 2 == 0 {
                    "semibold condensed"
                } else {
                    &long
                })
            })
            .collect();
        let big = Value::List(Items::from(items));
        let decoded = Value::from_bytes(&big.to_bytes()).unwrap();
        assert_eq!(decoded, big);
        let s = strs(&decoded);
        assert!(Str::ptr_eq(&s[198], &s[196]), "late repeats share");
        assert!(!Str::ptr_eq(&s[199], &s[197]), "long strings stay separate");
        let small = Value::List(Items::from(vec![
            Value::str("semibold condensed"),
            Value::str("semibold condensed"),
        ]));
        let s = strs(&Value::from_bytes(&small.to_bytes()).unwrap());
        assert!(!Str::ptr_eq(&s[0], &s[1]), "a small value makes no table");
    }

    fn objects(v: &Value) -> Vec<Items> {
        let Value::List(items) = v else {
            panic!("a list")
        };
        items
            .iter()
            .map(|v| match v {
                Value::Record(f) | Value::List(f) => f.clone(),
                _ => panic!("an object"),
            })
            .collect()
    }

    /// A large value shares its repeated small objects, keyed by their
    /// fields (a nested object by its own shared allocation); an object
    /// holding a long string, an option or a longer object is not looked
    /// up, a record never stands for a list, and every object decodes to
    /// what was encoded.
    #[test]
    fn repeated_small_objects_share_one_allocation() {
        let long = "y".repeat(25);
        let items: Vec<Value> = (0..400)
            .map(|i| match i % 4 {
                0 => Value::record(vec![Value::str("👍"), Value::Number(3.0)]),
                1 => Value::list(vec![Value::str("👍"), Value::Number(3.0)]),
                2 => Value::record(vec![Value::str(&long), Value::list(vec![])]),
                _ => Value::record(vec![Value::some(Value::Bool(true))]),
            })
            .collect();
        let big = Value::List(Items::from(items));
        let decoded = Value::from_bytes(&big.to_bytes()).unwrap();
        assert_eq!(decoded, big);
        let o = objects(&decoded);
        assert!(Items::ptr_eq(&o[396], &o[392]), "late repeats share");
        assert!(Items::ptr_eq(&o[397], &o[393]), "lists too");
        assert!(!Items::ptr_eq(&o[396], &o[397]), "a record is not a list");
        assert!(
            Items::ptr_eq(&list_of(&o[398][1]), &list_of(&o[394][1])),
            "empty lists share"
        );
        assert!(
            !Items::ptr_eq(&o[398], &o[394]),
            "an object with a long string stays separate"
        );
        assert!(
            !Items::ptr_eq(&o[399], &o[395]),
            "an object with an option stays separate"
        );
        let small = Value::List(Items::from(vec![Value::list(vec![]), Value::list(vec![])]));
        let o = objects(&Value::from_bytes(&small.to_bytes()).unwrap());
        assert!(!Items::ptr_eq(&o[0], &o[1]), "a small value makes no table");
    }

    fn list_of(v: &Value) -> Items {
        match v {
            Value::List(items) => items.clone(),
            _ => panic!("a list"),
        }
    }
}

#[test]
fn a_value_is_sixteen_bytes() {
    assert_eq!(std::mem::size_of::<Value>(), 16);
    assert_eq!(std::mem::size_of::<Option<Value>>(), 16);
    assert_eq!(std::mem::size_of::<InlineStr>(), 15);
}

const _: () = assert!(std::mem::size_of::<Value>() == 16);

#[cfg(test)]
mod text_tests {
    use super::Value;
    use crate::bytes::Writer;
    use crate::{HeapStr, InlineStr, Str};

    /// Construction picks: text of up to 14 bytes (not characters) inline,
    /// longer text shared.
    #[test]
    fn construction_picks_inline_for_short_text() {
        for (s, inline) in [
            ("", true),
            ("bold", true),
            ("exactly14bytes", true),
            ("fifteen bytes!!", false),
            ("é".repeat(7).as_str(), true),
            ("é".repeat(8).as_str(), false),
            ("👍👍👍", true),
            ("👍👍👍👍", false),
        ] {
            let v = Value::str(s);
            assert_eq!(matches!(v, Value::InlineStr(_)), inline, "{s:?}");
            assert_eq!(v.as_str(), Some(s));
            assert!(v.is_str());
            let from_str = Value::from(Str::from(s));
            assert_eq!(
                matches!(from_str, Value::InlineStr(_)),
                inline,
                "From<Str> {s:?}"
            );
            assert_eq!(Value::from(s), v);
            assert_eq!(Value::from(s.to_string()), v);
        }
    }

    /// Inline and shared text with the same bytes are one value to every
    /// comparison and to the encoding; a hand-made shared short string (which
    /// construction never makes) proves it.
    #[test]
    fn inline_and_heap_text_are_identical() {
        for s in ["", "bold", "exactly14bytes", "👍👍👍"] {
            let inline = Value::str(s);
            assert!(matches!(inline, Value::InlineStr(_)));
            let heap = Value::HeapStr(HeapStr(Str::from(s)));
            assert_eq!(inline, heap);
            assert_eq!(heap, inline);
            assert_eq!(inline.to_bytes(), heap.to_bytes());
            assert_eq!(format!("{inline:?}"), format!("{heap:?}"));
            assert_eq!(inline.to_shared_str().unwrap().as_str(), s);
            let decoded = Value::from_bytes(&heap.to_bytes()).unwrap();
            assert!(
                matches!(decoded, Value::InlineStr(_)),
                "decode picks inline"
            );
            assert_eq!(decoded, heap);
            let mut w = Writer::default();
            w.string(s);
            let mut tagged = vec![2];
            tagged.extend(w.into_vec());
            assert_eq!(inline.to_bytes(), tagged, "the encoding is the text's");
        }
        assert_ne!(Value::str("bold"), Value::str("bolder"));
        assert_ne!(Value::str("bold"), Value::Number(1.0));
        let long = "x".repeat(30);
        assert_eq!(
            Value::str(&long),
            Value::HeapStr(HeapStr(Str::from(long.as_str())))
        );
    }

    /// `same_str`: one allocation for shared text, equal bytes for inline.
    #[test]
    fn same_text_is_identity_or_equal_inline_bytes() {
        let a = Value::str("bold");
        let b = Value::str("bold");
        assert!(Value::same_str(&a, &b));
        assert_eq!(a.str_identity(), b.str_identity());
        assert!(!Value::same_str(&a, &Value::str("bolt")));
        let long = "y".repeat(20);
        let (x, y) = (Value::str(&long), Value::str(&long));
        assert!(!Value::same_str(&x, &y), "two allocations");
        assert!(Value::same_str(&x, &x.clone()));
        assert!(!Value::same_str(&a, &Value::Number(0.0)));
        assert_eq!(InlineStr::CAP, 14);
    }
}
