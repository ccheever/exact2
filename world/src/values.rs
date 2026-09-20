use crate::{Data, DataError, Reader, Value, Writer};
use std::rc::Rc;

// Keep Value's variants in saves: Record and List have the same JSON shape but
// different Contract types. The message envelope deliberately strips these tags.
#[derive(Clone, Default, PartialEq, Data)]
pub(crate) enum Stored {
    #[default]
    Unit,
    Number(f64),
    Bool(bool),
    Str(String),
    Option(Option<Box<Stored>>),
    List(Vec<Stored>),
    Record(Vec<Stored>),
    Object(std::collections::BTreeMap<String, Stored>),
}
impl From<Value> for Stored {
    fn from(value: Value) -> Self {
        match value {
            Value::Unit => Self::Unit,
            Value::Number(n) => Self::Number(n),
            Value::Bool(b) => Self::Bool(b),
            Value::Str(s) => Self::Str(s.to_string()),
            Value::Option(v) => Self::Option(v.map(|v| Box::new(Self::from((*v).clone())))),
            Value::List(v) => Self::List(v.iter().cloned().map(Self::from).collect()),
            Value::Record(v) => Self::Record(v.iter().cloned().map(Self::from).collect()),
        }
    }
}
impl Stored {
    fn value_allocation(&self) -> usize {
        let overhead = 2 * std::mem::size_of::<usize>();
        match self {
            Self::Str(s) => s.len() + overhead,
            Self::List(items) | Self::Record(items) => {
                overhead
                    + std::mem::size_of::<Vec<Value>>()
                    + items.len() * std::mem::size_of::<Value>()
                    + items.iter().map(Self::value_allocation).sum::<usize>()
            }
            Self::Option(Some(v)) => overhead + std::mem::size_of::<Value>() + v.value_allocation(),
            _ => 0,
        }
    }
    pub(crate) fn value(&self) -> Option<Value> {
        Some(match self {
            Self::Unit => Value::Unit,
            Self::Number(n) => Value::Number(*n),
            Self::Bool(b) => Value::Bool(*b),
            Self::Str(s) => Value::str(s),
            Self::Option(v) => Value::Option(match v {
                Some(v) => Some(Rc::new(v.value()?)),
                None => None,
            }),
            Self::List(v) => Value::list(v.iter().map(Self::value).collect::<Option<_>>()?),
            Self::Record(v) => Value::record(v.iter().map(Self::value).collect::<Option<_>>()?),
            Self::Object(_) => return None,
        })
    }
    pub(crate) fn validate(&self, remaining: &mut usize, depth: usize) -> Result<(), DataError> {
        let cost = match self {
            Self::Str(s) => 64usize.saturating_add(s.len().saturating_mul(6)),
            _ => 64,
        };
        *remaining = remaining
            .checked_sub(cost)
            .ok_or_else(|| DataError::new("publication exceeds 65536 bytes/visits"))?;
        if depth > 256 {
            return Err(DataError::new("publication depth limit"));
        }
        match self {
            Self::Object(fields) => {
                for (k, v) in fields {
                    *remaining = remaining
                        .checked_sub(k.len().saturating_mul(6))
                        .ok_or_else(|| DataError::new("publication key limit"))?;
                    v.validate(remaining, depth + 1)?;
                }
            }
            Self::List(items) | Self::Record(items) => {
                for v in items {
                    v.validate(remaining, depth + 1)?;
                }
            }
            Self::Option(Some(v)) => v.validate(remaining, depth + 1)?,
            Self::Number(n) if !n.is_finite() => {
                return Err(DataError::new("non-finite publication"))
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn inspect(&self, w: &mut dyn Writer) {
        match self {
            Self::Object(fields) => {
                w.begin_struct();
                for (k, v) in fields {
                    w.key(k);
                    v.inspect(w);
                }
                w.end_struct();
            }
            Self::List(items) | Self::Record(items) => {
                w.begin_seq(items.len());
                for v in items {
                    w.item();
                    v.inspect(w);
                }
                w.end_seq();
            }
            Self::Option(Some(v)) => v.inspect(w),
            Self::Unit | Self::Option(None) => w.unit(),
            Self::Str(s) => w.string(s),
            Self::Number(n) => w.number(crate::Number::F64(*n)),
            Self::Bool(b) => w.boolean(*b),
        }
    }
}

impl Data for Value {
    fn write(&self, w: &mut dyn Writer) {
        w.claim_decoded(128);
        if let Self::Str(s) = self {
            w.claim_decoded(s.len());
        }
        // Stored's derived variant framing, without an owned conversion tree.
        let (name, index) = match self {
            Self::Unit => ("Unit", 0),
            Self::Number(_) => ("Number", 1),
            Self::Bool(_) => ("Bool", 2),
            Self::Str(_) => ("Str", 3),
            Self::Option(_) => ("Option", 4),
            Self::List(_) => ("List", 5),
            Self::Record(_) => ("Record", 6),
        };
        w.variant(name, index);
        if matches!(self, Self::Unit) {
            w.begin_struct();
            w.end_struct();
        } else {
            w.begin_seq(1);
            w.item();
            match self {
                Self::Unit => {}
                Self::Number(n) => n.write(w),
                Self::Bool(b) => b.write(w),
                Self::Str(s) => w.string(s),
                Self::Option(value) => {
                    w.option(value.is_some());
                    if let Some(value) = value {
                        value.write(w);
                    }
                    w.end_option();
                }
                Self::List(values) | Self::Record(values) => values.as_ref().write(w),
            }
            w.end_seq();
        }
        w.end_variant();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let mut stored = Stored::default();
        stored.read(r)?;
        r.claim(stored.value_allocation())?;
        *self = stored
            .value()
            .ok_or_else(|| DataError::new("named record requires a Contract shape"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bin, hash, json};

    #[test]
    fn value_stream_matches_stored_derive_in_every_codec() {
        fn json_stream(value: &impl Data) -> Result<String, DataError> {
            let mut writer = json::Encoder::default();
            value.write(&mut writer);
            writer.finish()
        }
        let shared = Rc::new(Value::record(vec![
            Value::str("quote \" slash \\ line\n héllo 🌕"),
            Value::Number(-0.0),
            Value::list(vec![Value::Unit, Value::Bool(true)]),
        ]));
        let mut cases = vec![
            Value::Unit,
            Value::Bool(false),
            Value::Bool(true),
            Value::str(""),
            Value::str("héllo 🌕"),
            Value::Option(None),
            Value::Option(Some(Rc::new(Value::Unit))),
            Value::Option(Some(shared.clone())),
            Value::list(vec![]),
            Value::record(vec![]),
            shared.as_ref().clone(),
            Value::list(vec![
                Value::Option(Some(shared.clone())),
                shared.as_ref().clone(),
            ]),
        ];
        cases.extend(
            [
                0.0,
                -0.0,
                1.0,
                -1.0,
                f64::MAX,
                f64::MIN_POSITIVE,
                f64::from_bits(1),
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NAN,
                f64::from_bits(0xfff8_0000_0000_0001),
            ]
            .into_iter()
            .map(Value::Number),
        );
        let owners = Rc::strong_count(&shared);
        for value in &cases {
            let stored = Stored::from(value.clone());
            let expected = bin::to_vec(&stored).unwrap();
            assert_eq!(bin::to_vec(value).unwrap(), expected, "{value:?}");
            assert_eq!(hash::of(value), hash::of(&stored), "{value:?}");
            assert_eq!(json_stream(value), json_stream(&stored), "{value:?}");
            let decoded: Value = bin::from_slice(&expected).unwrap();
            assert_eq!(
                bin::to_vec(&decoded).unwrap(),
                expected,
                "round trip {value:?}"
            );
        }
        assert_eq!(Rc::strong_count(&shared), owners);
        // List and Record have equal public JSON shapes but distinct saved tags.
        assert_ne!(
            bin::to_vec(&Value::list(vec![])).unwrap(),
            bin::to_vec(&Value::record(vec![])).unwrap()
        );
        assert_ne!(hash::of(&Value::Number(0.)), hash::of(&Value::Number(-0.)));
    }
}

/// A Contract value accepted by World::publish, including ordinary game scalars.
pub struct Published(pub(crate) Stored);
impl From<Value> for Published {
    fn from(v: Value) -> Self {
        Self(v.into())
    }
}
impl From<bool> for Published {
    fn from(v: bool) -> Self {
        Self(Stored::Bool(v))
    }
}
impl From<&str> for Published {
    fn from(v: &str) -> Self {
        Self(Stored::Str(v.into()))
    }
}
impl From<String> for Published {
    fn from(v: String) -> Self {
        Self(Stored::Str(v))
    }
}
macro_rules! numbers {
    ($($ty:ty),*) => {$(impl From<$ty> for Published {
        fn from(v: $ty) -> Self { Self(Stored::Number(v as f64)) }
    })*};
}
numbers!(u8, u16, u32, i8, i16, i32, f32, f64);

// One Data traversal; field names remain names until the app's shape decoder.
#[derive(Default)]
struct RecordWriter {
    stack: Vec<Stored>,
    fields: Vec<String>,
    result: Stored,
}
impl RecordWriter {
    fn push(&mut self, value: Stored) {
        match self.stack.last_mut() {
            Some(Stored::Object(fields)) => {
                fields.insert(self.fields.pop().expect("record field"), value);
            }
            Some(Stored::List(items)) => items.push(value),
            Some(Stored::Option(item)) => *item = Some(Box::new(value)),
            None => self.result = value,
            _ => unreachable!(),
        }
    }
    fn end(&mut self) {
        let value = self.stack.pop().expect("Data container");
        self.push(value);
    }
}
impl Writer for RecordWriter {
    fn unit(&mut self) {
        self.push(Stored::Unit);
    }
    fn boolean(&mut self, v: bool) {
        self.push(Stored::Bool(v));
    }
    fn number(&mut self, v: crate::Number) {
        use crate::Number::*;
        self.push(Stored::Number(match v {
            Unsigned(n) => {
                assert!(
                    n <= 9_007_199_254_740_991,
                    "publish_record: integer outside Contract safe range"
                );
                n as f64
            }
            Signed(n) => {
                assert!(
                    (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n),
                    "publish_record: integer outside Contract safe range"
                );
                n as f64
            }
            F32(n) => n as f64,
            F64(n) => n,
        }));
    }
    fn string(&mut self, v: &str) {
        self.push(Stored::Str(v.into()));
    }
    fn bytes(&mut self, value: crate::data::Bulk<'_>) {
        self.push(Stored::List(value.numbers().map(Stored::Number).collect()));
    }
    fn begin_seq(&mut self, len: usize) {
        self.stack.push(Stored::List(Vec::with_capacity(len)));
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        self.end();
    }
    fn begin_struct(&mut self) {
        self.stack.push(Stored::Object(Default::default()));
    }
    fn field(&mut self, name: &'static str) {
        self.key(name);
    }
    fn key(&mut self, name: &str) {
        self.fields.push(name.into());
    }
    fn end_struct(&mut self) {
        self.end();
    }
    fn variant(&mut self, _: &'static str, _: u32) {
        panic!("publish_record expects ordinary records, not enum variants");
    }
    fn end_variant(&mut self) {}
    fn option(&mut self, _: bool) {
        self.stack.push(Stored::Option(None));
    }
    fn end_option(&mut self) {
        assert!(
            !matches!(self.stack.last(), Some(Stored::Option(Some(v))) if matches!(v.as_ref(), Stored::Unit)),
            "publish_record: Some(()) is ambiguous with None in Contract JSON"
        );
        self.end();
    }
}
impl crate::World {
    /// Publish named Data fields together. Records, lists, options and scalars use
    /// Contract JSON; the app validates field types against its declared shape.
    /// Typed numeric vectors become arrays; enum variants are not Contract values
    /// and panic. The argument itself must be a named record.
    pub fn publish_record(&self, record: &impl Data) {
        let mut writer = RecordWriter::default();
        record.write(&mut writer);
        let Stored::Object(fields) = writer.result else {
            panic!("publish_record expects a named record");
        };
        for (key, value) in fields {
            self.publish_value(&key, value);
        }
    }
}
