//! The JSON the host's own props carry back into a document: a text's
//! `markupPieces` and an element's `dataset`.

/// One piece of `markupPieces`, as the page reads it: numbers keep the
/// spelling the glue's template literals would give them.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Piece {
    pub(super) text: String,
    pub(super) scale: String,
    pub(super) weight: String,
    pub(super) flags: u8,
    pub(super) href: String,
    /// A list item's indent, CSS px; "0" outside a list.
    pub(super) indent: String,
    /// An inline atom's kind: its label is drawn as text, marked
    /// `data-exact-atom` for the page to draw again (LLP 1045.000 D6).
    pub(super) atom: Option<String>,
}

/// Read the host's own `markupPieces` back (`[[text, scale, weight, flags,
/// href, indent?], …]`), as the linked Markdown capability wrote them; anything else
/// is a defect.
pub(super) fn markup_pieces(json: &str) -> Result<Vec<Piece>, String> {
    let mut p = Json::new(json);
    let mut pieces = Vec::new();
    p.expect(b'[')?;
    if p.peek() == Some(b']') {
        return Ok(pieces);
    }
    loop {
        p.expect(b'[')?;
        let text = p.string()?;
        p.expect(b',')?;
        let scale = p.number()?;
        p.expect(b',')?;
        let weight = p.number()?;
        p.expect(b',')?;
        let flags = p.number()?.parse::<u8>().map_err(|e| e.to_string())?;
        p.expect(b',')?;
        let href = p.string()?;
        let indent = if p.peek() == Some(b',') {
            p.next();
            p.number()?
        } else {
            "0".to_owned()
        };
        // An inline atom's detail (LLP 1045.000 D6): its kind and source,
        // marked so the page draws its pill when it adopts this one. Its
        // range is the two numbers after six strings; its paint, its
        // direction and its accessible name, four more.
        let mut atom = None;
        if p.peek() == Some(b',') {
            p.next();
            p.expect(b'[')?;
            let mut fields = Vec::new();
            for n in 0..12 {
                if n > 0 {
                    p.expect(b',')?;
                }
                let range = matches!(n, 6 | 7);
                fields.push(if range { p.number()? } else { p.string()? });
            }
            p.expect(b']')?;
            atom = Some(fields[0].clone());
        }
        p.expect(b']')?;
        pieces.push(Piece {
            text,
            scale,
            weight,
            flags,
            href,
            indent,
            atom,
        });
        match p.next() {
            Some(b',') => continue,
            Some(b']') => return Ok(pieces),
            other => return Err(format!("markup pieces: unexpected {other:?}")),
        }
    }
}

/// A `dataset` row's pairs, the `dataset` capability's reading (LLP 1075.003
/// §3.3): the runner's object of strings (`NativeProps`,
/// LLP 1024 §9), written without whitespace and with JSON's escapes for
/// `"`, `\\` and control characters only. Anything else yields what it
/// parsed so far.
pub fn dataset(json: &str) -> Vec<(String, String)> {
    let mut p = Json::new(json);
    let mut pairs = Vec::new();
    if p.expect(b'{').is_err() {
        return pairs;
    }
    while p.peek() == Some(b'"') {
        let Ok(key) = p.string() else { break };
        let (Ok(()), Ok(value)) = (p.expect(b':'), p.string()) else {
            break;
        };
        pairs.push((key, value));
        if p.peek() == Some(b',') {
            p.next();
        }
    }
    pairs
}

/// A reader of the JSON the host and the runner write themselves: the
/// `markupPieces`, and a `dataset` row ([`dataset`]).
pub(super) struct Json<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) at: usize,
}

impl<'a> Json<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Json {
            bytes: text.as_bytes(),
            at: 0,
        }
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    pub(super) fn next(&mut self) -> Option<u8> {
        let b = self.peek();
        self.at += 1;
        b
    }

    pub(super) fn expect(&mut self, b: u8) -> Result<(), String> {
        match self.next() {
            Some(got) if got == b => Ok(()),
            got => Err(format!(
                "markup pieces: expected `{}`, found {got:?}",
                b as char
            )),
        }
    }

    pub(super) fn number(&mut self) -> Result<String, String> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'.' | b'-' | b'e' | b'E' | b'+')
        ) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?;
        // The glue's `${scale}` and `fontWeight = weight` spell the number
        // as JavaScript does; the host's `css::num` already writes it so.
        exact_num::parse_f64(text)
            .map(|_| text.to_owned())
            .map_err(|e| format!("markup pieces: number {text:?}: {e}"))
    }

    pub(super) fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.at;
            while !matches!(self.peek(), Some(b'"' | b'\\') | None) {
                self.at += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?,
            );
            match self.next() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.next() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'n') => out.push('\n'),
                    Some(b'r') => out.push('\r'),
                    Some(b't') => out.push('\t'),
                    Some(b'u') => {
                        let hex = self
                            .bytes
                            .get(self.at..self.at + 4)
                            .and_then(|h| std::str::from_utf8(h).ok())
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .and_then(char::from_u32)
                            .ok_or("markup pieces: bad \\u escape")?;
                        self.at += 4;
                        out.push(hex);
                    }
                    other => return Err(format!("markup pieces: bad escape {other:?}")),
                },
                _ => return Err("markup pieces: unterminated string".into()),
            }
        }
    }
}
