//! Streaming codecs share one walk over a value; no codec builds a value tree.

use std::fmt;

pub mod bin;
pub mod hash;
mod impls;
pub(crate) mod limits;
pub use limits::{MAX_LOAD_BYTES, MAX_LOAD_ENTITIES, MAX_LOAD_STRING};
pub mod json;

/// State that can survive a save, level load, or code reload.
///
/// Semantic state has no interior mutability; derives introduce none. A manual
/// implementation that changes semantic state through a shared reference is outside
/// this contract: quiescence and the hash cache are undefined for it.
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// struct Mutable { value: std::cell::Cell<u32> }
/// ```
///
/// Records keep fields the input lacks; sequences, maps and options are replaced whole.
///
/// Array implementations require the array itself to implement Default. Rust
/// currently supplies that only through length 32; Vec supports arbitrary lengths.
/// Platform-sized integers and unordered maps intentionally have no implementation.
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// struct Generic<T>(T);
/// ```
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// struct Borrowed<'a> { text: &'a str }
/// ```
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// struct PlatformSized { count: usize }
/// ```
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// struct Unordered { entries: std::collections::HashMap<String, u32> }
/// ```
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// #[data(skip)]
/// struct TypeAttribute { score: u32 }
/// ```
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// enum VariantAttribute { #[default] #[data(skip)] A }
/// ```
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// struct UnknownAttribute { #[data(typo)] score: u32 }
/// ```
///
/// ```compile_fail
/// use exact_game::Data;
/// #[derive(Default, Data)]
/// enum Discriminants { #[default] A = 1, B = 2 }
/// ```
pub trait Data: Sized + Default + 'static {
    /// Whether any nested spring is still moving. Derives walk non-transient fields.
    fn moving(&self, _now: crate::Now) -> bool {
        false
    }
    /// First resting world tick, or None for unsettled work without a deadline.
    /// Derives combine nested deadlines; this is inspected only on agent reads.
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        if self.moving(now) {
            None
        } else {
            Some(now.tick)
        }
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
    /// An unsigned integer, encoded as a varint.
    Unsigned(u64),
    /// A signed integer, encoded as a zigzag varint.
    Signed(i64),
    /// IEEE binary32.
    F32(f32),
    /// IEEE binary64.
    F64(f64),
}

/// Element kind of a little-endian bulk payload; part of both wire and hash framing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BulkKind {
    /// Bytes.
    U8,
    /// Unsigned 16-bit integers.
    U16,
    /// Unsigned 32-bit integers.
    U32,
    /// Canonical IEEE binary32 values.
    F32,
}
impl BulkKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::F32 => "f32",
        }
    }
}

/// An object-safe sink. Fallible sinks remember their first failure until finish.
pub trait Writer {
    /// Write a boolean.
    fn boolean(&mut self, value: bool);
    /// Write a number, canonicalizing NaNs in binary representations.
    fn number(&mut self, value: Number);
    /// Write a typed length-prefixed byte payload; JSON emits an inspection summary.
    fn bytes(&mut self, kind: BulkKind, value: &[u8]);
    /// Write a UTF-8 value (as opposed to a field name).
    fn string(&mut self, value: &str);
    /// Begin a sequence of exactly `len` elements.
    fn begin_seq(&mut self, len: usize);
    /// Separate sequence elements, including the first.
    fn item(&mut self);
    /// End a sequence.
    fn end_seq(&mut self);
    /// Begin a record whose fields are named in self-describing codecs.
    fn begin_struct(&mut self);
    /// Introduce the following field value.
    fn field(&mut self, name: &str);
    /// A map key is a value and therefore participates in the hash.
    fn key(&mut self, name: &str) {
        self.field(name);
    }
    /// End a record.
    fn end_struct(&mut self);
    /// Begin an enum arm; names survive schema changes, indices enter the hash.
    fn variant(&mut self, name: &str, index: u32);
    /// End the enum arm's single payload (record or sequence).
    fn end_variant(&mut self);
    /// Begin an option, followed by one value when present.
    fn option(&mut self, some: bool);
    /// End an option.
    fn end_option(&mut self);
}

/// An object-safe cursor. End markers are consumed by `item` and `field`.
pub trait Reader {
    /// Account decoded allocations before reserving input-controlled storage.
    fn claim(&mut self, _bytes: usize) -> Result<(), DataError> {
        Ok(())
    }
    /// Remaining sequence count, if the format declares it in advance.
    fn sequence_len(&self) -> Option<usize> {
        None
    }
    /// Read a boolean.
    fn boolean(&mut self) -> Result<bool, DataError>;
    /// Read a number without losing integer precision.
    fn number(&mut self) -> Result<Number, DataError>;
    /// Read a matching typed payload, claiming its byte size before reserving.
    /// This claim also covers conversion into a same-sized numeric destination.
    fn bytes(&mut self, kind: BulkKind) -> Result<Vec<u8>, DataError>;
    /// Read UTF-8 text.
    fn string(&mut self) -> Result<String, DataError>;
    /// Enter a sequence.
    fn begin_seq(&mut self) -> Result<(), DataError>;
    /// Begin the next element, or consume the end and return false.
    fn item(&mut self) -> Result<bool, DataError>;
    /// Enter a named record.
    fn begin_struct(&mut self) -> Result<(), DataError>;
    /// Read the next field name, or consume the record end.
    fn field(&mut self) -> Result<Option<String>, DataError>;
    /// Enter an enum payload and return its arm name.
    fn variant(&mut self) -> Result<String, DataError>;
    /// Consume the enum end.
    fn end_variant(&mut self) -> Result<(), DataError>;
    /// Enter an option and report whether a value follows.
    fn option(&mut self) -> Result<bool, DataError>;
    /// Consume the option end.
    fn end_option(&mut self) -> Result<(), DataError>;
    /// Whether authoring requires exact fields and tuple/array lengths.
    fn strict(&self) -> bool {
        false
    }
    /// An unknown record field or tuple element; saves skip it, authoring refuses it.
    fn unknown(&mut self) -> Result<(), DataError> {
        if self.strict() {
            Err(DataError::new("unknown field or excess element"))
        } else {
            self.skip()
        }
    }
    /// Discard one complete value, including names interned within it.
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
