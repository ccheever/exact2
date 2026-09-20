use crate::{bin, Data, DataError, Reader};

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
    pub(crate) fn read_bounded(
        r: &mut bin::Decoder<'_>,
        budget: &mut PublicationSize,
        depth: usize,
    ) -> Result<Self, DataError> {
        budget.add(1, 0)?;
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
                    budget.add(0, s.len())?;
                    r.claim(s.len())?;
                    let mut text = String::new();
                    text.try_reserve_exact(s.len())
                        .map_err(crate::data::limits::allocation)?;
                    text.push_str(s);
                    Self::Str(text)
                }
                "Option" => {
                    let v = if r.option()? {
                        {
                            r.claim(std::mem::size_of::<Self>())?;
                            Some(crate::storage::boxed(Self::read_bounded(
                                r,
                                budget,
                                depth + 1,
                            )?)?)
                        }
                    } else {
                        None
                    };
                    r.end_option()?;
                    Self::Option(v)
                }
                "List" | "Record" => {
                    r.begin_seq()?;
                    let n = r.sequence_len().unwrap_or(0);
                    if n > 65_536 - budget.nodes {
                        return Err(DataError::new("publication child limit"));
                    }
                    r.claim(
                        n.checked_mul(std::mem::size_of::<Self>())
                            .ok_or_else(|| DataError::new("publication allocation overflow"))?,
                    )?;
                    let mut v = Vec::new();
                    v.try_reserve_exact(n)
                        .map_err(crate::data::limits::allocation)?;
                    while r.item()? {
                        v.push(Self::read_bounded(r, budget, depth + 1)?);
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
                        budget.add(0, k.len())?;
                        r.claim(crate::data::limits::map_bytes::<String, Self>() + k.len())?;
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
    pub(crate) fn validate(
        &self,
        size: &mut PublicationSize,
        depth: usize,
    ) -> Result<(), DataError> {
        size.add(1, if let Self::Str(s) = self { s.len() } else { 0 })?;
        if depth > 80 {
            return Err(DataError::new("publication depth limit"));
        }
        match self {
            Self::Object(fields) => {
                for (k, v) in fields {
                    size.add(0, k.len())?;
                    v.validate(size, depth + 1)?;
                }
            }
            Self::List(items) | Self::Record(items) => {
                for v in items {
                    v.validate(size, depth + 1)?;
                }
            }
            Self::Option(Some(v)) => v.validate(size, depth + 1)?,
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

/// Structural publication caps, independent of the resident-byte load budget.
#[derive(Clone, Copy, Default)]
pub(crate) struct PublicationSize {
    pub nodes: usize,
    pub text: usize,
}
impl PublicationSize {
    pub fn add(&mut self, nodes: usize, text: usize) -> Result<(), DataError> {
        if nodes > 65_536 - self.nodes || text > 65_536 - self.text {
            return Err(DataError::new(
                "publication exceeds 65536 nodes or string bytes",
            ));
        }
        self.nodes += nodes;
        self.text += text;
        Ok(())
    }
    pub fn replace(&mut self, old: Self, new: Self) -> Result<(), DataError> {
        self.nodes -= old.nodes;
        self.text -= old.text;
        self.add(new.nodes, new.text)
    }
}
