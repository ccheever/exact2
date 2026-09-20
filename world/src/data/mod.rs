//! Streaming codecs share one walk over a value; no codec builds a value tree.

use std::fmt;

pub mod bin;
pub mod hash;
mod impls;
pub(crate) mod limits;
pub use limits::LoadBudget;
pub use limits::{MAX_LOAD_BYTES, MAX_LOAD_ENTITIES, MAX_LOAD_STRING};
pub mod json;
mod text;

/// State that can survive a save, level load, or code reload.
///
/// Semantic state has no interior mutability; derives introduce none. A manual
/// implementation that changes semantic state through a shared reference is outside
/// this contract: quiescence and the hash cache are undefined for it.
///
/// Records keep fields the input lacks; sequences, maps and options are replaced whole.
///
/// Array implementations require the array itself to implement Default. Rust
/// currently supplies that only through length 32; Vec supports arbitrary lengths.
/// Platform-sized integers and unordered maps intentionally have no implementation.
///
pub trait Data: Sized + Default + 'static {
    /// Construction-oriented decode. Allocation-free defaults may use this
    /// fallback; allocating manual defaults must override and claim first.
    fn default_size() -> usize {
        std::mem::size_of::<Self>()
    }
    fn read_new(r: &mut dyn Reader) -> Result<Self, DataError> {
        r.check_allocation(Self::default_size().max(1))?;
        r.claim(Self::default_size().saturating_sub(std::mem::size_of::<Self>()))?;
        let mut value = Self::default();
        value.read(r)?;
        Ok(value)
    }
    /// First resting world tick, or None for unsettled work without a deadline.
    /// Derives combine nested deadlines; this is inspected only on agent reads.
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        Some(now.tick)
    }
    /// Write fields in declaration order, omitting transient fields.
    fn write(&self, w: &mut dyn Writer);
    /// Read according to the record-patch and container-replacement rule above.
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError>;
}

/// A codec failure with a path from the root value to the offending field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataError {
    /// Field path, or the empty string before a caller attaches context.
    pub path: String,
    /// What the input failed to satisfy.
    pub message: String,
}
impl DataError {
    /// Describe a failure at the current value.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            message: message.into(),
        }
    }
    /// Prepend a field, type, or sequence index to the path.
    pub fn at(mut self, path: impl fmt::Display) -> Self {
        self.path = if self.path.is_empty() {
            path.to_string()
        } else {
            format!("{path}.{}", self.path)
        };
        self
    }
}
impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}
impl std::error::Error for DataError {}

/// Numeric widths that affect the binary and hash representations.
#[derive(Clone, Copy, Debug)]
pub enum Number {
    Unsigned(u64),
    Signed(i64),
    F32(f32),
    F64(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BulkKind {
    U8,
    U16,
    U32,
    F32,
}
impl BulkKind {
    pub(crate) fn name(self) -> &'static str {
        ["u8", "u16", "u32", "f32"][self as usize]
    }
}

pub trait Writer {
    /// Conservative decode-allocation admission; hashes and inspection ignore it.
    fn claim_decoded(&mut self, _bytes: usize) {}
    fn stopped(&self) -> bool {
        false
    }
    fn entity(&mut self, index: u32, generation: u32) {
        self.begin_struct();
        self.field("index");
        self.number(Number::Unsigned(index.into()));
        self.field("generation");
        self.number(Number::Unsigned(generation.into()));
        self.end_struct();
    }
    fn unit(&mut self) {
        self.begin_seq(0);
        self.end_seq();
    }
    fn boolean(&mut self, value: bool);
    fn number(&mut self, value: Number);
    fn bytes(&mut self, value: Bulk<'_>);
    fn string(&mut self, value: &str);
    fn begin_seq(&mut self, len: usize);
    fn item(&mut self);
    fn end_seq(&mut self);
    fn begin_struct(&mut self);
    fn field(&mut self, name: &'static str);
    fn key(&mut self, name: &str);
    fn end_struct(&mut self);
    fn variant(&mut self, name: &'static str, index: u32);
    fn end_variant(&mut self);
    fn option(&mut self, some: bool);
    fn end_option(&mut self);
}

pub trait Reader<'data> {
    fn check_allocation(&self, _bytes: usize) -> Result<(), DataError> {
        Ok(())
    }
    fn claim(&mut self, _bytes: usize) -> Result<(), DataError> {
        Ok(())
    }
    fn sequence_len(&self) -> Option<usize> {
        None
    }
    fn boolean(&mut self) -> Result<bool, DataError>;
    fn number(&mut self) -> Result<Number, DataError>;
    fn f32(&mut self) -> Result<f32, DataError> {
        Ok(match self.number()? {
            Number::Unsigned(n) => n as f32,
            Number::Signed(n) => n as f32,
            Number::F32(n) => n,
            Number::F64(n) => {
                let small = n as f32;
                if n.is_finite() && !small.is_finite() {
                    return Err(DataError::new("number outside f32 range"));
                }
                small
            }
        })
    }
    fn f64(&mut self) -> Result<f64, DataError> {
        Ok(match self.number()? {
            Number::Unsigned(n) => n as f64,
            Number::Signed(n) => n as f64,
            Number::F32(n) => n as f64,
            Number::F64(n) => n,
        })
    }
    fn bytes(&mut self, kind: BulkKind) -> Result<Vec<u8>, DataError>;
    fn string(&mut self) -> Result<String, DataError>;
    fn begin_seq(&mut self) -> Result<(), DataError>;
    fn item(&mut self) -> Result<bool, DataError>;
    fn required_item(&mut self, message: &str) -> Result<(), DataError> {
        self.item()?
            .then_some(())
            .ok_or_else(|| DataError::new(message))
    }
    fn begin_struct(&mut self) -> Result<(), DataError>;
    fn field(&mut self) -> Result<Option<&'data str>, DataError>;
    fn variant(&mut self) -> Result<&'data str, DataError>;
    fn end_variant(&mut self) -> Result<(), DataError>;
    fn option(&mut self) -> Result<bool, DataError>;
    fn end_option(&mut self) -> Result<(), DataError>;
    fn unknown(&mut self) -> Result<(), DataError> {
        self.skip()
    }
    fn skip(&mut self) -> Result<(), DataError>;
}

pub(crate) fn f32_bits(n: f32) -> u32 {
    if n.is_nan() {
        0x7fc0_0000
    } else {
        n.to_bits()
    }
}
pub(crate) fn f64_bits(n: f64) -> u64 {
    if n.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        n.to_bits()
    }
}
pub(crate) fn type_name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap()
}

/// Borrowed numeric payload; sinks stream canonical little-endian chunks.
#[derive(Clone, Copy)]
pub enum Bulk<'a> {
    U8(&'a [u8]),
    U16(&'a [u16]),
    U32(&'a [u32]),
    F32(&'a [f32]),
}
impl<'a> Bulk<'a> {
    pub fn shape(self) -> (BulkKind, usize) {
        match self {
            Self::U8(v) => (BulkKind::U8, v.len()),
            Self::U16(v) => (BulkKind::U16, v.len() * 2),
            Self::U32(v) => (BulkKind::U32, v.len() * 4),
            Self::F32(v) => (BulkKind::F32, v.len() * 4),
        }
    }
    /// At most 1024 stack bytes; false stops before the next chunk.
    pub fn chunks(self, mut sink: impl FnMut(&[u8]) -> bool) {
        let mut buffer = [0u8; 1024];
        macro_rules! chunks {
            ($v:expr, $width:expr, $convert:expr) => {
                for part in $v.chunks(1024 / $width) {
                    for (dst, value) in buffer.chunks_exact_mut($width).zip(part) {
                        dst.copy_from_slice(&$convert(value));
                    }
                    if !sink(&buffer[..part.len() * $width]) {
                        break;
                    }
                }
            };
        }
        match self {
            Self::U8(v) => {
                sink(v);
            }
            Self::U16(v) => chunks!(v, 2, |v: &u16| v.to_le_bytes()),
            Self::U32(v) => chunks!(v, 4, |v: &u32| v.to_le_bytes()),
            Self::F32(v) => chunks!(v, 4, |v: &f32| f32_bits(*v).to_le_bytes()),
        }
    }
    pub fn numbers(self) -> impl Iterator<Item = f64> + 'a {
        let (kind, bytes) = self.shape();
        (0..bytes / [1, 2, 4, 4][kind as usize]).map(move |i| match self {
            Self::U8(v) => v[i] as f64,
            Self::U16(v) => v[i] as f64,
            Self::U32(v) => v[i] as f64,
            Self::F32(v) => v[i] as f64,
        })
    }
}
