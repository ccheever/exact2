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
