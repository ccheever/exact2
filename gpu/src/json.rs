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
                                b'b' => out.push('\u{8}'),
                                b'f' => out.push('\u{c}'),
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
                                    let code = if (0xd800..=0xdbff).contains(&code) {
                                        if self.s.get(self.i..self.i + 2) != Some(b"\\u") {
                                            return Err("bad surrogate pair".into());
                                        }
                                        self.i += 2;
                                        let hex = self
                                            .s
                                            .get(self.i..self.i + 4)
                                            .ok_or("bad surrogate pair")?;
                                        self.i += 4;
                                        let low = u32::from_str_radix(
                                            std::str::from_utf8(hex)
                                                .map_err(|_| "bad surrogate pair")?,
                                            16,
                                        )
                                        .map_err(|_| "bad surrogate pair")?;
                                        if !(0xdc00..=0xdfff).contains(&low) {
                                            return Err("bad surrogate pair".into());
                                        }
                                        0x10000 + ((code - 0xd800) << 10) + low - 0xdc00
                                    } else {
                                        code
                                    };
                                    out.push(char::from_u32(code).ok_or("bad Unicode scalar")?);
                                }
                                _ => return Err("bad escape".into()),
                            }
                        }
                        0..=31 => return Err("unescaped control character".into()),
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
                let digits = text.strip_prefix('-').unwrap_or(text);
                let mantissa = digits.split(['e', 'E']).next().unwrap_or("");
                let integer = mantissa.split('.').next().unwrap_or("");
                if integer.is_empty()
                    || !integer.bytes().all(|b| b.is_ascii_digit())
                    || (integer.len() > 1 && integer.starts_with('0'))
                    || mantissa.ends_with('.')
                {
                    return Err(format!("bad number `{text}`"));
                }
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

/// Parse a single device event; every refusal names the event or its field.
pub fn parse_input(text: &str) -> Result<crate::InputEvent, String> {
    use crate::{InputEvent, PointerKind, PointerPhase};
    let parse = || -> Result<_, String> {
        let mut p = Parser {
            s: text.as_bytes(),
            i: 0,
        };
        p.ws();
        if p.s.get(p.i) != Some(&b'{') {
            return Err("expected an object".into());
        }
        p.i += 1;
        p.ws();
        let mut fields = std::collections::HashMap::new();
        if p.s.get(p.i) != Some(&b'}') {
            loop {
                if p.s.get(p.i) != Some(&b'"') {
                    return Err("expected a field name".into());
                }
                let Value::Str(name) = p.value()? else {
                    unreachable!()
                };
                p.ws();
                if p.s.get(p.i) != Some(&b':') {
                    return Err(format!("{name}: expected :"));
                }
                p.i += 1;
                p.ws();
                if matches!(p.s.get(p.i), Some(b'[' | b'{')) {
                    return Err(format!("{name}: expected a scalar"));
                }
                let value = p.value().map_err(|e| format!("{name}: {e}"))?;
                if fields.insert(name.to_string(), value).is_some() {
                    return Err(format!("{name}: duplicate field"));
                }
                p.ws();
                match p.s.get(p.i) {
                    Some(b',') => {
                        p.i += 1;
                        p.ws();
                    }
                    Some(b'}') => break,
                    _ => return Err("expected , or }".into()),
                }
            }
        }
        p.i += 1;
        p.ws();
        if p.i != p.s.len() {
            return Err("trailing input".into());
        }
        let string = |name: &str| match fields.get(name) {
            Some(Value::Str(s)) => Ok(s.to_string()),
            _ => Err(format!("{name}: expected a string")),
        };
        let boolean = |name: &str| match fields.get(name) {
            Some(Value::Bool(b)) => Ok(*b),
            _ => Err(format!("{name}: expected a boolean")),
        };
        let number = |name: &str| match fields.get(name) {
            Some(Value::Number(n)) if n.is_finite() => Ok(*n),
            _ => Err(format!("{name}: expected a finite number")),
        };
        let point = |name: &str| {
            let n = number(name)? as f32;
            if n.is_finite() {
                Ok(n)
            } else {
                Err(format!("{name}: outside point range"))
            }
        };
        let unsigned = |name: &str| {
            let n = number(name)?;
            if (0.0..=u32::MAX as f64).contains(&n) && n.fract() == 0.0 {
                Ok(n as u32)
            } else {
                Err(format!("{name}: expected a u32"))
            }
        };
        let at_ms = number("at")?;
        Ok(match string("t")?.as_str() {
            "key" => InputEvent::Key {
                code: string("code")?,
                key: string("key")?,
                down: boolean("down")?,
                repeat: boolean("repeat")?,
                at_ms,
            },
            "pointer" => InputEvent::Pointer {
                id: unsigned("id")?,
                phase: match string("phase")?.as_str() {
                    "down" => PointerPhase::Down,
                    "move" => PointerPhase::Move,
                    "up" => PointerPhase::Up,
                    "cancel" => PointerPhase::Cancel,
                    other => return Err(format!("phase: unknown `{other}`")),
                },
                x: point("x")?,
                y: point("y")?,
                kind: match string("kind")?.as_str() {
                    "mouse" => PointerKind::Mouse,
                    "touch" => PointerKind::Touch,
                    "pen" => PointerKind::Pen,
                    other => return Err(format!("kind: unknown `{other}`")),
                },
                buttons: unsigned("buttons")?,
                at_ms,
            },
            "control" => InputEvent::Control {
                name: string("name")?,
                id: unsigned("id")?,
                phase: match string("phase")?.as_str() {
                    "down" => PointerPhase::Down,
                    "move" => PointerPhase::Move,
                    "up" => PointerPhase::Up,
                    "cancel" => PointerPhase::Cancel,
                    other => return Err(format!("phase: unknown `{other}`")),
                },
                x: point("x")?,
                y: point("y")?,
                at_ms,
            },
            "wheel" => InputEvent::Wheel {
                dx: point("dx")?,
                dy: point("dy")?,
                x: point("x")?,
                y: point("y")?,
                at_ms,
            },
            "blur" => InputEvent::Blur { at_ms },
            other => return Err(format!("t: unknown `{other}`")),
        })
    };
    parse().map_err(|e| format!("input: {e}"))
}

/// Encode posted strings as a JSON array, preserving every character.
pub fn strings(values: &[String]) -> String {
    let mut out = String::from("[");
    for (i, value) in values.iter().enumerate() {
        if i != 0 {
            out.push(',');
        }
        out.push('"');
        for c in value.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\0'..='\u{1f}' => {
                    use std::fmt::Write;
                    write!(out, "\\u{:04x}", c as u32).expect("string write");
                }
                _ => out.push(c),
            }
        }
        out.push('"');
    }
    out.push(']');
    out
}
