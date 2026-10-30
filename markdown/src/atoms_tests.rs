use crate::{
    pieces_with, plain_range, plain_with, style_with, AtomForm, AtomRole, Atoms, ParagraphKind,
    Range,
};

const KINDS: &str = r#"[{"name":"citation","link":"t3-citation:"},{"name":"file","link":"relative"},{"name":"mention","prefix":"@","role":"link"},{"name":"skill","prefix":"$€"}]"#;

fn atoms(faces: &[(&str, &str)]) -> Atoms {
    let faces: Vec<String> = faces
        .iter()
        .map(|(s, l)| {
            format!(
                r#"{{"source":{},"label":{},"icon":"","title":""}}"#,
                json(s),
                json(l)
            )
        })
        .collect();
    Atoms::parse(KINDS, &format!("[{}]", faces.join(",")))
}

fn json(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// (source text, kind name, label) of each atom.
fn found(source: &str, atoms: &Atoms) -> Vec<(String, String, String)> {
    style_with(source, None, atoms)
        .atoms
        .into_iter()
        .map(|a| (a.source, atoms.kinds[a.kind].name.clone(), a.label))
        .collect()
}

fn one(source: &str, kind: &str, label: &str) -> Vec<(String, String, String)> {
    vec![(source.into(), kind.into(), label.into())]
}

#[test]
fn the_compiled_kinds_and_the_faces_parse() {
    let a = Atoms::parse(
        KINDS,
        r#"[{"source":"@x","label":"X \u00e9","icon":"a.svg","title":"t"}, {"source":"","label":"none"}]"#,
    );
    assert_eq!(a.kinds.len(), 4);
    assert_eq!(a.kinds[0].form, AtomForm::Scheme("t3-citation:".into()));
    assert_eq!(a.kinds[1].form, AtomForm::Relative);
    assert_eq!(a.kinds[2].form, AtomForm::Prefix(vec!['@']));
    assert_eq!(a.kinds[2].role, AtomRole::Link);
    assert_eq!(a.kinds[3].form, AtomForm::Prefix(vec!['$', '€']));
    assert_eq!(a.faces.len(), 1);
    assert_eq!(a.faces["@x"].label, "X é");
    assert_eq!(a.faces["@x"].icon, "a.svg");
    assert_eq!(a.faces["@x"].color, "", "no paint: its kind's");
    assert!(Atoms::parse("not json", "[").is_empty());
    let painted = Atoms::parse(
        KINDS,
        r##"[{"source":"@x","label":"","icon":"","title":"","color":"light-dark(#d52c36, #ff6762)","backgroundColor":"#d52c361c"}]"##,
    );
    assert_eq!(painted.faces["@x"].color, "light-dark(#d52c36, #ff6762)");
    assert_eq!(painted.faces["@x"].background_color, "#d52c361c");
    assert_eq!(
        painted.faces["@x"].direction, "",
        "no direction: its line's"
    );
    // A face pins its pill's direction with `ltr` or `rtl`; any other word
    // follows the line, as an empty one does.
    for (said, kept) in [("ltr", "ltr"), ("rtl", "rtl"), ("auto", ""), ("RTL", "")] {
        let faces = format!(
            r#"[{{"source":"@x","label":"","icon":"","title":"","color":"","backgroundColor":"","direction":"{said}"}}]"#
        );
        assert_eq!(
            Atoms::parse(KINDS, &faces).faces["@x"].direction,
            kept,
            "{said}"
        );
    }
}

#[test]
fn a_scheme_link_is_an_atom_without_a_face_and_its_syntax_is_not_hidden_inside() {
    let a = atoms(&[]);
    let s = "see [Quote](T3-Citation://v1/abc) here";
    assert_eq!(
        found(s, &a),
        one("[Quote](T3-Citation://v1/abc)", "citation", "Quote")
    );
    let styled = style_with(s, None, &a);
    assert_eq!(styled.atoms[0].range, Range::new(4, 33));
    assert_eq!(styled.atoms[0].href, "T3-Citation://v1/abc");
    assert!(styled.hidden.is_empty(), "{:?}", styled.hidden);
    assert!(styled.links.is_empty());
    // Without atoms the same link hides its syntax as before.
    assert_eq!(crate::style(s, None).hidden.len(), 2);
    // A face's label wins; an empty one keeps the default.
    let faced = atoms(&[("[Quote](T3-Citation://v1/abc)", "A quote")]);
    assert_eq!(found(s, &faced)[0].2, "A quote");
}

#[test]
fn gated_forms_are_atoms_only_when_a_face_names_their_source() {
    let s = "open [main.rs](src/main.rs) and @src/lib.rs now";
    assert!(found(s, &atoms(&[])).is_empty());
    let a = atoms(&[("[main.rs](src/main.rs)", ""), ("@src/lib.rs", "lib.rs")]);
    assert_eq!(
        found(s, &a),
        vec![
            (
                "[main.rs](src/main.rs)".into(),
                "file".into(),
                "main.rs".into()
            ),
            ("@src/lib.rs".into(), "mention".into(), "lib.rs".into()),
        ]
    );
}

#[test]
fn a_gated_token_needs_whitespace_around_it() {
    let a = atoms(&[("@x", ""), ("[a](b.rs)", "")]);
    // At the very end of the source, before punctuation, or after a letter: text.
    for s in [
        "@x",
        "say @x.",
        "a@x b",
        "**@x** b",
        "x[a](b.rs) c",
        "[a](b.rs)",
    ] {
        assert!(found(s, &a).is_empty(), "{s}");
    }
    // At a line's start and before a line break: atoms.
    assert_eq!(found("@x\nmore", &a), one("@x", "mention", "x"));
    assert_eq!(found("- @x \n", &a), one("@x", "mention", "x"));
    assert_eq!(found("[a](b.rs)\n", &a), one("[a](b.rs)", "file", "a"));
}

#[test]
fn a_relative_link_has_no_scheme_but_may_name_a_windows_drive() {
    let a = atoms(&[
        ("[x](C:/a.rs)", ""),
        ("[y](https://e.dev)", ""),
        ("[z](//e.dev/a)", ""),
        ("[w](mailto:a@b)", ""),
    ]);
    assert_eq!(found("[x](C:/a.rs) ", &a), one("[x](C:/a.rs)", "file", "x"));
    for s in ["[y](https://e.dev) ", "[z](//e.dev/a) ", "[w](mailto:a@b) "] {
        assert!(found(s, &a).is_empty(), "{s}");
    }
}

#[test]
fn a_quoted_token_takes_escapes_and_a_prefix_may_name_several_characters() {
    let a = atoms(&[(r#"@"a \"b\" c""#, ""), ("€tidy", ""), ("$tidy", "")]);
    assert_eq!(
        found(r#"use @"a \"b\" c" now"#, &a),
        one(r#"@"a \"b\" c""#, "mention", r#"a "b" c"#)
    );
    assert_eq!(found("run €tidy ", &a), one("€tidy", "skill", "tidy"));
    assert_eq!(found("run $tidy ", &a), one("$tidy", "skill", "tidy"));
    // An unclosed quote, or one across a line break, is none.
    assert!(found("@\"a b", &a).is_empty());
}

#[test]
fn a_token_is_never_in_code_a_link_an_autolink_or_a_footnote() {
    let a = atoms(&[("@x", "")]);
    for s in [
        "`@x` b",
        "[@x ](u) b",
        "[t](@x) b",
        "<https://e.dev/@x> b",
        "```\n@x \n```",
        "[^@x] b",
    ] {
        assert!(found(s, &a).is_empty(), "{s}");
    }
}

#[test]
fn a_token_inside_emphasis_strong_or_strikethrough_is_an_atom() {
    let a = atoms(&[("@x", ""), ("$skill", "")]);
    assert_eq!(found("*see @x here*", &a), one("@x", "mention", "x"));
    assert_eq!(found("**see @x here**", &a), one("@x", "mention", "x"));
    assert_eq!(
        found("~~see $skill now~~", &a),
        one("$skill", "skill", "skill")
    );
    // Its formats are the label's: the atom is one bold piece.
    let pieces = pieces_with("**see @x here**", &a);
    let atom = pieces.iter().find(|p| p.atom.is_some()).unwrap();
    assert_eq!((atom.text.as_str(), atom.weight), ("x", 700));
}

#[test]
fn a_scheme_link_wins_over_a_relative_one_and_a_link_over_a_token() {
    // The token inside the citation's label is not an atom of its own.
    let a = atoms(&[("@x", "")]);
    let s = "[ @x ](t3-citation:1) b";
    assert_eq!(
        found(s, &a),
        one(s.trim_end_matches(" b"), "citation", " @x ")
    );
    // An image inside a citation link: the outer link, which starts first.
    let s = "[![i](t3-citation:2)](t3-citation:1) b";
    assert_eq!(found(s, &atoms(&[])).len(), 1);
    assert_eq!(
        found(s, &atoms(&[]))[0].0,
        "[![i](t3-citation:2)](t3-citation:1)"
    );
}

#[test]
fn no_atom_crosses_a_line_break() {
    assert!(found("[a\nb](t3-citation:1) c", &atoms(&[])).is_empty());
}

#[test]
fn a_lone_image_atom_is_a_line_of_text_not_a_figure() {
    let kinds = r#"[{"name":"context","link":"t3-context:"}]"#;
    let a = Atoms::parse(kinds, "[]");
    let s = "![shot.png](t3-context://v1/image/7)";
    let styled = style_with(s, None, &a);
    assert_eq!(styled.atoms.len(), 1);
    assert_eq!(styled.paragraphs[0].kind, ParagraphKind::Body);
    assert_eq!(
        crate::style(s, None).paragraphs[0].kind,
        ParagraphKind::Image
    );
}

#[test]
fn an_atom_is_one_piece_with_its_label_in_the_formats_around_it() {
    let faces = r##"[{"source":"[q](t3-citation:1)","label":"A  long\n quote","icon":"q.svg","title":"The quote","color":"","backgroundColor":"#efe7ff","direction":"ltr"}]"##;
    let a = Atoms::parse(KINDS, faces);
    let pieces = pieces_with("say **[q](t3-citation:1)** ok", &a);
    let texts: Vec<&str> = pieces.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["say ", "A long quote", " ok"]);
    let atom = &pieces[1];
    assert_eq!(atom.weight, 700);
    assert_eq!(atom.source, Some(Range::new(6, 24)));
    let detail = atom.atom.as_ref().unwrap();
    assert_eq!(detail.kind, "citation");
    assert_eq!(detail.icon, "q.svg");
    assert_eq!(detail.title, "The quote");
    assert_eq!(detail.href, "t3-citation:1");
    assert_eq!(detail.source, "[q](t3-citation:1)");
    assert_eq!(
        [detail.color.as_str(), &detail.background_color],
        ["", "#efe7ff"]
    );
    assert_eq!(detail.direction, "ltr");
    // Two adjacent atoms stay two pieces.
    let two = pieces_with("[a](t3-citation:1)[b](t3-citation:2)", &atoms(&[]));
    assert_eq!(two.len(), 2);
}

#[test]
fn plain_copy_writes_each_atom_as_its_label() {
    let a = atoms(&[("@x", "Ex")]);
    assert_eq!(
        plain_with("hi @x and [q](t3-citation:1) **b**", &a),
        "hi Ex and q b"
    );
}

#[test]
fn plain_copy_of_a_selection_finds_atoms_in_the_whole_source() {
    let a = atoms(&[("@x", "Ex"), ("[f](a.rs)", "file a")]);
    let s = "hi @x and [f](a.rs) **b**";
    // The fragment `@x` alone fails the whitespace rule; the source does not.
    assert_eq!(plain_range(s, Range::new(3, 5), &a), "Ex");
    assert_eq!(plain_range(s, Range::new(10, 19), &a), "file a");
    assert_eq!(plain_range(s, Range::new(0, 25), &a), "hi Ex and file a b");
}
