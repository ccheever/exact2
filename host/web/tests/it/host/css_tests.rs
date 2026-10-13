use super::*;

#[test]
fn complex_svg_attributes_stay_omitted_across_state_and_segment_changes() {
    for length in [
        "env(viewport-segment-height 1 0)",
        "calc(env(viewport-segment-height 1 0) + 10%)",
        "min(env(viewport-segment-height 1 0), 50%)",
        "max(5px, env(viewport-segment-height 1 0))",
        "clamp(1px, env(viewport-segment-height 1 0), 100px)",
        "env(viewport-segment-height 1 0, 25%)",
        "ENV(viewport-segment-height 1 0, 25%)",
        "env(safe-area-inset-top)",
        "calc(10vh + 10px)",
        "min(10vw, 50%)",
        "max(5px, 10vw)",
        "clamp(1px, 10vw, 100px)",
    ] {
        let plan = contract::compile(&format!(
            r##"
component App
  state finite = false
  action toggle
    finite = not finite
  view
    column
      button "Toggle" testId="toggle" press=toggle
      svg width=200 height=400
        defs
          radialGradient id="gradient" testId="gradient" cx=(finite ? "0px" : "{length}")
            stop offset="0%" stop-color="#14b8a6"
        svg testId="static" height="{length}"
        svg testId="nested" height=(finite ? "0px" : "{length}")
          rect width="100%" height="100%" fill="url(#gradient)"
"##
        ))
        .unwrap();
        exact_web::link(exact_web_capabilities::ALL);
        let flat = exact_runner::Viewport::sized(800.0, 600.0);
        let (mut host, first) =
            Host::boot(&plan.encode(), caltrain_data::Caltrain, flat, "/").unwrap();
        let first: serde_json::Value = serde_json::from_str(&first).unwrap();
        let ids: Vec<_> = [
            ("static", "height"),
            ("nested", "height"),
            ("gradient", "cx"),
        ]
        .into_iter()
        .map(|(test_id, attr)| {
            let op = first["ops"]
                .as_array()
                .unwrap()
                .iter()
                .find(|op| op["props"]["data-testid"] == test_id)
                .unwrap();
            assert!(op["props"][attr].is_null(), "{length}: {op}");
            (op["id"].clone(), attr)
        })
        .collect();
        let folded = exact_runner::Viewport {
            fold: exact_runner::Fold {
                cols: 2,
                ..Default::default()
            },
            ..flat
        };
        for viewport in [folded, flat] {
            let batch: serde_json::Value =
                serde_json::from_str(&host.resize(viewport, 0.0)).unwrap();
            assert!(
                batch["ops"].as_array().unwrap().is_empty(),
                "{length}: {batch}"
            );
            for finite in [true, false] {
                let button = view_with_test_id(&host, "toggle");
                let batch: serde_json::Value =
                    serde_json::from_str(&host.dispatch(button, Event::Press)).unwrap();
                for (id, attr) in &ids[1..] {
                    let op = batch["ops"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|op| op["op"] == "props" && &op["id"] == id)
                        .unwrap();
                    if finite {
                        assert_eq!(op["set"][attr], "0", "{length}: {batch}");
                    } else {
                        assert_eq!(op["clear"], serde_json::json!([attr]), "{length}: {batch}");
                    }
                }
                let document = exact_web::document::project(host.runner()).unwrap();
                assert!(
                    !document.root.contains("env("),
                    "{length}: {}",
                    document.root
                );
                assert_eq!(
                    document.root.contains("cx=\"0\""),
                    finite,
                    "{}",
                    document.root
                );
            }
        }
    }
}

#[test]
fn style_rows_lower_to_css_by_their_names() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut s = StyleProps::default();
    s.set_dynamic(StyleId::Width, &StyleValue::Percent(100.0))
        .unwrap();
    s.set_dynamic(StyleId::MaxWidth, &StyleValue::Number(640.0))
        .unwrap();
    s.set_dynamic(StyleId::Height, &StyleValue::Auto).unwrap();
    s.set_dynamic(StyleId::FlexGrow, &StyleValue::Number(1.0))
        .unwrap();
    s.set_dynamic(StyleId::TextColor, &StyleValue::Text("#c0392b".into()))
        .unwrap();
    s.set_dynamic(StyleId::BorderRadiusTopLeft, &StyleValue::Number(16.0))
        .unwrap();
    s.set_dynamic(StyleId::BorderWidthBottom, &StyleValue::Number(1.0))
        .unwrap();
    s.set_dynamic(
        StyleId::BorderStyleBottom,
        &StyleValue::Text("solid".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::BorderColorBottom,
        &StyleValue::Text("currentColor".into()),
    )
    .unwrap();
    s.set_dynamic(StyleId::AlignSelf, &StyleValue::Text("center".into()))
        .unwrap();
    s.set_dynamic(StyleId::PositionType, &StyleValue::Text("absolute".into()))
        .unwrap();
    s.set_dynamic(StyleId::Rotate, &StyleValue::Number(45.0))
        .unwrap();
    s.set_dynamic(StyleId::Translate, &StyleValue::Vec2(10.0, -4.5))
        .unwrap();
    s.set_dynamic(StyleId::Opacity, &StyleValue::Number(0.5))
        .unwrap();
    s.set_dynamic(StyleId::LetterSpacing, &StyleValue::Number(1.2))
        .unwrap();
    s.set_dynamic(StyleId::TextIndent, &StyleValue::Number(-24.0))
        .unwrap();
    s.set_dynamic(StyleId::Hyphens, &StyleValue::Text("auto".into()))
        .unwrap();
    s.set_dynamic(StyleId::TextWrapStyle, &StyleValue::Text("auto".into()))
        .unwrap();
    // LLP 1093: multi-column layout reaches CSS by its own names.
    for (row, value) in [
        (StyleId::ColumnCount, StyleValue::Number(3.0)),
        (StyleId::ColumnWidth, StyleValue::Number(120.0)),
        (StyleId::ColumnFill, StyleValue::Text("auto".into())),
        (StyleId::ColumnRuleWidth, StyleValue::Number(1.0)),
        (StyleId::ColumnRuleStyle, StyleValue::Text("solid".into())),
        (StyleId::Widows, StyleValue::Number(3.0)),
        (StyleId::Orphans, StyleValue::Number(1.0)),
        (StyleId::BreakBefore, StyleValue::Text("column".into())),
        (
            StyleId::BreakInside,
            StyleValue::Text("avoid-column".into()),
        ),
    ] {
        s.set_dynamic(row, &value).unwrap();
    }
    s.set_dynamic(
        StyleId::PaddingTop,
        &StyleValue::Text("env(safe-area-inset-top)".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::PaddingBottom,
        &StyleValue::Text("calc(env(safe-area-inset-bottom) + 12px)".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::MarginLeft,
        &StyleValue::Text("calc(env(safe-area-inset-left) - 2px)".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::MinWidth,
        &StyleValue::Text("calc(100% - 89px)".into()),
    )
    .unwrap();
    s.set_dynamic(StyleId::LineClamp, &StyleValue::Number(2.0))
        .unwrap();
    s.set_dynamic(StyleId::ScrollBehavior, &StyleValue::Text("smooth".into()))
        .unwrap();
    s.set_dynamic(StyleId::ScrollPaddingTop, &StyleValue::Number(92.0))
        .unwrap();
    s.set_dynamic(
        StyleId::ScrollPaddingBottom,
        &StyleValue::Text("calc(env(safe-area-inset-bottom) + 49px)".into()),
    )
    .unwrap();
    let (css, skipped) = css_text(&s, &[]);
    for expected in [
        "scroll-behavior:smooth;",
        "scroll-padding-top:92px;",
        "scroll-padding-bottom:calc(env(safe-area-inset-bottom) + 49px);",
        "display:-webkit-box;",
        "-webkit-box-orient:vertical;",
        "-webkit-line-clamp:2;",
        "width:100%;",
        "min-width:calc(100% - 89px);",
        "max-width:640px;",
        "height:auto;",
        "flex-grow:1;",
        "color:rgba(192,57,43,1);",
        "border-top-left-radius:16px;",
        "border-bottom-width:1px;",
        "border-bottom-style:solid;",
        "border-bottom-color:currentcolor;",
        "align-self:center;",
        "position:absolute;",
        "rotate:45deg;",
        "translate:10px -4.5px;",
        "opacity:0.5;",
        "letter-spacing:1.2px;",
        "text-indent:-24px;",
        "hyphens:auto;",
        "text-wrap-style:auto;",
        "column-count:3;",
        "column-width:120px;",
        "column-fill:auto;",
        "column-rule-width:1px;",
        "column-rule-style:solid;",
        "widows:3;",
        "orphans:1;",
        "break-before:column;",
        "break-inside:avoid-column;",
        "padding-top:env(safe-area-inset-top);",
        "padding-bottom:calc(env(safe-area-inset-bottom) + 12px);",
        "margin-left:calc(env(safe-area-inset-left) - 2px);",
    ] {
        assert!(css.contains(expected), "{expected} in {css}");
    }
    assert!(skipped.is_empty(), "{skipped:?}");

    s.set_dynamic(StyleId::FontFamily, &StyleValue::Number(2.0))
        .unwrap();
    let families = vec![String::new(), String::new(), "sans-serif".into()];
    let (css, skipped) = css_text(&s, &families);
    assert!(css.contains("font-family:sans-serif;"), "{css}");
    assert!(!css.contains("font-family:\"sans-serif\";"), "{css}");
    assert!(skipped.is_empty(), "{skipped:?}");
    // Chrome knows no `ui-*` family: bare, each rendered as Times.
    for (family, stack) in [
        ("ui-monospace", "ui-monospace,monospace"),
        ("ui-serif", "ui-serif,serif"),
        ("ui-sans-serif", "ui-sans-serif,system-ui,sans-serif"),
        ("ui-rounded", "ui-rounded,system-ui,sans-serif"),
        ("system-ui", "system-ui"),
    ] {
        let families = vec![String::new(), String::new(), family.into()];
        let (css, _) = css_text(&s, &families);
        assert!(css.contains(&format!("font-family:{stack};")), "{css}");
    }
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn the_transition_row_lowers_to_css_transition_and_springs_are_named() {
    use exact_motion::{
        Easing, LinearStop, SpringConfig, StepPosition, TimingFunction, Transition,
        TransitionProperty, Transitions,
    };
    let rows = Transitions(vec![
        Transition {
            property: TransitionProperty::Property(exact_motion::Property::Opacity),
            duration: 0.25,
            delay: 0.0,
            timing: TimingFunction::Easing(Easing::EaseInOut),
        },
        Transition::new(
            TransitionProperty::All,
            0.5,
            TimingFunction::Easing(Easing::CubicBezier {
                x1: 0.4,
                y1: 0.0,
                x2: 0.2,
                y2: 1.0,
            }),
        ),
        Transition::new(
            TransitionProperty::Property(exact_motion::Property::Translate),
            0.0,
            TimingFunction::Spring(SpringConfig::default()),
        ),
    ]);
    let (css, spring) = transition_css(&rows);
    assert_eq!(
        css,
        "opacity 0.25s ease-in-out 0s,all 0.5s cubic-bezier(0.4,0,0.2,1) 0s"
    );
    assert!(spring, "the spring is reported, not silently dropped");
    assert_eq!(
        easing_css(&Easing::Steps {
            count: 4,
            position: StepPosition::JumpBoth
        }),
        "steps(4,jump-both)"
    );
    assert_eq!(
        easing_css(&Easing::PiecewiseLinear(vec![
            LinearStop {
                input: 0.0,
                output: 0.0
            },
            LinearStop {
                input: 0.5,
                output: 0.9
            },
            LinearStop {
                input: 1.0,
                output: 1.0
            },
        ])),
        "linear(0 0%,0.9 50%,1 100%)"
    );
    // This row is behind DerefMut; it cannot be a StyleProps initializer.
    let mut s = exact_kernel::StyleProps::default();
    s.transition = rows;
    s.mask.set(exact_kernel::StyleId::Transition);
    let (css, skipped) = css_text(&s, &[]);
    assert!(css.starts_with("transition:opacity 0.25s"));
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].reason.contains("spring"));
}

#[test]
fn dynamic_unavailable_colors_take_the_initial_value() {
    struct NoData;
    impl exact_runner::DataSource for NoData {
        fn query(
            &mut self,
            _: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            unreachable!()
        }
    }
    let plan = contract::compile(
        r#"component A
  state ink = "red"
  action hdr
    ink = "color(rec2100-linear 4 4 4)"
  action profile
    ink = "color(--dci-p3 1 0 0)"
  view
    column
      box testId="paint" background-color=ink
      button testId="hdr" press=hdr
        text "HDR"
      button testId="profile" press=profile
        text "Profile"
"#,
    )
    .unwrap();
    let (mut host, _) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    for target in ["hdr", "profile"] {
        let id = view_with_test_id_any(&host, target);
        let batch = host.dispatch(id, Event::Press);
        let paint = view_with_test_id_any(&host, "paint");
        assert_eq!(
            host.runner()
                .kernel()
                .node(paint)
                .unwrap()
                .style
                .background_color,
            exact_kernel::StyleProps::default().background_color,
            "{batch}"
        );
        assert!(
            !batch.contains("rec2100") && !batch.contains("--dci-p3"),
            "{batch}"
        );
    }
}

/// LLP 1069.000 D3 (#136): the live page writes the pixels the kernel
/// resolves a `rem`/`em` row to, and re-resolves them; a page that keeps no
/// kernel (the JS target's stylesheet) writes the units, for the browser.
#[test]
fn relative_lengths_are_pixels_live_and_units_ahead_of_time() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut s = StyleProps::default();
    for (id, value) in [
        (StyleId::Width, "10rem"),
        (StyleId::Height, "48px"),
        (StyleId::MarginLeft, "-0.5em"),
        (StyleId::FontSize, "1.5em"),
        (StyleId::LineHeight, "1.25em"),
        (StyleId::LetterSpacing, "0.1em"),
        (StyleId::BorderRadiusTopLeft, "0.5rem"),
        (StyleId::BorderRadiusBottomLeft, "0.5rem"),
        (StyleId::CornerShape, "-exact-continuous round round round"),
    ] {
        s.set_dynamic(id, &StyleValue::Text(value.into())).unwrap();
    }
    let (live, skipped) = css_text(&s, &[]);
    assert!(skipped.is_empty(), "{skipped:?}");
    let (ahead, skipped) = exact_web::css::css_text_relative(&s, &[]);
    assert!(skipped.is_empty(), "{skipped:?}");
    let k = exact_num::Shortest32(exact_kernel::corner::APPLE_ON_THE_WEB.1);
    for (live_decl, ahead_decl) in [
        ("width:160px;".to_string(), "width:10rem;".to_string()),
        ("height:48px;".into(), "height:48px;".into()),
        ("margin-left:-8px;".into(), "margin-left:-0.5em;".into()),
        ("font-size:24px;".into(), "font-size:1.5em;".into()),
        ("line-height:20px;".into(), "line-height:1.25em;".into()),
        (
            "letter-spacing:1.6px;".into(),
            "letter-spacing:0.1em;".into(),
        ),
        (
            format!("border-top-left-radius:calc(8px * {k});"),
            format!("border-top-left-radius:calc(0.5rem * {k});"),
        ),
        (
            "border-bottom-left-radius:8px;".into(),
            "border-bottom-left-radius:0.5rem;".into(),
        ),
    ] {
        assert!(live.contains(&live_decl), "{live_decl} in {live}");
        assert!(ahead.contains(&ahead_decl), "{ahead_decl} in {ahead}");
    }
}

/// LLP 1115 D3: a text style is the ramp's pixels live, and the ramp's size
/// at CSS's `medium` in `rem` ahead of time, so the browser's root scales it.
#[test]
fn a_text_style_is_pixels_live_and_rem_ahead_of_time() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut s = StyleProps::default();
    s.set_dynamic(StyleId::FontSize, &StyleValue::Text("-exact-title1".into()))
        .unwrap();
    assert!(css_text(&s, &[]).0.contains("font-size:27px;"));
    let ahead = exact_web::css::css_text_relative(&s, &[]).0;
    assert!(ahead.contains("font-size:1.6875rem;"), "{ahead}");
}

/// LLP 1001 §2 (2026-10-07): a `min()`, `max()` or `clamp()` length lowers to
/// CSS's own function, which the browser resolves with the insets.
#[test]
fn comparison_lengths_lower_to_css_comparisons() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut s = StyleProps::default();
    for (id, value) in [
        (
            StyleId::PaddingBottom,
            "clamp(15px, env(safe-area-inset-bottom), 60px)",
        ),
        (
            StyleId::Bottom,
            "calc(clamp(15px, env(safe-area-inset-bottom), 60px) + 15px + 44px)",
        ),
        (
            StyleId::PaddingTop,
            "max(15px,env(safe-area-inset-top) - 4px)",
        ),
        (StyleId::Width, "min(50vw, 300px)"),
        (StyleId::Height, "max(10px, 2px)"),
    ] {
        s.set_dynamic(id, &StyleValue::Text(value.into())).unwrap();
    }
    let (css, skipped) = css_text(&s, &[]);
    assert!(skipped.is_empty(), "{skipped:?}");
    for expected in [
        "padding-bottom:clamp(15px, env(safe-area-inset-bottom), 60px);",
        "bottom:calc(59px + clamp(15px, env(safe-area-inset-bottom), 60px));",
        "padding-top:max(15px, -4px + env(safe-area-inset-top));",
        "width:min(50vw, 300px);",
        "height:10px;",
    ] {
        assert!(css.contains(expected), "{expected} in {css}");
    }
}

#[test]
fn a_negative_calculation_on_a_size_reaches_the_browser_clamped() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    // `width: -10px` is invalid CSS (the kernel refuses it); `calc(10px -
    // 20px)` is valid and clamps to 0 at computed-value time (CSS Values 4
    // §9.1), so the browser gets 0, while a margin keeps the negative length.
    let mut s = StyleProps::default();
    let text = || StyleValue::Text("calc(10px - 20px)".into());
    s.set_dynamic(StyleId::Width, &text()).unwrap();
    s.set_dynamic(StyleId::MarginLeft, &text()).unwrap();
    let (css, _) = css_text(&s, &[]);
    assert!(css.contains("width:0px;"), "{css}");
    assert!(css.contains("margin-left:-10px;"), "{css}");
    let mut s = StyleProps::default();
    s.set_dynamic(
        StyleId::Width,
        &StyleValue::Text("calc(10vw - 20vw)".into()),
    )
    .unwrap();
    let (css, _) = css_text(&s, &[]);
    assert!(css.contains("width:0vw;"), "{css}");
}
