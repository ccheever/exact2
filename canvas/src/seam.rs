//! The TypeScript seam's reply (LLP 1056 D1): what `canvasSeam(draw).draw`
//! in `canvas/recorder.js` returns, `{"lists":[base64…],"frame":bool,
//! "error":null|string,"notes":[string…]}`, read without a JSON library so
//! the web's wasm and native hosts share it.

/// A draw's reply from a TypeScript module.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SeamReply {
    /// The recorded lists, decoded.
    pub lists: Vec<Vec<u8>>,
    /// Whether it asked for another frame.
    pub frame: bool,
    /// What it threw.
    pub error: Option<String>,
    /// Development notes.
    pub notes: Vec<String>,
}

struct Cursor<'a> {
    s: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn ws(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }
    fn eat(&mut self, b: u8) -> Result<(), String> {
        self.ws();
        if self.s.get(self.at) == Some(&b) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!(
                "seam reply: expected `{}` at {}",
                b as char, self.at
            ))
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.at).copied()
    }
    fn word(&mut self, w: &str) -> bool {
        self.ws();
        if self.s[self.at..].starts_with(w.as_bytes()) {
            self.at += w.len();
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&c) = self.s.get(self.at) else {
                return Err("seam reply: unterminated string".into());
            };
            self.at += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = *self.s.get(self.at).ok_or("seam reply: bad escape")?;
                    self.at += 1;
                    match e {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = std::str::from_utf8(
                                self.s
                                    .get(self.at..self.at + 4)
                                    .ok_or("seam reply: bad \\u")?,
                            )
                            .map_err(|e| e.to_string())?;
                            let mut code =
                                u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                            self.at += 4;
                            if (0xD800..0xDC00).contains(&code)
                                && self.s[self.at..].starts_with(b"\\u")
                            {
                                let low = std::str::from_utf8(&self.s[self.at + 2..self.at + 6])
                                    .map_err(|e| e.to_string())?;
                                let low =
                                    u32::from_str_radix(low, 16).map_err(|e| e.to_string())?;
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                self.at += 6;
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        other => out.push(other as char),
                    }
                }
                _ => {
                    // UTF-8 continues as bytes; collect the whole character.
                    let start = self.at - 1;
                    let len = match c {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    self.at = (start + len).min(self.s.len());
                    out.push_str(
                        std::str::from_utf8(&self.s[start..self.at]).unwrap_or("\u{fffd}"),
                    );
                }
            }
        }
    }
    fn strings(&mut self) -> Result<Vec<String>, String> {
        self.eat(b'[')?;
        let mut out = Vec::new();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(out);
        }
        loop {
            out.push(self.string()?);
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(out);
                }
                _ => return Err("seam reply: bad array".into()),
            }
        }
    }
}

/// Decode standard base64 (with or without padding).
pub fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let value = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(format!("base64: `{}`", c as char)),
        } as u32)
    };
    let bytes: Vec<u8> = text.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// Read a seam reply.
pub fn reply(json: &str) -> Result<SeamReply, String> {
    let mut c = Cursor {
        s: json.as_bytes(),
        at: 0,
    };
    let mut out = SeamReply::default();
    c.eat(b'{')?;
    if c.peek() == Some(b'}') {
        return Ok(out);
    }
    loop {
        let key = c.string()?;
        c.eat(b':')?;
        match key.as_str() {
            "lists" => {
                out.lists = c
                    .strings()?
                    .iter()
                    .map(|s| base64_decode(s))
                    .collect::<Result<_, _>>()?
            }
            "notes" => out.notes = c.strings()?,
            "frame" => out.frame = c.word("true") || !c.word("false"),
            "error" => {
                out.error = if c.word("null") {
                    None
                } else {
                    Some(c.string()?)
                }
            }
            other => return Err(format!("seam reply: unknown field `{other}`")),
        }
        match c.peek() {
            Some(b',') => c.at += 1,
            Some(b'}') => return Ok(out),
            _ => return Err("seam reply: bad object".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_reads_back() {
        let r = reply(
            r#"{"lists":["RUMyRAEAAAA="],"frame":true,"error":"TypeError: \"x\" é é","notes":[]}"#,
        )
        .unwrap();
        assert_eq!(r.lists, vec![b"EC2D\x01\0\0\0".to_vec()]);
        assert!(r.frame);
        assert_eq!(r.error.as_deref(), Some("TypeError: \"x\" é é"));
        assert_eq!(
            reply(r#"{"lists":[],"frame":false,"error":null,"notes":["n"]}"#)
                .unwrap()
                .notes,
            ["n"]
        );
        assert_eq!(base64_decode("YWI").unwrap(), b"ab");
    }
}
