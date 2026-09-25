//! LLP 1017 P6: named styles, proven on the kernel's rows after boot.

use exact_kernel::{Color, Dimension, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn a_class_applies_its_style_and_the_nodes_own_attribute_wins() {
    let plan = contract::compile(&corpus("styles.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let style_of = |id: &str| {
        let key = k.find_by_test_id(id)[0];
        k.node_by_key(key).unwrap().style.clone()
    };
    let card = style_of("card");
    assert_eq!(card.padding_top, Dimension::Points(16.0));
    assert_eq!(card.padding_left, Dimension::Points(16.0));
    assert_eq!(card.border_radius_top_left, 16.0);
    assert_eq!(card.row_gap, 10.0);
    assert_eq!(
        card.background_color,
        Color::parse_hex("#ffffffd9").unwrap().into()
    );
    let tight = style_of("tight");
    assert_eq!(tight.padding_top, Dimension::Points(4.0));
    assert_eq!(tight.padding_bottom, Dimension::Points(4.0));
    assert_eq!(tight.border_radius_top_left, 16.0);
    assert_eq!(
        tight.background_color,
        Color::parse_hex("#000000").unwrap().into()
    );
    // A `calc()` of a percentage and a length is one row, not text.
    assert_eq!(style_of("calc").width, Dimension::Calc(100.0, -89.0));
}

#[test]
fn a_style_is_refused_with_the_css_name_for_an_old_spelling() {
    let src =
        "style Card\n  radius=16\ncomponent A\n  view\n    column class=Card\n      text \"a\"\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "lower-unknown-attr");
    assert!(e.message.contains("`border-radius`"), "{e}");
}

#[test]
fn attribute_typos_suggest_only_unambiguous_accepted_spellings() {
    for (typo, correct, value) in [
        ("widht", "width", "100"),
        ("align-item", "align-items", "\"center\""),
        ("paddng", "padding", "12"),
        ("opaccity", "opacity", "0.5"),
        ("backgrounf-color", "background-color", "\"#123456\""),
    ] {
        for style in [false, true] {
            let source = if style {
                format!(
                    "style Card\n  {typo}={value}\ncomponent App\n  view\n    view class=Card\n"
                )
            } else {
                format!("component App\n  view\n    view {typo}={value}\n")
            };
            let error = contract::compile(&source).unwrap_err();
            assert_eq!(error.id, "lower-unknown-attr");
            assert!(
                error
                    .message
                    .ends_with(&format!("; did you mean `{correct}`?")),
                "{error}"
            );
            assert_eq!(error.span.line, if style { 2 } else { 3 });
            contract::compile(&source.replace(typo, correct)).unwrap();
        }
    }
    for name in [
        "overflow-z",
        "unrelated-property",
        "éwidth",
        "xy",
        &"w".repeat(65),
    ] {
        let source = format!("component App\n  view\n    view {name}=1\n");
        let error = contract::compile(&source).unwrap_err();
        assert!(!error.message.contains("did you mean"), "{error}");
    }
    let error = contract::compile(
        "style Card\n  tesId=\"x\"\ncomponent App\n  view\n    view class=Card\n",
    )
    .unwrap_err();
    assert!(
        !error.message.contains("did you mean"),
        "style must not suggest props: {error}"
    );
    let error = contract::compile("component App\n  view\n    view tesId=\"x\"\n").unwrap_err();
    assert!(error.message.contains("did you mean `testId`"), "{error}");
}

#[test]
fn enum_refusals_list_accepted_values_and_each_suggestion_compiles() {
    use exact_kernel::StyleId;
    for (attr, row, invalid) in [
        ("align-items", StyleId::AlignItems, "middle"),
        ("display", StyleId::Display, "flexbox"),
        ("overflow", StyleId::OverflowX, "clip"),
        ("object-fit", StyleId::ObjectFit, "stretch"),
        ("font-style", StyleId::FontStyle, "slanted"),
        ("position", StyleId::PositionType, "static"),
        ("border-style", StyleId::BorderStyleTop, "dashed"),
        ("align-self", StyleId::AlignSelf, "middle"),
        (
            "overscroll-behavior",
            StyleId::OverscrollBehaviorX,
            "bounce",
        ),
        (
            "overscroll-behavior-x",
            StyleId::OverscrollBehaviorX,
            "bounce",
        ),
        (
            "overscroll-behavior-y",
            StyleId::OverscrollBehaviorY,
            "bounce",
        ),
        ("scrollbar-width", StyleId::ScrollbarWidth, "wide"),
        ("touch-action", StyleId::TouchAction, "swipe"),
    ] {
        for named_style in [false, true] {
            let source = if named_style {
                format!("style Card\n  {attr}=\"{invalid}\"\ncomponent App\n  view\n    view class=Card testId=\"target\"\n")
            } else {
                format!("component App\n  view\n    view {attr}=\"{invalid}\" testId=\"target\"\n")
            };
            let error = contract::compile(&source).unwrap_err();
            assert_eq!(error.id, "lower-attr-value");
            assert_eq!(error.span.line, if named_style { 2 } else { 3 });
            let values = error.message.split_once("expected one of ").unwrap().1;
            let expected = row
                .enum_names()
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            assert_eq!(values, expected);
            // Use the actual diagnostic's choices as authored literals.
            for value in values.split(", ") {
                let corrected = source.replace(&format!("\"{invalid}\""), value);
                let plan =
                    contract::compile(&corrected).unwrap_or_else(|e| panic!("{corrected}: {e}"));
                let r = Runner::boot(
                    plan,
                    NoData,
                    Kernel::with_monospace(),
                    Default::default(),
                    "/",
                )
                .unwrap();
                let k = r.kernel();
                let style = &k.node_by_key(k.find_by_test_id("target")[0]).unwrap().style;
                assert!(style.mask.has(row), "authored row must reach the kernel");
                let exact_kernel::RowValue::Enum(actual) = style.get(row) else {
                    panic!("expected enum row")
                };
                assert_eq!(format!("{actual:?}"), value, "{corrected}");
            }
        }
    }
    let error =
        contract::compile("component App\n  view\n    view wrap-flow=\"start\"\n").unwrap_err();
    assert_eq!(error.id, "lower-attr-value");
    assert!(
        error.message.contains("implements `both` (or `auto`)"),
        "{error}"
    );
    assert!(
        !error.message.contains("expected one of"),
        "narrow authoring rule remains authoritative"
    );
}

#[test]
fn auto_enum_literals_in_branches_keep_dimension_refusals_separate() {
    let source = "component App\n  state chosen = true\n  view\n    view testId=\"target\" align-self=(chosen ? \"auto\" : \"center\") width=\"auto\"\n";
    let plan = contract::compile(source).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let style = &k.node_by_key(k.find_by_test_id("target")[0]).unwrap().style;
    assert_eq!(style.align_self, exact_kernel::AlignSelf::Auto);
    assert_eq!(style.width, Dimension::Auto);
    let error =
        contract::compile("component App\n  view\n    view padding=\"auto\"\n").unwrap_err();
    assert_eq!(error.id, "lower-attr-value");
    assert!(
        error.message.ends_with("`auto` is not admitted here"),
        "{error}"
    );
}
