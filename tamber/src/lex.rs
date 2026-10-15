//! Lines, with Contract's layout rules (`contract/syntax/src/lexer.rs`):
//! spaces indent, `//` starts a comment, blank lines are skipped. Contract's
//! lexer makes tokens, and a `.tamber` value is CSS text read whole (§3),
//! so the structure is lexed here and each value kept as its text.

use crate::Diagnostic;

/// One logical line: its indent in spaces, its text without the comment,
/// and where it starts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    pub indent: usize,
    pub text: String,
    pub line: u32,
    /// The column of `text`'s first character, 1-based.
    pub col: u32,
}

/// The source's lines, or the first refusal on each line that has one.
pub(crate) fn lines(src: &str) -> (Vec<Line>, Vec<Diagnostic>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let line = i as u32 + 1;
        let body = strip_comment(raw).trim_end();
        let text = body.trim_start_matches(' ');
        if text.is_empty() {
            continue;
        }
        let indent = body.len() - text.len();
        if text.starts_with('\t') {
            errors.push(Diagnostic::new(
                "tamber-indent",
                "indent with spaces, as Contract does",
                line,
                indent as u32 + 1,
            ));
            continue;
        }
        out.push(Line {
            indent,
            text: text.to_string(),
            line,
            col: indent as u32 + 1,
        });
    }
    (out, errors)
}

/// The line up to a `//` outside a string and outside parentheses (so a
/// URL in `icon(svg "…")` or `url(…)` keeps its slashes).
fn strip_comment(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for i in 0..bytes.len() {
        if std::mem::take(&mut escaped) {
            continue;
        }
        match bytes[i] {
            b'\\' if quoted => escaped = true,
            b'"' => quoted = !quoted,
            b'(' if !quoted => depth += 1,
            b')' if !quoted => depth = depth.saturating_sub(1),
            b'/' if !quoted && depth == 0 && bytes.get(i + 1) == Some(&b'/') => {
                return &raw[..i];
            }
            _ => {}
        }
    }
    raw
}

/// Split `text` at the first top-level `sep` (outside parentheses and
/// strings).
pub(crate) fn split_top(text: &str, sep: char) -> Option<(&str, &str)> {
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for (i, c) in text.char_indices() {
        if std::mem::take(&mut escaped) {
            continue;
        }
        match c {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth = depth.saturating_sub(1),
            c if c == sep && !quoted && depth == 0 => {
                return Some((&text[..i], &text[i + c.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

/// The byte offsets of each top-level word of `text` (separated by
/// spaces outside parentheses and strings).
pub(crate) fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let (mut depth, mut quoted, mut start, mut escaped) = (0usize, false, None, false);
    for (i, c) in text.char_indices() {
        if std::mem::take(&mut escaped) {
            continue;
        }
        match c {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth = depth.saturating_sub(1),
            ' ' if !quoted && depth == 0 => {
                if let Some(s) = start.take() {
                    out.push((s, &text[s..i]));
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

/// `a = b` as `a=b`: Contract's `=` is its own token, so spaces around it
/// say nothing.
pub(crate) fn tight_equals(text: &str) -> String {
    let parts: Vec<&str> = text.split('=').collect();
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let p = if i > 0 { p.trim_start() } else { p };
        out.push(if i + 1 < parts.len() { p.trim_end() } else { p });
    }
    out.join("=")
}

/// A Contract string's contents (`contract/syntax/src/lexer.rs`): quotes
/// off and `\n`, `\t`, `\"`, `\\`, `` \` `` and `\$` decoded; another escape
/// is refused.
pub(crate) fn unquote(s: &str) -> Result<String, String> {
    let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return Ok(s.to_string());
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match chars.next() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some(c @ ('"' | '\\' | '`' | '$')) => c,
            Some(c) => return Err(format!("unknown escape `\\{c}`")),
            None => return Err("a `\\` ends the string, escaping nothing".into()),
        });
    }
    Ok(out)
}

/// An identifier as Contract writes one: a letter or `_` first, then
/// letters, digits, `_` and `-`.
pub(crate) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A token name: identifiers joined by `.`.
pub(crate) fn is_name(s: &str) -> bool {
    !s.is_empty() && s.split('.').all(is_ident)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_end_outside_strings_and_parentheses() {
        assert_eq!(strip_comment("a = #fff // why"), "a = #fff ");
        assert_eq!(
            strip_comment("i = icon(svg \"a//b\")"),
            "i = icon(svg \"a//b\")"
        );
        assert_eq!(
            strip_comment("u = url(http://x) // y"),
            "u = url(http://x) "
        );
    }

    #[test]
    fn words_keep_functions_whole() {
        let w: Vec<&str> = words("color: #fff in 0..2 label=\"a b\"")
            .into_iter()
            .map(|(_, w)| w)
            .collect();
        assert_eq!(w, ["color:", "#fff", "in", "0..2", "label=\"a b\""]);
        let w: Vec<&str> = words("color-mix(in oklab, a 50%, b) in")
            .into_iter()
            .map(|(_, w)| w)
            .collect();
        assert_eq!(w, ["color-mix(in oklab, a 50%, b)", "in"]);
        let w: Vec<&str> = words("label=\"say \\\"hi there\\\"\" step=1")
            .into_iter()
            .map(|(_, w)| w)
            .collect();
        assert_eq!(w, ["label=\"say \\\"hi there\\\"\"", "step=1"]);
        assert_eq!(unquote("\"say \\\"hi\\\"\"").unwrap(), "say \"hi\"");
        assert_eq!(unquote("\"a\\nb\"").unwrap(), "a\nb");
        assert!(unquote("\"a\\qb\"").is_err());
        assert_eq!(
            tight_equals("density = compact  dark-style= dim"),
            "density=compact  dark-style=dim"
        );
    }
}
