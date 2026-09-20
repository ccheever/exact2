//! Tagged little-endian values, unsigned/zigzag varints, and an incremental
//! name table. Unknown fields must still be walked to intern their names.
use super::limits::{Budget, LoadBudget, MAX_LOAD_BYTES, MAX_LOAD_STRING};
use super::BulkKind;
use super::{f32_bits, f64_bits, Data, DataError, Number, Reader, Writer};
use std::collections::BTreeMap;

pub fn to_vec<T: Data>(value: &T) -> Result<Vec<u8>, DataError> {
    let mut w = Encoder::default();
    value.write(&mut w);
    w.finish()
}
pub fn from_slice<T: Data>(bytes: &[u8]) -> Result<T, DataError> {
    from_slice_in(bytes, None)
}
pub fn from_slice_in<T: Data>(bytes: &[u8], budget: Option<&LoadBudget>) -> Result<T, DataError> {
    let mut r = Decoder::for_load(bytes, budget);
    let value = T::read_new(&mut r).map_err(|e| e.at(super::type_name::<T>()))?;
    r.finish()?;
    Ok(value)
}
pub fn read_into<T: Data>(bytes: &[u8], value: &mut T) -> Result<(), DataError> {
    let budget = LoadBudget::new(MAX_LOAD_BYTES);
    let mut w = Encoder {
        budget: Some(Budget::shared(&budget)),
        ..Encoder::default()
    };
    value.write(&mut w);
    let mut next = from_slice_in::<T>(&w.finish()?, Some(&budget))?;
    let mut r = Decoder::for_load(bytes, Some(&budget));
    next.read(&mut r)
        .and_then(|()| r.finish())
        .map_err(|e| e.at(super::type_name::<T>()))?;
    *value = next;
    Ok(())
}

/// A binary stream sink, usable for a world whose component types are erased.
pub struct Encoder {
    bytes: Vec<u8>,
    names: BTreeMap<std::borrow::Cow<'static, str>, u64>,
    limit: usize,
    budget: Option<Budget>,
    depth: usize,
    error: Option<DataError>,
}
impl Default for Encoder {
    fn default() -> Self {
        Self::bounded(MAX_LOAD_BYTES)
    }
}
impl Encoder {
    fn allow_bytes(&mut self, len: usize) -> bool {
        if self.bytes.len().saturating_add(len) > self.limit {
            self.fail("encoded size exceeds save limit");
        }
        !self.stopped()
    }

    pub fn bounded(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            names: BTreeMap::new(),
            limit,
            budget: None,
            depth: 0,
            error: None,
        }
    }
    pub(crate) fn prefixed(prefix: &[u8]) -> Self {
        let mut w = Self::bounded(128 * 1024 * 1024);
        w.append(prefix);
        w
    }
    pub(crate) fn for_validation(prefix: &[u8], budget: Budget) -> Self {
        let mut w = Self::bounded(128 * 1024 * 1024);
        w.budget = Some(budget);
        w.append(prefix);
        w
    }
    fn allocation(&mut self, bytes: usize) -> bool {
        if let Some(budget) = &mut self.budget {
            if let Err(error) = budget.claim(bytes) {
                self.error = Some(error);
            }
        }
        !self.stopped()
    }
    pub fn finish(self) -> Result<Vec<u8>, DataError> {
        self.error.map_or(Ok(self.bytes), Err)
    }
    fn fail(&mut self, message: &str) {
        if self.error.is_none() {
            self.error = Some(DataError::new(message));
        }
    }
    fn append(&mut self, bytes: &[u8]) {
        if !self.allow_bytes(bytes.len()) {
            return;
        }
        let needed = self.bytes.len() + bytes.len();
        if needed > self.bytes.capacity() {
            let capacity = needed
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            if !self.allocation(capacity) {
                return;
            }
            if self
                .bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .is_err()
            {
                self.fail("cannot allocate encoded value");
                return;
            }
        }
        self.bytes.extend_from_slice(bytes);
    }
    fn enter(&mut self) {
        self.depth += 1;
        if self.depth > 256 {
            self.fail("nesting exceeds 256");
        }
    }
    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
    fn var(&mut self, mut n: u64) {
        while n >= 128 {
            self.append(&[n as u8 | 128]);
            n >>= 7;
        }
        self.append(&[n as u8]);
    }
    fn text(&mut self, s: &str) {
        if s.len() > MAX_LOAD_STRING {
            self.fail("string exceeds load limit");
            return;
        }
        self.var(s.len() as u64);
        self.append(s.as_bytes());
    }
    fn name(&mut self, s: std::borrow::Cow<'static, str>) {
        if self.stopped() {
            return;
        }
        if let Some(&i) = self.names.get(s.as_ref()) {
            self.var(i + 1);
        } else {
            if !self.allocation(super::limits::map_bytes::<std::borrow::Cow<str>, u64>()) {
                return;
            }
            self.var(0);
            self.text(&s);
            if !self.stopped() {
                self.names.insert(s, self.names.len() as u64);
            }
        }
    }
}
impl Writer for Encoder {
    fn reject(&mut self, message: &str) {
        self.fail(message);
    }

    fn stopped(&self) -> bool {
        self.error.is_some()
    }
    fn bytes(&mut self, value: super::Bulk<'_>) {
        let (kind, len) = value.shape();
        if !self.allow_bytes(len) {
            return;
        }
        self.append(&[12 + kind as u8]);
        self.var(len as u64);
        value.chunks(|part| {
            self.append(part);
            !self.stopped()
        });
    }
    fn boolean(&mut self, n: bool) {
        self.append(&[u8::from(n)]);
    }
    fn number(&mut self, n: Number) {
        match n {
            Number::Unsigned(n) => {
                self.append(&[2]);
                self.var(n);
            }
            Number::Signed(n) => {
                self.append(&[3]);
                self.var(((n as u64) << 1) ^ ((n >> 63) as u64));
            }
            Number::F32(n) => {
                self.append(&[4]);
                self.append(&f32_bits(n).to_le_bytes());
            }
            Number::F64(n) => {
                self.append(&[5]);
                self.append(&f64_bits(n).to_le_bytes());
            }
        }
    }
    fn string(&mut self, s: &str) {
        self.append(&[6]);
        self.text(s);
    }
    fn begin_seq(&mut self, len: usize) {
        self.enter();
        self.append(&[7]);
        self.var(len as u64);
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        self.leave();
    }
    fn begin_struct(&mut self) {
        self.enter();
        self.append(&[8]);
    }
    fn field(&mut self, name: &'static str) {
        self.append(&[1]);
        self.name(name.into());
    }
    fn static_key(&mut self, name: &'static str) {
        self.field(name);
    }
    fn key(&mut self, name: &str) {
        if self.stopped() {
            return;
        }
        self.append(&[1]);
        if self.allocation(name.len()) {
            self.name(name.to_owned().into());
        }
    }
    fn end_struct(&mut self) {
        self.leave();
        self.append(&[0]);
    }
    fn variant(&mut self, name: &'static str, index: u32) {
        self.enter();
        self.append(&[9]);
        self.var(index.into());
        self.name(name.into());
    }
    fn end_variant(&mut self) {
        self.leave();
    }
    fn option(&mut self, some: bool) {
        self.enter();
        self.append(&[if some { 11 } else { 10 }]);
    }
    fn end_option(&mut self) {
        self.leave();
    }
}

/// A checked streaming cursor; nesting is bounded to protect untrusted saves.
pub struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    names: Vec<&'a str>,
    frames: Vec<Frame>,
    marks: BTreeMap<&'a str, usize>,
    fields: Vec<(&'a str, usize)>,
    budget: Budget,
}
enum Frame {
    Seq(u64),
    Struct(usize),
    Variant,
    Option,
}
impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            names: vec![],
            frames: vec![],
            marks: BTreeMap::new(),
            fields: vec![],
            budget: Budget::default(),
        }
    }
    pub(crate) fn with_budget(bytes: &'a [u8], allocation_bytes: usize) -> Self {
        Self {
            budget: Budget::new(allocation_bytes),
            ..Self::new(bytes)
        }
    }
    pub fn for_load(bytes: &'a [u8], budget: Option<&LoadBudget>) -> Self {
        let mut r = Self::new(bytes);
        if let Some(budget) = budget {
            r.budget = Budget::shared(budget);
        }
        r
    }
    pub(crate) fn into_budget(self) -> Budget {
        self.budget
    }
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
    pub(crate) fn borrowed_string(&mut self) -> Result<&'a str, DataError> {
        self.tag(6, "expected a string")?;
        self.text()
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
    fn push(&mut self, f: Frame) -> Result<(), DataError> {
        if self.frames.len() >= 256 {
            return Err(self.err("nesting exceeds 256"));
        }
        self.budget.reserve(&mut self.frames)?;
        self.frames.push(f);
        Ok(())
    }
}
impl<'a> Reader<'a> for Decoder<'a> {
    fn check_allocation(&self, bytes: usize) -> Result<(), DataError> {
        self.budget.check(bytes)
    }
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
                let n = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
                Number::F32(f32::from_bits(f32_bits(n)))
            }
            5 => {
                let n = f64::from_le_bytes(self.take(8)?.try_into().unwrap());
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
    fn shared_string(&mut self) -> Result<std::rc::Rc<str>, DataError> {
        self.tag(6, "expected a string")?;
        let text = self.text()?;
        self.claim(super::limits::rc_str_bytes(text.len())?)?;
        Ok(text.into())
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
        self.push(Frame::Struct(self.fields.len()))
    }
    fn field(&mut self) -> Result<Option<&'a str>, DataError> {
        let Some(&Frame::Struct(start)) = self.frames.last() else {
            return Err(self.err("not inside a record"));
        };
        match self.byte()? {
            0 => {
                for (name, previous) in self.fields.drain(start..) {
                    *self.marks.get_mut(name).unwrap() = previous;
                }
                self.frames.pop();
                Ok(None)
            }
            1 => {
                let name = self.name()?;
                if !self.marks.contains_key(name) {
                    self.budget
                        .claim(super::limits::map_bytes::<&str, usize>())?;
                }
                let mark = self.marks.entry(name).or_insert(0);
                if *mark == self.frames.len() {
                    return Err(DataError::new("duplicate field").at(name));
                }
                self.budget.reserve(&mut self.fields)?;
                self.fields.push((name, *mark));
                *mark = self.frames.len();
                Ok(Some(name))
            }
            _ => Err(self.err("expected a field")),
        }
    }
    fn variant(&mut self) -> Result<&'a str, DataError> {
        self.tag(9, "expected an enum")?;
        self.var()?;
        let name = self.name()?;
        self.push(Frame::Variant)?;
        Ok(name)
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
                self.byte()?;
                self.text()?;
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
