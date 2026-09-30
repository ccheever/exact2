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
    // A `calc()` of a percentage and a length is one row, not text.
    assert_eq!(style_of("calc").width, Dimension::Calc(100.0, -89.0));
    assert_eq!(tight.white_space, WhiteSpace::Nowrap);
}

#[test]
fn a_class_chooses_between_two_styles_by_state() {
    let plan = contract::compile(&corpus("styles.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let chip = |r: &Runner<NoData>| {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id("chip")[0]).unwrap();
        (node.id, node.style.clone())
    };
    let (chip_id, idle) = chip(&r);
    assert_eq!(
        idle.background_color,
        Color::parse_hex("#cccccc").unwrap().into()
    );
    assert_eq!(idle.opacity, 0.5);
    // Only `Active` sets padding: the kernel's default while `Idle` is chosen.
    assert_eq!(idle.padding_top, Dimension::Points(0.0));
    r.dispatch(chip_id, exact_runner::Event::Press).unwrap();
    let (_, active) = chip(&r);
    assert_eq!(
        active.background_color,
        Color::parse_hex("#0000ff").unwrap().into()
    );
    assert_eq!(active.opacity, 1.0);
    assert_eq!(active.padding_top, Dimension::Points(12.0));
    assert_eq!(active.padding_left, Dimension::Points(12.0));
    r.dispatch(chip_id, exact_runner::Event::Press).unwrap();
    let (_, again) = chip(&r);
    assert_eq!(again.opacity, 0.5);
    assert_eq!(again.padding_top, Dimension::Points(0.0));
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
        ("position", StyleId::PositionType, "sticky"),
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
        ("scroll-behavior", StyleId::ScrollBehavior, "instant"),
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
fn nowrap_and_tabular_nums_reach_the_kernel_and_other_numeric_keywords_are_refused_by_name() {
    // @ref LLP 1053 §0 G4, G5
    let source = "component App\n  state tab = true\n  view\n    text \"111\" testId=\"target\" white-space=\"nowrap\" font-variant-numeric=(tab ? \"tabular-nums\" : \"normal\")\n";
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
    assert_eq!(style.white_space, exact_kernel::WhiteSpace::Nowrap);
    assert_eq!(style.font_variant_numeric, 1);
    for word in ["oldstyle-nums", "slashed-zero", "tabular-nums slashed-zero"] {
        let error = contract::compile(&format!(
            "component App\n  view\n    text \"1\" font-variant-numeric=\"{word}\"\n"
        ))
        .unwrap_err();
        assert_eq!(error.id, "lower-attr-value");
        assert!(
            error
                .message
                .contains("implements only `normal` and `tabular-nums`"),
            "{error}"
        );
    }
    let error = contract::compile(
        "component App\n  view\n    text \"1\" font-variant-numeric=\"tabular\"\n",
    )
    .unwrap_err();
    assert!(
        error
            .message
            .ends_with("expected one of \"normal\", \"tabular-nums\""),
        "{error}"
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

/// CSS's `transparent` is a colour wherever a colour is: a literal, a
/// computed value, a `light-dark()` arm and a border side.
#[test]
fn transparent_is_a_colour() {
    let src = "component A\n  state on = false\n  action flip writes on\n    on = not on\n  view\n    column\n      button testId=\"flip\" press=flip width=10 height=10\n      box testId=\"box\" background-color=\"transparent\" color=(on ? \"#ff0000\" : \"TRANSPARENT\") border-color=\"transparent currentcolor\"\n      text \"a\" testId=\"text\" background-color=\"light-dark(transparent, #000000)\"\n";
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let clear = Color::rgba(0, 0, 0, 0);
    fn style(r: &Runner<NoData>, id: &str) -> exact_kernel::StyleProps {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .clone()
    }
    let b = style(&r, "box");
    assert_eq!(b.background_color, clear.into());
    assert_eq!(b.text_color, clear.into());
    assert_eq!(b.border_colors(clear.into())[0], clear.into());
    assert_eq!(
        style(&r, "text").background_color,
        exact_kernel::ColorValue::LightDark(clear, Color::parse_hex("#000000").unwrap())
    );
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_eq!(
        style(&r, "box").text_color,
        Color::parse_hex("#ff0000").unwrap().into()
    );
}

/// LLP 1066: `background-image` takes `none` or one gradient — as an
/// attribute, in a `style`, and in a conditional — and what no host draws
/// is refused at compile time, by name.
#[test]
fn background_image_takes_one_gradient_and_refuses_the_rest_by_name() {
    use exact_kernel::gradient::BackgroundImage;
    let fade = "linear-gradient(transparent, light-dark(#ffffff, #000000) 40%)";
    let r = boot(&format!("style Veil\n  background-image=\"{fade}\"\n\ncomponent App\n  state on = true\n  view\n    column\n      view class=Veil testId=\"a\"\n      view background-image=(on ? \"radial-gradient(circle at top, #000, #fff)\" : \"none\") testId=\"b\"\n      view background-image=\"none\" testId=\"c\"\n"));
    assert_eq!(
        style_of(&r, "a").background_image,
        BackgroundImage::parse(fade).unwrap()
    );
    assert!(style_of(&r, "a")
        .background_image
        .gradient()
        .unwrap()
        .is_scheme_aware());
    assert_eq!(
        style_of(&r, "b").background_image,
        BackgroundImage::parse("radial-gradient(circle at top, #000, #fff)").unwrap()
    );
    assert_eq!(style_of(&r, "c").background_image.gradient(), None);
    for (value, says) in [
        (
            "repeating-linear-gradient(#000, #fff 10%)",
            "repeating-linear-gradient() is not implemented",
        ),
        (
            "conic-gradient(#000, #fff)",
            "conic gradients are not implemented",
        ),
        ("url(a.png)", "an image as a background is not implemented"),
        (
            "linear-gradient(#000, #fff), linear-gradient(#fff, #000)",
            "several background layers",
        ),
        ("linear-gradient(red, blue)", "a stop's colour is"),
        (
            "linear-gradient(#000 10px, #fff)",
            "a stop's position is a percentage",
        ),
    ] {
        let e = refused(&format!("background-image=\"{value}\""));
        assert_eq!(e.id, "lower-attr-value", "{value}: {e}");
        assert!(e.message.contains(says), "{value}: {e}");
    }
    let e = refused("background-image=(true ? \"none\" : \"conic-gradient(#000, #fff)\")");
    assert!(e.message.contains("conic"), "{e}");
}

#[test]
fn pre_and_backdrop_filter_reach_the_kernel_and_the_rest_of_css_filters_is_refused_by_name() {
    // @ref LLP 1053 G5; LLP 1053.000 D1 — a dynamic value takes the same
    // grammar at run time as a literal at compile time.
    let source = "component App\n  state blur = 8\n  view\n    main\n      box testId=\"glass\" backdrop-filter=\"blur(\" + toString(blur) + \"px)\"\n        text \"a\\tb\" testId=\"code\" white-space=\"pre\"\n      box testId=\"plain\" backdrop-filter=\"none\"\n";
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
    let style = |id: &str| {
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .clone()
    };
    assert_eq!(style("code").white_space, exact_kernel::WhiteSpace::Pre);
    assert_eq!(style("glass").backdrop_blur, 8.0);
    assert_eq!(style("plain").backdrop_blur, 0.0);
    for (value, named) in [
        ("blur(20px) saturate(180%)", "`saturate()` is CSS"),
        ("brightness(1.2)", "`brightness()` is CSS"),
        ("url(#f)", "`url()` is CSS"),
        ("blur(2em)", "a length in px"),
    ] {
        let error = contract::compile(&format!(
            "component App\n  view\n    box backdrop-filter=\"{value}\"\n"
        ))
        .unwrap_err();
        assert_eq!(error.id, "lower-attr-value");
        assert!(error.message.contains(named), "{error}");
    }
    // @ref LLP 1053.000 D4 — every platform material by name; others refused.
    for name in [
        "ultra-thin",
        "chrome-dark",
        "prominent",
        "sidebar",
        "hud-window",
        "glass-clear",
    ] {
        contract::compile(&format!(
            "component App\n  view\n    box backgroundMaterial=\"{name}\"\n"
        ))
        .unwrap();
    }
    let error =
        contract::compile("component App\n  view\n    box backgroundMaterial=\"frosted\"\n")
            .unwrap_err();
    assert!(
        error
            .message
            .contains("`backgroundMaterial=\"frosted\"` is not a material; materials: ultra-thin,"),
        "{error}"
    );
}

/// LLP 1074 T1: `position` is `static` unless authored, except on a box that
/// contains its absolutely positioned descendants on every host, which is
/// lowered `relative`; an authored `static` there is refused.
#[test]
fn a_box_that_clips_transforms_or_animates_is_lowered_relative() {
    use exact_kernel::PositionType::{Absolute, Relative, Static};
    let plan = contract::compile(
        r#"component App
  view
    column testId="plain"
      box testId="clips" overflow="hidden"
      box testId="moves" translate="4px 0px"
      box testId="presses" press-scale=0.96
      box testId="fades" transition="opacity 100ms"
      box testId="glass" backgroundMaterial="glass"
      box testId="dim" opacity=0.5
      box testId="raised" z-index=2 top=4
      scroll testId="scrolls" height=40
      canvas testId="draws"
      box testId="pinned" position="absolute" overflow="hidden"
      box testId="named" position="relative"
"#,
    )
    .unwrap();
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
    for (id, position) in [
        ("plain", Static),
        ("clips", Relative),
        ("moves", Relative),
        ("presses", Relative),
        ("fades", Relative),
        ("glass", Relative),
        ("dim", Static),
        ("raised", Static),
        ("scrolls", Relative),
        ("draws", Relative),
        ("pinned", Absolute),
        ("named", Relative),
    ] {
        let key = k.find_by_test_id(id)[0];
        assert_eq!(
            k.node_by_key(key).unwrap().style.position_type,
            position,
            "{id}"
        );
    }
    let error = contract::compile(
        "component App\n  view\n    box position=\"static\" overflow=\"hidden\"\n",
    )
    .unwrap_err();
    assert_eq!(error.id, "lower-attr-value");
    assert!(error.message.contains("remove `position`"), "{error:?}");
    contract::compile("component App\n  view\n    box position=\"static\"\n").unwrap();
}
