//! Drag timelines through the kernel (LLP 1057.003 D1, D2): the three rows
//! parse as CSS text, refuse what is not their grammar, round-trip through
//! EXWF, and reach the engine with the node's other motion rows, so a
//! consumer follows its source's hold in the frame it moves, is handed back
//! to the clock when its row is cleared, and holds its start once the source
//! is destroyed.

use exact_kernel::timeline::{AnimationRange, AnimationTimeline, Axis, DragTimeline};
use exact_kernel::{
    motion_node, wire, CommitReceipt, Kernel, NodeType, Op, StyleId, StyleProps, StyleValue,
    StyleValueError,
};
use exact_motion::{Engine, Property, Value};

const SOURCE: u32 = 2;
const CONSUMER: u32 = 3;

fn rows(pairs: &[(StyleId, &str)]) -> Box<StyleProps> {
    exact_kernel::timeline::link();
    let mut s = StyleProps::default();
    for (id, css) in pairs {
        s.set_dynamic(*id, &StyleValue::Text((*css).into()))
            .unwrap();
    }
    Box::new(s)
}

fn fade() -> Box<StyleProps> {
    let mut s = StyleProps::default();
    s.animation =
        crate::keyframed("fade 1s linear both @keyframes fade{from{opacity:1}to{opacity:0}}");
    s.mask.set(StyleId::Animation);
    Box::new(s)
}

fn tree(k: &mut Kernel) -> CommitReceipt {
    let create = |id| Op::CreateView {
        id,
        node_type: NodeType::View,
    };
    k.apply(
        0,
        1,
        &[
            create(1),
            create(SOURCE),
            create(CONSUMER),
            Op::SetStyle {
                id: SOURCE,
                patch: rows(&[(StyleId::DragTimeline, "--dismiss y")]),
            },
            Op::SetStyle {
                id: CONSUMER,
                patch: fade(),
            },
            Op::SetStyle {
                id: CONSUMER,
                patch: rows(&[
                    (StyleId::AnimationTimeline, "--dismiss"),
                    (StyleId::AnimationRange, "0px 300px"),
                ]),
            },
            Op::SetChildren {
                id: 1,
                children: vec![CONSUMER, SOURCE],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap()
}

fn opacity(e: &mut Engine, node: u64) -> Option<f64> {
    e.frame()
        .iter()
        .find(|p| p.node == node && p.property == Property::Opacity)
        .map(|p| p.value.x)
}

#[test]
fn the_rows_are_css_text_and_refuse_what_is_not_their_grammar() {
    exact_kernel::timeline::link();
    let mut s = StyleProps::default();
    for (id, css, canonical) in [
        (StyleId::DragTimeline, "--dismiss", "--dismiss y"),
        (StyleId::DragTimeline, "--pan inline", "--pan x"),
        (StyleId::DragTimeline, "none", "none"),
        (StyleId::AnimationTimeline, "--dismiss", "--dismiss"),
        (StyleId::AnimationTimeline, "auto", "auto"),
        (StyleId::AnimationRange, "-300 300px", "-300px 300px"),
        (StyleId::AnimationRange, "normal", "normal"),
    ] {
        s.set_dynamic(id, &StyleValue::Text(css.into())).unwrap();
        let written = match id {
            StyleId::DragTimeline => s.drag_timeline.css(),
            StyleId::AnimationTimeline => s.animation_timeline.css(),
            _ => s.animation_range.css(),
        };
        assert_eq!(written, canonical, "{css}");
        assert!(s.mask.has(id));
    }
    for (id, css) in [
        (StyleId::DragTimeline, "dismiss"),
        (StyleId::DragTimeline, "--dismiss z"),
        (StyleId::DragTimeline, "--dismiss y x"),
        (StyleId::AnimationTimeline, "scroll()"),
        (StyleId::AnimationRange, "0px"),
        (StyleId::AnimationRange, "10px 10px"),
        (StyleId::AnimationRange, "0% 100%"),
    ] {
        let refused = s.set_dynamic(id, &StyleValue::Text(css.into()));
        assert!(
            matches!(
                refused,
                Err(StyleValueError::BadDragTimeline { .. }
                    | StyleValueError::BadAnimationTimeline { .. }
                    | StyleValueError::BadAnimationRange { .. })
            ),
            "{css}: {refused:?}"
        );
    }
}

#[test]
fn the_rows_round_trip_through_the_wire() {
    let mut k = Kernel::with_monospace();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: rows(&[
                (StyleId::DragTimeline, "--pan x"),
                (StyleId::AnimationTimeline, "--dismiss"),
                (StyleId::AnimationRange, "-40px 260.5px"),
            ]),
        },
        Op::AttachRoot { id: 1 },
    ];
    k.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
    let style = k.node(1).unwrap().style;
    assert_eq!(
        style.drag_timeline,
        DragTimeline {
            name: Some("--pan".into()),
            axis: Axis::X
        }
    );
    assert_eq!(
        style.animation_timeline,
        AnimationTimeline(Some("--dismiss".into()))
    );
    assert_eq!(style.animation_range, AnimationRange(Some([-40.0, 260.5])));
}

#[test]
fn a_consumer_follows_its_source_hold_until_its_row_is_cleared() {
    let mut k = Kernel::with_monospace();
    let receipt = tree(&mut k);
    let source = motion_node(k.node(SOURCE).unwrap().key);
    let consumer = motion_node(k.node(CONSUMER).unwrap().key);
    let mut e = Engine::new();
    k.motion_sync(&receipt).apply(&mut e).unwrap();
    assert!(e.timeline_bound(consumer));
    assert!(e.quiescent(), "a bound animation keeps no clock busy");
    // Held at the drag, in the frame that presents it.
    let hold = e
        .begin_hold(source, Property::Translate, 0.0, None)
        .unwrap()
        .unwrap();
    for (y, expected) in [(75.0, 0.75), (150.0, 0.5), (420.0, 0.0), (-30.0, 1.0)] {
        e.update_hold(hold.token, 0.1, Value::new(0.0, y)).unwrap();
        assert_eq!(opacity(&mut e, consumer), Some(expected), "at {y}");
    }
    e.update_hold(hold.token, 0.2, Value::new(0.0, 150.0))
        .unwrap();
    assert_eq!(opacity(&mut e, consumer), Some(0.5));
    // Clearing the row hands the animation back to the clock from there.
    let cleared = k
        .apply(
            1,
            1,
            &[Op::SetStyle {
                id: CONSUMER,
                patch: rows(&[(StyleId::AnimationTimeline, "auto")]),
            }],
        )
        .unwrap();
    k.motion_sync(&cleared).apply(&mut e).unwrap();
    assert!(!e.timeline_bound(consumer));
    e.advance(0.45).unwrap();
    let shown = opacity(&mut e, consumer).unwrap();
    assert!((shown - 0.25).abs() < 1e-9, "{shown}");
}

#[test]
fn a_consumer_whose_source_is_destroyed_holds_its_start() {
    let mut k = Kernel::with_monospace();
    let receipt = tree(&mut k);
    let source = motion_node(k.node(SOURCE).unwrap().key);
    let consumer = motion_node(k.node(CONSUMER).unwrap().key);
    let mut e = Engine::new();
    k.motion_sync(&receipt).apply(&mut e).unwrap();
    let hold = e
        .begin_hold(source, Property::Translate, 0.0, None)
        .unwrap()
        .unwrap();
    e.update_hold(hold.token, 0.1, Value::new(0.0, 240.0))
        .unwrap();
    assert!((opacity(&mut e, consumer).unwrap() - 0.2).abs() < 1e-9);
    let gone = k
        .apply(
            1,
            1,
            &[
                Op::SetChildren {
                    id: 1,
                    children: vec![CONSUMER],
                },
                Op::DestroyView { id: SOURCE },
            ],
        )
        .unwrap();
    k.motion_sync(&gone).apply(&mut e).unwrap();
    assert!(e.timeline_bound(consumer), "still bound, to no source");
    assert_eq!(opacity(&mut e, consumer), Some(1.0));
    e.advance(5.0).unwrap();
    assert_eq!(
        opacity(&mut e, consumer),
        None,
        "and the clock moves nothing"
    );
}
