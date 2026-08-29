//! Plan values as the batch carries them: numbers, strings, booleans,
//! `null` (unit or `none`), and lists — records arrive as positional lists.

use crate::Value;

/// Parse a JSON array of values, as a `surface` op's `values`.
pub fn parse_values(text: &str) -> Result<Vec<Value>, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("trailing input at {}", p.i));
    }
    match v {
        Value::List(items) => Ok(items.to_vec()),
        other => Ok(vec![other]),
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.s.get(self.i) {
            None => Err("unexpected end".into()),
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        break;
                    }
                    items.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        _ => return Err(format!("expected , or ] at {}", self.i)),
                    }
                }
                Ok(Value::List(items.into()))
            }
            Some(b'"') => {
                self.i += 1;
                let mut out = String::new();
                loop {
                    let Some(&c) = self.s.get(self.i) else {
                        return Err("unterminated string".into());
                    };
                    self.i += 1;
                    match c {
                        b'"' => break,
                        b'\\' => {
                            let Some(&e) = self.s.get(self.i) else {
                                return Err("bad escape".into());
                            };
                            self.i += 1;
                            match e {
                                b'"' => out.push('"'),
                                b'\\' => out.push('\\'),
                                b'/' => out.push('/'),
                                b'n' => out.push('\n'),
                                b'r' => out.push('\r'),
                                b't' => out.push('\t'),
                                b'u' => {
                                    let hex = self.s.get(self.i..self.i + 4).ok_or("bad \\u")?;
                                    self.i += 4;
                                    let code = u32::from_str_radix(
                                        std::str::from_utf8(hex).map_err(|_| "bad \\u")?,
                                        16,
                                    )
                                    .map_err(|_| "bad \\u")?;
                                    out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                                }
                                _ => return Err("bad escape".into()),
                            }
                        }
                        _ => {
                            // Copy one UTF-8 sequence.
                            let start = self.i - 1;
                            let len = utf8_len(c);
                            let end = (start + len).min(self.s.len());
                            out.push_str(
                                std::str::from_utf8(&self.s[start..end])
                                    .map_err(|_| "bad utf-8")?,
                            );
                            self.i = end;
                        }
                    }
                }
                Ok(Value::str(&out))
            }
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Value::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Value::Bool(false))
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Value::Unit)
            }
            Some(_) => {
                let start = self.i;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "bad number")?;
                text.parse::<f64>()
                    .map(Value::Number)
                    .map_err(|_| format!("bad number `{text}`"))
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// A list's items, or an error naming the input.
pub fn list(v: &Value, what: &str) -> Result<Vec<Value>, crate::SurfaceError> {
    match v {
        Value::List(items) | Value::Record(items) => Ok(items.to_vec()),
        _ => Err(crate::SurfaceError(format!("{what}: expected a list"))),
    }
}

/// A number, or an error naming the input.
pub fn number(v: &Value, what: &str) -> Result<f64, crate::SurfaceError> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err(crate::SurfaceError(format!("{what}: expected a number"))),
    }
}

/// A string, or an error naming the input.
pub fn text(v: &Value, what: &str) -> Result<String, crate::SurfaceError> {
    match v {
        Value::Str(s) => Ok(s.to_string()),
        _ => Err(crate::SurfaceError(format!("{what}: expected a string"))),
    }
}
