//! Markdown on the web: a `markup="markdown"` text node's source as pieces.
//!
//! @ref LLP 1045 D3, D4 (the page builds the pieces into spans, never HTML)

use exact_web::Linked;

/// Link Markdown into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.markup = Some(pieces);
    linked
}

/// The pieces of a Markdown source as a JSON array of
/// `[text, scale, weight, flags, href]`, flags being italic 1, mono 2,
/// strike 4, link 8, quiet (marker or quote) 16.
pub fn pieces(source: &str) -> String {
    fn quoted(out: &mut String, s: &str) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    let mut out = String::from("[");
    for (n, p) in exact_markdown::pieces(source).iter().enumerate() {
        if n > 0 {
            out.push(',');
        }
        out.push('[');
        quoted(&mut out, &p.text);
        let flags = u8::from(p.italic)
            | u8::from(p.mono) << 1
            | u8::from(p.strike) << 2
            | u8::from(p.role == exact_markdown::Role::Link) << 3
            | u8::from(matches!(
                p.role,
                exact_markdown::Role::Marker | exact_markdown::Role::Quote
            )) << 4;
        out.push_str(&format!(
            ",{},{},{},",
            exact_web::css::num(p.scale),
            p.weight,
            flags
        ));
        quoted(&mut out, &p.href);
        out.push(']');
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::pieces;

    #[test]
    fn markup_pieces_are_json_the_page_builds_spans_from() {
        let json = pieces("# T \"q\"\n\n**b** [l](https://e.dev/a?b=1) `c`");
        assert_eq!(
            json,
            r#"[["T \"q\"",1.6,700,0,""],["\n",1,0,0,""],["\n",0.5,0,0,""],["b",1,700,0,""],[" ",1,0,0,""],["l",1,0,8,"https://e.dev/a?b=1"],[" ",1,0,0,""],["c",0.92,0,2,""]]"#
        );
        assert_eq!(pieces(""), "[]");
        assert_eq!(pieces("a\\b\tc"), r#"[["a\\b\tc",1,0,0,""]]"#);
        assert_eq!(pieces("a\u{1}"), r#"[["a\u0001",1,0,0,""]]"#);
    }
}
