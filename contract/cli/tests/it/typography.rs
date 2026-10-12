//! The reader diary's book typography: `hyphens` and `text-indent` are rows
//! every host lays out, and so are CSS multi-column layout and the break rules
//! inside it (LLP 1093); what exact2 leaves out of fragmentation (the
//! spanning element, pages, regions) is refused by what it would need.

use exact_kernel::{Hyphens, Kernel, StyleMask};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn hyphens_none_carries_no_soft_hyphen_break_and_inherits() {
    let r = boot(concat!(
        "component App\n  view\n    column hyphens=\"none\"\n",
        "      text \"incom\u{ad}prehensibly\" testId=\"none\"\n",
        "      text \"incom\u{ad}prehensibly\" hyphens=\"manual\" testId=\"manual\"\n",
        "      text \"incom\u{ad}prehensibly\" hyphens=\"auto\" testId=\"auto\"\n",
        "      text testId=\"para\"\n        text \"a\u{ad}b \" testId=\"leaf\"\n",
    ));
    let k = r.kernel();
    let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
    // U+034F: as invisible, the same length in UTF-8 and UTF-16, and no break.
    assert_eq!(
        node("none").shown_text().as_deref(),
        Some("incom\u{34f}prehensibly")
    );
    assert_eq!(
        node("manual").shown_text().as_deref(),
        Some("incom\u{ad}prehensibly")
    );
    assert_eq!(
        node("auto").shown_text().as_deref(),
        Some("incom\u{ad}prehensibly")
    );
    let runs: Vec<String> = node("para")
        .text_runs()
        .iter()
        .map(|r| r.text.to_string())
        .collect();
    assert_eq!(runs, ["a\u{34f}b "]);
    assert_eq!(node("leaf").shown_text().as_deref(), Some("a\u{34f}b "));
    let rows = StyleMask::of(exact_kernel::StyleId::Hyphens);
    assert_eq!(node("auto").computed_style(rows).hyphens, Hyphens::Auto);
    assert_eq!(node("leaf").computed_style(rows).hyphens, Hyphens::None);
}

#[test]
fn text_indent_is_a_length_that_inherits_and_reaches_the_paragraph() {
    let r = boot(concat!(
        "component App\n  view\n    column text-indent=\"2em\" font-size=16\n",
        "      text \"a\" testId=\"em\"\n",
        "      text \"b\" text-indent=-12 testId=\"hang\"\n",
    ));
    let k = r.kernel();
    let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
    let rows = StyleMask::of(exact_kernel::StyleId::TextIndent);
    assert_eq!(node("em").computed_style(rows).text_indent, 32.0);
    assert_eq!(node("hang").computed_style(rows).text_indent, -12.0);
    let p = exact_kernel::text::Paragraph::from_style(&node("em").computed_style(StyleMask::ALL));
    assert_eq!(p.text_indent, 32.0);
    for value in ["\"10%\"", "\"2em hanging\"", "\"1em each-line\""] {
        let e = contract::compile(&format!(
            "component App\n  view\n    text \"a\" text-indent={value}\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, "lower-attr-value", "{e}");
        assert!(
            e.message
                .contains("negative length with the same `padding-left`"),
            "{e}"
        );
    }
    let e =
        contract::compile("component App\n  view\n    text \"a\" hyphens=\"all\"\n").unwrap_err();
    assert!(
        e.message
            .ends_with("expected one of \"none\", \"manual\", \"auto\""),
        "{e}"
    );
}

#[test]
fn multicol_rows_lower_with_their_keywords() {
    use exact_kernel::{BorderStyle, BreakBetween, BreakInside, ColumnFill, Dimension, StyleId};
    let r = boot(concat!(
        "component App\n  view\n    column\n",
        "      view columns=\"120px 3\" column-gap=10 column-fill=\"auto\" widows=3 testId=\"flow\"\n",
        "        text \"a\" break-before=\"column\" break-inside=\"avoid\" testId=\"para\"\n",
        "      view column-count=\"auto\" column-width=200 column-rule=\"thin solid red\" testId=\"rule\"\n",
        "      view columns=\"2\" column-rule-width=\"thick\" break-after=\"avoid-column\" testId=\"two\"\n",
    ));
    let k = r.kernel();
    let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
    let flow = node("flow").style;
    assert_eq!(
        (flow.column_width, flow.column_count),
        (Dimension::Points(120.0), 3)
    );
    assert_eq!(flow.column_fill, ColumnFill::Auto);
    assert_eq!(flow.column_gap, 10.0);
    let rows = exact_kernel::StyleMask::of(StyleId::Widows)
        .union(exact_kernel::StyleMask::of(StyleId::Orphans));
    let para = node("para").computed_style(rows);
    // `widows` inherits; `orphans` keeps CSS's initial 2.
    assert_eq!((para.widows, para.orphans), (3, 2));
    assert_eq!(node("para").style.break_before, BreakBetween::Column);
    assert_eq!(node("para").style.break_inside, BreakInside::Avoid);
    let rule = node("rule").style;
    assert_eq!(
        (rule.column_count, rule.column_width),
        (0, Dimension::Points(200.0))
    );
    assert_eq!(
        (rule.column_rule_width, rule.column_rule_style),
        (1.0, BorderStyle::Solid)
    );
    assert!(rule.column_rule_color.is_some());
    let two = node("two").style;
    assert_eq!((two.column_count, two.column_width), (2, Dimension::Auto));
    assert_eq!(two.column_rule_width, 5.0);
    assert_eq!(two.break_after, BreakBetween::AvoidColumn);
}

#[test]
fn fragmentation_outside_columns_is_refused_by_what_it_would_need() {
    for (attr, id, needs) in [
        (
            "column-span=\"all\"",
            "lower-unknown-attr",
            "only `column-span: none`",
        ),
        (
            "page-break-before=\"always\"",
            "lower-unknown-attr",
            "does not print",
        ),
        (
            "break-before=\"page\"",
            "lower-attr-value",
            "page or region break",
        ),
        (
            "break-after=\"recto\"",
            "lower-attr-value",
            "page or region break",
        ),
        (
            "break-inside=\"avoid-page\"",
            "lower-attr-value",
            "page or region value",
        ),
        (
            "column-fill=\"balance-all\"",
            "lower-attr-value",
            "paged media",
        ),
        (
            "column-rule-style=\"dashed\"",
            "lower-attr-value",
            "native hosts paint",
        ),
        (
            "column-rule=\"2px dotted red\"",
            "lower-css-shorthand",
            "not implemented by native painters",
        ),
        // A border paints `inset` (batch 6); a column rule does not.
        (
            "column-rule=\"2px inset red\"",
            "lower-css-shorthand",
            "supported styles are none, hidden and solid",
        ),
        (
            "column-width=\"50%\"",
            "lower-attr-value",
            "never a percentage",
        ),
        ("column-count=0", "lower-attr-value", "positive integer"),
        (
            "column-count=\"none\"",
            "lower-attr-value",
            "positive integer or `auto`",
        ),
        (
            "columns=\"2 3\"",
            "lower-css-shorthand",
            "one positive column count",
        ),
        ("widows=0", "lower-attr-value", "positive integer"),
    ] {
        let e =
            contract::compile(&format!("component App\n  view\n    view {attr}\n")).unwrap_err();
        assert_eq!(e.id, id, "{attr}: {e}");
        assert!(e.message.contains(needs), "{attr}: {e}");
    }
    for name in ["column-span", "page-break-after"] {
        assert_eq!(contract_lower::vocab::open_set(name), None, "{name}");
    }
    // A flex or grid container is never a multi-column one (CSS ignores the
    // rows there): the diagnostic says to write `view`.
    for tag in ["column", "row", "view display=\"grid\""] {
        let e = contract::compile(&format!(
            "component App\n  view\n    {tag} column-count=2\n"
        ))
        .unwrap_err();
        assert!(e.message.contains("write `view`"), "{tag}: {e}");
    }
}

/// CSS `text-box` (Inline Layout 3 §4): the shorthand, `text-box-edge`'s two
/// words, the trim's own row, and a laid-out box that keeps only the cap
/// height (the monospace face's 0.7 em) when trimmed to cap and baseline.
#[test]
fn text_box_trims_a_paragraph_to_its_edges() {
    use exact_kernel::{TextBoxEdge, TextBoxTrim};
    let r = boot(concat!(
        "component App\n  view\n    column align-items=\"flex-start\" text-box-edge=\"cap alphabetic\"\n",
        "      text \"TTI\" font-size=10 testId=\"plain\"\n",
        "      text \"TTI\" font-size=10 text-box=\"trim-both cap alphabetic\" testId=\"both\"\n",
        "      text \"TTI\" font-size=10 text-box-trim=\"trim-start\" testId=\"start\"\n",
        "      text \"TTI\" font-size=10 text-box=\"ex text\" testId=\"ex\"\n",
        "      text \"TTI\" font-size=10 text-box=\"normal\" testId=\"normal\"\n",
        "      text \"TTI\" font-size=10 text-box=\"cap alphabetic trim-start\" testId=\"after\"\n",
        "      text \"TTI\" font-size=10 text-box=\"trim-end auto\" testId=\"auto\"\n",
    ));
    let mut r = r;
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, exact_kernel::Offer::definite(400.0, 400.0))
        .unwrap();
    let k = r.kernel();
    let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
    let rows = exact_kernel::StyleMask::ALL;
    let style = |t: &str| node(t).computed_style(rows);
    assert_eq!(
        (style("both").text_box_trim, style("both").text_box_edge),
        (TextBoxTrim::TrimBoth, TextBoxEdge::CapAlphabetic)
    );
    // The edge inherits; the trim does not.
    assert_eq!(
        (style("plain").text_box_trim, style("plain").text_box_edge),
        (TextBoxTrim::None, TextBoxEdge::CapAlphabetic)
    );
    assert_eq!(
        (style("start").text_box_trim, style("start").text_box_edge),
        (TextBoxTrim::TrimStart, TextBoxEdge::CapAlphabetic)
    );
    assert_eq!(
        (style("ex").text_box_trim, style("ex").text_box_edge),
        (TextBoxTrim::TrimBoth, TextBoxEdge::Ex)
    );
    assert_eq!(style("normal").text_box_trim, TextBoxTrim::None);
    // The trim may follow the edge, and `auto` is an edge, in the shorthand.
    assert_eq!(
        (style("after").text_box_trim, style("after").text_box_edge),
        (TextBoxTrim::TrimStart, TextBoxEdge::CapAlphabetic)
    );
    assert_eq!(
        (style("auto").text_box_trim, style("auto").text_box_edge),
        (TextBoxTrim::TrimEnd, TextBoxEdge::Auto)
    );
    // 10 pt on a 12 pt line, baseline at 9.6: trimmed to cap and baseline, 7.
    assert_eq!(node("plain").frame.height, 12.0);
    assert!(
        (node("both").frame.height - 7.0).abs() < 1e-3,
        "{:?}",
        node("both").frame
    );
    assert!(
        (node("start").frame.height - 9.4).abs() < 1e-3,
        "{:?}",
        node("start").frame
    );
    for (value, says) in [
        ("\"trim-both ideographic\"", "not implemented"),
        ("\"cap cap\"", "not a text-box value"),
        ("\"cap\"", "needs its under edge"),
        ("\"\"", "an empty value"),
        ("\"cap trim-both alphabetic\"", "out of place"),
    ] {
        let e = contract::compile(&format!(
            "component App\n  view\n    text \"a\" text-box={value}\n"
        ))
        .unwrap_err();
        assert!(e.message.contains(says), "{value}: {e}");
    }
    // The trim is a text's; a block's would cut lines the hosts measure on
    // its text.
    let e = contract::compile("component App\n  view\n    column text-box=\"trim-both cap alphabetic\"\n      text \"a\"\n")
        .unwrap_err();
    assert!(e.message.contains("trims a `text`"), "{e}");
    // A paragraph that is its own columns, and an inline run, are refused
    // rather than drawn untrimmed.
    for (src, says) in [
        (
            "    text \"a\" column-count=2 text-box=\"trim-both cap alphabetic\"\n",
            "every column",
        ),
        (
            "    text \"a\" text-box-trim=\"trim-start\" columns=\"2\"\n",
            "every column",
        ),
        (
            "    text \"a\" text-box-trim=\"trim-start\" column-width=1\n",
            "every column",
        ),
        (
            "    text \"a\" column-count=1 text-box=\"trim-both cap alphabetic\"\n",
            "every column",
        ),
        (
            "    text\n      text \"a\" text-box-trim=\"trim-both\"\n",
            "inline `text` run",
        ),
        (
            "    text \"a\" display=\"flex\" text-box=\"trim-both cap alphabetic\"\n",
            "flex or grid",
        ),
        (
            "    text \"a\" display=(yes ? \"grid\" : \"block\") text-box-trim=\"trim-end\"\n",
            "flex or grid",
        ),
        (
            "    text \"a\" column-count=(yes ? 2 : \"auto\") text-box-trim=\"trim-end\"\n",
            "every column",
        ),
    ] {
        let e = contract::compile(&format!("component App\n  state yes = true\n  view\n{src}"))
            .unwrap_err();
        assert!(e.message.contains(says), "{src}: {e}");
    }
    // Columns that make no multi-column container, a shorthand that trims
    // nothing, and declarations a later one overrides are not refused.
    for src in [
        "    text \"a\" column-count=2 text-box-trim=\"none\"\n",
        "    text \"a\" column-count=2 text-box=\"none cap alphabetic\"\n",
        "    text \"a\" columns=\"auto\" text-box=\"trim-both cap alphabetic\"\n",
        "    text \"a\" column-count=2 text-box=\"trim-both cap alphabetic\" text-box-trim=\"none\"\n",
        "    text \"a\" columns=\"2\" column-count=\"auto\" text-box=\"trim-both cap alphabetic\"\n",
        "    text \"a\" column-count=2 text-box-trim=(yes ? \"none\" : \"none\")\n",
        "    text \"a\" columns=(yes ? \"auto\" : \"auto\") text-box=\"trim-both cap alphabetic\"\n",
        "    text \"a\" display=(yes ? \"block\" : \"none\") text-box=\"trim-both cap alphabetic\"\n",
        "    text \"a\" column-count=2 text-box-trim=\"unset\"\n",
        "    text \"a\" column-width=\"unset\" text-box=\"trim-both cap alphabetic\"\n",
    ] {
        let compiled = contract::compile(&format!("component App\n  state yes = true\n  view\n{src}"));
        assert!(compiled.is_ok(), "{src}: {:?}", compiled.err());
    }
    // A class's declarations come before the node's own, which override them
    let classed =
        "style Columns\n  column-count=2\ncomponent App\n  view\n    text \"a\" class=Columns";
    let trimmed = " text-box=\"trim-both cap alphabetic\"";
    let e = contract::compile(&format!("{classed}{trimmed}\n")).unwrap_err();
    assert!(e.message.contains("every column"), "{e}");
    let compiled = contract::compile(&format!("{classed} column-count=\"auto\"{trimmed}\n"));
    assert!(compiled.is_ok(), "{:?}", compiled.err());
    // A conditional class's branch that declares a row not at all leaves it
    // initial, which trims nothing and lays nothing out as flex or grid
    for src in [
        "style Off\n  text-box-trim=\"none\"\nstyle Plain\n  color=\"#ff0000\"\ncomponent App\n  state yes = true\n  view\n    text \"a\" class=(yes ? Off : Plain) column-count=2\n",
        "style Block\n  display=\"block\"\nstyle Plain\n  color=\"#ff0000\"\ncomponent App\n  state yes = true\n  view\n    text \"a\" class=(yes ? Block : Plain) text-box=\"trim-both cap alphabetic\"\n",
    ] {
        let compiled = contract::compile(src);
        assert!(compiled.is_ok(), "{src}: {:?}", compiled.err());
    }
}

/// A trimmed paragraph split across an ancestor's columns is cut at its outer
/// ends only, as `box-decoration-break: slice` has it (Inline Layout 3 §4.2):
/// four 12 pt lines in 24 pt columns, the first column losing the first
/// line's 2.6 above the cap height, the second the last line's 2.4 below the
/// baseline, and the lines either side of the break kept whole. (A fragment
/// before a break runs to its column's end, as the cutter has every box.)
#[test]
fn a_trimmed_paragraph_split_by_columns_is_cut_at_its_outer_ends() {
    let mut r = boot(concat!(
        "component App\n  view\n    view width=200 height=24 column-width=90 column-gap=20 column-fill=\"auto\"\n",
        "      text \"T\\nT\\nT\\nT\" white-space=\"pre\" font-size=10 text-box=\"trim-both cap alphabetic\" testId=\"split\"\n",
    ));
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, exact_kernel::Offer::definite(400.0, 400.0))
        .unwrap();
    let k = r.kernel();
    let key = k.find_by_test_id("split")[0];
    let f = k.fragments(key).expect("split");
    assert_eq!(
        f.iter().map(|f| f.lines).collect::<Vec<_>>(),
        [(0, 2), (2, 4)],
        "{f:?}"
    );
    // The second column starts after the first two lines, the first trimmed
    // at its top (9.4 + 12), and holds a whole line and the last, trimmed at
    // its baseline (12 + 9.6).
    assert!(
        (f[1].dy + 21.4).abs() < 1e-3 && (f[1].height - 21.6).abs() < 1e-3,
        "{f:?}"
    );
}
