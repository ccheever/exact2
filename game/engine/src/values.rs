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
            v @ exact_plan::str_value!() => Self::Str(v.text().to_string()),
            Value::Option(v) => Self::Option(v.map(|v| Box::new(Self::from((*v).clone())))),
            Value::List(v) => Self::List(v.iter().cloned().map(Self::from).collect()),
            Value::Record(v) => Self::Record(v.iter().cloned().map(Self::from).collect()),
        }
    }
}
impl Stored {
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
    pub(crate) fn append_json(&self, out: &mut String, rounded: bool) {
        match self {
            Self::Object(fields) => {
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    crate::json::quote_into(out, key);
                    out.push(':');
                    value.append_json(out, rounded);
                }
                out.push('}');
            }
            Self::List(items) | Self::Record(items) => {
                out.push('[');
                for (i, value) in items.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    value.append_json(out, rounded);
                }
                out.push(']');
            }
            Self::Option(Some(v)) => v.append_json(out, rounded),
            Self::Unit | Self::Option(None) => out.push_str("null"),
            Self::Str(s) => crate::json::quote_into(out, s),
            Self::Number(n) => number_json(out, *n, rounded),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        }
    }
}

impl Data for Value {
    fn write(&self, w: &mut dyn Writer) {
        // Stored's derived variant framing, without an owned conversion tree.
        let (name, index) = match self {
            Self::Unit => ("Unit", 0),
            Self::Number(_) => ("Number", 1),
            Self::Bool(_) => ("Bool", 2),
            exact_plan::str_value!() => ("Str", 3),
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
                s @ exact_plan::str_value!() => w.string(s.text()),
                Self::Option(value) => {
                    w.option(value.is_some());
                    if let Some(value) = value {
                        value.write(w);
                    }
                    w.end_option();
                }
                Self::List(values) | Self::Record(values) => values.to_vec().write(w),
            }
            w.end_seq();
        }
        w.end_variant();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        fn payload<T: Data>(r: &mut dyn Reader) -> Result<T, DataError> {
            let mut value = (T::default(),);
            value.read(r)?;
            Ok(value.0)
        }
        let arm = r.variant()?;
        let overhead = 2 * std::mem::size_of::<usize>();
        let value = match arm.as_str() {
            "Unit" => {
                r.begin_struct()?;
                while let Some(field) = r.field()? {
                    r.skip().map_err(|e| e.at(field))?;
                }
                Self::Unit
            }
            "Number" => Self::Number(payload(r)?),
            "Bool" => Self::Bool(payload(r)?),
            "Str" => {
                let text: String = payload(r)?;
                r.claim(text.len() + overhead)?;
                Self::str(&text)
            }
            "Option" => Self::Option(match payload::<Option<Value>>(r)? {
                Some(value) => {
                    r.claim(overhead + std::mem::size_of::<Value>())?;
                    Some(Rc::new(value))
                }
                None => None,
            }),
            "List" | "Record" => {
                let values: Vec<Value> = payload(r)?;
                r.claim(overhead + std::mem::size_of::<Vec<Value>>())?;
                if arm == "List" {
                    Self::list(values)
                } else {
                    Self::record(values)
                }
            }
            "Object" => return Err(DataError::new("named record requires a Contract shape")),
            _ => return Err(DataError::new(format!("unknown variant {arm}"))),
        };
        r.end_variant()?;
        *self = value;
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
            let expected = bin::to_vec(&stored);
            assert_eq!(bin::to_vec(value), expected, "{value:?}");
            assert_eq!(hash::of(value), hash::of(&stored), "{value:?}");
            assert_eq!(json_stream(value), json_stream(&stored), "{value:?}");
            let decoded: Value = bin::from_slice(&expected).unwrap();
            assert_eq!(bin::to_vec(&decoded), expected, "round trip {value:?}");
        }
        assert_eq!(Rc::strong_count(&shared), owners);
        // List and Record have equal public JSON shapes but distinct saved tags.
        assert_ne!(
            bin::to_vec(&Value::list(vec![])),
            bin::to_vec(&Value::record(vec![]))
        );
        assert_ne!(hash::of(&Value::Number(0.)), hash::of(&Value::Number(-0.)));
    }

    #[test]
    fn value_decode_keeps_defaults_extensions_and_refusal_atomicity() {
        for (text, expected) in [
            (r#"{"Unit":{"ignored":[1,2]}}"#, Value::Unit),
            (r#"{"Number":[]}"#, Value::Number(0.)),
            (r#"{"Bool":[]}"#, Value::Bool(false)),
            (r#"{"Str":[]}"#, Value::str("")),
            (r#"{"Option":[]}"#, Value::Option(None)),
            (r#"{"List":[]}"#, Value::list(vec![])),
            (r#"{"Record":[]}"#, Value::record(vec![])),
            (r#"{"Number":[7,{"ignored":[1,2]}]}"#, Value::Number(7.)),
        ] {
            assert_eq!(json::from_str::<Value>(text).unwrap(), expected, "{text}");
        }
        for text in [
            r#"{"Object":[{}]}"#,
            r#"{"Unknown":[]}"#,
            r#"{"List":[[{"Number":[1]},{"Bool":["wrong"]}]]}"#,
            r#"{"Option":[[{"Str":["truncated"]}]]"#,
        ] {
            let mut value = Value::str("unchanged");
            assert!(json::read_into(text, &mut value).is_err(), "{text}");
            assert_eq!(value, Value::str("unchanged"), "{text}");
        }
        let bytes = bin::to_vec(&Value::list(vec![Value::str("saved"); 16]));
        let mut reader = bin::Decoder::new(&bytes);
        reader.claim(crate::data::MAX_LOAD_BYTES - 128).unwrap();
        let mut value = Value::Number(42.);
        let error = value.read(&mut reader).unwrap_err();
        assert!(error.message.contains("load budget"), "{error}");
        assert_eq!(value, Value::Number(42.));
    }
}

/// A Contract value accepted by World::publish, including ordinary game scalars.
pub struct Published<'a>(pub(crate) Incoming<'a>);
pub(crate) enum Incoming<'a> {
    Borrowed(&'a str),
    Owned(Stored),
}
impl Incoming<'_> {
    pub(crate) fn matches(&self, stored: &Stored) -> bool {
        match self {
            Self::Borrowed(value) => {
                matches!(stored, Stored::Str(current) if current.as_str() == *value)
            }
            Self::Owned(value) => stored == value,
        }
    }
    pub(crate) fn into_stored(self) -> Stored {
        match self {
            Self::Borrowed(value) => Stored::Str(value.into()),
            Self::Owned(value) => value,
        }
    }
}
impl<'a> From<Value> for Published<'a> {
    fn from(v: Value) -> Self {
        Self(Incoming::Owned(v.into()))
    }
}
impl<'a> From<bool> for Published<'a> {
    fn from(v: bool) -> Self {
        Self(Incoming::Owned(Stored::Bool(v)))
    }
}
impl<'a> From<&'a str> for Published<'a> {
    fn from(v: &'a str) -> Self {
        Self(Incoming::Borrowed(v))
    }
}
impl<'a> From<String> for Published<'a> {
    fn from(v: String) -> Self {
        Self(Incoming::Owned(Stored::Str(v)))
    }
}
macro_rules! numbers {
    ($($ty:ty),*) => {$(impl<'a> From<$ty> for Published<'a> {
        fn from(v: $ty) -> Self { Self(Incoming::Owned(Stored::Number(v as f64))) }
    })*};
}
numbers!(u8, u16, u32, i8, i16, i32, f32, f64);

pub(crate) fn quote(s: &str) -> String {
    let mut w = crate::json::Encoder::default();
    w.string(s);
    w.finish().unwrap()
}
fn number_json(out: &mut String, n: f64, rounded: bool) {
    if !n.is_finite() {
        out.push_str("null");
        return;
    }
    let start = out.len();
    let n = if rounded { crate::json::rounded(n) } else { n };
    crate::data::text::shortest(out, n, !rounded).unwrap();
    if out[start..].ends_with(".0") {
        out.truncate(out.len() - 2);
    }
}
pub(crate) fn value_json(v: &Value, rounded: bool) -> String {
    let mut out = String::new();
    append_value(&mut out, v, rounded);
    out
}
fn append_value(out: &mut String, v: &Value, rounded: bool) {
    match v {
        Value::Unit | Value::Option(None) => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => number_json(out, *n, rounded),
        s @ exact_plan::str_value!() => crate::json::quote_into(out, s.text()),
        Value::Option(Some(v)) => append_value(out, v, rounded),
        Value::List(v) | Value::Record(v) => {
            out.push('[');
            for (i, value) in v.iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                append_value(out, value, rounded);
            }
            out.push(']');
        }
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
        use crate::data::Bulk;
        let values = match value {
            Bulk::U8(v) => v.iter().map(|&n| Stored::Number(n as f64)).collect(),
            Bulk::U16(v) => v.iter().map(|&n| Stored::Number(n as f64)).collect(),
            Bulk::U32(v) => v.iter().map(|&n| Stored::Number(n as f64)).collect(),
            Bulk::F32(v) => v
                .iter()
                .map(|&n| Stored::Number(f32::from_bits(crate::data::f32_bits(n)) as f64))
                .collect(),
        };
        self.push(Stored::List(values));
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
            self.publish_value(&key, Incoming::Owned(value));
        }
    }
}
