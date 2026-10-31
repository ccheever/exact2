use super::*;
use exact_kernel::StyleValue;

fn css(rows: &[(StyleId, StyleValue)], fonts: &[&str]) -> String {
    let mut style = StyleProps::default();
    for (id, value) in rows {
        style.set_dynamic(*id, value).unwrap();
    }
    let fonts: Vec<String> = fonts.iter().map(|f| f.to_string()).collect();
    css_text(&style, &fonts).0
}

/// LLP 1001 §6's balance; LLP 1104 D3's `::placeholder` colour.
#[test]
fn balance_and_the_placeholder_colour_are_written_for_the_browser() {
    let text = |id, v: &str| css(&[(id, StyleValue::Text(v.into()))], &[]);
    let wrap = text(StyleId::TextWrapStyle, "balance");
    assert_eq!(wrap, "text-wrap-style:balance;");
    let ink = text(StyleId::PlaceholderColor, "#00800080");
    assert_eq!(ink, "--exact-placeholder-color:rgba(0,128,0,0.5019608);");
}

#[test]
fn backdrop_filters_keep_order_and_identity_in_css() {
    exact_kernel::style::link_backdrop_filter();
    for value in [
        "none",
        "blur(0px)",
        "saturate(1)",
        "saturate(0)",
        "blur(12px) saturate(1.14)",
        "saturate(1.8) blur(4px)",
    ] {
        assert_eq!(
            css(
                &[(StyleId::BackdropFilter, StyleValue::Text(value.into()))],
                &[]
            ),
            format!("backdrop-filter:{value};")
        );
    }
}

#[test]
fn cursor_emits_the_css_keyword_including_explicit_auto_override() {
    for value in ["auto", "default", "crosshair"] {
        assert_eq!(
            css(&[(StyleId::Cursor, StyleValue::Text(value.into()))], &[]),
            format!("cursor:{value};")
        );
    }
}

/// The declarations `css_text` composes itself, as the `write!`-built
/// text had them, byte for byte.
#[test]
fn composed_declarations_keep_their_text() {
    let cases = [
        css(
            &[(StyleId::FontFamily, StyleValue::Number(0.0))],
            &["Inter \"Var\"\n"],
        ),
        css(
            &[(StyleId::FontFamily, StyleValue::Number(1.0))],
            &["x", "ui-monospace"],
        ),
        css(
            &[(
                StyleId::Transition,
                StyleValue::Text(
                    "opacity 250ms ease-in-out, all 0.5s cubic-bezier(0.4, 0, 0.2, 1) 100ms".into(),
                ),
            )],
            &[],
        ),
        css(&[(StyleId::LineClamp, StyleValue::Number(3.0))], &[]),
        // LLP 1077 D4: one row, CSS's list.
        css(
            &[(
                StyleId::BoxShadow,
                StyleValue::Text(
                    "0 2.5px 12px #11223359, inset 0 1px 0 2px light-dark(#00000080, #ffffff)"
                        .into(),
                ),
            )],
            &[],
        ),
        css(
            &[(StyleId::BoxShadow, StyleValue::Text("none".into()))],
            &[],
        ),
        css(
            &[(StyleId::TextTransform, StyleValue::Text("uppercase".into()))],
            &[],
        ),
        css(
            &[
                (
                    StyleId::FontVariantNumeric,
                    StyleValue::Text("tabular-nums".into()),
                ),
                (StyleId::WhiteSpace, StyleValue::Text("nowrap".into())),
            ],
            &[],
        ),
        css(
            &[(
                StyleId::FontVariantNumeric,
                StyleValue::Text("normal".into()),
            )],
            &[],
        ),
        css(
            &[(StyleId::WhiteSpace, StyleValue::Text("pre-line".into()))],
            &[],
        ),
        css(
            &[
                (StyleId::Opacity, StyleValue::Number(0.125)),
                (StyleId::Width, StyleValue::Number(33.5)),
            ],
            &[],
        ),
    ];
    let golden = [
        r#"font-family:"Inter \"Var\"\a ";"#,
        "font-family:ui-monospace,monospace;",
        "transition:opacity 0.25s ease-in-out 0s,all 0.5s cubic-bezier(0.4,0,0.2,1) 0.1s;",
        "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:3;overflow:hidden;",
        "box-shadow:0px 2.5px 12px 0px #11223359, inset 0px 1px 0px 2px light-dark(#00000080, #ffffffff);",
        "box-shadow:none;",
        "text-transform:uppercase;",
        "font-variant-numeric:tabular-nums;white-space:nowrap;",
        "font-variant-numeric:normal;",
        "white-space:pre-line;",
        "width:33.5px;opacity:0.125;",
    ];
    assert_eq!(cases, golden);
}

/// LLP 1053: `aspect-ratio` as authored (never a rounded float),
/// `direction`, the `flex-grow` longhand and `transform-origin` reach
/// the page as CSS.
#[test]
fn layout_rows_keep_their_css() {
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    for (rows, want) in [
        (
            vec![(StyleId::AspectRatio, t("16/9"))],
            "aspect-ratio:16 / 9;",
        ),
        (
            vec![(StyleId::AspectRatio, StyleValue::Number(2.0))],
            "aspect-ratio:2 / 1;",
        ),
        (
            vec![(StyleId::AspectRatio, t("4/3 auto"))],
            "aspect-ratio:auto 4 / 3;",
        ),
        (
            vec![(StyleId::AspectRatio, StyleValue::Auto)],
            "aspect-ratio:auto;",
        ),
        (
            vec![(StyleId::Direction, t("rtl"))],
            "direction:rtl;--exact-separator-x:100%;",
        ),
        // LLP 1061 D6: canonical, each axis a percentage or px.
        (
            vec![(StyleId::TransformOrigin, t("top left"))],
            "transform-origin:0% 0%;",
        ),
        (
            vec![(StyleId::TransformOrigin, StyleValue::Percent(25.0))],
            "transform-origin:25% 50%;",
        ),
        (
            vec![(StyleId::TransformOrigin, t("right 4px 0"))],
            "transform-origin:100% 4px;",
        ),
        (
            vec![(StyleId::FlexGrow, StyleValue::Number(1.0))],
            "flex-grow:1;",
        ),
    ] {
        assert_eq!(css(&rows, &[]), want);
    }
}

#[test]
fn every_grid_row_has_css_and_none_is_skipped() {
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    for (row, value, want) in [
        (
            StyleId::GridTemplateColumns,
            t("repeat(2, minmax(80px, 1fr)) 25%"),
            "grid-template-columns:repeat(2, minmax(80px, 1fr)) 25%;",
        ),
        (
            StyleId::GridTemplateRows,
            t("40px auto min-content max-content"),
            "grid-template-rows:40px auto min-content max-content;",
        ),
        (
            StyleId::GridColumn,
            t("-3 / span 2"),
            "grid-column:-3 / span 2;",
        ),
        (StyleId::GridRow, t("2 / -1"), "grid-row:2 / -1;"),
        (
            StyleId::GridAutoFlow,
            t("row dense"),
            "grid-auto-flow:row dense;",
        ),
        (StyleId::JustifyItems, t("center"), "justify-items:center;"),
    ] {
        let mut style = StyleProps::default();
        style.set_dynamic(row, &value).unwrap();
        let (text, skipped) = css_text(&style, &[]);
        assert_eq!(text, want, "{row:?}");
        assert!(skipped.is_empty(), "{row:?}: {skipped:?}");
    }
}

/// LLP 1066: a gradient is one `background-image` declaration after the
/// colour it paints over; a `light-dark()` stop is the browser's to
/// resolve, and `none` clears.
#[test]
fn a_3d_rotate_and_translate_are_one_declaration_each() {
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    let text = css(
        &[
            (StyleId::Rotate, t("y 30deg")),
            (StyleId::RotateAxis, t("y 30deg")),
            (StyleId::Translate, t("1px 2px 3px")),
            (StyleId::TranslateZ, t("1px 2px 3px")),
            (StyleId::Perspective, StyleValue::Number(800.0)),
        ],
        &[],
    );
    assert!(text.contains("rotate:y 30deg;"), "{text}");
    assert!(text.contains("translate:1px 2px 3px;"), "{text}");
    assert!(text.contains("perspective:800px;"), "{text}");
    assert_eq!(text.matches("rotate").count(), 1, "{text}");
    // Chess diary #4: a percentage is the browser's to resolve.
    let text = css(
        &[
            (StyleId::Translate, t("-50% 4px")),
            (StyleId::TranslatePercent, t("-50% 4px")),
            (StyleId::TranslateZ, t("-50% 4px")),
        ],
        &[],
    );
    assert!(text.contains("translate:-50% 4px;"), "{text}");
    assert_eq!(text.matches("translate").count(), 1, "{text}");
}

#[test]
fn text_stroke_is_the_compat_standards_two_properties() {
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    let text = css(
        &[
            (StyleId::TextStrokeWidth, t("2px #ff0000")),
            (StyleId::TextStrokeColor, t("2px #ff0000")),
        ],
        &[],
    );
    assert!(text.contains("-webkit-text-stroke-width:2px;"), "{text}");
    assert!(text.contains("-webkit-text-stroke-color:"), "{text}");
}

#[test]
fn apple_continuous_is_a_superellipse_over_a_scaled_radius() {
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    let text = css(
        &[
            (StyleId::BorderRadiusTopLeft, StyleValue::Number(10.0)),
            (StyleId::BorderRadiusTopRight, StyleValue::Number(10.0)),
            (StyleId::CornerShape, t("-exact-continuous squircle")),
        ],
        &[],
    );
    assert!(
        text.contains("border-top-left-radius:calc(10px * 1.52);"),
        "{text}"
    );
    assert!(text.contains("border-top-right-radius:10px;"), "{text}");
    assert!(
        text.contains("corner-shape:superellipse(1.6) squircle superellipse(1.6) squircle;"),
        "{text}"
    );
}

#[test]
fn a_capsule_control_takes_no_curve() {
    fn corner(style: &str) -> String {
        css(
            &[(StyleId::ControlCornerStyle, StyleValue::Text(style.into()))],
            &[],
        )
    }
    assert!(
        corner("capsule")
            .contains("--exact-control-radius:calc(infinity * 1px);--exact-control-curve:round;"),
        "{}",
        corner("capsule")
    );
    // Any other corners leave the platform's curve to the shell (index.html).
    assert!(
        corner("medium").contains("--exact-control-radius:revert;--exact-control-curve:initial;"),
        "{}",
        corner("medium")
    );
}

#[test]
fn background_image_is_one_declaration_over_the_colour() {
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    assert_eq!(
        css(
            &[
                (StyleId::BackgroundColor, t("#102030")),
                (StyleId::BackgroundImage, t("linear-gradient(to top, transparent, light-dark(#fff, #000) 40%)")),
            ],
            &[]
        ),
        "background-color:rgba(16,32,48,1);background-image:linear-gradient(0deg, #00000000 0%, light-dark(#ffffffff, #000000ff) 40%);"
    );
    assert_eq!(
        css(&[(StyleId::BackgroundImage, t("radial-gradient(circle at 10px bottom, #000 25%, #fff)"))], &[]),
        "background-image:radial-gradient(circle farthest-corner at 10px 100%, #000000ff 25%, #ffffffff 100%);"
    );
    assert_eq!(
        css(&[(StyleId::BackgroundImage, t("none"))], &[]),
        "background-image:none;"
    );
    // LLP 1008: the admitted snap alignments are CSS's own.
    assert_eq!(
        css(&[(StyleId::ScrollSnapAlign, t("end"))], &[]),
        "scroll-snap-align:end;"
    );
    // LLP 1066 D7: the browser's own `fixed`, the gradient box the viewport.
    assert_eq!(
        css(&[(StyleId::BackgroundAttachment, t("fixed"))], &[]),
        "background-attachment:fixed;"
    );
}

/// The feedback's separate factor leaves the row's scale and
/// authored transition list intact; 1 needs no effect.
#[test]
fn a_press_scale_keeps_the_authored_scale_and_transitions() {
    let n = StyleValue::Number;
    fn t(s: &str) -> StyleValue<'_> {
        StyleValue::Text(s.into())
    }
    assert_eq!(
        css(&[(StyleId::PressScale, n(0.97))], &[]),
        "scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.97;"
    );
    assert_eq!(
        css(
            &[
                (StyleId::Scale, n(1.5)),
                (StyleId::Transition, t("all 200ms ease")),
                (StyleId::PressScale, n(0.994)),
            ],
            &[]
        ),
        "--exact-scale:1.5;transition:all 0.2s ease 0s,scale 0s,--exact-scale 0.2s ease 0s;scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.994;"
    );
    assert_eq!(css(&[(StyleId::PressScale, n(1.0))], &[]), "");
    assert_eq!(
        css(
            &[
                (StyleId::Transition, t("opacity 1s")),
                (StyleId::PressScale, n(1.0))
            ],
            &[]
        ),
        "transition:opacity 1s ease 0s;"
    );
}
/// Only a pressable node's scale animation also animates
/// `--exact-scale`, under a rule of its own: an animated custom property
/// keeps Chrome from compositing the animation (a pan/zoom re-rastered
/// its picture every frame).
#[test]
fn only_a_pressable_node_animates_the_scale_custom_property() {
    use exact_motion::animation::{Animations, Keyframes};
    let mut list = Animations::parse("grow 1s linear infinite, fade 2s").unwrap();
    list.0[0].keyframes = Keyframes::parse("from{scale:1;translate:0px 0px}to{scale:1.6}").unwrap();
    list.0[1].keyframes = Keyframes::parse("to{opacity:0}").unwrap();
    let (grow, fade) = (&list.0[0], &list.0[1]);
    assert_eq!(keyframes_name(grow, false), "grow");
    assert!(
        !keyframes_css(grow, false).contains("--exact"),
        "{}",
        keyframes_css(grow, false)
    );
    assert_eq!(keyframes_name(grow, true), "grow-exact-press");
    assert!(keyframes_css(grow, true).contains("scale:1.6;--exact-scale:1.6;"));
    // A pressable node's animation that leaves `scale` alone keeps its rule.
    assert_eq!(keyframes_name(fade, true), "fade");
    assert_eq!(animations_css(&list, false), list.css());
    assert_eq!(
        animations_css(&list, true),
        list.css().replace(" grow,", " grow-exact-press,")
    );
}
