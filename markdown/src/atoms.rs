//! Inline atoms: spans of the source a field draws and edits as one pill.
//!
//! @ref LLP 1045.000 D1, D2 — an app declares which spans are atoms by their
//! source form (a link with a scheme it names, a relative link, a word after
//! a prefix character) and supplies a face for each, keyed by the atom's whole
//! source text. The styler recognizes the spans from the source on every call,
//! so nothing is kept in step with the text: two spans with equal source are
//! the same atom, and new text is a new key.

use crate::block::B;
use crate::inline::Out;
use crate::{CODE, IMAGE, LINK};
use std::collections::BTreeMap;

/// How an atom kind is written in the source (LLP 1045.000 D1's forms).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomForm {
    /// `[text](dest)` or `![text](dest)` whose destination begins with this
    /// scheme, `:` included, compared ASCII case-insensitively. Lowercase.
    Scheme(String),
    /// `[text](dest)` or `![text](dest)` whose destination has no scheme,
    /// between whitespace, when a face names it.
    Relative,
    /// One of these characters at the start of a word, then a bare run or a
    /// quoted string, followed by whitespace, when a face names it.
    Prefix(Vec<char>),
}

/// What assistive technology calls an atom (LLP 1045.000 D8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AtomRole {
    /// A button: the default.
    #[default]
    Button,
    /// A link.
    Link,
}

/// One declared `atom` kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomKind {
    /// The declaration's name, the `AtomEvent`'s `kind`.
    pub name: String,
    /// Its source form.
    pub form: AtomForm,
    /// Its role.
    pub role: AtomRole,
}

/// What the app draws for an atom (LLP 1045.000 D2). Empty fields take the
/// defaults: the default label, no icon, no tooltip, the kind's paint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AtomFace {
    /// The text drawn in the pill and read by assistive technology.
    pub label: String,
    /// An image source for the icon before the label.
    pub icon: String,
    /// The tooltip and accessible description.
    pub title: String,
    /// The label's colour over the kind's `color`, as text the host reads
    /// as a colour.
    pub color: String,
    /// The pill's fill over the kind's `background-color`, as text the host
    /// reads as a colour.
    pub background_color: String,
}

/// A field's declared kinds and the app's faces, by source text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Atoms {
    /// The kinds, in declaration order.
    pub kinds: Vec<AtomKind>,
    /// The faces, by the atom's whole source text.
    pub faces: BTreeMap<String, AtomFace>,
}

/// An atom the styler recognized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    /// Its source range, UTF-16: for a link from `[` or `![` to `)`.
    pub range: crate::Range,
    /// Its kind, an index into [`Atoms::kinds`].
    pub kind: usize,
    /// Its whole source text, the faces' key.
    pub source: String,
    /// A link atom's destination; empty for a prefix token.
    pub href: String,
    /// The label drawn: the face's, else the default (a link's text, or a
    /// token without its prefix character and quotes).
    pub label: String,
}

impl Atoms {
    /// The kinds the compiler wrote (`atoms`, `[{"name", "link" | "prefix",
    /// "role"}, …]`) and the faces the app bound (`atomFaces`, `[{"source",
    /// "label", "icon", "title", "color", "backgroundColor"}, …]`). Malformed
    /// input reads as none.
    pub fn parse(kinds: &str, faces: &str) -> Self {
        let mut out = Atoms::default();
        if let Some(Json::Array(rows)) = Json::parse(kinds) {
            for row in rows {
                let field = |name: &str| row.get(name).and_then(Json::as_str);
                let Some(name) = field("name").filter(|n| !n.is_empty()) else {
                    continue;
                };
                let form = match (field("link"), field("prefix")) {
                    (Some("relative"), None) => AtomForm::Relative,
                    (Some(scheme), None) if scheme.ends_with(':') && scheme.len() > 1 => {
                        AtomForm::Scheme(scheme.to_ascii_lowercase())
                    }
                    (None, Some(chars)) if !chars.is_empty() => {
                        AtomForm::Prefix(chars.chars().collect())
                    }
                    _ => continue,
                };
                let role = match field("role") {
                    Some("link") => AtomRole::Link,
                    _ => AtomRole::Button,
                };
                out.kinds.push(AtomKind {
                    name: name.to_string(),
                    form,
                    role,
                });
            }
        }
        if let Some(Json::Array(rows)) = Json::parse(faces) {
            for row in rows {
                let field = |name: &str| row.get(name).and_then(Json::as_str).unwrap_or("");
                let source = field("source");
                if source.is_empty() {
                    continue;
                }
                out.faces.insert(
                    source.to_string(),
                    AtomFace {
                        label: field("label").to_string(),
                        icon: field("icon").to_string(),
                        title: field("title").to_string(),
                        color: field("color").to_string(),
                        background_color: field("backgroundColor").to_string(),
                    },
                );
            }
        }
        out
    }

    /// Whether no kind is declared: nothing is an atom.
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// The face for `source`, if the app supplied one.
    pub fn face(&self, source: &str) -> Option<&AtomFace> {
        self.faces.get(source)
    }
}

/// An atom in bytes, as the styler finds it.
#[derive(Clone, Debug)]
pub(crate) struct Found {
    pub range: B,
    pub kind: usize,
    pub href: String,
    /// The default label.
    pub default: String,
}

/// Whether `dest` has a scheme by RFC 3986 §3.1: a letter, then letters,
/// digits, `+`, `-`, `.`, then `:`; one letter and `:` before `\` or `/` is
/// a Windows drive.
fn has_scheme(dest: &str) -> bool {
    let b = dest.as_bytes();
    if !b.first().is_some_and(u8::is_ascii_alphabetic) {
        return false;
    }
    let Some(colon) = b
        .iter()
        .position(|c| !(c.is_ascii_alphanumeric() || b"+-.".contains(c)))
    else {
        return false;
    };
    if b[colon] != b':' {
        return false;
    }
    !(colon == 1 && matches!(b.get(2), Some(b'\\' | b'/')))
}

fn space_before(source: &str, at: usize, line_start: bool) -> bool {
    line_start
        || source[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
}

/// Followed by whitespace: a token at the very end of the source is not one
/// (the reference's `composerInlineTokens.ts`, LLP 1045.000 D1).
fn space_after(source: &str, at: usize) -> bool {
    source[at..].chars().next().is_some_and(char::is_whitespace)
}

/// The atoms of one block, from the inline pass's output, best first by the
/// forms' order: scheme links, relative links, prefix tokens; within a form
/// the earlier start. None crosses a line break or holds another.
pub(crate) fn recognize(source: &str, content: &[B], out: &Out, atoms: &Atoms) -> Vec<Found> {
    // (form order, range, kind, href, default label)
    let mut candidates: Vec<(u8, B, usize, String, String)> = Vec::new();
    let relative = atoms
        .kinds
        .iter()
        .position(|k| k.form == AtomForm::Relative);
    let starts: Vec<usize> = content.iter().map(|r| r.start).collect();
    // The hidden markers inside a range, for a link's default label.
    let label_of = |inner: &B| -> String {
        let mut text = String::new();
        let mut at = inner.start;
        let mut markers: Vec<&B> = out
            .hidden
            .iter()
            .map(|(m, _)| m)
            .filter(|m| m.start >= inner.start && m.end <= inner.end && !m.is_empty())
            .collect();
        markers.sort_by_key(|m| m.start);
        for m in markers {
            if m.start >= at {
                text.push_str(&source[at..m.start]);
                at = m.end;
            }
        }
        text.push_str(&source[at..inner.end]);
        text
    };
    for c in &out.constructs {
        if c.kind & (LINK | IMAGE) == 0 || !source[c.inner.end..].starts_with("](") {
            continue;
        }
        if source[c.outer.clone()].contains('\n') {
            continue;
        }
        let href = c.href.clone().unwrap_or_default();
        let lower = href.to_ascii_lowercase();
        if let Some(kind) = atoms
            .kinds
            .iter()
            .position(|k| matches!(&k.form, AtomForm::Scheme(s) if lower.starts_with(s.as_str())))
        {
            candidates.push((0, c.outer.clone(), kind, href, label_of(&c.inner)));
            continue;
        }
        if let Some(kind) = relative {
            let whole = &source[c.outer.clone()];
            if !has_scheme(&href)
                && !href.starts_with("//")
                && space_before(source, c.outer.start, starts.contains(&c.outer.start))
                && space_after(source, c.outer.end)
                && atoms.faces.contains_key(whole)
            {
                candidates.push((1, c.outer.clone(), kind, href, label_of(&c.inner)));
            }
        }
    }
    // Prefix tokens, outside code, links, images, autolinks, bare URLs and
    // footnote references; inside emphasis, strong and strikethrough too, as
    // the reference finds them in its plain text.
    let mut excluded: Vec<B> = out
        .constructs
        .iter()
        .filter(|c| c.kind & (CODE | LINK | IMAGE) != 0)
        .map(|c| c.outer.clone())
        .collect();
    excluded.extend(
        out.marks
            .iter()
            .filter(|m| m.flags & LINK != 0)
            .map(|m| m.range.clone()),
    );
    excluded.extend(out.footnotes.iter().map(|(r, _)| r.clone()));
    excluded.retain(|x| !x.is_empty());
    let overlaps = |r: &B| excluded.iter().any(|x| x.start < r.end && r.start < x.end);
    for (kind, k) in atoms.kinds.iter().enumerate() {
        let AtomForm::Prefix(chars) = &k.form else {
            continue;
        };
        for range in content {
            let text = &source[range.clone()];
            for (i, c) in text.char_indices() {
                if !chars.contains(&c) {
                    continue;
                }
                let at = range.start + i;
                if !space_before(source, at, i == 0) {
                    continue;
                }
                let body = at + c.len_utf8();
                let Some((end, default)) = token(source, body, range.end, chars) else {
                    continue;
                };
                let r = at..end;
                if !space_after(source, end)
                    || overlaps(&r)
                    || !atoms.faces.contains_key(&source[r.clone()])
                {
                    continue;
                }
                candidates.push((2, r, kind, String::new(), default));
            }
        }
    }
    candidates.sort_by_key(|c| (c.0, c.1.start));
    let mut found: Vec<Found> = Vec::new();
    for (_, range, kind, href, default) in candidates {
        if found
            .iter()
            .any(|f| f.range.start < range.end && range.start < f.range.end)
        {
            continue;
        }
        found.push(Found {
            range,
            kind,
            href,
            default,
        });
    }
    found.sort_by_key(|f| f.range.start);
    found
}

/// A token's end and default label from just after its prefix character:
/// `"` … `"` with `\"` and `\\` escapes and no line break, or one or more
/// code points that are not whitespace, a prefix character or `"`.
fn token(source: &str, body: usize, line_end: usize, chars: &[char]) -> Option<(usize, String)> {
    let rest = &source[body..line_end];
    if let Some(quoted) = rest.strip_prefix('"') {
        let mut label = String::new();
        let mut it = quoted.char_indices();
        while let Some((i, c)) = it.next() {
            match c {
                '"' => return Some((body + 1 + i + 1, label)),
                '\\' => match it.next() {
                    Some((_, e @ ('"' | '\\'))) => label.push(e),
                    Some((_, e)) => {
                        label.push('\\');
                        label.push(e);
                    }
                    None => return None,
                },
                '\n' | '\r' => return None,
                c => label.push(c),
            }
        }
        return None;
    }
    let len = rest
        .char_indices()
        .find(|&(_, c)| c.is_whitespace() || c == '"' || chars.contains(&c))
        .map_or(rest.len(), |(i, _)| i);
    (len > 0).then(|| (body + len, rest[..len].to_string()))
}

/// The JSON the compiler and the runner write for `atoms` and `atomFaces`:
/// arrays, objects, strings, numbers and literals, nothing else assumed.
#[derive(Debug, PartialEq)]
enum Json {
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
    Str(String),
    Other,
}

impl Json {
    fn parse(text: &str) -> Option<Json> {
        let mut p = Reader {
            b: text.as_bytes(),
            at: 0,
        };
        let value = p.value(0)?;
        p.space();
        (p.at == p.b.len()).then_some(value)
    }

    fn get(&self, name: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self.b.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.space();
        let hit = self.b.get(self.at) == Some(&c);
        self.at += usize::from(hit);
        hit
    }

    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > 16 {
            return None;
        }
        self.space();
        match *self.b.get(self.at)? {
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                if self.eat(b']') {
                    return Some(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.eat(b']') {
                        return Some(Json::Array(items));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            b'{' => {
                self.at += 1;
                let mut fields = Vec::new();
                if self.eat(b'}') {
                    return Some(Json::Object(fields));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    if !self.eat(b':') {
                        return None;
                    }
                    fields.push((key, self.value(depth + 1)?));
                    if self.eat(b'}') {
                        return Some(Json::Object(fields));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            b'"' => self.string().map(Json::Str),
            _ => {
                let start = self.at;
                while self
                    .b
                    .get(self.at)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || b"+-.".contains(c))
                {
                    self.at += 1;
                }
                (self.at > start).then_some(Json::Other)
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        if self.b.get(self.at) != Some(&b'"') {
            return None;
        }
        self.at += 1;
        let mut out: Vec<u16> = Vec::new();
        loop {
            let c = *self.b.get(self.at)?;
            self.at += 1;
            match c {
                b'"' => return String::from_utf16(&out).ok(),
                b'\\' => {
                    let e = *self.b.get(self.at)?;
                    self.at += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(u16::from(e)),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'u' => {
                            let hex =
                                std::str::from_utf8(self.b.get(self.at..self.at + 4)?).ok()?;
                            out.push(u16::from_str_radix(hex, 16).ok()?);
                            self.at += 4;
                        }
                        _ => return None,
                    }
                }
                _ => {
                    // The rest of a UTF-8 sequence, copied whole.
                    let start = self.at - 1;
                    let len = match c {
                        0..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    let s = std::str::from_utf8(self.b.get(start..start + len)?).ok()?;
                    out.extend(s.encode_utf16());
                    self.at = start + len;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "atoms_tests.rs"]
mod tests;
