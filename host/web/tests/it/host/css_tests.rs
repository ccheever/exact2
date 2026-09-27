use super::*;

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
    let (css, skipped) = css_text(&s, &[]);
    for expected in [
        "scroll-behavior:smooth;",
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
    let mut s = exact_kernel::StyleProps {
        transition: rows,
        ..Default::default()
    };
    s.mask.set(exact_kernel::StyleId::Transition);
    let (css, skipped) = css_text(&s, &[]);
    assert!(css.starts_with("transition:opacity 0.25s"));
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].reason.contains("spring"));
}
