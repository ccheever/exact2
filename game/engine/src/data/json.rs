//! Human-readable streaming JSON. Records are objects, tuples/vectors are arrays,
//! enums are one-key objects, and options are zero/one-element arrays.
use super::limits::{allocation, Budget, MAX_LOAD_BYTES, MAX_LOAD_STRING};
use super::{Data, DataError, Number, Reader, Writer};
use std::collections::BTreeSet;
use std::fmt::Write;

/// Encode a value; JSON refuses infinities and NaNs.
pub fn to_string<T: Data>(value: &T) -> Result<String, DataError> {
    let mut w = Encoder::default();
    value.write(&mut w);
    w.finish().map_err(|e| e.at(super::type_name::<T>()))
}
/// Parse one complete value over its defaults.
pub fn from_str<T: Data>(text: &str) -> Result<T, DataError> {
    let mut value = T::default();
    read_into(text, &mut value)?;
    Ok(value)
}
/// Read into an existing value using Data's patch/replacement rules.
pub fn read_into<T: Data>(text: &str, value: &mut T) -> Result<(), DataError> {
    if text.len() > MAX_LOAD_BYTES {
        return Err(DataError::new("input exceeds load size limit"));
    }
    let mut r = Decoder::new(text);
    value
        .read(&mut r)
        .and_then(|()| r.finish())
        .map_err(|e| e.at(super::type_name::<T>()))
}

#[derive(Default)]
struct Frame {
    count: usize,
    path: String,
}
/// A streaming JSON sink with a deferred first error.
#[derive(Default)]
pub struct Encoder {
    text: String,
    frames: Vec<Frame>,
    error: Option<DataError>,
    rounded: bool,
}
impl Encoder {
    /// Round floating-point output to four decimal places, for agent replies only.
    pub fn rounded() -> Self {
        Self {
            rounded: true,
            ..Self::default()
        }
    }
    /// Return JSON, or the first unrepresentable number and its field path.
    pub fn finish(self) -> Result<String, DataError> {
        self.error.map_or(Ok(self.text), Err)
    }
    fn quoted(&mut self, s: &str) {
        self.text.push('"');
        for c in s.chars() {
            match c {
                '"' => self.text.push_str("\\\""),
                '\\' => self.text.push_str("\\\\"),
                '\n' => self.text.push_str("\\n"),
                '\r' => self.text.push_str("\\r"),
                '\t' => self.text.push_str("\\t"),
                c if c < ' ' => {
                    write!(self.text, "\\u{:04x}", c as u32).unwrap();
                }
                c => self.text.push(c),
            }
        }
        self.text.push('"');
    }
    fn separate(&mut self) {
        let f = self.frames.last_mut().expect("JSON container");
        if f.count != 0 {
            self.text.push(',');
        }
        f.count += 1;
    }
    fn finite(&mut self, finite: bool) {
        if !finite && self.error.is_none() {
            let path = self
                .frames
                .iter()
                .map(|f| f.path.as_str())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(".");
            self.error = Some(DataError {
                path,
                message: "non-finite number is not JSON".into(),
            });
        }
    }
}
impl Writer for Encoder {
    fn boolean(&mut self, n: bool) {
        self.text.push_str(if n { "true" } else { "false" });
    }
    fn number(&mut self, n: Number) {
        match n {
            Number::Unsigned(n) => write!(self.text, "{n}").unwrap(),
            Number::Signed(n) => write!(self.text, "{n}").unwrap(),
            Number::F32(n) => {
                self.finite(n.is_finite());
                if self.rounded {
                    let n = rounded(n as f64);
                    write!(self.text, "{n}").unwrap();
                } else {
                    write!(self.text, "{n:?}").unwrap();
                }
            }
            Number::F64(n) => {
                self.finite(n.is_finite());
                if self.rounded {
                    let n = rounded(n);
                    write!(self.text, "{n}").unwrap();
                } else {
                    write!(self.text, "{n:?}").unwrap();
                }
            }
        }
    }
    fn string(&mut self, s: &str) {
        self.quoted(s);
    }
    fn begin_seq(&mut self, _: usize) {
        self.text.push('[');
        self.frames.push(Frame::default());
    }
    fn item(&mut self) {
        self.separate();
        let f = self.frames.last_mut().unwrap();
        f.path = (f.count - 1).to_string();
    }
    fn end_seq(&mut self) {
        self.frames.pop();
        self.text.push(']');
    }
    fn begin_struct(&mut self) {
        self.text.push('{');
        self.frames.push(Frame::default());
    }
    fn field(&mut self, name: &str) {
        self.separate();
        self.frames.last_mut().unwrap().path = name.into();
        self.quoted(name);
        self.text.push(':');
    }
    fn end_struct(&mut self) {
        self.frames.pop();
        self.text.push('}');
    }
    fn variant(&mut self, name: &str, _: u32) {
        self.begin_struct();
        self.field(name);
    }
    fn end_variant(&mut self) {
        self.end_struct();
    }
    fn option(&mut self, some: bool) {
        self.begin_seq(usize::from(some));
        if some {
            self.item();
        }
    }
    fn end_option(&mut self) {
        self.end_seq();
    }
}

/// A streaming parser with exact integers and checked Unicode escapes.
pub struct Decoder<'a> {
    text: &'a str,
    pos: usize,
    frames: Vec<ReadFrame>,
    budget: Budget,
}
struct ReadFrame {
    end: u8,
    first: bool,
    names: BTreeSet<String>,
}
impl<'a> Decoder<'a> {
    /// Start at the first JSON value.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            frames: vec![],
            budget: Budget::default(),
        }
    }
    /// Reject trailing input or an unfinished container.
    pub fn finish(&mut self) -> Result<(), DataError> {
        self.ws();
        if self.pos == self.text.len() && self.frames.is_empty() {
            Ok(())
        } else {
            Err(self.err("trailing or unfinished JSON"))
        }
    }
    fn err(&self, s: &str) -> DataError {
        DataError::new(format!("{s} at byte {}", self.pos))
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\r' | b'\n' | b'\t')) {
            self.pos += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }
    fn eat(&mut self, b: u8) -> Result<(), DataError> {
        self.ws();
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected {}", b as char)))
        }
    }
    fn word(&mut self, word: &str) -> bool {
        self.ws();
        if self.text[self.pos..].starts_with(word) {
            self.pos += word.len();
            true
        } else {
            false
        }
    }
    fn enter(&mut self, start: u8, end: u8) -> Result<(), DataError> {
        self.eat(start)?;
        if self.frames.len() >= 256 {
            return Err(self.err("nesting exceeds 256"));
        }
        self.budget.reserve(&mut self.frames)?;
        self.frames.push(ReadFrame {
            end,
            first: true,
            names: BTreeSet::new(),
        });
        Ok(())
    }
    fn next(&mut self, end: u8) -> Result<bool, DataError> {
        self.ws();
        let frame = self
            .frames
            .last()
            .ok_or_else(|| self.err("not inside a container"))?;
        let (want, first) = (frame.end, frame.first);
        if want != end {
            return Err(self.err("wrong container"));
        }
        if self.peek() == Some(end) {
            self.pos += 1;
            self.frames.pop();
            return Ok(false);
        }
        if !first {
            self.eat(b',')?;
        }
        self.frames
            .last_mut()
            .ok_or_else(|| DataError::new("expected container"))?
            .first = false;
        self.ws();
        if self.peek() == Some(end) {
            return Err(self.err("trailing comma"));
        }
        Ok(true)
    }
    fn hex(&mut self) -> Result<u32, DataError> {
        let b = self
            .text
            .as_bytes()
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.err("truncated Unicode escape"))?;
        let mut n = 0;
        for &c in b {
            n = n * 16
                + (c as char)
                    .to_digit(16)
                    .ok_or_else(|| self.err("invalid Unicode escape"))?;
        }
        self.pos += 4;
        Ok(n)
    }
}
impl Reader for Decoder<'_> {
    fn claim(&mut self, bytes: usize) -> Result<(), DataError> {
        self.budget.claim(bytes)
    }
    fn boolean(&mut self) -> Result<bool, DataError> {
        if self.word("true") {
            Ok(true)
        } else if self.word("false") {
            Ok(false)
        } else {
            Err(self.err("expected a boolean"))
        }
    }
    fn number(&mut self) -> Result<Number, DataError> {
        self.ws();
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err("expected a number")),
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            float = true;
            self.pos += 1;
            let digits = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if digits == self.pos {
                return Err(self.err("expected fractional digits"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let digits = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if digits == self.pos {
                return Err(self.err("expected exponent digits"));
            }
        }
        let s = &self.text[start..self.pos];
        if !float && s != "-0" {
            if s.starts_with('-') {
                return s
                    .parse()
                    .map(Number::Signed)
                    .map_err(|_| self.err("integer outside i64 range"));
            }
            return s
                .parse()
                .map(Number::Unsigned)
                .map_err(|_| self.err("integer outside u64 range"));
        }
        let n: f64 = s.parse().map_err(|_| self.err("expected a number"))?;
        if !n.is_finite() {
            return Err(self.err("non-finite number is not JSON"));
        }
        Ok(Number::F64(n))
    }
    fn string(&mut self) -> Result<String, DataError> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            if self.peek() != Some(b'"') {
                if out.len() >= MAX_LOAD_STRING {
                    return Err(self.err("string exceeds load limit"));
                }
                if out.capacity() - out.len() < 4 {
                    let capacity = (out.capacity() * 2).clamp(8, MAX_LOAD_STRING + 4);
                    self.budget.claim(capacity - out.capacity())?;
                    out.try_reserve_exact(capacity - out.len())
                        .map_err(allocation)?;
                }
            }
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    if out.len() > MAX_LOAD_STRING {
                        return Err(self.err("string exceeds load limit"));
                    }
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let e = self.peek().ok_or_else(|| self.err("truncated escape"))?;
                    self.pos += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e as char),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut n = self.hex()?;
                            if (0xd800..=0xdbff).contains(&n) {
                                if !self.text[self.pos..].starts_with("\\u") {
                                    return Err(self.err("missing low surrogate"));
                                }
                                self.pos += 2;
                                let low = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err(self.err("invalid low surrogate"));
                                }
                                n = 0x10000 + ((n - 0xd800) << 10) + low - 0xdc00;
                            }
                            out.push(
                                char::from_u32(n)
                                    .ok_or_else(|| self.err("invalid Unicode scalar"))?,
                            );
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                Some(0..=31) => return Err(self.err("unescaped control character")),
                Some(_) => {
                    let c = self.text[self.pos..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.err("unterminated string"))?;
                    self.pos += c.len_utf8();
                    out.push(c);
                }
                None => return Err(self.err("unterminated string")),
            }
        }
    }
    fn begin_seq(&mut self) -> Result<(), DataError> {
        self.enter(b'[', b']')
    }
    fn item(&mut self) -> Result<bool, DataError> {
        self.next(b']')
    }
    fn begin_struct(&mut self) -> Result<(), DataError> {
        self.enter(b'{', b'}')
    }
    fn field(&mut self) -> Result<Option<String>, DataError> {
        if !self.next(b'}')? {
            return Ok(None);
        }
        let name = self.string()?;
        self.eat(b':')?;
        let seen = &mut self
            .frames
            .last_mut()
            .ok_or_else(|| DataError::new("expected object"))?
            .names;
        if seen.contains(&name) {
            return Err(DataError::new("duplicate field").at(name));
        }
        self.budget.claim(64)?;
        seen.insert(self.budget.text(&name)?);
        Ok(Some(name))
    }
    fn variant(&mut self) -> Result<String, DataError> {
        self.begin_struct()?;
        self.field()?
            .ok_or_else(|| self.err("expected an enum variant"))
    }
    fn end_variant(&mut self) -> Result<(), DataError> {
        if self.field()?.is_none() {
            Ok(())
        } else {
            Err(self.err("expected one enum variant"))
        }
    }
    fn option(&mut self) -> Result<bool, DataError> {
        self.begin_seq()?;
        // Keep an empty option's frame until end_option, like a nonempty one.
        self.ws();
        if self.peek() == Some(b']') {
            Ok(false)
        } else {
            self.item()
        }
    }
    fn end_option(&mut self) -> Result<(), DataError> {
        if self.item()? {
            Err(self.err("expected at most one option value"))
        } else {
            Ok(())
        }
    }
    fn skip(&mut self) -> Result<(), DataError> {
        self.ws();
        match self.peek() {
            Some(b'{') => {
                self.begin_struct()?;
                while self.field()?.is_some() {
                    self.skip()?;
                }
            }
            Some(b'[') => {
                self.begin_seq()?;
                while self.item()? {
                    self.skip()?;
                }
            }
            Some(b'"') => {
                self.string()?;
            }
            Some(b't' | b'f') => {
                self.boolean()?;
            }
            Some(b'n') if self.word("null") => {}
            _ => {
                self.number()?;
            }
        }
        Ok(())
    }
}

fn rounded(n: f64) -> f64 {
    if n.abs() > f64::MAX / 10000.0 {
        n
    } else {
        (n * 10000.0).round() / 10000.0
    }
}
