//! A strict XML tokenizer for SVG documents (LLP 1011.001 D2): elements,
//! attributes and character data as events, with the five predefined
//! entities and numeric references decoded, comments and processing
//! instructions skipped, CDATA read as text, and a DOCTYPE accepted only
//! without an internal subset (an entity declaration is the file's way into
//! unbounded expansion, refused). A closing tag must match its opening one:
//! a designer's export is well-formed, and a file that is not is refused
//! with its line, as a browser refuses it.

use crate::Error;

/// One event of the document, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// An element's start, with its attributes in document order; `empty`
    /// for `<a/>`, which has no end event.
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        empty: bool,
        line: usize,
    },
    /// An element's end.
    End { name: String, line: usize },
    /// Character data, entities decoded; CDATA as written.
    Text(String),
}

/// The events of `text`, or the first error with its line.
pub fn tokenize(text: &str) -> Result<Vec<Event>, Error> {
    let mut t = Tokenizer {
        src: text.trim_start_matches('\u{feff}'),
        at: 0,
        line: 1,
        out: Vec::new(),
        open: Vec::new(),
    };
    t.run()?;
    if let Some((name, line)) = t.open.pop() {
        return Err(Error::at(line, format!("`<{name}>` is never closed")));
    }
    Ok(t.out)
}

/// The root start tag alone: its name and attributes, from a prefix of the
/// document, for a header read that stops before the content (D4's
/// metadata). `None` when no start tag lies in the prefix.
pub fn root_tag(prefix: &str) -> Option<(String, Vec<(String, String)>)> {
    let mut t = Tokenizer {
        src: prefix.trim_start_matches('\u{feff}'),
        at: 0,
        line: 1,
        out: Vec::new(),
        open: Vec::new(),
    };
    while t.at < t.src.len() {
        if t.step().is_err() {
            break;
        }
        if let Some(Event::Start { name, attrs, .. }) = t.out.first() {
            return Some((name.clone(), attrs.clone()));
        }
    }
    None
}

struct Tokenizer<'a> {
    src: &'a str,
    at: usize,
    line: usize,
    out: Vec<Event>,
    open: Vec<(String, usize)>,
}

impl<'a> Tokenizer<'a> {
    fn run(&mut self) -> Result<(), Error> {
        while self.at < self.src.len() {
            self.step()?;
        }
        Ok(())
    }

    /// The unread text: a borrow of the source alone, not of the tokenizer.
    fn rest(&self) -> &'a str {
        &self.src[self.at..]
    }

    fn advance(&mut self, n: usize) {
        self.line += self.src[self.at..self.at + n].matches('\n').count();
        self.at += n;
    }

    fn err(&self, message: impl Into<String>) -> Error {
        Error::at(self.line, message)
    }

    /// Skip to just past `end`, or fail naming what was never closed.
    fn skip_past(&mut self, end: &str, what: &str) -> Result<(), Error> {
        match self.rest().find(end) {
            Some(i) => {
                self.advance(i + end.len());
                Ok(())
            }
            None => Err(self.err(format!("{what} is never closed"))),
        }
    }

    fn step(&mut self) -> Result<(), Error> {
        let rest = self.rest();
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = decode(&rest[..end], self.line)?;
            if !self.open.is_empty() {
                self.out.push(Event::Text(text));
            } else if !text.trim().is_empty() {
                return Err(self.err("text outside the root element"));
            }
            self.advance(end);
            return Ok(());
        }
        if rest.starts_with("<!--") {
            return self.skip_past("-->", "a comment");
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let Some(i) = body.find("]]>") else {
                return Err(self.err("a CDATA section is never closed"));
            };
            self.out.push(Event::Text(body[..i].to_string()));
            self.advance(9 + i + 3);
            return Ok(());
        }
        if rest.starts_with("<?") {
            return self.skip_past("?>", "a processing instruction");
        }
        if rest.starts_with("<!DOCTYPE") || rest.starts_with("<!doctype") {
            // The internal subset (`[...]`) may declare entities: refused,
            // as the one way a small file expands without bound.
            let close = rest
                .find('>')
                .ok_or_else(|| self.err("the DOCTYPE is never closed"))?;
            if let Some(open) = rest[..close].find('[') {
                let subset_end = rest
                    .find("]>")
                    .ok_or_else(|| self.err("the DOCTYPE's internal subset is never closed"))?;
                if rest[open..subset_end].contains("<!ENTITY") {
                    return Err(self.err("an entity declaration in the DOCTYPE is refused: only the predefined entities and numeric references are read"));
                }
                self.advance(subset_end + 2);
                return Ok(());
            }
            self.advance(close + 1);
            return Ok(());
        }
        if rest.starts_with("<!") {
            return Err(self.err("a declaration other than DOCTYPE is refused"));
        }
        if let Some(after) = rest.strip_prefix("</") {
            let close = after
                .find('>')
                .ok_or_else(|| self.err("an end tag is never closed"))?;
            let name = after[..close].trim().to_string();
            let line = self.line;
            match self.open.pop() {
                Some((opened, _)) if opened == name => {}
                Some((opened, at)) => {
                    return Err(self.err(format!("`</{name}>` closes `<{opened}>` from line {at}")));
                }
                None => return Err(self.err(format!("`</{name}>` closes nothing"))),
            }
            self.out.push(Event::End { name, line });
            self.advance(2 + close + 1);
            return Ok(());
        }
        self.start_tag()
    }

    fn start_tag(&mut self) -> Result<(), Error> {
        let line = self.line;
        let rest = self.rest();
        let bytes = rest.as_bytes();
        let mut i = 1;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let name = &rest[1..i];
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'))
        {
            return Err(self.err("a tag name is malformed"));
        }
        let mut attrs = Vec::new();
        loop {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(self.err(format!("`<{name}>` is never closed")));
            }
            if bytes[i] == b'>' {
                i += 1;
                break;
            }
            if bytes[i] == b'/' {
                if bytes.get(i + 1) != Some(&b'>') {
                    return Err(self.err(format!("`<{name}>`: `/` is not followed by `>`")));
                }
                let empty = Event::Start {
                    name: name.to_string(),
                    attrs,
                    empty: true,
                    line,
                };
                self.out.push(empty);
                self.advance(i + 2);
                return Ok(());
            }
            let start = i;
            while i < bytes.len()
                && bytes[i] != b'='
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            let key = &rest[start..i];
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if bytes.get(i) != Some(&b'=') {
                return Err(self.err(format!("`<{name}>`: attribute `{key}` has no value")));
            }
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let quote = match bytes.get(i) {
                Some(b'"') => b'"',
                Some(b'\'') => b'\'',
                _ => return Err(self.err(format!("`<{name}>`: attribute `{key}` is not quoted"))),
            };
            let value_start = i + 1;
            let Some(len) = bytes[value_start..].iter().position(|b| *b == quote) else {
                return Err(self.err(format!("`<{name}>`: attribute `{key}` is never closed")));
            };
            let raw = &rest[value_start..value_start + len];
            attrs.push((key.to_string(), decode(raw, line)?));
            i = value_start + len + 1;
        }
        self.open.push((name.to_string(), line));
        self.out.push(Event::Start {
            name: name.to_string(),
            attrs,
            empty: false,
            line,
        });
        self.advance(i);
        Ok(())
    }
}

/// `raw` with its entity and character references decoded: the five
/// predefined entities, `&#NNN;` and `&#xHH;`; anything else is refused,
/// since no declaration could have defined it.
pub fn decode(raw: &str, line: usize) -> Result<String, Error> {
    if !raw.contains('&') {
        return Ok(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(end) = after.find(';').filter(|e| *e <= 10) else {
            return Err(Error::at(line, "a `&` that starts no reference"));
        };
        let name = &after[..end];
        let decoded = match name {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()));
                match code.and_then(char::from_u32) {
                    Some(c) => c,
                    None => {
                        return Err(Error::at(
                            line,
                            format!(
                                "the reference `&{name};` names no predefined entity or character"
                            ),
                        ))
                    }
                }
            }
        };
        out.push(decoded);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_text_and_references_in_order() {
        let events = tokenize("<?xml version=\"1.0\"?>\n<!-- a -->\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox='0 0 2 2'>\n  <path d=\"M0 0h1\"/>\n  <text>A &amp; B &#65;&#x42;<![CDATA[<c>]]></text>\n</svg>").unwrap();
        assert!(
            matches!(&events[0], Event::Start { name, attrs, empty: false, line: 3 } if name == "svg" && attrs[1] == ("viewBox".into(), "0 0 2 2".into()))
        );
        assert!(
            matches!(&events[2], Event::Start { name, empty: true, line: 4, .. } if name == "path")
        );
        assert_eq!(events[5], Event::Text("A & B AB".into()));
        assert_eq!(events[6], Event::Text("<c>".into()));
        assert!(matches!(&events[7], Event::End { name, .. } if name == "text"));
    }

    #[test]
    fn a_mismatched_close_an_unclosed_tag_and_an_entity_declaration_are_refused_with_a_line() {
        assert_eq!(
            tokenize("<svg>\n<g>\n</svg>").unwrap_err().to_string(),
            "line 3: `</svg>` closes `<g>` from line 2"
        );
        assert_eq!(
            tokenize("<svg>\n<g>").unwrap_err().to_string(),
            "line 2: `<g>` is never closed"
        );
        let doctype = "<!DOCTYPE svg [ <!ENTITY a \"aaaa\"> ]>\n<svg/>";
        assert!(tokenize(doctype)
            .unwrap_err()
            .to_string()
            .contains("entity declaration"));
        assert!(
            tokenize("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\">\n<svg/>").is_ok()
        );
        assert!(tokenize("<svg>&nbsp;</svg>")
            .unwrap_err()
            .to_string()
            .contains("&nbsp;"));
    }

    #[test]
    fn the_root_tag_is_read_from_a_prefix() {
        let (name, attrs) =
            root_tag("<?xml version=\"1.0\"?><svg width=\"10\" height=\"5\"><rect").unwrap();
        assert_eq!(name, "svg");
        assert_eq!(
            attrs,
            vec![
                ("width".to_string(), "10".to_string()),
                ("height".to_string(), "5".to_string())
            ]
        );
        assert!(root_tag("<?xml version=\"1.0\"?><!-- only").is_none());
    }
}
