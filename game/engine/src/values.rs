use crate::{Data, DataError, Reader, Value, Writer};
use std::rc::Rc;

// Keep Value's variants in saves: Record and List have the same JSON shape but
// different Contract types. The message envelope deliberately strips these tags.
#[derive(Clone, Debug, Default, PartialEq, Data)]
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
    pub(crate) fn json(&self, rounded: bool) -> String {
        match self {
            Self::Object(fields) => format!(
                "{{{}}}",
                fields
                    .iter()
                    .map(|(k, v)| format!("{}:{}", quote(k), v.json(rounded)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::List(items) | Self::Record(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(|v| v.json(rounded))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::Option(Some(v)) => v.json(rounded),
            _ => value_json(&self.value().unwrap(), rounded),
        }
    }
}
impl Data for Value {
    fn write(&self, w: &mut dyn Writer) {
        Stored::from(self.clone()).write(w);
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

/// A Contract value accepted by World::publish, including ordinary game scalars.
pub struct Published(pub Value);
impl From<Value> for Published {
    fn from(v: Value) -> Self {
        Self(v)
    }
}
impl From<bool> for Published {
    fn from(v: bool) -> Self {
        Self(Value::Bool(v))
    }
}
impl From<&str> for Published {
    fn from(v: &str) -> Self {
        Self(Value::str(v))
    }
}
impl From<String> for Published {
    fn from(v: String) -> Self {
        Self(Value::str(&v))
    }
}
macro_rules! numbers {
    ($($ty:ty),*) => {$(impl From<$ty> for Published {
        fn from(v: $ty) -> Self { Self(Value::Number(v as f64)) }
    })*};
}
numbers!(u8, u16, u32, i8, i16, i32, f32, f64);

pub(crate) fn quote(s: &str) -> String {
    let mut w = crate::json::Encoder::default();
    w.string(s);
    w.finish().unwrap()
}
pub(crate) fn value_json(v: &Value, rounded: bool) -> String {
    match v {
        Value::Unit | Value::Option(None) => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            let mut w = if rounded {
                crate::json::Encoder::rounded()
            } else {
                crate::json::Encoder::default()
            };
            n.write(&mut w);
            {
                let text = w.finish().unwrap_or_else(|_| "null".into());
                text.strip_suffix(".0").unwrap_or(&text).to_string()
            }
        }
        Value::Str(s) => quote(s),
        Value::Option(Some(v)) => value_json(v, rounded),
        Value::List(v) | Value::Record(v) => format!(
            "[{}]",
            v.iter()
                .map(|v| value_json(v, rounded))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

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
    fn boolean(&mut self, v: bool) {
        self.push(Stored::Bool(v));
    }
    fn number(&mut self, v: crate::Number) {
        use crate::Number::*;
        self.push(Stored::Number(match v {
            Unsigned(n) => n as f64,
            Signed(n) => n as f64,
            F32(n) => n as f64,
            F64(n) => n,
        }));
    }
    fn string(&mut self, v: &str) {
        self.push(Stored::Str(v.into()));
    }
    fn bytes(&mut self, kind: crate::data::BulkKind, bytes: &[u8]) {
        use crate::data::BulkKind;
        let width = match kind {
            BulkKind::U8 => 1,
            BulkKind::U16 => 2,
            BulkKind::U32 | BulkKind::F32 => 4,
        };
        self.push(Stored::List(
            bytes
                .chunks_exact(width)
                .map(|chunk| {
                    Stored::Number(match kind {
                        BulkKind::U8 => chunk[0] as f64,
                        BulkKind::U16 => u16::from_le_bytes(chunk.try_into().unwrap()) as f64,
                        BulkKind::U32 => u32::from_le_bytes(chunk.try_into().unwrap()) as f64,
                        BulkKind::F32 => f32::from_le_bytes(chunk.try_into().unwrap()) as f64,
                    })
                })
                .collect(),
        ));
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
    fn field(&mut self, name: &str) {
        self.fields.push(name.into());
    }
    fn end_struct(&mut self) {
        self.end();
    }
    fn variant(&mut self, _: &str, _: u32) {
        panic!("publish_record expects ordinary records, not enum variants");
    }
    fn end_variant(&mut self) {}
    fn option(&mut self, _: bool) {
        self.stack.push(Stored::Option(None));
    }
    fn end_option(&mut self) {
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
