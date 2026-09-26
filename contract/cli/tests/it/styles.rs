//! LLP 1017 P6: named styles, proven on the kernel's rows after boot.

use exact_kernel::{Color, Dimension, Kernel, WhiteSpace};
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
        Color::parse_hex("#ffffffd9").unwrap().into(),
        "`rgba(255, 255, 255, 0.85)` is `#ffffffd9`"
    );
    let tight = style_of("tight");
    assert_eq!(tight.padding_top, Dimension::Points(4.0));
    assert_eq!(tight.padding_bottom, Dimension::Points(4.0));
    assert_eq!(tight.border_radius_top_left, 16.0);
    assert_eq!(
        tight.background_color,
        Color::parse_hex("#000000").unwrap().into()
    );
    assert_eq!(tight.white_space, WhiteSpace::Nowrap);
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

fn style_of(r: &Runner<NoData>, id: &str) -> exact_kernel::StyleProps {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .style
        .clone()
}

fn refused(attrs: &str) -> contract::CompileError {
    contract::compile(&format!("component App\n  view\n    view {attrs}\n")).unwrap_err()
}

/// LLP 1053 G1: CSS `aspect-ratio` — `auto`, a ratio, or both.
#[test]
fn aspect_ratio_takes_the_css_grammar() {
    use exact_kernel::ratio::AspectRatio;
    let r = boot("component App\n  state wide = true\n  view\n    column\n      view aspect-ratio=\"16 / 9\" testId=\"a\"\n      view aspect-ratio=1.5 testId=\"b\"\n      view aspect-ratio=\"auto 4/3\" testId=\"c\"\n      view aspect-ratio=\"auto\" testId=\"d\"\n      view aspect-ratio=(wide ? \"2/1\" : \"1\") testId=\"e\"\n");
    for (id, css) in [
        ("a", "16/9"),
        ("b", "1.5"),
        ("c", "auto 4/3"),
        ("d", "auto"),
        ("e", "2/1"),
    ] {
        assert_eq!(
            style_of(&r, id).aspect_ratio,
            AspectRatio::parse(css).unwrap(),
            "{id}"
        );
    }
    for value in ["-1", "\"16:9\"", "\"auto auto\"", "\"50%\"", "\"1 / -2\""] {
        let e = refused(&format!("aspect-ratio={value}"));
        assert_eq!(e.id, "lower-attr-value", "{value}: {e}");
    }
    assert!(refused("aspect-ratio=\"16:9\"")
        .message
        .contains("auto 4 / 3"));
}

/// LLP 1053 G3: `flex-grow` is the longhand (`flex-basis` stays `auto`);
/// the later of `flex` and `flex-grow` sets the grow factor, as the later
/// CSS declaration wins; a negative factor is refused.
#[test]
fn flex_grow_is_a_longhand_and_the_later_binding_wins() {
    let r = boot("component App\n  view\n    row\n      view flex-grow=1 testId=\"grow\"\n      view flex=1 flex-grow=3 testId=\"flex-then-grow\"\n      view flex-grow=3 flex=1 testId=\"grow-then-flex\"\n");
    let grow = style_of(&r, "grow");
    assert_eq!(
        (grow.flex_grow, grow.flex_shrink, grow.flex_basis),
        (1.0, 1.0, Dimension::Auto)
    );
    let a = style_of(&r, "flex-then-grow");
    assert_eq!((a.flex_grow, a.flex_basis), (3.0, Dimension::Percent(0.0)));
    let b = style_of(&r, "grow-then-flex");
    assert_eq!((b.flex_grow, b.flex_basis), (1.0, Dimension::Percent(0.0)));
    for attrs in ["flex-grow=-1", "flex-shrink=-1", "flex=-2"] {
        let e = refused(attrs);
        assert_eq!(e.id, "lower-attr-value", "{attrs}: {e}");
        assert!(e.message.contains("nonnegative"), "{e}");
    }
}

/// LLP 1053: `direction` is CSS `direction` (inherited), no longer an old
/// spelling of `flex-direction`.
#[test]
fn direction_is_css_direction() {
    use exact_kernel::Direction;
    let r = boot("component App\n  view\n    column direction=\"rtl\" testId=\"outer\"\n      text \"שלום\" testId=\"inner\"\n");
    assert_eq!(style_of(&r, "outer").direction, Direction::Rtl);
    let k = r.kernel();
    let inner = k.node_by_key(k.find_by_test_id("inner")[0]).unwrap();
    assert_eq!(
        inner
            .computed_style(exact_kernel::StyleMask::INHERITED)
            .direction,
        Direction::Rtl
    );
    let e = refused("direction=\"row\"");
    assert_eq!(e.id, "lower-attr-value");
    assert!(e.message.contains("\"ltr\", \"rtl\""), "{e}");
    let e = refused("flexDirection=\"row\"");
    assert!(e.message.contains("`flex-direction`"), "{e}");
}
