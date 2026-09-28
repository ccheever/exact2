//! What the parser must get right, checked against the corpus it exists to
//! read: this repository's own Markdown. @ref LLP 1033

use markdown_parse::{parse, plain, Kind};

fn here(source: &str) -> markdown_parse::Document {
    parse(source, &|target: &str| target.to_string())
}

#[test]
fn document_values_share_repeated_text_without_changing_keys_or_encoding() {
    use exact_plan::Value;
    fn fields(value: &Value) -> &[Value] {
        match value {
            Value::Record(values) | Value::List(values) => values,
            _ => panic!("expected fields or items"),
        }
    }
    // One allocation for shared text; equal bytes for inline text, which has none.
    fn shared(a: &Value, b: &Value) {
        assert!(a.is_str() && b.is_str(), "expected strings");
        assert!(Value::same_str(a, b));
    }
    let doc = here("alpha **β** [site](next.md)\n\nalpha **β** [site](next.md)");
    let value = markdown_parse::value::blocks(&doc);
    let blocks = fields(&value);
    let first = fields(&blocks[0]);
    let second = fields(&blocks[1]);
    assert_eq!(first[0].as_str(), Some("0"));
    assert_eq!(second[0].as_str(), Some("1"));
    for i in [1, 3, 4, 5] {
        shared(&first[i], &second[i]);
    }
    shared(&first[3], &first[4]);
    let a = fields(&fields(&first[7])[0]);
    let b = fields(&fields(&second[7])[0]);
    for i in [0, 1, 3, 5] {
        shared(&a[i], &b[i]);
    }
    shared(&first[0], &a[0]);
    let (Value::List(a), Value::List(b)) = (&first[8], &second[8]) else {
        panic!("expected empty cells");
    };
    assert!(a.is_empty() && exact_plan::Items::ptr_eq(a, b));
    assert_eq!(Value::from_bytes(&value.to_bytes()).unwrap(), value);
    // A separate conversion has independent ownership; closing a document
    // must not leave its content in an intern pool.
    let kept = first[1].to_shared_str().expect("a string");
    drop(value);
    assert_eq!(exact_plan::Str::strong_count(&kept), 1);
    assert_eq!(
        fields(&markdown_parse::value::block(usize::MAX, &doc.blocks[0]))[0].as_str(),
        Some(usize::MAX.to_string().as_str())
    );
    let encoded = markdown_parse::value::blocks(&doc).to_bytes();
    assert_eq!(markdown_parse::value::into_blocks(doc).to_bytes(), encoded);
}

#[test]
fn document_string_sharing_preserves_styles_links_and_indices() {
    use markdown_parse::{Block, Document, Run};
    let plain = Run::text("same");
    let doc = Document {
        blocks: [
            plain.clone(),
            Run {
                bold: true,
                ..plain.clone()
            },
            Run {
                italic: true,
                ..plain.clone()
            },
            Run {
                code: true,
                ..plain.clone()
            },
            Run {
                href: "next.md".into(),
                ..plain.clone()
            },
            plain,
        ]
        .into_iter()
        .map(|run| Block {
            runs: vec![run],
            ..Block::default()
        })
        .collect(),
        ..Document::default()
    };
    let expected = exact_plan::Value::list(
        doc.blocks
            .iter()
            .enumerate()
            .map(|(i, b)| markdown_parse::value::block(i, b))
            .collect(),
    );
    assert_eq!(markdown_parse::value::into_blocks(doc), expected);
}

#[test]
fn headings_paragraphs_and_emphasis() {
    let doc = here("# Title\n\nSome **bold** and *slanted* and `code` text.\n");
    assert_eq!(doc.title, "Title");
    assert_eq!(doc.blocks.len(), 2);
    assert_eq!(doc.blocks[0].kind, Kind::Heading);
    assert_eq!(doc.blocks[0].depth, 1);
    assert_eq!(doc.blocks[0].href, "title");
    let runs = &doc.blocks[1].runs;
    assert_eq!(plain(runs), "Some bold and slanted and code text.");
    assert!(runs.iter().any(|r| r.text == "bold" && r.bold));
    assert!(runs.iter().any(|r| r.text == "slanted" && r.italic));
    assert!(runs.iter().any(|r| r.text == "code" && r.code));
}

#[test]
fn a_link_carries_its_target_through_the_resolver() {
    let doc = parse("See [the rules](rules/RULES.md).", &|target: &str| {
        format!("/repo/{target}")
    });
    let link = doc.blocks[0]
        .runs
        .iter()
        .find(|r| r.text == "the rules")
        .expect("the label is a run");
    assert_eq!(link.href, "/repo/rules/RULES.md");
    // The text around it is not a link.
    assert!(doc.blocks[0]
        .runs
        .iter()
        .any(|r| r.text == "See " && r.href.is_empty()));
}

#[test]
fn fenced_code_keeps_its_text_and_never_parses_it() {
    let doc = here("```sh\ncargo build --workspace\n# not a heading\n```\n");
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].kind, Kind::Code);
    assert_eq!(doc.blocks[0].href, "sh");
    assert_eq!(
        doc.blocks[0].text,
        "cargo build --workspace\n# not a heading"
    );
}

#[test]
fn lists_carry_their_marker_and_their_depth() {
    let doc = here("- one\n- two\n  - nested\n\n1. first\n2. second\n");
    let items: Vec<_> = doc.blocks.iter().filter(|b| b.kind == Kind::Item).collect();
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].marker, "•");
    assert_eq!(items[0].depth, 0);
    assert_eq!(items[2].depth, 1);
    assert_eq!(plain(&items[2].runs), "nested");
    assert_eq!(items[3].marker, "1.");
    assert_eq!(items[4].marker, "2.");
}

#[test]
fn a_wrapped_list_item_is_one_item() {
    let doc = here("- a long item that\n  continues on the next line\n- another\n");
    let items: Vec<_> = doc.blocks.iter().filter(|b| b.kind == Kind::Item).collect();
    assert_eq!(items.len(), 2);
    assert_eq!(
        plain(&items[0].runs),
        "a long item that continues on the next line"
    );
}

#[test]
fn quotes_rules_and_images() {
    let doc = here("> quoted\n\n---\n\n![a train](assets/caltrain.png)\n");
    assert_eq!(doc.blocks[0].kind, Kind::Quote);
    assert_eq!(plain(&doc.blocks[0].runs), "quoted");
    assert_eq!(doc.blocks[1].kind, Kind::Rule);
    assert_eq!(doc.blocks[2].kind, Kind::Image);
    assert_eq!(doc.blocks[2].text, "a train");
    assert_eq!(doc.blocks[2].href, "assets/caltrain.png");
}

#[test]
fn a_pipe_table_is_rows_of_cells_and_prose_with_pipes_is_not() {
    let doc = here("| Crate | What |\n| --- | --- |\n| `plan` | the format |\n");
    let rows: Vec<_> = doc
        .blocks
        .iter()
        .filter(|b| b.kind == Kind::TableRow)
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].header);
    assert_eq!(plain(&rows[0].cells[0]), "Crate");
    assert!(!rows[1].header);
    assert_eq!(plain(&rows[1].cells[0]), "plan");
    assert!(rows[1].cells[0][0].code);

    let prose = here("Run `a | b` in a shell.\n");
    assert!(prose.blocks.iter().all(|b| b.kind != Kind::TableRow));
}

#[test]
fn raw_html_stays_visible_and_inert() {
    let doc = here("<div class=\"note\">hello</div>\n");
    assert_eq!(doc.blocks[0].kind, Kind::Html);
    assert_eq!(doc.blocks[0].text, "<div class=\"note\">hello</div>");
}

#[test]
fn snake_case_and_arithmetic_are_not_emphasis() {
    let doc = here("The value of a_b_c is 2 * 3 * 4.\n");
    assert_eq!(
        plain(&doc.blocks[0].runs),
        "The value of a_b_c is 2 * 3 * 4."
    );
    assert!(doc.blocks[0].runs.iter().all(|r| !r.italic));
}

#[test]
fn a_bare_url_becomes_a_link_without_its_sentence_stop() {
    let doc = here("See https://example.com/a, then stop.\n");
    let link = doc.blocks[0]
        .runs
        .iter()
        .find(|r| !r.href.is_empty())
        .unwrap();
    assert_eq!(link.text, "https://example.com/a");
    assert_eq!(link.href, "https://example.com/a");
}

#[test]
fn unicode_survives_inline_tokens_and_bare_url_detection() {
    let source = "hé 中🙂 \\* **hé🙂** [中](../中.md) `é` https://example.com/中, h🙂";
    let doc = here(source);
    assert_eq!(
        plain(&doc.blocks[0].runs),
        "hé 中🙂 * hé🙂 中 é https://example.com/中, h🙂"
    );
    assert!(doc.blocks[0]
        .runs
        .iter()
        .any(|r| r.bold && r.text == "hé🙂"));
    assert!(doc.blocks[0].runs.iter().any(|r| r.href == "../中.md"));
    assert!(doc.blocks[0]
        .runs
        .iter()
        .any(|r| r.href == "https://example.com/中"));
}

#[test]
fn rules_require_three_matching_markers_and_only_spaces_between_them() {
    for source in ["---", "* * *", "_ _ _ _", "  - - -  "] {
        assert_eq!(here(source).blocks[0].kind, Kind::Rule, "{source:?}");
    }
    for source in ["--", "-*-", "___é", "---\t", "prose ---"] {
        assert!(
            here(source).blocks.iter().all(|b| b.kind != Kind::Rule),
            "{source:?}"
        );
    }
}

#[test]
fn the_outline_is_every_heading_in_order() {
    let doc = here("# One\n\ntext\n\n## Two\n\n### Three\n");
    let outline = doc.outline();
    assert_eq!(outline.len(), 3);
    assert_eq!(outline[0], (1, "One".to_string(), 0));
    assert_eq!(outline[2].0, 3);
    assert_eq!(outline[2].1, "Three");
}

/// The corpus this reader exists for. Not a golden file — a shape check that
/// would fail loudly if block detection regressed on real documents.
#[test]
fn this_repositorys_own_documents_parse_into_blocks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../llp");
    let mut files = 0;
    let mut headings = 0;
    let mut tables = 0;
    for entry in std::fs::read_dir(&root).expect("llp/") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read");
        let doc = here(&source);
        assert!(
            !doc.blocks.is_empty(),
            "{} parsed to nothing",
            path.display()
        );
        // Every LLP opens with its own `# LLP NNNN:` title.
        assert!(!doc.title.is_empty(), "{} has no title", path.display());
        headings += doc
            .blocks
            .iter()
            .filter(|b| b.kind == Kind::Heading)
            .count();
        tables += doc
            .blocks
            .iter()
            .filter(|b| b.kind == Kind::TableRow)
            .count();
        files += 1;
    }
    assert!(files > 20, "expected the corpus, found {files} documents");
    assert!(headings > 200, "expected headings, found {headings}");
    assert!(tables > 20, "expected tables, found {tables}");
}
