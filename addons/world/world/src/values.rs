use crate::{bin, Data, DataError, Reader};

/// Kernel publication Data. List and positional Record retain distinct saved tags;
/// named Object fields remain names until an external adapter supplies a shape.
#[derive(Debug, Clone, Default, Data)]
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
impl PartialEq for Published {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unit, Self::Unit) => true,
            (Self::Number(a), Self::Number(b)) => a.to_bits() == b.to_bits(),
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Option(a), Self::Option(b)) => a == b,
            (Self::List(a), Self::List(b)) | (Self::Record(a), Self::Record(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            _ => false,
        }
    }
}
impl Published {
    pub(crate) fn read_bounded(
        r: &mut bin::Decoder<'_>,
        budget: &mut usize,
        depth: usize,
    ) -> Result<Self, DataError> {
        charge(budget, 64)?;
        Self::read_reserved(r, budget, depth)
    }
    fn read_reserved(
        r: &mut bin::Decoder<'_>,
        budget: &mut usize,
        depth: usize,
    ) -> Result<Self, DataError> {
        if depth > 80 {
            return Err(DataError::new("publication depth limit (80)"));
        }
        let arm = r.variant()?;
        let value = if arm == "Unit" {
            r.begin_struct()?;
            if r.field()?.is_some() {
                return Err(DataError::new("invalid Unit publication"));
            }
            Self::Unit
        } else {
            r.begin_seq()?;
            r.required_item("missing publication payload")?;
            let value = match arm {
                "Number" => {
                    let n = r.f64()?;
                    if !n.is_finite() {
                        return Err(DataError::new("non-finite publication"));
                    }
                    Self::Number(n)
                }
                "Bool" => Self::Bool(r.boolean()?),
                "Str" => {
                    let s = r.borrowed_string()?;
                    charge(budget, s.len().saturating_mul(6))?;
                    r.claim(s.len())?;
                    Self::Str(s.into())
                }
                "Option" => {
                    let v = if r.option()? {
                        Some(Box::new(Self::read_bounded(r, budget, depth + 1)?))
                    } else {
                        None
                    };
                    r.end_option()?;
                    Self::Option(v)
                }
                "List" | "Record" => {
                    r.begin_seq()?;
                    let n = r.sequence_len().unwrap_or(0);
                    if n > *budget / 64 {
                        return Err(DataError::new("publication child limit"));
                    }
                    charge(budget, n * 64)?;
                    r.claim(n * 64)?;
                    let mut v = Vec::with_capacity(n);
                    while r.item()? {
                        v.push(Self::read_reserved(r, budget, depth + 1)?);
                    }
                    if arm == "List" {
                        Self::List(v)
                    } else {
                        Self::Record(v)
                    }
                }
                "Object" => {
                    r.begin_struct()?;
                    let mut v = std::collections::BTreeMap::new();
                    while let Some(k) = r.field()? {
                        charge(budget, k.len().saturating_mul(6))?;
                        r.claim(64usize.saturating_add(k.len()))?;
                        let value = Self::read_bounded(r, budget, depth + 1)?;
                        v.insert(k.into(), value);
                    }
                    Self::Object(v)
                }
                _ => return Err(DataError::new("unknown publication variant")),
            };
            if r.item()? {
                return Err(DataError::new("extra publication payload"));
            }
            value
        };
        r.end_variant()?;
        Ok(value)
    }
    pub(crate) fn validate(&self, remaining: &mut usize, depth: usize) -> Result<(), DataError> {
        let cost = match self {
            Self::Str(s) => 64usize.saturating_add(s.len().saturating_mul(6)),
            _ => 64,
        };
        *remaining = remaining
            .checked_sub(cost)
            .ok_or_else(|| DataError::new("publication exceeds 65536 bytes/visits"))?;
        if depth > 80 {
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

pub(crate) fn charge(budget: &mut usize, cost: usize) -> Result<(), DataError> {
    *budget = budget
        .checked_sub(cost)
        .ok_or_else(|| DataError::new("publication exceeds 65536 bytes/visits"))?;
    Ok(())
}
