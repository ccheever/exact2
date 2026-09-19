//! Tagged little-endian values, unsigned/zigzag varints, and an incremental
//! name table. Unknown fields must still be walked to intern their names.
use super::limits::{Budget, MAX_LOAD_BYTES, MAX_LOAD_STRING};
use super::BulkKind;
use super::{f32_bits, f64_bits, Data, DataError, Number, Reader, Writer};
use std::collections::{BTreeMap, BTreeSet};

/// Encode one value, canonicalizing all NaNs while preserving negative zero.
pub fn to_vec<T: Data>(value: &T) -> Vec<u8> {
    let mut w = Encoder::default();
    value.write(&mut w);
    w.finish()
}
/// Read one value over its defaults, rejecting trailing input.
pub fn from_slice<T: Data>(bytes: &[u8]) -> Result<T, DataError> {
    let mut v = T::default();
    read_into(bytes, &mut v)?;
    Ok(v)
}
/// Read into an existing value using Data's patch/replacement rules.
pub fn read_into<T: Data>(bytes: &[u8], value: &mut T) -> Result<(), DataError> {
    let mut r = Decoder::new(bytes);
    value
        .read(&mut r)
        .and_then(|()| r.finish())
        .map_err(|e| e.at(super::type_name::<T>()))
}

/// A binary stream sink, usable for a world whose component types are erased.
#[derive(Default)]
pub struct Encoder {
    bytes: Vec<u8>,
    names: BTreeMap<String, u64>,
}
impl Encoder {
    /// Return the completed stream.
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
    fn var(&mut self, mut n: u64) {
        while n >= 128 {
            self.bytes.push(n as u8 | 128);
            n >>= 7;
        }
        self.bytes.push(n as u8);
    }
    fn text(&mut self, s: &str) {
        self.var(s.len() as u64);
        self.bytes.extend_from_slice(s.as_bytes());
    }
    fn name(&mut self, s: &str) {
        if let Some(&i) = self.names.get(s) {
            self.var(i + 1);
        } else {
            self.var(0);
            self.text(s);
            self.names.insert(s.into(), self.names.len() as u64);
        }
    }
}
impl Writer for Encoder {
    fn bytes(&mut self, kind: BulkKind, value: &[u8]) {
        self.bytes.push(12 + kind as u8);
        self.var(value.len() as u64);
        self.bytes.extend_from_slice(value);
    }
    fn boolean(&mut self, n: bool) {
        self.bytes.push(u8::from(n));
    }
    fn number(&mut self, n: Number) {
        match n {
            Number::Unsigned(n) => {
                self.bytes.push(2);
                self.var(n);
            }
            Number::Signed(n) => {
                self.bytes.push(3);
                self.var(((n as u64) << 1) ^ ((n >> 63) as u64));
            }
            Number::F32(n) => {
                self.bytes.push(4);
                self.bytes.extend_from_slice(&f32_bits(n).to_le_bytes());
            }
            Number::F64(n) => {
                self.bytes.push(5);
                self.bytes.extend_from_slice(&f64_bits(n).to_le_bytes());
            }
        }
    }
    fn string(&mut self, s: &str) {
        self.bytes.push(6);
        self.text(s);
    }
    fn begin_seq(&mut self, len: usize) {
        self.bytes.push(7);
        self.var(len as u64);
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {}
    fn begin_struct(&mut self) {
        self.bytes.push(8);
    }
    fn field(&mut self, name: &str) {
        self.bytes.push(1);
        self.name(name);
    }
    fn end_struct(&mut self) {
        self.bytes.push(0);
    }
    fn variant(&mut self, name: &str, index: u32) {
        self.bytes.push(9);
        self.var(index.into());
        self.name(name);
    }
    fn end_variant(&mut self) {}
    fn option(&mut self, some: bool) {
        self.bytes.push(if some { 11 } else { 10 });
    }
    fn end_option(&mut self) {}
}

/// A checked streaming cursor; nesting is bounded to protect untrusted saves.
pub struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    names: Vec<&'a str>,
    frames: Vec<Frame<'a>>,
    budget: Budget,
}
enum Frame<'a> {
    Seq(u64),
    Struct(BTreeSet<&'a str>),
    Variant,
    Option,
}
impl<'a> Decoder<'a> {
    /// Read a single stream, starting with an empty name table.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            names: vec![],
            frames: vec![],
            budget: Budget::default(),
        }
    }
    /// Check that the caller consumed the entire stream.
    pub fn finish(&self) -> Result<(), DataError> {
        if self.pos == self.bytes.len() && self.frames.is_empty() {
            Ok(())
        } else {
            Err(self.err("trailing or unfinished data"))
        }
    }
    fn err(&self, s: &str) -> DataError {
        DataError::new(format!("{s} at byte {}", self.pos))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], DataError> {
        if self.bytes.len() > MAX_LOAD_BYTES {
            return Err(self.err("input exceeds load size limit"));
        }
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| self.err("length overflow"))?;
        let b = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| self.err("truncated value"))?;
        self.pos = end;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8, DataError> {
        Ok(self.take(1)?[0])
    }
    fn tag(&mut self, tag: u8, what: &str) -> Result<(), DataError> {
        if self.byte()? == tag {
            Ok(())
        } else {
            Err(self.err(what))
        }
    }
    fn var(&mut self) -> Result<u64, DataError> {
        let mut n = 0;
        for shift in (0..70).step_by(7) {
            let b = self.byte()?;
            if shift == 63 && b > 1 {
                return Err(self.err("varint overflow"));
            }
            n |= u64::from(b & 127) << shift;
            if b < 128 {
                return Ok(n);
            }
        }
        Err(self.err("varint overflow"))
    }
    fn text(&mut self) -> Result<&'a str, DataError> {
        let len = usize::try_from(self.var()?).map_err(|_| self.err("length overflow"))?;
        if len > MAX_LOAD_STRING {
            return Err(self.err("string exceeds load limit"));
        }
        let b = self.take(len)?;
        std::str::from_utf8(b).map_err(|_| self.err("invalid UTF-8"))
    }
    fn name(&mut self) -> Result<&'a str, DataError> {
        let n = self.var()?;
        if n == 0 {
            let s = self.text()?;
            self.budget.reserve(&mut self.names)?;
            self.names.push(s);
            Ok(s)
        } else {
            self.names
                .get(usize::try_from(n - 1).map_err(|_| self.err("name overflow"))?)
                .copied()
                .ok_or_else(|| self.err("unknown name index"))
        }
    }
    fn push(&mut self, f: Frame<'a>) -> Result<(), DataError> {
        if self.frames.len() >= 256 {
            return Err(self.err("nesting exceeds 256"));
        }
        self.budget.reserve(&mut self.frames)?;
        self.frames.push(f);
        Ok(())
    }
}
impl Reader for Decoder<'_> {
    fn bytes(&mut self, kind: BulkKind) -> Result<Vec<u8>, DataError> {
        let tag = self.byte()?;
        if tag != 12 + kind as u8 {
            let seen = match tag {
                12 => "u8",
                13 => "u16",
                14 => "u32",
                15 => "f32",
                _ => "non-bulk value",
            };
            return Err(self.err(&format!("expected bulk {}; found {seen}", kind.name())));
        }
        let len = usize::try_from(self.var()?).map_err(|_| self.err("length overflow"))?;
        let bytes = self.take(len)?;
        self.claim(len)?;
        let mut out = Vec::new();
        out.try_reserve_exact(len)
            .map_err(super::limits::allocation)?;
        out.extend_from_slice(bytes);
        Ok(out)
    }
    fn claim(&mut self, bytes: usize) -> Result<(), DataError> {
        self.budget.claim(bytes)
    }
    fn sequence_len(&self) -> Option<usize> {
        match self.frames.last() {
            Some(Frame::Seq(n)) => usize::try_from(*n).ok(),
            _ => None,
        }
    }
    fn boolean(&mut self) -> Result<bool, DataError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.err("expected a boolean")),
        }
    }
    fn number(&mut self) -> Result<Number, DataError> {
        Ok(match self.byte()? {
            2 => Number::Unsigned(self.var()?),
            3 => {
                let n = self.var()?;
                Number::Signed(((n >> 1) as i64) ^ -((n & 1) as i64))
            }
            4 => {
                let n = f32::from_le_bytes(
                    self.take(4)?
                        .try_into()
                        .map_err(|_| self.err("invalid f32 bytes"))?,
                );
                Number::F32(f32::from_bits(f32_bits(n)))
            }
            5 => {
                let n = f64::from_le_bytes(
                    self.take(8)?
                        .try_into()
                        .map_err(|_| self.err("invalid f64 bytes"))?,
                );
                Number::F64(f64::from_bits(f64_bits(n)))
            }
            _ => return Err(self.err("expected a number")),
        })
    }
    fn string(&mut self) -> Result<String, DataError> {
        self.tag(6, "expected a string")?;
        let text = self.text()?;
        self.budget.text(text)
    }
    fn begin_seq(&mut self) -> Result<(), DataError> {
        self.tag(7, "expected a sequence")?;
        let n = self.var()?;
        if n > MAX_LOAD_BYTES as u64 || n > (self.bytes.len() - self.pos) as u64 {
            return Err(self.err("sequence count exceeds load limit or remaining input"));
        }
        self.push(Frame::Seq(n))
    }
    fn item(&mut self) -> Result<bool, DataError> {
        match self.frames.last_mut() {
            Some(Frame::Seq(0)) => {
                self.frames.pop();
                Ok(false)
            }
            Some(Frame::Seq(n)) => {
                *n -= 1;
                Ok(true)
            }
            _ => Err(self.err("not inside a sequence")),
        }
    }
    fn begin_struct(&mut self) -> Result<(), DataError> {
        self.tag(8, "expected a record")?;
        self.push(Frame::Struct(BTreeSet::new()))
    }
    fn field(&mut self) -> Result<Option<String>, DataError> {
        if !matches!(self.frames.last(), Some(Frame::Struct(_))) {
            return Err(self.err("not inside a record"));
        }
        match self.byte()? {
            0 => {
                self.frames.pop();
                Ok(None)
            }
            1 => {
                let name = self.name()?;
                let Some(Frame::Struct(seen)) = self.frames.last_mut() else {
                    unreachable!()
                };
                if seen.contains(name) {
                    return Err(DataError::new("duplicate field").at(name));
                }
                self.budget.claim(64)?;
                seen.insert(name);
                Ok(Some(self.budget.text(name)?))
            }
            _ => Err(self.err("expected a field")),
        }
    }
    fn variant(&mut self) -> Result<String, DataError> {
        self.tag(9, "expected an enum")?;
        self.var()?;
        let name = self.name()?;
        self.push(Frame::Variant)?;
        self.budget.text(name)
    }
    fn end_variant(&mut self) -> Result<(), DataError> {
        if matches!(self.frames.pop(), Some(Frame::Variant)) {
            Ok(())
        } else {
            Err(self.err("not inside an enum"))
        }
    }
    fn option(&mut self) -> Result<bool, DataError> {
        let some = match self.byte()? {
            10 => false,
            11 => true,
            _ => return Err(self.err("expected an option")),
        };
        self.push(Frame::Option)?;
        Ok(some)
    }
    fn end_option(&mut self) -> Result<(), DataError> {
        if matches!(self.frames.pop(), Some(Frame::Option)) {
            Ok(())
        } else {
            Err(self.err("not inside an option"))
        }
    }
    fn skip(&mut self) -> Result<(), DataError> {
        match self.bytes.get(self.pos) {
            Some(0 | 1) => {
                self.boolean()?;
            }
            Some(2..=5) => {
                self.number()?;
            }
            Some(6) => {
                self.string()?;
            }
            Some(12..=15) => {
                self.byte()?;
                let len = usize::try_from(self.var()?).map_err(|_| self.err("length overflow"))?;
                self.take(len)?;
            }
            Some(7) => {
                self.begin_seq()?;
                while self.item()? {
                    self.skip()?;
                }
            }
            Some(8) => {
                self.begin_struct()?;
                while self.field()?.is_some() {
                    self.skip()?;
                }
            }
            Some(9) => {
                self.variant()?;
                self.skip()?;
                self.end_variant()?;
            }
            Some(10 | 11) => {
                if self.option()? {
                    self.skip()?;
                }
                self.end_option()?;
            }
            _ => return Err(self.err("unknown or missing value tag")),
        }
        Ok(())
    }
}
