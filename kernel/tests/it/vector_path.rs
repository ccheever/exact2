//! LLP 1065: the `path` node — its rows through both ingress paths, and its
//! stroke fractions through the motion seam, transitioned and keyframed
//! under the seekable clock like the compositor rows.

use exact_kernel::wire::codec::{Reader, Writer};
use exact_kernel::{
    motion_node, ColorValue, Kernel, NodeType, Op, PropId, PropValue, StyleId, StyleProps,
    StyleValue,
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
        Some(ColorValue::Fixed(exact_kernel::Color(0xff)))
    );
    assert_eq!(defaults.stroke, None);
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
    ]);
    assert_eq!(
        style.fill, None,
        "`none` is the keyword, not transparent paint"
    );
    assert!(matches!(style.stroke, Some(ColorValue::LightDark(..))));
    let mut bytes = Writer::new();
    style.encode_patch(&mut bytes);
    assert_eq!(
        StyleProps::decode_patch(&mut Reader::new(bytes.as_slice())).unwrap(),
        style
    );
    for (row, bad) in [
        (StyleId::StrokeLinecap, "rounded"),
        (StyleId::Fill, "currentcolor"),
        (StyleId::Stroke, "nothing"),
    ] {
        assert!(
            StyleProps::default().set_dynamic(row, &text(bad)).is_err(),
            "{bad}"
        );
    }
    // Paint rows inherit, as SVG's do; the stroke's fractions do not.
    for row in [StyleId::Fill, StyleId::Stroke, StyleId::StrokeWidth] {
        assert!(row.inherited(), "{row:?}");
    }
    assert!(!StyleId::StrokeEnd.inherited());
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
