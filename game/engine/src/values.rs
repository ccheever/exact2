use crate::{Data, DataError, Reader, Value, Writer};
use std::rc::Rc;

// Keep Value's variants in saves: Record and List have the same JSON shape but
// different Contract types. The message envelope deliberately strips these tags.
#[derive(Default, Data)]
enum Stored {
    #[default]
    Unit,
    Number(f64),
    Bool(bool),
    Str(String),
    Option(Option<Box<Value>>),
    List(Vec<Value>),
    Record(Vec<Value>),
}
impl Data for Value {
    fn write(&self, w: &mut dyn Writer) {
        let stored = match self {
            Self::Unit => Stored::Unit,
            Self::Number(n) => Stored::Number(*n),
            Self::Bool(b) => Stored::Bool(*b),
            Self::Str(s) => Stored::Str(s.to_string()),
            Self::Option(v) => Stored::Option(v.as_ref().map(|v| Box::new((**v).clone()))),
            Self::List(v) => Stored::List((**v).clone()),
            Self::Record(v) => Stored::Record((**v).clone()),
        };
        stored.write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let mut s = Stored::default();
        s.read(r)?;
        *self = match s {
            Stored::Unit => Self::Unit,
            Stored::Number(n) => Self::Number(n),
            Stored::Bool(b) => Self::Bool(b),
            Stored::Str(s) => {
                r.claim(s.len() + 2 * std::mem::size_of::<usize>())?;
                Self::str(&s)
            }
            Stored::Option(v) => {
                if v.is_some() {
                    r.claim(std::mem::size_of::<Value>() + 2 * std::mem::size_of::<usize>())?;
                }
                Self::Option(v.map(|v| Rc::new(*v)))
            }
            Stored::List(v) => {
                r.claim(std::mem::size_of::<Vec<Value>>() + 2 * std::mem::size_of::<usize>())?;
                Self::list(v)
            }
            Stored::Record(v) => {
                r.claim(std::mem::size_of::<Vec<Value>>() + 2 * std::mem::size_of::<usize>())?;
                Self::record(v)
            }
        };
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
