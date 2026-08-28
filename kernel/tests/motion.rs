//! The kernel→motion seam, end to end: a `transition` row and a target land
//! through the one write path (bytes or structured), the receipt restates them
//! for the engine, and the engine — under a seekable clock — presents what a
//! browser would for the same CSS.

use exact_kernel::{
    motion_node, wire, ApplyError, DecodeError, Kernel, KernelError, NodeType, Op, StyleId,
    StyleProps, Transitions, Vec2,
};
use exact_motion::{
    Easing, Engine, LinearStop, Property, SpringConfig, StepPosition, TimingFunction, Transition,
    TransitionError, TransitionProperty, Value,
};

fn style(f: impl FnOnce(&mut StyleProps)) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    f(&mut s);
    Box::new(s)
}

fn transition(t: Transition) -> Box<StyleProps> {
    style(|s| {
        s.transition = Transitions(vec![t]);
        s.mask.set(StyleId::Transition);
    })
}

fn opacity(v: f32) -> Box<StyleProps> {
    style(|s| {
        s.opacity = v;
        s.mask.set(StyleId::Opacity);
    })
}

fn linear_all(duration: f64) -> Transition {
    Transition::new(
        TransitionProperty::All,
        duration,
        TimingFunction::Easing(Easing::Linear),
    )
}

/// One root view with a `transition` row, synced into a fresh engine.
fn boot(t: Transition) -> (Kernel, Engine, u64) {
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
                Op::SetStyle {
                    id: 1,
                    patch: transition(t),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let mut engine = Engine::new();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    let node = motion_node(kernel.node(1).unwrap().key);
    (kernel, engine, node)
}

#[test]
fn a_created_node_takes_its_style_without_a_transition() {
    let (_, mut engine, node) = boot(linear_all(1.0));
    assert!(engine.quiescent());
    let painted = engine.frame();
    assert_eq!(
        painted.len(),
        4,
        "every animatable property is presented once"
    );
    assert_eq!(
        engine.value(node, Property::Opacity),
        Some(Value::scalar(1.0))
    );
    assert_eq!(
        engine.value(node, Property::Scale),
        Some(Value::scalar(1.0))
    );
    assert_eq!(engine.value(node, Property::Translate), Some(Value::ZERO));
}

#[test]
fn a_style_change_transitions_under_the_row_and_the_clock_seeks() {
    let (mut kernel, mut engine, node) = boot(linear_all(1.0));
    engine.frame();
    let receipt = kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: opacity(0.0),
            }],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(!engine.quiescent());
    assert_eq!(engine.settle_time(), Some(1.0));

    engine.advance(0.25).unwrap();
    let painted = engine.frame();
    assert_eq!(painted.len(), 1, "only the moving row is repainted");
    assert_eq!(painted[0].property, Property::Opacity);
    assert!((painted[0].value.x - 0.75).abs() < 1e-9);

    // An agent's `clock` op: advance to settle and read, never wait.
    engine.advance(engine.settle_time().unwrap()).unwrap();
    assert!(engine.quiescent());
    assert_eq!(
        engine.value(node, Property::Opacity),
        Some(Value::scalar(0.0))
    );
    // The kernel's target never moved; only the presentation did.
    assert_eq!(kernel.node(1).unwrap().style.opacity, 0.0);
}

#[test]
fn a_node_without_a_row_changes_immediately() {
    let mut kernel = Kernel::with_monospace();
    let mut engine = Engine::new();
    let receipt = kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    let receipt = kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: opacity(0.5),
            }],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(engine.quiescent());
    let node = motion_node(kernel.node(1).unwrap().key);
    assert_eq!(
        engine.value(node, Property::Opacity),
        Some(Value::scalar(0.5))
    );
}

#[test]
fn a_destroyed_node_is_forgotten_and_its_slot_never_inherits() {
    let (mut kernel, mut engine, node) = boot(linear_all(1.0));
    let receipt = kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: opacity(0.0),
            }],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(!engine.quiescent());
    let receipt = kernel.apply(0, 3, &[Op::DestroyView { id: 1 }]).unwrap();
    let sync = kernel.motion_sync(&receipt);
    assert_eq!(sync.removed, vec![node]);
    sync.apply(&mut engine).unwrap();
    assert!(engine.quiescent());
    assert_eq!(engine.value(node, Property::Opacity), None);

    let receipt = kernel
        .apply(
            0,
            4,
            &[
                Op::CreateView {
                    id: 9,
                    node_type: NodeType::View,
                },
                Op::AttachRoot { id: 9 },
            ],
        )
        .unwrap();
    let reused = motion_node(kernel.node(9).unwrap().key);
    assert_ne!(reused, node, "a reused slot carries a new generation");
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(engine.quiescent());
}

#[test]
fn the_transition_row_round_trips_through_exwf_bytes() {
    let rows = Transitions(vec![
        Transition {
            property: TransitionProperty::Property(Property::Translate),
            duration: 0.0,
            delay: 0.0,
            timing: TimingFunction::Spring(SpringConfig {
                stiffness: 180.0,
                damping: 12.0,
                mass: 1.0,
            }),
        },
        Transition {
            property: TransitionProperty::Property(Property::Opacity),
            // Times and control points ride the wire as f32 like every other
            // style row, so the fixture uses f32-representable numbers.
            duration: 0.25,
            delay: -0.125,
            timing: TimingFunction::Easing(Easing::CubicBezier {
                x1: 0.375,
                y1: 0.0,
                x2: 0.25,
                y2: 1.0,
            }),
        },
        Transition::new(
            TransitionProperty::All,
            0.5,
            TimingFunction::Easing(Easing::Steps {
                count: 4,
                position: StepPosition::JumpBoth,
            }),
        ),
        Transition::new(
            TransitionProperty::Property(Property::Rotate),
            1.0,
            TimingFunction::Easing(Easing::PiecewiseLinear(vec![
                LinearStop {
                    input: 0.0,
                    output: 0.0,
                },
                LinearStop {
                    input: 0.5,
                    output: 0.75,
                },
                LinearStop {
                    input: 1.0,
                    output: 1.0,
                },
            ])),
        ),
    ]);
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: style(|s| {
                s.transition = rows.clone();
                s.mask.set(StyleId::Transition);
                s.translate = Vec2 { x: 10.0, y: -4.0 };
                s.mask.set(StyleId::Translate);
                s.scale = 1.5;
                s.mask.set(StyleId::Scale);
                s.rotate = 45.0;
                s.mask.set(StyleId::Rotate);
            }),
        },
        Op::AttachRoot { id: 1 },
    ];
    let bytes = wire::encode(0, 1, &ops);
    let mut kernel = Kernel::with_monospace();
    kernel.apply_frame(&bytes).unwrap();
    let node = kernel.node(1).unwrap();
    assert_eq!(node.style.transition, rows);
    assert_eq!(node.style.translate, Vec2 { x: 10.0, y: -4.0 });
    assert_eq!(node.style.scale, 1.5);
    assert_eq!(node.style.rotate, 45.0);
}

#[test]
fn an_invalid_row_is_refused_on_both_ingress_paths_and_applies_nothing() {
    let bad = Transition::new(
        TransitionProperty::All,
        0.3,
        TimingFunction::Spring(SpringConfig::default()),
    );
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: transition(bad),
        },
    ];
    let mut kernel = Kernel::with_monospace();
    let before = kernel.export(None).unwrap();

    let err = kernel.apply(0, 1, &ops).unwrap_err();
    assert!(matches!(
        err,
        KernelError::Apply(ApplyError::InvalidTransition {
            op_index: 1,
            error: TransitionError::SpringDeclaresDuration
        })
    ));
    assert_eq!(
        kernel.export(None).unwrap(),
        before,
        "structured apply changed nothing"
    );

    let bytes = wire::encode(0, 1, &ops);
    let err = kernel.apply_frame(&bytes).unwrap_err();
    assert!(matches!(
        err,
        KernelError::Decode(DecodeError::InvalidTransition(
            TransitionError::SpringDeclaresDuration
        ))
    ));
    assert_eq!(
        kernel.export(None).unwrap(),
        before,
        "the frame changed nothing"
    );
    assert_eq!(kernel.live_count(), 0);
}

#[test]
fn clearing_the_row_makes_later_changes_immediate() {
    let (mut kernel, mut engine, node) = boot(linear_all(1.0));
    let mut mask = exact_kernel::StyleMask::EMPTY;
    mask.set(StyleId::Transition);
    let receipt = kernel
        .apply(0, 2, &[Op::ClearStyle { id: 1, mask }])
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert_eq!(kernel.node(1).unwrap().style.transition, Transitions::NONE);
    let receipt = kernel
        .apply(
            0,
            3,
            &[Op::SetStyle {
                id: 1,
                patch: opacity(0.0),
            }],
        )
        .unwrap();
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(engine.quiescent());
    assert_eq!(
        engine.value(node, Property::Opacity),
        Some(Value::scalar(0.0))
    );
}
