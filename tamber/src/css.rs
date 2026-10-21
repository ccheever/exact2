//! CSS value text (LLP 1121 §3): what follows a token's `=`, read into
//! items, with dotted token names where CSS would write `var()`.

/// One component of a CSS value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    /// An identifier, or a dotted token name.
    Ident(String),
    /// `#rrggbb` and the like, without the `#`.
    Hash(String),
    /// A number.
    Number(f64),
    /// A percentage, as its number (`50%` is 50).
    Percent(f64),
    /// A dimension: a number and its unit (`12px`, `90deg`).
    Dim(f64, String),
    /// A quoted string, unescaped.
    Str(String),
    /// A function: its lower-cased name, its comma-separated arguments,
    /// and its text as written.
    Func(String, Vec<Vec<Item>>, String),
    /// `/`
    Slash,
    /// `+`, `-` or `*` between calc operands.
    Op(char),
}

/// Read `text` as space-separated items, or say why it is not a value.
pub(crate) fn parse(text: &str) -> Result<Vec<Item>, String> {
    let mut p = Reader {
        s: text.as_bytes(),
        text,
        i: 0,
    };
    let items = p.items(false)?;
    if p.i < p.s.len() {
        return Err(format!("`{}` is not CSS value text", &text[p.i..]));
    }
    if items.is_empty() {
        return Err("an empty value".into());
    }
    Ok(items)
}

struct Reader<'a> {
    s: &'a [u8],
    text: &'a str,
    i: usize,
}

fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn ident_char(b: u8) -> bool {
    ident_start(b) || b.is_ascii_digit() || b == b'-'
}

impl Reader<'_> {
    fn peek(&self, at: usize) -> Option<u8> {
        self.s.get(self.i + at).copied()
    }

    fn skip_space(&mut self) {
        while self.peek(0).is_some_and(|b| b.is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    /// Items up to `,` or `)` inside a function, or the end at top level.
    fn items(&mut self, nested: bool) -> Result<Vec<Item>, String> {
        let mut out = Vec::new();
        loop {
            self.skip_space();
            match self.peek(0) {
                None => return Ok(out),
                Some(b',' | b')') if nested => return Ok(out),
                Some(b',' | b')') => {
                    return Err(format!(
                        "`{}` outside a function",
                        self.peek(0).unwrap() as char
                    ))
                }
                _ => out.push(self.item()?),
            }
        }
    }

    fn item(&mut self) -> Result<Item, String> {
        let b = self.peek(0).unwrap_or(0);
        let start = self.i;
        if b == b'/' {
            self.i += 1;
            return Ok(Item::Slash);
        }
        if b == b'*' {
            self.i += 1;
            return Ok(Item::Op('*'));
        }
        if b == b'(' {
            // A calc() group, `(r + g)`: a function with no name.
            return self.call(start, "");
        }
        if b == b'#' {
            self.i += 1;
            while self.peek(0).is_some_and(|b| b.is_ascii_alphanumeric()) {
                self.i += 1;
            }
            return Ok(Item::Hash(
                self.text[start + 1..self.i].to_ascii_lowercase(),
            ));
        }
        if b == b'"' {
            let end = self.text[start + 1..]
                .find('"')
                .ok_or("a string with no closing `\"`")?;
            self.i = start + 1 + end + 1;
            return Ok(Item::Str(self.text[start + 1..start + 1 + end].to_string()));
        }
        let signed = matches!(b, b'+' | b'-');
        let after_sign = if signed { self.peek(1) } else { Some(b) };
        let numeric = after_sign.is_some_and(|c| c.is_ascii_digit())
            || (after_sign == Some(b'.')
                && self
                    .peek(if signed { 2 } else { 1 })
                    .is_some_and(|c| c.is_ascii_digit()));
        if numeric {
            return self.number();
        }
        if signed && self.peek(1).is_none_or(|c| c.is_ascii_whitespace()) {
            self.i += 1;
            return Ok(Item::Op(b as char));
        }
        if ident_start(b)
            || (b == b'-' && self.peek(1).is_some_and(|c| ident_start(c) || c == b'-'))
        {
            return self.ident();
        }
        Err(format!("`{}` is not CSS value text", &self.text[start..]))
    }

    fn number(&mut self) -> Result<Item, String> {
        let start = self.i;
        if matches!(self.peek(0), Some(b'+' | b'-')) {
            self.i += 1;
        }
        while self.peek(0).is_some_and(|b| b.is_ascii_digit()) {
            self.i += 1;
        }
        if self.peek(0) == Some(b'.') && self.peek(1).is_some_and(|b| b.is_ascii_digit()) {
            self.i += 1;
            while self.peek(0).is_some_and(|b| b.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(0), Some(b'e' | b'E')) {
            let sign = usize::from(matches!(self.peek(1), Some(b'+' | b'-')));
            if self.peek(1 + sign).is_some_and(|b| b.is_ascii_digit()) {
                self.i += 1 + sign;
                while self.peek(0).is_some_and(|b| b.is_ascii_digit()) {
                    self.i += 1;
                }
            }
        }
        let n = exact_num::parse_f64(&self.text[start..self.i])
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("`{}` is not a number", &self.text[start..self.i]))?;
        if self.peek(0) == Some(b'%') {
            self.i += 1;
            return Ok(Item::Percent(n));
        }
        let unit = self.i;
        while self.peek(0).is_some_and(|b| b.is_ascii_alphabetic()) {
            self.i += 1;
        }
        if self.i > unit {
            return Ok(Item::Dim(n, self.text[unit..self.i].to_ascii_lowercase()));
        }
        Ok(Item::Number(n))
    }

    fn ident(&mut self) -> Result<Item, String> {
        let start = self.i;
        self.i += 1;
        loop {
            match self.peek(0) {
                Some(b) if ident_char(b) => self.i += 1,
                // A dotted token name: `surface.page`.
                Some(b'.') if self.peek(1).is_some_and(ident_start) => self.i += 1,
                _ => break,
            }
        }
        let name = &self.text[start..self.i];
        if self.peek(0) != Some(b'(') {
            return Ok(Item::Ident(name.to_string()));
        }
        self.call(start, name)
    }

    /// `name(` at `self.i`, through its `)`.
    fn call(&mut self, start: usize, name: &str) -> Result<Item, String> {
        self.i += 1;
        if name.eq_ignore_ascii_case("-exact-platform-color") {
            return self.platform(start, name);
        }
        let mut args = Vec::new();
        loop {
            args.push(self.items(true)?);
            match self.peek(0) {
                Some(b',') => self.i += 1,
                Some(b')') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(format!("`{name}(` has no closing `)`")),
            }
        }
        if args.len() == 1 && args[0].is_empty() {
            args.clear();
        }
        Ok(Item::Func(
            name.to_ascii_lowercase(),
            args,
            self.text[start..self.i].to_string(),
        ))
    }
}

impl Reader<'_> {
    /// `-exact-platform-color(ios <name>, …, <fallback>)`: the platform
    /// names are the kernel's text (`named:My Asset` included), kept as
    /// written; only the fallback is CSS value text.
    fn platform(&mut self, start: usize, name: &str) -> Result<Item, String> {
        let text = self.text;
        let open = self.i;
        let (mut depth, mut quoted) = (0usize, false);
        let close = text[open..]
            .char_indices()
            .find(|&(_, c)| {
                match c {
                    '"' => quoted = !quoted,
                    '(' if !quoted => depth += 1,
                    ')' if !quoted && depth == 0 => return true,
                    ')' if !quoted => depth -= 1,
                    _ => {}
                }
                false
            })
            .map(|(i, _)| open + i)
            .ok_or_else(|| format!("`{name}(` has no closing `)`"))?;
        let mut parts = Vec::new();
        let mut rest = &text[open..close];
        while let Some((part, tail)) = crate::lex::split_top(rest, ',') {
            parts.push(part);
            rest = tail;
        }
        let mut args: Vec<Vec<Item>> = parts
            .iter()
            .map(|p| vec![Item::Str(p.trim().to_string())])
            .collect();
        args.push(parse(rest.trim())?);
        self.i = close + 1;
        Ok(Item::Func(
            name.to_ascii_lowercase(),
            args,
            text[start..self.i].to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_functions_names_and_numbers() {
        let items = parse("color-mix(in oklab, accent 88%, #0b0f14)").unwrap();
        let [Item::Func(name, args, text)] = items.as_slice() else {
            panic!("{items:?}")
        };
        assert_eq!(name, "color-mix");
        assert_eq!(text, "color-mix(in oklab, accent 88%, #0b0f14)");
        assert_eq!(
            args[0],
            [Item::Ident("in".into()), Item::Ident("oklab".into())]
        );
        assert_eq!(args[1], [Item::Ident("accent".into()), Item::Percent(88.0)]);
        assert_eq!(args[2], [Item::Hash("0b0f14".into())]);
        assert_eq!(
            parse("surface.page -exact-label 1e2 -.5 12px / a - b").unwrap(),
            [
                Item::Ident("surface.page".into()),
                Item::Ident("-exact-label".into()),
                Item::Number(100.0),
                Item::Number(-0.5),
                Item::Dim(12.0, "px".into()),
                Item::Slash,
                Item::Ident("a".into()),
                Item::Op('-'),
                Item::Ident("b".into()),
            ]
        );
        assert!(parse("rgb(1 2 3").is_err());
        assert!(parse("a)").is_err());
    }
}
