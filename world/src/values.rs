use crate::{Data, DataError};

/// Kernel publication Data. List and positional Record retain distinct saved tags;
/// named Object fields remain names until an external adapter supplies a shape.
#[derive(Debug, Clone, Default, PartialEq, Data)]
pub enum Published {
    #[default]
    Unit,
    Number(f64),
    Bool(bool),
    Str(String),
    Option(Option<Box<Published>>),
    List(Vec<Published>),
    Record(Vec<Published>),
    Object(std::collections::BTreeMap<String, Published>),
}
impl Published {
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
}

impl From<bool> for Published {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<&str> for Published {
    fn from(v: &str) -> Self {
        Self::Str(v.into())
    }
}
impl From<String> for Published {
    fn from(v: String) -> Self {
        Self::Str(v)
    }
}
macro_rules! numbers {
    ($($ty:ty),*) => {$(impl From<$ty> for Published {
        fn from(v: $ty) -> Self { Self::Number(v as f64) }
    })*};
}
numbers!(u8, u16, u32, i8, i16, i32, f32, f64);
