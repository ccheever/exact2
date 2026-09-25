//! Bounds-checked little-endian reading and writing for the plan codec.
//!
//! Every read reports the exact shortfall; nothing indexes a slice it has not
//! measured first. The kernel has its own copy of this idea for EXWF; the two
//! crates share no code by design (`exact-plan` depends on nothing).

use crate::PlanError;

/// Most rows, strings, or pool bytes one count may announce; a hostile count
/// is refused before anything is allocated from it.
pub const MAX_COUNT: usize = 1 << 24;

/// Most elements reserved up front from an announced count; a larger count
/// grows the vector as elements actually decode, so a short payload cannot
/// make the decoder reserve more than it will ever fill.
pub const RESERVE: usize = 1024;

/// A cursor over a byte slice.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Start at byte 0.
    pub fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    /// Bytes not yet read.
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// Current offset.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Whether everything has been read.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Take `n` bytes.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], PlanError> {
        if self.remaining() < n {
            return Err(PlanError::Truncated {
                needed: n,
                available: self.remaining(),
            });
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    /// A count that will be allocated from: bounded before it is trusted.
    pub fn count(&mut self) -> Result<usize, PlanError> {
        let n = self.u32()? as usize;
        if n > MAX_COUNT || n > self.remaining() {
            return Err(PlanError::BadCount(n as u32));
        }
        Ok(n)
    }

    /// One byte.
    pub fn u8(&mut self) -> Result<u8, PlanError> {
        Ok(self.bytes(1)?[0])
    }

    /// Little-endian u16.
    pub fn u16(&mut self) -> Result<u16, PlanError> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    /// Little-endian u32.
    pub fn u32(&mut self) -> Result<u32, PlanError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    /// Little-endian i32.
    pub fn i32(&mut self) -> Result<i32, PlanError> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    /// Little-endian u64.
    pub fn u64(&mut self) -> Result<u64, PlanError> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    /// Little-endian f64.
    pub fn f64(&mut self) -> Result<f64, PlanError> {
        Ok(f64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    /// A length-prefixed UTF-8 string.
    pub fn string(&mut self) -> Result<String, PlanError> {
        let n = self.count()?;
        let bytes = self.bytes(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| PlanError::BadUtf8)
    }
}

/// A reader that keeps its first error. Every read after it gives zero and
/// reads nothing, so a decoder reads whole rows with no branch per field,
/// then [`Sticky::finish`] returns the error the first failing read (or
/// [`Sticky::fail`]) would have returned at once.
#[derive(Debug)]
pub struct Sticky<'a> {
    r: Reader<'a>,
    error: Option<PlanError>,
}

impl<'a> Sticky<'a> {
    /// Continue from `r`.
    pub fn new(r: Reader<'a>) -> Self {
        Sticky { r, error: None }
    }

    /// Whether a read has failed.
    pub fn failed(&self) -> bool {
        self.error.is_some()
    }

    /// Record `error` unless one came first, and go on with `placeholder`.
    pub fn fail<T>(&mut self, error: PlanError, placeholder: T) -> T {
        self.error.get_or_insert(error);
        placeholder
    }

    fn read<T: Default>(
        &mut self,
        read: impl FnOnce(&mut Reader<'a>) -> Result<T, PlanError>,
    ) -> T {
        if self.error.is_some() {
            return T::default();
        }
        read(&mut self.r).unwrap_or_else(|error| self.fail(error, T::default()))
    }

    /// [`Reader::count`]; zero after a failure.
    #[inline(never)]
    pub fn count(&mut self) -> usize {
        self.read(Reader::count)
    }

    /// [`Reader::u8`].
    #[inline(never)]
    pub fn u8(&mut self) -> u8 {
        self.read(Reader::u8)
    }

    /// [`Reader::u16`].
    #[inline(never)]
    pub fn u16(&mut self) -> u16 {
        self.read(Reader::u16)
    }

    /// [`Reader::u32`].
    #[inline(never)]
    pub fn u32(&mut self) -> u32 {
        self.read(Reader::u32)
    }

    /// [`Reader::i32`].
    #[inline(never)]
    pub fn i32(&mut self) -> i32 {
        self.read(Reader::i32)
    }

    /// [`Reader::f64`].
    #[inline(never)]
    pub fn f64(&mut self) -> f64 {
        self.read(Reader::f64)
    }

    /// The reader, or the first error.
    pub fn finish(self) -> Result<Reader<'a>, PlanError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.r),
        }
    }
}

/// A growable little-endian byte buffer.
#[derive(Debug, Default, Clone)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// The bytes.
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Raw bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// One byte.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Little-endian u16.
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Little-endian u32.
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Little-endian i32.
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Little-endian u64.
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Little-endian f64.
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A length-prefixed UTF-8 string.
    pub fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.bytes(s.as_bytes());
    }
}
