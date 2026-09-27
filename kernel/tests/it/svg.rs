//! LLP 1055 D3: an `svg` is a 300×150 replaced box whose SVG children never
//! enter box layout; the content model is enforced; motion hears animations.
use exact_kernel::{
    ApplyError, Kernel, KernelError, NodeType, Offer, Op, PropId, PropValue, StyleId, StyleProps,
    StyleValue,
};
use exact_motion::{Engine, Keyframes, Property};

fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

fn tree() -> Vec<Op> {
    vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Svg,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::SvgPolyline,
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::SvgGroup,
        },
        Op::CreateView {
            id: 5,
            node_type: NodeType::SvgCircle,
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Points,
            value: PropValue::Str("0,0 96,32".into()),
        },
        style(5, &[(StyleId::R, StyleValue::Number(3.0))]),
        Op::SetChildren {
            id: 4,
            children: vec![5],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]
}

#[test]
fn an_svg_is_a_replaced_box_and_its_content_takes_no_layout() {
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &tree()).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    let svg = k.node(2).unwrap().frame;
    assert_eq!(
        (svg.width, svg.height),
        (300.0, 150.0),
        "a bare <svg> is 300×150"
    );
    for id in [3, 4, 5] {
        let f = k.node(id).unwrap().frame;
        assert_eq!(
            (f.width, f.height),
            (0.0, 0.0),
            "node {id} is never laid out"
        );
    }
    // Its rows size it; its content never does.
    k.apply(
        0,
        2,
        &[style(
            2,
            &[
                (StyleId::Width, StyleValue::Number(96.0)),
                (StyleId::Height, StyleValue::Number(32.0)),
            ],
        )],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    let svg = k.node(2).unwrap().frame;
    assert_eq!((svg.width, svg.height), (96.0, 32.0));
}

#[test]
fn the_content_model_is_enforced() {
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &tree()).unwrap();
    let refused = |k: &mut Kernel, ops: &[Op]| {
        matches!(
            k.apply(0, 9, ops),
            Err(KernelError::Apply(ApplyError::SvgContent { .. }))
        )
    };
    // A shape outside an svg.
    assert!(refused(
        &mut k,
        &[
            Op::CreateView {
                id: 9,
                node_type: NodeType::SvgRect
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 9]
            }
        ]
    ));
    // A box inside an svg, and a nested svg.
    assert!(refused(
        &mut k,
        &[
            Op::CreateView {
                id: 9,
                node_type: NodeType::View
            },
            Op::SetChildren {
                id: 2,
                children: vec![3, 9]
            }
        ]
    ));
    assert!(refused(
        &mut k,
        &[
            Op::CreateView {
                id: 9,
                node_type: NodeType::Svg
            },
            Op::SetChildren {
                id: 4,
                children: vec![5, 9]
            }
        ]
    ));
    // A shape holds nothing.
    assert!(k
        .apply(
            0,
            9,
            &[
                Op::CreateView {
                    id: 9,
                    node_type: NodeType::SvgCircle
                },
                Op::SetChildren {
                    id: 3,
                    children: vec![9]
                }
            ]
        )
        .is_err());
}

#[test]
fn motion_hears_the_animation_row_and_svg_targets() {
    let mut k = Kernel::with_monospace();
    let receipt = k.apply(0, 1, &tree()).unwrap();
    let mut engine = Engine::new();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let circle = k.node(5).unwrap().key;
    let id = exact_kernel::motion_node(circle);
    assert_eq!(engine.target(id, Property::R).map(|v| v.x), Some(3.0));
    // An animation row, resolved as the runner resolves it.
    let mut patch = StyleProps::default();
    patch
        .set_dynamic(
            StyleId::Animation,
            &StyleValue::Text("grow 1s linear infinite".into()),
        )
        .unwrap();
    let rule = Keyframes::parse("to{r:9}").unwrap();
    patch.animation.resolve(|_| Some(&rule));
    let receipt = k
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 5,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    engine.advance(10.0).unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    engine.advance(10.5).unwrap();
    let r = engine
        .frame()
        .into_iter()
        .find(|p| p.node == id && p.property == Property::R)
        .unwrap();
    assert_eq!(r.value.x, 6.0);
    // Round trip through the wire keeps the resolved keyframes.
    let node = k.node(5).unwrap();
    let mut w = exact_kernel::wire::codec::Writer::default();
    w.animations(&node.style.animation);
    let bytes = w.into_vec();
    let mut r = exact_kernel::wire::codec::Reader::new(&bytes);
    assert_eq!(r.animations().unwrap(), node.style.animation);
}

/// LLP 1055.000 D15: `display: none` on a node or an ancestor cancels its
/// animations; showing it again restarts them (CSS Animations 1 §3).
#[test]
fn display_none_cancels_and_restarts_animations() {
    let mut k = Kernel::with_monospace();
    let receipt = k.apply(0, 1, &tree()).unwrap();
    let mut engine = Engine::new();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let mut patch = StyleProps::default();
    patch
        .set_dynamic(
            StyleId::Animation,
            &StyleValue::Text("grow 1s linear infinite".into()),
        )
        .unwrap();
    let rule = Keyframes::parse("to{r:9}").unwrap();
    patch.animation.resolve(|_| Some(&rule));
    let receipt = k
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 5,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let circle = exact_kernel::motion_node(k.node(5).unwrap().key);
    assert_eq!(engine.animation_plays(circle).len(), 1);
    // The group above it is hidden: the circle's animation is cancelled.
    let hide = |d: &str| style(4, &[(StyleId::Display, StyleValue::Text(d.into()))]);
    let receipt = k.apply(0, 3, &[hide("none")]).unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(engine.animation_plays(circle).is_empty());
    // Shown again, it starts anew at the clock's now.
    engine.advance(2.0).unwrap();
    let receipt = k.apply(0, 4, &[hide("block")]).unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let plays = engine.animation_plays(circle);
    assert_eq!(plays.len(), 1);
    assert_eq!(plays[0].start, 2.0);
}
