//! LLP 1065: the `path` node — its rows through both ingress paths, and its
//! stroke fractions through the motion seam, transitioned and keyframed
//! under the seekable clock like the compositor rows.

use exact_kernel::wire::codec::{Reader, Writer};
use exact_kernel::{
    motion_node, ColorValue, FillRule, Kernel, NodeType, Op, Paint, PropId, PropValue, StyleId,
    StyleProps, StyleValue, VectorEffect,
};
use exact_motion::{Animations, Engine, Property, Transitions, Value};

fn path_kernel(style: StyleProps) -> (Kernel, Engine, u64) {
    let mut kernel = Kernel::with_monospace();
    let receipt = kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::Path,
                },
                Op::SetProp {
                    id: 2,
                    prop: PropId::PathData,
                    value: PropValue::Str("M0 0 H10 M0 5 H30".into()),
                },
                Op::SetStyle {
                    id: 2,
                    patch: Box::new(style),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let mut engine = Engine::new();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    let node = motion_node(kernel.node(2).unwrap().key);
    (kernel, engine, node)
}

fn rows(pairs: &[(StyleId, StyleValue)]) -> StyleProps {
    let mut style = StyleProps::default();
    for (row, value) in pairs {
        style.set_dynamic(*row, value).unwrap();
    }
    style
}

fn text(s: &str) -> StyleValue {
    StyleValue::Text(s.into())
}

#[test]
fn painting_rows_follow_svg_and_cross_the_wire() {
    let defaults = StyleProps::default();
    // SVG's initial values: fill black, stroke none, width 1, the whole stroke.
    assert_eq!(
        defaults.fill,
        Paint::Color(ColorValue::Fixed(exact_kernel::Color(0xff)))
    );
    assert_eq!(defaults.stroke, Paint::None);
    assert_eq!(
        (defaults.fill_rule, defaults.stroke_miterlimit),
        (FillRule::Nonzero, 4.0)
    );
    assert_eq!(defaults.vector_effect, VectorEffect::None);
    assert!(!defaults.stroke_dasharray.dashes());
    assert_eq!(
        (
            defaults.stroke_width,
            defaults.stroke_start,
            defaults.stroke_end
        ),
        (1.0, 0.0, 1.0)
    );
    let style = rows(&[
        (StyleId::Fill, text("none")),
        (StyleId::Stroke, text("light-dark(#111111, #eeeeee)")),
        (StyleId::StrokeWidth, StyleValue::Number(110.0)),
        (StyleId::StrokeLinecap, text("round")),
        (StyleId::StrokeLinejoin, text("round")),
        (StyleId::StrokeStart, StyleValue::Number(0.25)),
        (StyleId::StrokeEnd, StyleValue::Number(0.5)),
        (StyleId::FillRule, text("evenodd")),
        (StyleId::StrokeMiterlimit, StyleValue::Number(10.0)),
        (StyleId::StrokeDasharray, text("4, 2")),
        (StyleId::StrokeDashoffset, StyleValue::Number(3.0)),
        (StyleId::VectorEffect, text("non-scaling-stroke")),
    ]);
    assert_eq!(
        style.fill,
        Paint::None,
        "`none` is the keyword, not transparent paint"
    );
    assert!(matches!(
        style.stroke,
        Paint::Color(ColorValue::LightDark(..))
    ));
    assert_eq!(style.stroke_dasharray.0, [4.0, 2.0]);
    assert_eq!(
        rows(&[(StyleId::Fill, text("currentColor"))]).fill,
        Paint::CurrentColor
    );
    let mut bytes = Writer::new();
    style.encode_patch(&mut bytes);
    assert_eq!(
        StyleProps::decode_patch(&mut Reader::new(bytes.as_slice())).unwrap(),
        style
    );
    for (row, bad) in [
        (StyleId::StrokeLinecap, "rounded"),
        (StyleId::Stroke, "nothing"),
        (StyleId::StrokeDasharray, "1 -2"),
        (StyleId::FillRule, "odd"),
        (StyleId::VectorEffect, "non-scaling-size"),
    ] {
        assert!(
            StyleProps::default().set_dynamic(row, &text(bad)).is_err(),
            "{bad}"
        );
    }
    // Paint rows inherit, as SVG's do; the stroke's fractions do not.
    for row in [
        StyleId::Fill,
        StyleId::Stroke,
        StyleId::StrokeWidth,
        StyleId::FillRule,
        StyleId::StrokeMiterlimit,
        StyleId::StrokeDasharray,
        StyleId::StrokeDashoffset,
    ] {
        assert!(row.inherited(), "{row:?}");
    }
    assert!(!StyleId::StrokeEnd.inherited());
    assert!(!StyleId::VectorEffect.inherited());
    assert!(!NodeType::Path.can_hold_children());
}

#[test]
fn only_a_path_hands_the_engine_its_stroke() {
    let (kernel, engine, node) = path_kernel(StyleProps::default());
    assert_eq!(
        engine.value(node, Property::StrokeEnd),
        Some(Value::scalar(1.0))
    );
    assert_eq!(
        engine.value(node, Property::StrokeStart),
        Some(Value::scalar(0.0))
    );
    let view = motion_node(kernel.node(1).unwrap().key);
    assert_eq!(engine.value(view, Property::StrokeEnd), None);
    assert_eq!(
        engine.value(view, Property::Opacity),
        Some(Value::scalar(1.0))
    );
}

#[test]
fn stroke_end_transitions_under_the_row_and_the_clock_seeks() {
    let (mut kernel, mut engine, node) = path_kernel(rows(&[
        (StyleId::StrokeEnd, StyleValue::Number(0.0)),
        (StyleId::Transition, text("stroke-end 1s linear")),
    ]));
    // First seen takes its value: nothing draws until the target moves.
    assert_eq!(
        engine.value(node, Property::StrokeEnd),
        Some(Value::scalar(0.0))
    );
    engine.frame();
    let receipt = kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(rows(&[(StyleId::StrokeEnd, StyleValue::Number(1.0))])),
            }],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert_eq!(engine.settle_time(), Some(1.0));
    engine.advance(0.25).unwrap();
    let painted = engine.frame();
    assert_eq!(painted.len(), 1);
    assert_eq!(painted[0].property, Property::StrokeEnd);
    assert!((painted[0].value.x - 0.25).abs() < 1e-9);
    engine.advance(1.0).unwrap();
    assert_eq!(
        engine.value(node, Property::StrokeEnd),
        Some(Value::scalar(1.0))
    );
}

#[test]
fn stroke_end_plays_in_keyframes_from_creation() {
    // The compiler's text form: the shorthand and its rule (LLP 1057 D3), the
    // rule naming the property the browser animates.
    let row = "draw 2s linear 0s 1 normal both running @keyframes draw{0%{--exact-stroke-end:0;}100%{--exact-stroke-end:1;}}";
    let animations = Animations::parse(row).unwrap();
    assert!(animations.0[0].keyframes.affects(Property::StrokeEnd));
    assert!(animations.0[0]
        .keyframes
        .rule("d")
        .contains("--exact-stroke-end:0"));
    let (_, mut engine, node) = path_kernel(rows(&[(StyleId::Animation, text(row))]));
    engine.advance(0.5).unwrap();
    assert!((engine.value(node, Property::StrokeEnd).unwrap().x - 0.25).abs() < 1e-9);
    engine.advance(3.0).unwrap();
    // `both`: the last keyframe holds.
    assert_eq!(
        engine.value(node, Property::StrokeEnd),
        Some(Value::scalar(1.0))
    );
    // The transition vocabulary names the stroke rows too, on the wire after `layout`.
    let t = Transitions::parse("stroke-start 1s, stroke-end 2s").unwrap();
    assert_eq!(t.0.len(), 2);
    assert_eq!(Property::StrokeStart as u8, 15);
    assert_eq!(
        Property::from_name("--exact-stroke-end"),
        Some(Property::StrokeEnd)
    );
}

/// A spring drives a stroke fraction as its curve from rest, as it drives
/// paint (LLP 1062 D3, LLP 1065): the value moves rather than jumps.
#[test]
fn a_spring_drives_the_stroke_fractions() {
    let (mut kernel, mut engine, node) = path_kernel(rows(&[
        (StyleId::StrokeEnd, StyleValue::Number(0.0)),
        (StyleId::Transition, text("stroke-end spring(170, 26, 1)")),
    ]));
    engine.frame();
    let receipt = kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(rows(&[(StyleId::StrokeEnd, StyleValue::Number(1.0))])),
            }],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    engine.advance(0.1).unwrap();
    let x = engine.value(node, Property::StrokeEnd).unwrap().x;
    assert!(x > 0.0 && x < 1.0, "{x}");
}

/// `fill` and `stroke` are paint motion's (LLP 1062) when a path names them:
/// computed colours, `light-dark()` resolved; `none` and `currentcolor` are
/// no target, as SVG's `<paint>` interpolates only colour to colour and a
/// `currentcolor` paint's computed value is the keyword.
#[test]
fn fill_and_stroke_are_paint_targets_unless_none() {
    use exact_kernel::motion::PaintOwners;
    let (mut kernel, _, node) = path_kernel(rows(&[
        (StyleId::TextColor, text("#0000ff")),
        (StyleId::Fill, text("#0000ff")),
        (StyleId::Stroke, text("light-dark(#ff0000, #00ff00)")),
        (StyleId::Transition, text("fill 1s, stroke 1s")),
    ]));
    let key = kernel.node(2).unwrap().key;
    let targets = |k: &Kernel, dark| k.paint_targets(key, dark);
    assert_eq!(
        targets(&kernel, false),
        [
            (Property::Fill, Value::rgba(0.0, 0.0, 1.0, 1.0)),
            (Property::Stroke, Value::rgba(1.0, 0.0, 0.0, 1.0)),
        ]
    );
    assert_eq!(targets(&kernel, true)[1].1, Value::rgba(0.0, 1.0, 0.0, 1.0));
    let mut owners = PaintOwners::default();
    kernel.paint_adopt([key], false, &mut owners);
    assert!(owners.owns(node, Property::Fill) && owners.owns(node, Property::Stroke));
    // To `none` or `currentcolor`: owning ends (the change is discrete).
    let receipt = kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(rows(&[(StyleId::Fill, text("none"))])),
            }],
        )
        .unwrap();
    let sync = kernel.paint_sync(&receipt, false, &mut owners);
    assert_eq!(sync.retired, [(node, Property::Fill)]);
    assert!(!owners.owns(node, Property::Fill) && owners.owns(node, Property::Stroke));
    let receipt = kernel
        .apply(
            0,
            3,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(rows(&[(StyleId::Stroke, text("currentColor"))])),
            }],
        )
        .unwrap();
    let sync = kernel.paint_sync(&receipt, false, &mut owners);
    assert_eq!(sync.retired, [(node, Property::Stroke)]);
    // `transition: all` covers them; the wire carries them after the strokes.
    let t = Transitions::parse("fill 1s, stroke 2s").unwrap();
    let mut w = Writer::new();
    w.transitions(&t);
    assert_eq!(Reader::new(w.as_slice()).transitions().unwrap(), t);
    assert_eq!(w.as_slice()[1], Property::Fill as u8 + 1);
}
