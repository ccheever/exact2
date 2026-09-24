//! JSON text to values, as serde_json 1.0 reads it: its grammar, its errors
//! and the line and column it reports them at, and its recursion limit of
//! 128 — except that a number with a fraction or an exponent (or past 64
//! bits) is correctly rounded (`exact_num`), so a value JavaScript wrote
//! arrives as exactly JavaScript's value. serde_json's parser can land one
//! unit in the last place away for 16–17 digit numbers.

use crate::json::{Json, Number, Object};
use std::fmt;

/// Why text is not JSON, and where: serde_json's message and position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    code: Code,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code {
    EofWhileParsingList,
    EofWhileParsingObject,
    EofWhileParsingString,
    EofWhileParsingValue,
    ExpectedColon,
    ExpectedListCommaOrEnd,
    ExpectedObjectCommaOrEnd,
    ExpectedSomeIdent,
    ExpectedSomeValue,
    InvalidEscape,
    InvalidNumber,
    NumberOutOfRange,
    InvalidUnicodeCodePoint,
    ControlCharacterWhileParsingString,
    KeyMustBeAString,
    LoneLeadingSurrogateInHexEscape,
    TrailingComma,
    TrailingCharacters,
    UnexpectedEndOfHexEscape,
    RecursionLimitExceeded,
}

impl Code {
    fn text(self) -> &'static str {
        match self {
            Code::EofWhileParsingList => "EOF while parsing a list",
            Code::EofWhileParsingObject => "EOF while parsing an object",
            Code::EofWhileParsingString => "EOF while parsing a string",
            Code::EofWhileParsingValue => "EOF while parsing a value",
            Code::ExpectedColon => "expected `:`",
            Code::ExpectedListCommaOrEnd => "expected `,` or `]`",
            Code::ExpectedObjectCommaOrEnd => "expected `,` or `}`",
            Code::ExpectedSomeIdent => "expected ident",
            Code::ExpectedSomeValue => "expected value",
            Code::InvalidEscape => "invalid escape",
            Code::InvalidNumber => "invalid number",
            Code::NumberOutOfRange => "number out of range",
            Code::InvalidUnicodeCodePoint => "invalid unicode code point",
            Code::ControlCharacterWhileParsingString => {
                "control character (\\u0000-\\u001F) found while parsing a string"
            }
            Code::KeyMustBeAString => "key must be a string",
            Code::LoneLeadingSurrogateInHexEscape => "lone leading surrogate in hex escape",
            Code::TrailingComma => "trailing comma",
            Code::TrailingCharacters => "trailing characters",
            Code::UnexpectedEndOfHexEscape => "unexpected end of hex escape",
            Code::RecursionLimitExceeded => "recursion limit exceeded",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {} column {}",
            self.code.text(),
            self.line,
            self.column
        )
    }
}

impl std::error::Error for Error {}

/// `bytes` as one JSON value, with nothing but whitespace after it.
pub fn parse(bytes: &[u8]) -> Result<Json, Error> {
    let mut parser = Parser::new(bytes);
    let value = parser.value(Tree)?;
    parser.end()?;
    Ok(value)
}

/// What a caller makes of each kind of value, as the parser reaches it.
pub(crate) trait Visit: Sized {
    type Out;
    fn null(self) -> Self::Out;
    fn bool(self, value: bool) -> Self::Out;
    fn number(self, value: Number) -> Self::Out;
    fn string(self, value: &str) -> Self::Out;
    /// Pull every element through `items` (until it answers `None`).
    fn array(self, items: &mut Items<'_, '_>) -> Result<Self::Out, Error>;
    /// Pull every member through `members` (until its key is `None`).
    fn object(self, members: &mut Members<'_, '_>) -> Result<Self::Out, Error>;
}

/// The value tree.
pub(crate) struct Tree;

impl Visit for Tree {
    type Out = Json;
    fn null(self) -> Json {
        Json::Null
    }
    fn bool(self, value: bool) -> Json {
        Json::Bool(value)
    }
    fn number(self, value: Number) -> Json {
        Json::Number(value)
    }
    fn string(self, value: &str) -> Json {
        Json::String(value.to_owned())
    }
    fn array(self, items: &mut Items<'_, '_>) -> Result<Json, Error> {
        let mut out = Vec::new();
        while let Some(item) = items.next(Tree)? {
            out.push(item);
        }
        Ok(Json::Array(out))
    }
    fn object(self, members: &mut Members<'_, '_>) -> Result<Json, Error> {
        let mut out = Object::new();
        while let Some(key) = members.key()? {
            let key = key.to_owned();
            let value = members.value(Tree)?;
            out.insert(key, value);
        }
        Ok(Json::Object(out))
    }
}

pub(crate) struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    /// serde_json's `remaining_depth`.
    depth: u8,
    scratch: Vec<u8>,
}

/// An array's elements, pulled one at a time.
pub(crate) struct Items<'p, 'a> {
    parser: &'p mut Parser<'a>,
    first: bool,
}

/// An object's members, pulled a key and then its value at a time.
pub(crate) struct Members<'p, 'a> {
    parser: &'p mut Parser<'a>,
    first: bool,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Parser {
            bytes,
            at: 0,
            depth: 128,
            scratch: Vec::new(),
        }
    }

    /// serde_json's line (from 1) and column (bytes since the line began).
    fn error_at(&self, at: usize, code: Code) -> Error {
        let line_start = self.bytes[..at]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |p| p + 1);
        let line = 1 + self.bytes[..line_start]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        Error {
            code,
            line,
            column: at - line_start,
        }
    }

    /// An error caused by a byte already consumed.
    fn error(&self, code: Code) -> Error {
        self.error_at(self.at, code)
    }

    /// An error caused by the byte not yet consumed.
    fn peek_error(&self, code: Code) -> Error {
        self.error_at((self.at + 1).min(self.bytes.len()), code)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.at += 1;
        Some(byte)
    }

    /// The next byte after whitespace, not consumed.
    fn whitespace(&mut self) -> Option<u8> {
        while let Some(b' ' | b'\n' | b'\t' | b'\r') = self.peek() {
            self.at += 1;
        }
        self.peek()
    }

    /// Only whitespace remains.
    pub(crate) fn end(&mut self) -> Result<(), Error> {
        match self.whitespace() {
            Some(_) => Err(self.peek_error(Code::TrailingCharacters)),
            None => Ok(()),
        }
    }

    pub(crate) fn value<V: Visit>(&mut self, visit: V) -> Result<V::Out, Error> {
        let Some(peek) = self.whitespace() else {
            return Err(self.peek_error(Code::EofWhileParsingValue));
        };
        match peek {
            b'n' => {
                self.at += 1;
                self.ident(b"ull")?;
                Ok(visit.null())
            }
            b't' => {
                self.at += 1;
                self.ident(b"rue")?;
                Ok(visit.bool(true))
            }
            b'f' => {
                self.at += 1;
                self.ident(b"alse")?;
                Ok(visit.bool(false))
            }
            b'-' => {
                let start = self.at;
                self.at += 1;
                Ok(visit.number(self.number(start, false)?))
            }
            b'0'..=b'9' => {
                let start = self.at;
                Ok(visit.number(self.number(start, true)?))
            }
            b'"' => {
                self.at += 1;
                let text = self.string()?;
                Ok(visit.string(text))
            }
            b'[' => {
                self.enter()?;
                self.at += 1;
                let out = visit.array(&mut Items {
                    parser: self,
                    first: true,
                });
                self.depth += 1;
                let out = out?;
                self.end_array()?;
                Ok(out)
            }
            b'{' => {
                self.enter()?;
                self.at += 1;
                let out = visit.object(&mut Members {
                    parser: self,
                    first: true,
                });
                self.depth += 1;
                let out = out?;
                self.end_object()?;
                Ok(out)
            }
            _ => Err(self.peek_error(Code::ExpectedSomeValue)),
        }
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth -= 1;
        if self.depth == 0 {
            return Err(self.peek_error(Code::RecursionLimitExceeded));
        }
        Ok(())
    }

    fn end_array(&mut self) -> Result<(), Error> {
        match self.whitespace() {
            Some(b']') => {
                self.at += 1;
                Ok(())
            }
            Some(b',') => {
                self.at += 1;
                match self.whitespace() {
                    Some(b']') => Err(self.peek_error(Code::TrailingComma)),
                    _ => Err(self.peek_error(Code::TrailingCharacters)),
                }
            }
            Some(_) => Err(self.peek_error(Code::TrailingCharacters)),
            None => Err(self.peek_error(Code::EofWhileParsingList)),
        }
    }

    fn end_object(&mut self) -> Result<(), Error> {
        match self.whitespace() {
            Some(b'}') => {
                self.at += 1;
                Ok(())
            }
            Some(b',') => Err(self.peek_error(Code::TrailingComma)),
            Some(_) => Err(self.peek_error(Code::TrailingCharacters)),
            None => Err(self.peek_error(Code::EofWhileParsingObject)),
        }
    }

    fn ident(&mut self, rest: &[u8]) -> Result<(), Error> {
        for &expected in rest {
            match self.next() {
                None => return Err(self.error(Code::EofWhileParsingValue)),
                Some(byte) if byte != expected => return Err(self.error(Code::ExpectedSomeIdent)),
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// A number whose text began at `start` (its `-`, if negative, already
    /// consumed). An integer that fits 64 bits stays an integer; anything
    /// else is a float, read from its whole text and correctly rounded.
    fn number(&mut self, start: usize, positive: bool) -> Result<Number, Error> {
        let digit = |byte: Option<u8>| byte.filter(u8::is_ascii_digit);
        let mut significand = 0u64;
        let mut overflowed = false;
        let mut zero = true;
        match self.next() {
            None => return Err(self.error(Code::EofWhileParsingValue)),
            Some(b'0') => {
                if digit(self.peek()).is_some() {
                    return Err(self.peek_error(Code::InvalidNumber));
                }
            }
            Some(first @ b'1'..=b'9') => {
                significand = u64::from(first - b'0');
                zero = false;
                while let Some(d) = digit(self.peek()) {
                    self.at += 1;
                    match significand
                        .checked_mul(10)
                        .and_then(|n| n.checked_add(u64::from(d - b'0')))
                    {
                        Some(n) if !overflowed => significand = n,
                        _ => overflowed = true,
                    }
                }
            }
            Some(_) => return Err(self.error(Code::InvalidNumber)),
        }
        let mut float = overflowed;
        if self.peek() == Some(b'.') {
            float = true;
            self.at += 1;
            let mut any = false;
            while let Some(d) = digit(self.peek()) {
                self.at += 1;
                any = true;
                zero &= d == b'0';
            }
            if !any {
                return Err(match self.peek() {
                    Some(_) => self.peek_error(Code::InvalidNumber),
                    None => self.peek_error(Code::EofWhileParsingValue),
                });
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            float = true;
            self.at += 1;
            let positive_exponent = match self.peek() {
                Some(b'+') => {
                    self.at += 1;
                    true
                }
                Some(b'-') => {
                    self.at += 1;
                    false
                }
                _ => true,
            };
            let mut exponent = match self.next() {
                None => return Err(self.error(Code::EofWhileParsingValue)),
                Some(d @ b'0'..=b'9') => i32::from(d - b'0'),
                Some(_) => return Err(self.error(Code::InvalidNumber)),
            };
            while let Some(d) = digit(self.peek()) {
                self.at += 1;
                let d = i32::from(d - b'0');
                if exponent > (i32::MAX - d) / 10 {
                    // serde_json stops here: a nonzero number is out of
                    // range, the rest (a zero, or a vanishing negative
                    // exponent) is a zero of the number's sign.
                    if !zero && positive_exponent {
                        return Err(self.error(Code::NumberOutOfRange));
                    }
                    while digit(self.peek()).is_some() {
                        self.at += 1;
                    }
                    return Ok(Number::Float(if positive { 0.0 } else { -0.0 }));
                }
                exponent = exponent * 10 + d;
            }
        }
        if !float {
            return Ok(if positive {
                Number::PosInt(significand)
            } else if significand == 0 {
                Number::Float(-0.0)
            } else if significand <= 1 << 63 {
                Number::NegInt((significand as i64).wrapping_neg())
            } else {
                Number::Float(-(significand as f64))
            });
        }
        // The text is JSON number syntax, which is ASCII and which the
        // decimal reader accepts.
        let text = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default();
        match exact_num::parse_f64(text) {
            Ok(value) if value.is_finite() => Ok(Number::Float(value)),
            _ => Err(self.error(Code::NumberOutOfRange)),
        }
    }

    /// A string after its opening quote: borrowed from the input when it has
    /// no escapes. Its UTF-8 is checked; escapes must pair their surrogates.
    fn string(&mut self) -> Result<&str, Error> {
        self.scratch.clear();
        let mut start = self.at;
        loop {
            while let Some(&byte) = self.bytes.get(self.at) {
                if byte == b'"' || byte == b'\\' || byte < 0x20 {
                    break;
                }
                self.at += 1;
            }
            match self.peek() {
                None => return Err(self.error(Code::EofWhileParsingString)),
                Some(b'"') => {
                    let text: &[u8] = if self.scratch.is_empty() {
                        &self.bytes[start..self.at]
                    } else {
                        self.scratch.extend_from_slice(&self.bytes[start..self.at]);
                        &self.scratch
                    };
                    self.at += 1;
                    return std::str::from_utf8(text)
                        .map_err(|_| self.error_at(self.at, Code::InvalidUnicodeCodePoint));
                }
                Some(b'\\') => {
                    self.scratch.extend_from_slice(&self.bytes[start..self.at]);
                    self.at += 1;
                    self.escape()?;
                    start = self.at;
                }
                Some(_) => {
                    self.at += 1;
                    return Err(self.error(Code::ControlCharacterWhileParsingString));
                }
            }
        }
    }

    fn escape(&mut self) -> Result<(), Error> {
        let byte = match self.next() {
            None => return Err(self.error(Code::EofWhileParsingString)),
            Some(b'"') => b'"',
            Some(b'\\') => b'\\',
            Some(b'/') => b'/',
            Some(b'b') => 0x08,
            Some(b'f') => 0x0c,
            Some(b'n') => b'\n',
            Some(b'r') => b'\r',
            Some(b't') => b'\t',
            Some(b'u') => return self.unicode_escape(),
            Some(_) => return Err(self.error(Code::InvalidEscape)),
        };
        self.scratch.push(byte);
        Ok(())
    }

    fn hex4(&mut self) -> Result<u16, Error> {
        let Some(four) = self.bytes.get(self.at..self.at + 4) else {
            self.at = self.bytes.len();
            return Err(self.error(Code::EofWhileParsingString));
        };
        self.at += 4;
        four.iter()
            .try_fold(0u16, |n, &b| {
                Some(n * 16 + (b as char).to_digit(16)? as u16)
            })
            .ok_or_else(|| self.error(Code::InvalidEscape))
    }

    fn unicode_escape(&mut self) -> Result<(), Error> {
        let n = self.hex4()?;
        let code = match n {
            0xDC00..=0xDFFF => return Err(self.error(Code::LoneLeadingSurrogateInHexEscape)),
            0xD800..=0xDBFF => {
                for expected in *b"\\u" {
                    match self.peek() {
                        None => return Err(self.error(Code::EofWhileParsingString)),
                        Some(byte) => {
                            self.at += 1;
                            if byte != expected {
                                return Err(self.error(Code::UnexpectedEndOfHexEscape));
                            }
                        }
                    }
                }
                let low = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(self.error(Code::LoneLeadingSurrogateInHexEscape));
                }
                0x1_0000 + ((u32::from(n - 0xD800) << 10) | u32::from(low - 0xDC00))
            }
            _ => u32::from(n),
        };
        // Every code here is a scalar value: surrogates were paired above.
        let c = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
        self.scratch
            .extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
        Ok(())
    }
}

impl Items<'_, '_> {
    /// The next element, visited; `None` at the closing bracket.
    pub(crate) fn next<V: Visit>(&mut self, visit: V) -> Result<Option<V::Out>, Error> {
        let parser = &mut *self.parser;
        let Some(peek) = parser.whitespace() else {
            return Err(parser.peek_error(Code::EofWhileParsingList));
        };
        if peek == b']' {
            return Ok(None);
        }
        if self.first {
            self.first = false;
        } else if peek == b',' {
            parser.at += 1;
            match parser.whitespace() {
                Some(b']') => return Err(parser.peek_error(Code::TrailingComma)),
                Some(_) => {}
                None => return Err(parser.peek_error(Code::EofWhileParsingValue)),
            }
        } else {
            return Err(parser.peek_error(Code::ExpectedListCommaOrEnd));
        }
        parser.value(visit).map(Some)
    }
}

impl Members<'_, '_> {
    /// The next member's key; `None` at the closing brace.
    pub(crate) fn key(&mut self) -> Result<Option<&str>, Error> {
        let parser = &mut *self.parser;
        let Some(peek) = parser.whitespace() else {
            return Err(parser.peek_error(Code::EofWhileParsingObject));
        };
        if peek == b'}' {
            return Ok(None);
        }
        if self.first {
            self.first = false;
            if peek != b'"' {
                return Err(parser.peek_error(Code::KeyMustBeAString));
            }
        } else if peek == b',' {
            parser.at += 1;
            match parser.whitespace() {
                Some(b'"') => {}
                Some(b'}') => return Err(parser.peek_error(Code::TrailingComma)),
                Some(_) => return Err(parser.peek_error(Code::KeyMustBeAString)),
                None => return Err(parser.peek_error(Code::EofWhileParsingValue)),
            }
        } else {
            return Err(parser.peek_error(Code::ExpectedObjectCommaOrEnd));
        }
        parser.at += 1;
        parser.string().map(Some)
    }

    /// The value of the key just read, visited.
    pub(crate) fn value<V: Visit>(&mut self, visit: V) -> Result<V::Out, Error> {
        let parser = &mut *self.parser;
        match parser.whitespace() {
            Some(b':') => parser.at += 1,
            Some(_) => return Err(parser.peek_error(Code::ExpectedColon)),
            None => return Err(parser.peek_error(Code::EofWhileParsingObject)),
        }
        parser.value(visit)
    }
}
