use super::*;

fn paragraph(source: &str) -> Vec<Run> {
    match parse(source).into_iter().next() {
        Some(Block::Paragraph(runs)) => runs,
        other => panic!("expected paragraph, got {other:?}"),
    }
}

fn text(runs: &[Run]) -> String {
    runs_text(runs)
}

fn formulas(blocks: &[Block]) -> Vec<Run> {
    fn runs(runs: &[Run], out: &mut Vec<Run>) {
        out.extend(runs.iter().filter(|r| r.math != MathKind::None).cloned());
    }
    let mut out = Vec::new();
    for block in blocks {
        match block {
            Block::Heading(_, r) | Block::Paragraph(r) => runs(r, &mut out),
            Block::BlockQuote(inner) => out.extend(formulas(inner)),
            Block::List(_, items) => {
                for item in items {
                    out.extend(formulas(&item.blocks));
                }
            }
            Block::Table(_, header, rows) => {
                for cell in header.iter().chain(rows.iter().flatten()) {
                    runs(cell, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn nested_emphasis_code_and_unicode_have_correct_runs() {
    let runs = paragraph("Hello **bold *café*** and `a_b()` with ~~old~~.");
    assert_eq!(text(&runs), "Hello bold café and a_b() with old.");
    let cafe = runs.iter().find(|r| r.text == "café").unwrap();
    assert!(cafe.bold && cafe.italic);
    let code = runs.iter().find(|r| r.text == "a_b()").unwrap();
    assert!(code.code);
    assert!(runs.iter().find(|r| r.text == "old").unwrap().strike);
}

#[test]
fn progress_styling_applies_to_every_run() {
    let doc = parse("**Working** on `file.rs`; see [details](https://example.com).");
    let flat = to_json(&doc, true);
    assert_eq!(flat.len(), 1);
    let runs = flat[0]["runs"].as_array().unwrap();
    assert!(!runs.is_empty());
    for run in runs {
        assert_eq!(run["subdued"], true);
    }
    assert_eq!(
        runs.iter()
            .filter(|r| r["url"] == "https://example.com")
            .count(),
        1
    );
}

#[test]
fn links_use_rendered_text_and_only_open_web_mail_or_files() {
    let runs =
        paragraph("é [**docs**](https://example.com) [run](javascript:alert) [local](/tmp/file)");
    assert_eq!(text(&runs), "é docs run local");
    let docs = runs.iter().find(|r| r.text == "docs").unwrap();
    assert!(docs.bold);
    assert_eq!(docs.url.as_deref(), Some("https://example.com"));
    let urls: Vec<&str> = runs.iter().filter_map(|r| r.url.as_deref()).collect();
    assert_eq!(urls, vec!["https://example.com", "/tmp/file"]);
    assert!(web_link("mailto:hello@example.com"));
    assert!(!web_link("file:///tmp/example"));
    assert!(!web_link("command:delete"));
    assert!(!link_allowed("javascript:alert"));
    assert!(link_allowed("file:///tmp/example"));
}

#[test]
fn session_links_accept_paths_with_spaces() {
    let runs = paragraph(
        "[report](</tmp/Ocho remote report.pdf>) [binary](<file:///tmp/Ocho%20sample.bin>)",
    );
    let urls: Vec<&str> = runs.iter().filter_map(|r| r.url.as_deref()).collect();
    assert_eq!(
        urls,
        vec![
            "/tmp/Ocho remote report.pdf",
            "file:///tmp/Ocho%20sample.bin"
        ]
    );
    let runs = paragraph("[report](./out/report.pdf) [web](https://example.com)");
    assert_eq!(runs[0].url.as_deref(), Some("./out/report.pdf"));
    assert_eq!(runs[1].text, " ");
    assert_eq!(runs[2].url.as_deref(), Some("https://example.com"));
}

#[test]
fn file_links_follow_remote_file_rules() {
    assert_eq!(
        path_from_link("./out/report.pdf"),
        Some("./out/report.pdf".into())
    );
    assert_eq!(
        path_from_link("file://localhost/tmp/a%20b.txt#L3"),
        Some("/tmp/a b.txt".into())
    );
    assert_eq!(path_from_link("file://host/tmp/x"), None);
    assert_eq!(path_from_link("README"), None);
    assert_eq!(path_from_link("notes.md"), Some("notes.md".into()));
    assert_eq!(path_from_link("//cdn/x.js"), None);
    assert_eq!(path_from_link("/tmp/dir/"), None);
}

#[test]
fn preserves_nested_lists_tasks_tables_and_literal_code() {
    let doc = parse(
        "## Result\n\n3. First\n   - Nested\n4. Next\n\n- [x] Done\n\n> Quoted\n\n| Name | Value |\n| --- | ---: |\n| **one** | 2 |\n\n```rust\nlet x = \"**literal**\";\n```",
    );
    assert!(matches!(&doc[0], Block::Heading(2, _)));
    let Block::List(Some(3), items) = &doc[1] else {
        panic!("ordered list lost: {:?}", doc[1]);
    };
    assert_eq!(items.len(), 2);
    assert!(items[0]
        .blocks
        .iter()
        .any(|b| matches!(b, Block::List(None, _))));
    let Block::List(None, items) = &doc[2] else {
        panic!("task list lost");
    };
    assert_eq!(items[0].task, Some(true));
    assert_eq!(plain_text(&items[0].blocks), "Done");
    assert!(matches!(&doc[3], Block::BlockQuote(_)));
    let Block::Table(align, header, rows) = &doc[4] else {
        panic!("table lost: {:?}", doc[4]);
    };
    assert_eq!(align, &[Align::None, Align::Right]);
    assert_eq!(header.len(), 2);
    assert_eq!(rows.len(), 1);
    assert!(rows[0][0][0].bold);
    let Block::Code(language, code) = &doc[5] else {
        panic!("code lost");
    };
    assert_eq!(language, "rust");
    assert_eq!(code, "let x = \"**literal**\";");
}

#[test]
fn streaming_fences_breaks_and_html_preserve_visible_text() {
    let doc = parse("```sh\necho 'hello'");
    assert_eq!(doc[0], Block::Code("sh".into(), "echo 'hello'".into()));
    assert_eq!(text(&paragraph("one\ntwo  \nthree")), "one two\nthree");
    assert_eq!(
        text(&paragraph("a <b>literal</b> tag")),
        "a <b>literal</b> tag"
    );
    assert_eq!(
        text(&paragraph("![a diagram](https://example.com/image.png)")),
        "[Image: a diagram]"
    );
    assert_eq!(
        text(&paragraph("<https://example.com> and a\\*b")),
        "https://example.com and a*b"
    );
}

#[test]
fn autolinks_entities_and_escaped_destinations() {
    let runs = paragraph("<me@example.com> &amp; &#169; &#x2192; &bogus; <ftp://x>");
    assert_eq!(text(&runs), "me@example.com & © → &bogus; ftp://x");
    assert_eq!(runs[0].url.as_deref(), Some("mailto:me@example.com"));
    assert!(runs.last().unwrap().url.is_none());
    let runs = paragraph("[x](https://example.com/\\(y\\))");
    assert_eq!(runs[0].url.as_deref(), Some("https://example.com/(y)"));
}

#[test]
fn underscores_inside_words_are_text() {
    let runs = paragraph("snake_case stays, _em_ goes, __strong__ too");
    assert_eq!(text(&runs), "snake_case stays, em goes, strong too");
    assert!(runs.iter().find(|r| r.text == "em").unwrap().italic);
    assert!(runs.iter().find(|r| r.text == "strong").unwrap().bold);
    assert_eq!(text(&paragraph("a * b * c")), "a * b * c");
    assert_eq!(text(&paragraph("***both***")), "both");
    let both = paragraph("***both***");
    assert!(both[0].bold && both[0].italic);
}

#[test]
fn flat_json_carries_markers_depth_and_quotes() {
    let doc = parse("- one\n  - two\n\n  more\n- [ ] todo\n\n> # Q\n> text\n\n---");
    let flat = to_json(&doc, false);
    let rows: Vec<(String, u64, String, u64)> = flat
        .iter()
        .map(|b| {
            (
                b["kind"].as_str().unwrap().to_string(),
                b["depth"].as_u64().unwrap(),
                b["marker"].as_str().unwrap().to_string(),
                b["quote"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("paragraph".into(), 1, "•".into(), 0),
            ("paragraph".into(), 2, "•".into(), 0),
            ("paragraph".into(), 1, "".into(), 0),
            ("paragraph".into(), 1, "".into(), 0),
            ("heading".into(), 0, "".into(), 1),
            ("paragraph".into(), 0, "".into(), 1),
            ("rule".into(), 0, "".into(), 0),
        ]
    );
    // A task's glyph replaces the bullet and leads its text.
    assert_eq!(flat[3]["runs"][0]["text"], "☐ ");
    assert_eq!(flat[3]["runs"][1]["text"], "todo");
    assert_eq!(flat[4]["level"], 1);
    assert_eq!(flat[2]["runs"][0]["text"], "more");
    let table = to_json(&parse("| a | b |\n|:-:|--|\n| 1 | 2 |"), false);
    assert_eq!(table[0]["kind"], "table");
    assert_eq!(table[0]["rows"][0]["header"], true);
    assert_eq!(table[0]["rows"][1]["cells"][1]["runs"][0]["text"], "2");
    assert_eq!(table[0]["align"][0], "center");
}

#[test]
fn headings_rules_and_ordered_markers() {
    let doc = parse(
        "# One\n##### Deep\n* * *\n1) a\n2) b\n\ntext\n---\n\nTitle\n===\n\n# C#\n## Done ##",
    );
    assert!(matches!(&doc[0], Block::Heading(1, _)));
    assert!(matches!(&doc[1], Block::Heading(4, _)));
    assert_eq!(doc[2], Block::Rule);
    let Block::List(Some(1), items) = &doc[3] else {
        panic!("ordered list: {:?}", doc[3]);
    };
    assert_eq!(items.len(), 2);
    let flat = to_json(&doc, false);
    assert_eq!(flat[4]["marker"], "2.");
    // Setext underlines make headings, as pulldown-cmark does.
    assert!(matches!(&doc[4], Block::Heading(2, r) if text(r) == "text"));
    assert!(matches!(&doc[5], Block::Heading(1, r) if text(r) == "Title"));
    assert!(matches!(&doc[6], Block::Heading(1, r) if text(r) == "C#"));
    assert!(matches!(&doc[7], Block::Heading(2, r) if text(r) == "Done"));
}

#[test]
fn indented_code_list_changes_and_paragraph_interruptions() {
    let doc = parse("Para\n\n    let x = 1;\n\n    y\n\nafter");
    assert_eq!(doc[1], Block::Code(String::new(), "let x = 1;\n\ny".into()));
    // A bullet change starts a new list.
    let doc = parse("- a\n* b");
    assert_eq!(doc.len(), 2);
    // Only an ordered list starting at 1 interrupts a paragraph.
    let doc = parse("In 2024\n2024. was a year");
    assert_eq!(doc.len(), 1);
    let doc = parse("Steps:\n1. go");
    assert_eq!(doc.len(), 2);
    // A table needs as many separator cells as header cells.
    assert!(matches!(
        parse("| a | b |\n| --- |")[0],
        Block::Paragraph(_)
    ));
}

// ----- math (markdown.rs tests) ----------------------------------------------

#[test]
fn recognizes_all_four_math_delimiters_and_preserves_markdown() {
    let doc =
        parse("**Euler:** $e^{i\\pi}+1=0$ and \\(a_b\\).\n\n\\[\\frac{1}{2}\\]\n\n$$\\sqrt{x}$$");
    let math = formulas(&doc);
    assert_eq!(math.len(), 4);
    assert_eq!(
        math.iter().map(|f| f.math).collect::<Vec<_>>(),
        [
            MathKind::Inline,
            MathKind::Inline,
            MathKind::Display,
            MathKind::Display
        ]
    );
    assert_eq!(math[1].text, "a_b");
    let Block::Paragraph(runs) = &doc[0] else {
        panic!("paragraph")
    };
    assert!(runs.iter().any(|r| r.bold));
}

#[test]
fn code_currency_and_unfinished_math_stay_readable() {
    let doc = parse("`$x$` and `\\(x\\)`\n\n```tex\n$$x$$\n\\[x\\]\n```");
    assert!(formulas(&doc).is_empty());
    assert_eq!(
        text(&paragraph("Costs $5 and $10; escaped \\$x\\$.")),
        "Costs $5 and $10; escaped $x$."
    );
    assert_eq!(text(&paragraph("Waiting for $x^2")), "Waiting for $x^2");
    assert_eq!(text(&paragraph(r"$\frac{a}{$")), r"$\frac{a}{$");
    assert_eq!(formulas(&parse(r"$\frac{a}{b}$")).len(), 1);
}

#[test]
fn math_in_lists_and_tables_keeps_links() {
    let doc =
        parse("- Café $x^2$ [docs](https://example.com)\n\n| Value |\n| --- |\n| \\(a_b\\) |");
    assert_eq!(formulas(&doc).len(), 2);
    let flat = to_json(&doc, false);
    assert_eq!(flat[0]["flow"], true);
    let runs = flat[0]["runs"].as_array().unwrap();
    assert_eq!(runs[1]["math"], "inline");
    assert_eq!(runs[1]["text"], "x^2");
    assert_eq!(runs[3]["url"], "https://example.com");
    assert_eq!(flat[1]["rows"][1]["cells"][0]["flow"], true);
    assert_eq!(flat[1]["rows"][1]["cells"][0]["runs"][0]["text"], "a_b");
}

#[test]
fn compact_text_keeps_math_literal() {
    let doc = parse("Found $x^2$ and $$y^2$$.");
    assert_eq!(plain_text(&doc), "Found $x^2$ and $$y^2$$.");
}

#[test]
fn delimiter_conversion_respects_code_links_and_escapes() {
    let doc = parse("Inline \\( x^2 \\), display \\[x^2\\].\n\n`\\(literal\\)`\n\n```tex\n\\[literal\\]\n```\n\n    \\[indented\\]\n\n[link](https://example.com/\\(x\\)) and \\\\(escaped\\\\).");
    let math = formulas(&doc);
    assert_eq!(math.len(), 2);
    assert_eq!(math[0].text, "x^2");
    assert_eq!(math[1].math, MathKind::Display);
    assert_eq!(doc[2], Block::Code("tex".into(), "\\[literal\\]".into()));
    assert_eq!(doc[3], Block::Code(String::new(), "\\[indented\\]".into()));
    let Block::Paragraph(runs) = &doc[4] else {
        panic!("paragraph")
    };
    assert_eq!(text(runs), "link and \\(escaped\\).");
    // Inside a link label the delimiters stay literal.
    assert!(formulas(&parse("[\\(x\\)](https://e.com)")).is_empty());
}

#[test]
fn display_math_cuts_its_paragraph_into_rows() {
    let flat = to_json(
        &parse("- If $a=dx$, then\n  \\[sa+tb=d(sx+ty).\\]\n  so it divides."),
        true,
    );
    let kinds: Vec<&str> = flat.iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["paragraph", "math", "paragraph"]);
    assert_eq!(flat[0]["marker"], "•");
    assert_eq!(flat[0]["flow"], true);
    assert_eq!(flat[1]["marker"], "");
    assert_eq!(flat[1]["text"], "sa+tb=d(sx+ty).");
    assert_eq!(flat[1]["subdued"], true);
    assert_eq!(flat[2]["runs"][0]["text"], " so it divides.");
}

#[test]
fn transcript_math_fixture_parses() {
    let source = include_str!("transcript-math.md");
    let doc = parse(source);
    let math = formulas(&doc);
    assert!(math.len() > 40, "{}", math.len());
    let flat = to_json(&doc, false);
    let display = flat.iter().filter(|b| b["kind"] == "math").count();
    assert_eq!(display, 3);
    // Code spans, currency and the unfinished formula stay text.
    let text = plain_text(&doc);
    assert!(text.contains("Keep $literal$, \\(code\\), $5 and $10 readable."));
    assert!(text.ends_with("Streaming source remains readable: $\\frac{a}{$."));
    let table = flat.iter().find(|b| b["kind"] == "table").unwrap();
    assert_eq!(table["rows"][2]["cells"][0]["runs"][0]["math"], "inline");
}

#[test]
fn bare_urls_are_links_without_their_punctuation() {
    let runs = paragraph("Live at **https://mtr.eliot.sh**, see www.example.com/a_(b) (or https://x.dev/p). Not `https://code.span` or xhttps://no.");
    let links: Vec<(&str, &str)> = runs
        .iter()
        .filter_map(|r| r.url.as_deref().map(|u| (r.text.as_str(), u)))
        .collect();
    assert_eq!(
        links,
        vec![
            ("https://mtr.eliot.sh", "https://mtr.eliot.sh"),
            ("www.example.com/a_(b)", "https://www.example.com/a_(b)"),
            ("https://x.dev/p", "https://x.dev/p"),
        ]
    );
    assert!(runs
        .iter()
        .any(|r| r.code && r.text == "https://code.span" && r.url.is_none()));
    assert_eq!(text(&runs), "Live at https://mtr.eliot.sh, see www.example.com/a_(b) (or https://x.dev/p). Not https://code.span or xhttps://no.");
}
