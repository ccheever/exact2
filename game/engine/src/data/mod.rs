//! Streaming codecs share one walk over a value; no codec builds a value tree.

use std::fmt;

pub mod bin;
pub mod hash;
mod impls;
pub mod json;

/// State that can survive a save, level load, or code reload.
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
pub trait Data: Sized + Default + 'static {
    /// Write fields in declaration order, omitting transient fields.
    fn write(&self, w: &mut dyn Writer);
    /// Overwrite present fields; missing fields retain their current values.
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

/// An object-safe sink. Fallible sinks remember their first failure until finish.
pub trait Writer {
    /// Write a boolean.
    fn boolean(&mut self, value: bool);
    /// Write a number, canonicalizing NaNs in binary representations.
    fn number(&mut self, value: Number);
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
    /// Read a boolean.
    fn boolean(&mut self) -> Result<bool, DataError>;
    /// Read a number without losing integer precision.
    fn number(&mut self) -> Result<Number, DataError>;
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
