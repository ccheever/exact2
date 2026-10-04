//! The reader diary's book typography: `hyphens` and `text-indent` are rows
//! every host lays out; what needs fragmentation (`widows`, `orphans`,
//! multi-column) is refused by what it would need, not as a misspelling.

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
fn fragmentation_properties_are_refused_by_what_they_need() {
    for (name, needs) in [
        ("widows", "no fragmentation context"),
        ("orphans", "no fragmentation context"),
        ("column-count", "Multi-column Layout"),
        ("columns", "Multi-column Layout"),
        ("break-inside", "no fragmentation context"),
    ] {
        let e = contract::compile(&format!("component App\n  view\n    text \"a\" {name}=2\n"))
            .unwrap_err();
        assert_eq!(e.id, "lower-unknown-attr", "{e}");
        assert!(e.message.contains(needs), "{name}: {e}");
        // `contract vocab` says the same, never that a hyphenated name is a module.
        assert_eq!(contract_lower::vocab::open_set(name), None, "{name}");
        assert!(
            contract_lower::vocab::refusal(name).contains(needs),
            "{name}"
        );
    }
}
