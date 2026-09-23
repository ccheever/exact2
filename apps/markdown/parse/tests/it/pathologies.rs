//! Malformed delimiters stay literal, with the pre-S2 parser as a test oracle.
use markdown_parse::{parse, Run};

#[path = "support/inline_legacy.rs"]
mod legacy;

fn runs(text: &str, link: &dyn Fn(&str) -> String) -> Vec<Run> {
    // Keep block grammar and trimming out of this inline comparison.
    let doc = parse(text, link);
    assert_eq!(doc.blocks.len(), 1, "{text:?}");
    doc.blocks.into_iter().next().unwrap().runs
}

#[test]
fn malformed_delimiters_keep_literal_text() {
    for text in [
        "[x".repeat(32_768),
        "x<".repeat(32_768),
        format!("{}>", "x<".repeat(32_768)),
        format!("{} @/>", "x<".repeat(32_768)),
        "[x](".repeat(16_384),
        "x * ".repeat(16_384),
        "x _ ".repeat(16_384),
    ] {
        let text = format!("p {text} end");
        assert_eq!(runs(&text, &str::to_string), vec![Run::text(&text)]);
    }
}

#[test]
fn deeply_nested_labels_do_not_copy_or_recurse_over_their_descendants() {
    let text = format!("p {}x{} end", "[".repeat(16_384), "](u)".repeat(16_384));
    assert_eq!(
        runs(&text, &str::to_string),
        vec![
            Run::text("p "),
            Run {
                text: "x".into(),
                href: "u".into(),
                ..Run::default()
            },
            Run::text(" end"),
        ]
    );
}

fn equivalent(body: &str) {
    use std::cell::RefCell;
    let text = format!("p {body} end");
    for empty_links in [false, true] {
        let old_calls = RefCell::new(Vec::new());
        let new_calls = RefCell::new(Vec::new());
        let resolve = |target: &str, calls: &RefCell<Vec<String>>| {
            calls.borrow_mut().push(target.to_string());
            if empty_links {
                String::new()
            } else {
                format!("resolved:{target}")
            }
        };
        let old = legacy::runs(&text, &|t| resolve(t, &old_calls));
        let new = runs(&text, &|t| resolve(t, &new_calls));
        assert_eq!(new, old, "{text:?}, empty_links={empty_links}");
        assert_eq!(new_calls, old_calls, "resolver calls for {text:?}");
    }
}

#[test]
fn indexed_parser_matches_old_behavior_over_seeded_corpus() {
    for text in [
        r"[outer [inner](b)](a)",
        r"[unclosed [label](ok)",
        r"[x\]y](a\(b\))",
        r"[x](a(b)c)",
        r"[x](a(b)",
        r"**[a *b*](c)**",
        r"\`a``b`",
        "````` x ``` y `` z `",
        "``````",
        r"**\*** _\__ **a\*b**",
        r"[*x*](u)tail",
        r"[x](u)tail",
        r"[](u)tail",
        r"![x](u)tail",
        r"![](u)tail",
        r"[![x](u)](v)",
        r"<bad <bad> <a@b> <x y@z> <a/b@c> <https://a b>",
        r"[https://example.com](outer)",
        r"[<a@b>](outer)",
        "東京 [é 🦀](路径) **e\u{301}**",
        "*\t* __ __ ** **",
    ] {
        equivalent(text);
    }
    let atoms = [
        "[",
        "]",
        "(",
        ")",
        "<",
        ">",
        "`",
        "``",
        "```",
        "*",
        "**",
        "_",
        "__",
        "\\",
        "\\\\",
        "!",
        "@",
        "/",
        "\"",
        "'",
        " ",
        "\t",
        "x",
        "h",
        "é",
        "🦀",
        "東京",
        "http://",
        "https://example.com",
        "[x](y)",
        "*a*",
        "`b`",
    ];
    let mut seed = 1044_u64;
    for _ in 0..10_000 {
        let mut text = String::new();
        for _ in 0..64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            text.push_str(atoms[seed as usize % atoms.len()]);
        }
        equivalent(&text);
    }
    for line in include_str!("../../../../../llp/1033-markdown-viewer.plan.md")
        .lines()
        .chain(include_str!("../../../../../llp/1044.000-text-performance-claude.plan.md").lines())
    {
        equivalent(line);
    }
}

#[test]
fn cancellation_discards_partial_documents_at_scan_checkpoints() {
    use std::cell::Cell;
    for body in [
        "plain prose ".repeat(65_536),
        "[x".repeat(32_768),
        "x<".repeat(32_768),
        "`".repeat(65_536),
        "*x ".repeat(32_768),
        "\\".repeat(65_536),
        "[x](".repeat(16_384),
        "a\nb\nc\n".repeat(4096),
        "```\ncode\n```\n".repeat(4096),
        "| a |\n| --- |\n| b |\n".repeat(4096),
    ] {
        let text = format!("# Title\n\np {body} end");
        let calls = Cell::new(0);
        let cancelled = markdown_parse::parse_cancellable(&text, &str::to_string, &|| {
            calls.set(calls.get() + 1);
            calls.get() == 12 // deliberately transient: cancellation must latch
        });
        assert!(cancelled.is_none(), "{:?}", &text[..30]);
        assert_eq!(calls.get(), 12);
        assert_eq!(
            markdown_parse::parse_cancellable(&text, &str::to_string, &|| false),
            Some(parse(&text, &str::to_string))
        );
    }
}
