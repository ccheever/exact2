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
/// `[text, scale, weight, flags, href, indent]`, flags being italic 1,
/// mono 2, strike 4, link 8, quiet (marker or quote) 16, a list marker that
/// hangs in the indent 32, an inline atom 64; the indent in CSS px, left out
/// when 0 unless an atom follows it. An atom (LLP 1045.000 D6) adds
/// `[kind, role, icon, title, href, source, start, end]`, its text being its
/// label and its range UTF-16 in the source; `atoms` and `faces` are the
/// node's `atoms` and `atomFaces` JSON.
pub fn pieces(source: &str, atoms: &str, faces: &str) -> String {
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
    let atoms = exact_markdown::Atoms::parse(atoms, faces);
    for (n, p) in exact_markdown::pieces_with(source, &atoms)
        .iter()
        .enumerate()
    {
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
            )) << 4
            | u8::from(p.hang) << 5
            | u8::from(p.atom.is_some()) << 6;
        out.push_str(&format!(
            ",{},{},{},",
            exact_web::css::num(p.scale),
            p.weight,
            flags
        ));
        quoted(&mut out, &p.href);
        if p.indent != 0.0 || p.atom.is_some() {
            out.push(',');
            out.push_str(&exact_web::css::num(p.indent));
        }
        if let (Some(a), Some(range)) = (&p.atom, p.source) {
            out.push_str(",[");
            let role = match a.role {
                exact_markdown::AtomRole::Button => "button",
                exact_markdown::AtomRole::Link => "link",
            };
            for (i, field) in [a.kind.as_str(), role, &a.icon, &a.title, &a.href, &a.source]
                .into_iter()
                .enumerate()
            {
                if i > 0 {
                    out.push(',');
                }
                quoted(&mut out, field);
            }
            out.push_str(&format!(",{},{}]", range.start, range.end));
        }
        out.push(']');
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::pieces as with_atoms;

    fn pieces(source: &str) -> String {
        with_atoms(source, "", "")
    }

    #[test]
    fn markup_pieces_are_json_the_page_builds_spans_from() {
        let json = pieces("# T \"q\"\n\n**b** [l](https://e.dev/a?b=1) `c`");
        assert_eq!(
            json,
            r#"[["T \"q\"",1.6,700,0,""],["\n",1,0,0,""],["\n",0.5,0,0,""],["b",1,700,0,""],[" ",1,0,0,""],["l",1,0,8,"https://e.dev/a?b=1"],[" ",1,0,0,""],["c",0.92,0,2,""]]"#
        );
        assert_eq!(
            pieces("- a\n\n1. b"),
            r#"[["• ",1,0,48,"",40],["a",1,0,0,"",40],["\n",1,0,0,"",40],["\n",0.5,0,0,""],["1. ",1,0,48,"",40],["b",1,0,0,"",40]]"#
        );
        assert_eq!(pieces(""), "[]");
        assert_eq!(pieces("a\\b\tc"), r#"[["a\\b\tc",1,0,0,""]]"#);
        assert_eq!(pieces("a\u{1}"), r#"[["a\u0001",1,0,0,""]]"#);
    }

    /// An atom piece carries its label, flag 64, its indent and its detail.
    #[test]
    fn an_atom_is_a_piece_with_its_detail() {
        let atoms = r#"[{"name":"citation","link":"t3-citation:"}]"#;
        let faces =
            r#"[{"source":"[q](t3-citation:1)","label":"Quote","icon":"i.svg","title":"T"}]"#;
        assert_eq!(
            with_atoms("a [q](t3-citation:1)", atoms, faces),
            r#"[["a ",1,0,0,""],["Quote",1,0,72,"",0,["citation","button","i.svg","T","t3-citation:1","[q](t3-citation:1)",2,20]]]"#
        );
    }
}
