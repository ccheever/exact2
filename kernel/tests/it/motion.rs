//! The kernel→motion seam, end to end: a `transition` row and a target land
//! through the one write path (bytes or structured), the receipt restates them
//! for the engine, and the engine — under a seekable clock — presents what a
//! browser would for the same CSS.

use exact_kernel::{
    motion_node, wire, ApplyError, DecodeError, Dimension, Display, Kernel, KernelError, NodeType,
    Op, PropId, PropValue, StyleId, StyleProps, Transitions, Vec2,
};
use exact_motion::{
    Easing, Engine, HoldEnd, LinearStop, Property, SpringConfig, StepPosition, TimingFunction,
    Transition, TransitionError, TransitionProperty, Value,
};

fn height_patch(value: Dimension) -> Box<StyleProps> {
    style(|s| {
        s.height = value;
        s.mask.set(StyleId::Height);
    })
}

fn height_tree() -> (Kernel, Engine, exact_kernel::NodeKey) {
    let mut k = Kernel::with_monospace();
    let receipt = k
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
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 4,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 3,
                    patch: height_patch(Dimension::Points(180.0)),
                },
                Op::SetStyle {
                    id: 4,
                    patch: height_patch(Dimension::Points(75.0)),
                },
                Op::SetStyle {
                    id: 3,
                    patch: transition(linear_all(1.0)),
                },
                Op::SetChildren {
                    id: 2,
                    children: vec![3],
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 4],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let owner = k.node(3).unwrap().key;
    let mut e = Engine::new();
    k.motion_sync(&receipt).apply(&mut e).unwrap();
    (k, e, owner)
}

#[test]
fn numeric_height_needs_explicit_owner_adoption_including_boot() {
    let (k, mut e, owner) = height_tree();
    let node = motion_node(owner);
    assert_eq!(e.value(node, Property::Height), None);
    assert_eq!(
        e.value(motion_node(k.node(4).unwrap().key), Property::Height),
        None
    );
    assert!(exact_kernel::motion::targets(k.node(3).unwrap().style)
        .into_iter()
        .all(|(p, _)| p != Property::Height));
    assert_eq!(k.height_target(owner), Some(Value::scalar(180.0)));
    let sync = k.height_motion_sync(owner);
    assert!(sync.removed.is_empty());
    assert!(sync.retired.is_empty());
    assert_eq!(sync.changes.len(), 1);
    sync.apply(&mut e).unwrap();
    assert_eq!(e.value(node, Property::Height), Some(Value::scalar(180.0)));
    assert!(!e.is_active(node, Property::Height));
    assert_eq!(
        e.value(motion_node(k.node(4).unwrap().key), Property::Height),
        None
    );
}

#[test]
fn registered_height_tracks_latest_target_and_declaration_through_real_commits() {
    let (mut k, mut e, owner) = height_tree();
    let node = motion_node(owner);
    k.height_motion_sync(owner).apply(&mut e).unwrap();
    let held = e
        .begin_hold(node, Property::Height, 0.0, Some(Value::scalar(120.0)))
        .unwrap()
        .unwrap();
    e.frame();
    let r = k
        .apply(
            0,
            2,
            &[
                Op::SetStyle {
                    id: 3,
                    patch: height_patch(Dimension::Points(400.0)),
                },
                Op::SetStyle {
                    id: 3,
                    patch: transition(linear_all(2.0)),
                },
            ],
        )
        .unwrap();
    k.motion_sync(&r).apply(&mut e).unwrap();
    k.height_motion_sync(owner).apply(&mut e).unwrap();
    assert_eq!(e.value(node, Property::Height), Some(Value::scalar(120.0)));
    assert_eq!(e.target(node, Property::Height), Some(Value::scalar(400.0)));
    assert!(e.frame().iter().all(|p| p.property != Property::Height));
    e.end_hold(held.token, 0.0, HoldEnd::Cancel).unwrap();
    e.advance(1.0).unwrap();
    assert_eq!(e.value(node, Property::Height), Some(Value::scalar(260.0)));
    e.advance(2.0).unwrap();
    assert_eq!(e.value(node, Property::Height), Some(Value::scalar(400.0)));
}

#[test]
fn registered_owner_reconciles_untouched_descendant_after_ancestor_hide_or_detach() {
    for hide in [false, true] {
        let (mut k, mut e, owner) = height_tree();
        let node = motion_node(owner);
        k.height_motion_sync(owner).apply(&mut e).unwrap();
        let held = e
            .begin_hold(node, Property::Height, 0.0, None)
            .unwrap()
            .unwrap();
        e.observe(exact_motion::Change {
            node,
            property: Property::Translate,
            value: Value::new(90.0, 0.0),
            velocity: None,
        })
        .unwrap();
        let change = if hide {
            Op::SetStyle {
                id: 2,
                patch: style(|s| {
                    s.display = Display::None;
                    s.mask.set(StyleId::Display);
                }),
            }
        } else {
            Op::SetChildren {
                id: 1,
                children: vec![4],
            }
        };
        let r = k.apply(0, 2, &[change]).unwrap();
        assert!(
            !r.touched.contains(&owner),
            "target node is not in the receipt"
        );
        k.motion_sync(&r).apply(&mut e).unwrap();
        assert_eq!(k.height_target(owner), None);
        let retirement = k.height_motion_sync(owner);
        assert_eq!(retirement.retired, vec![(node, Property::Height)]);
        assert!(retirement.removed.is_empty());
        retirement.apply(&mut e).unwrap();
        assert!(!e.has_hold(held.token));
        assert!(!e
            .update_hold(held.token, f64::NAN, Value::new(f64::NAN, 1.0))
            .unwrap());
        assert_eq!(e.now(), 0.0);
        assert!(e.is_active(node, Property::Translate));
        assert_eq!(e.value(node, Property::Height), None);
        let restore = if hide {
            Op::SetStyle {
                id: 2,
                patch: style(|s| {
                    s.display = Display::Block;
                    s.mask.set(StyleId::Display);
                }),
            }
        } else {
            Op::SetChildren {
                id: 1,
                children: vec![2, 4],
            }
        };
        let r = k.apply(0, 3, &[restore]).unwrap();
        k.motion_sync(&r).apply(&mut e).unwrap();
        k.height_motion_sync(owner).apply(&mut e).unwrap();
        assert_eq!(e.value(node, Property::Height), Some(Value::scalar(180.0)));
        assert!(
            !e.is_active(node, Property::Height),
            "readoption starts from current numeric authoring"
        );
    }
}

#[test]
fn unsupported_height_retires_only_height_and_never_synthesizes_auto_zero() {
    for value in [
        Dimension::Auto,
        Dimension::Percent(50.0),
        Dimension::Points(-1.0),
        Dimension::Env(exact_kernel::Edge::Top, 0.0),
    ] {
        let (mut k, mut e, owner) = height_tree();
        let node = motion_node(owner);
        k.height_motion_sync(owner).apply(&mut e).unwrap();
        let held = e
            .begin_hold(node, Property::Height, 0.0, None)
            .unwrap()
            .unwrap();
        let r = k
            .apply(
                0,
                2,
                &[Op::SetStyle {
                    id: 3,
                    patch: height_patch(value),
                }],
            )
            .unwrap();
        k.motion_sync(&r).apply(&mut e).unwrap();
        k.height_motion_sync(owner).apply(&mut e).unwrap();
        assert_eq!(k.height_target(owner), None);
        assert_eq!(e.value(node, Property::Height), None);
        assert!(!e.has_hold(held.token));
        assert_eq!(e.value(node, Property::Opacity), Some(Value::scalar(1.0)));
        assert!(e
            .begin_hold(node, Property::Height, f64::NAN, None)
            .unwrap()
            .is_none());
    }
}

#[test]
fn inline_detached_and_reused_keys_are_ineligible_but_numeric_roots_are_allowed() {
    let (mut k, mut e, old) = height_tree();
    let r = k
        .apply(
            0,
            2,
            &[
                Op::DestroyView { id: 3 },
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 3,
                    patch: height_patch(Dimension::Points(90.0)),
                },
                Op::CreateView {
                    id: 5,
                    node_type: NodeType::Text,
                },
                Op::CreateView {
                    id: 6,
                    node_type: NodeType::Text,
                },
                Op::SetStyle {
                    id: 6,
                    patch: height_patch(Dimension::Points(90.0)),
                },
                Op::SetChildren {
                    id: 5,
                    children: vec![6],
                },
                Op::SetChildren {
                    id: 2,
                    children: vec![5],
                },
                Op::SetStyle {
                    id: 1,
                    patch: height_patch(Dimension::Points(600.0)),
                },
            ],
        )
        .unwrap();
    k.motion_sync(&r).apply(&mut e).unwrap();
    assert_eq!(k.height_target(old), None);
    assert_eq!(
        k.height_target(k.node(3).unwrap().key),
        None,
        "detached numeric node"
    );
    assert_eq!(
        k.height_target(k.node(6).unwrap().key),
        None,
        "inline numeric text run"
    );
    assert_eq!(
        k.height_target(k.node(1).unwrap().key),
        Some(Value::scalar(600.0))
    );
    assert_eq!(
        k.height_motion_sync(old).retired,
        vec![(motion_node(old), Property::Height)]
    );
}

#[test]
fn height_transition_roundtrips_exwf_without_changing_previous_property_codes() {
    for (code, property) in Property::ALL.into_iter().enumerate() {
        if property == Property::ShadowColor {
            continue; // named only by `box-shadow`; refused on the wire (LLP 1062)
        }
        assert_eq!(property as usize, code);
        let rows = Transitions(vec![Transition::new(
            TransitionProperty::Property(property),
            1.0,
            TimingFunction::Easing(Easing::Linear),
        )]);
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
                }),
            },
            Op::AttachRoot { id: 1 },
        ];
        let mut k = Kernel::with_monospace();
        k.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
        assert_eq!(k.node(1).unwrap().style.transition, rows);
    }
}

#[test]
fn property_retirement_precedes_readoption_and_preserves_other_live_holds() {
    let (mut k, mut e, owner) = height_tree();
    let node = motion_node(owner);
    k.height_motion_sync(owner).apply(&mut e).unwrap();
    let height_hold = e
        .begin_hold(node, Property::Height, 0.0, None)
        .unwrap()
        .unwrap();
    let opacity_hold = e
        .begin_hold(node, Property::Opacity, 0.0, None)
        .unwrap()
        .unwrap();
    e.update_hold(height_hold.token, 0.0, Value::scalar(90.0))
        .unwrap();
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 3,
            patch: height_patch(Dimension::Points(0.0)),
        }],
    )
    .unwrap();
    let mut sync = k.height_motion_sync(owner);
    sync.retired.push((node, Property::Height));
    sync.apply(&mut e).unwrap();
    assert_eq!(
        e.value(node, Property::Height),
        Some(Value::ZERO),
        "explicit numeric zero is eligible"
    );
    assert!(
        !e.is_active(node, Property::Height),
        "new adoption has no old curve"
    );
    assert!(!e.has_hold(height_hold.token));
    assert!(e.has_hold(opacity_hold.token));
    let mut mask = exact_kernel::StyleMask::EMPTY;
    mask.set(StyleId::Height);
    k.apply(0, 3, &[Op::ClearStyle { id: 3, mask }]).unwrap();
    k.height_motion_sync(owner).apply(&mut e).unwrap();
    assert_eq!(
        e.value(node, Property::Height),
        None,
        "auto is distinct from explicit zero"
    );
    assert!(e.has_hold(opacity_hold.token));
}

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

#[test]
fn reset_slot_reuse_cannot_inherit_retained_motion_state() {
    let (mut kernel, mut engine, old_node) = boot(linear_all(1.0));
    assert_eq!(
        engine.value(old_node, Property::Opacity),
        Some(Value::scalar(1.0))
    );

    kernel.reset();
    let receipt = kernel
        .apply(
            0,
            2,
            &[
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 2,
                    patch: opacity(0.25),
                },
                Op::AttachRoot { id: 2 },
            ],
        )
        .unwrap();
    let new_node = motion_node(kernel.node(2).unwrap().key);
    assert_ne!(new_node, old_node);
    kernel.motion_sync(&receipt).apply(&mut engine).unwrap();

    assert_eq!(
        engine.value(old_node, Property::Opacity),
        Some(Value::scalar(1.0)),
        "the retained cache entry still names only the old allocation"
    );
    assert_eq!(
        engine.value(new_node, Property::Opacity),
        Some(Value::scalar(0.25))
    );
}

fn translate(x: f32, y: f32) -> Box<StyleProps> {
    style(|s| {
        s.translate = Vec2 { x, y };
        s.mask.set(StyleId::Translate);
    })
}

fn spring_translate() -> Transition {
    Transition::new(
        TransitionProperty::Property(Property::Translate),
        0.0,
        TimingFunction::Spring(SpringConfig::default()),
    )
}

fn commit(kernel: &mut Kernel, engine: &mut Engine, sequence: u64, ops: &[Op]) {
    let receipt = kernel.apply(0, sequence, ops).unwrap();
    kernel.motion_sync(&receipt).apply(engine).unwrap();
}

#[test]
fn a_caught_spring_stays_held_through_an_unrelated_kernel_prop_commit() {
    // Native takes the engine sample; Web supplies the actual DOM presentation.
    for supplied in [None, Some(Value::new(42.0, -7.0))] {
        let (mut kernel, mut engine, node) = boot(spring_translate());
        commit(
            &mut kernel,
            &mut engine,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: translate(100.0, 20.0),
            }],
        );
        engine.advance(0.1).unwrap();
        let sampled = engine.value(node, Property::Translate).unwrap();
        assert_ne!(sampled, Value::new(100.0, 20.0));
        let hold = engine
            .begin_hold(node, Property::Translate, 0.1, supplied)
            .unwrap()
            .unwrap();
        assert_eq!(hold.value, supplied.unwrap_or(sampled));
        assert!(engine.update_hold(hold.token, 0.125, hold.value).unwrap());
        assert_eq!(engine.value(node, Property::Translate), Some(hold.value));
        assert_eq!(
            engine.target(node, Property::Translate),
            Some(Value::new(100.0, 20.0))
        );
        engine.frame();

        let receipt = kernel
            .apply(
                0,
                3,
                &[
                    Op::SetProp {
                        id: 1,
                        prop: PropId::TestId,
                        value: PropValue::Str("held-row".into()),
                    },
                    Op::SetStyle {
                        id: 1,
                        patch: opacity(0.25),
                    },
                ],
            )
            .unwrap();
        let sync = kernel.motion_sync(&receipt);
        assert!(
            sync.changes.iter().any(|change| {
                change.node == node
                    && change.property == Property::Translate
                    && change.value == Value::new(100.0, 20.0)
            }),
            "unrelated commit must exercise the restated authored target"
        );
        sync.apply(&mut engine).unwrap();
        assert_eq!(engine.value(node, Property::Translate), Some(hold.value));
        assert_eq!(
            engine.value(node, Property::Opacity),
            Some(Value::scalar(0.25))
        );
        assert_eq!(
            kernel.node(1).unwrap().style.translate,
            Vec2 { x: 100.0, y: 20.0 }
        );
        assert!(engine.quiescent());
        assert_eq!(engine.settle_time(), None);
        assert_eq!(engine.spring_frames(node, Property::Translate), None);
        engine.frame();
        engine.advance(1.0).unwrap();
        assert!(
            engine.frame().is_empty(),
            "a held property must not schedule idle frames"
        );
        assert_eq!(engine.value(node, Property::Translate), Some(hold.value));
    }
}

#[test]
fn release_uses_latest_kernel_target_and_transition_even_when_target_is_unchanged_at_end() {
    let (mut kernel, mut engine, node) = boot(linear_all(4.0));
    let hold = engine
        .begin_hold(node, Property::Translate, 0.0, None)
        .unwrap()
        .unwrap();
    let held = Value::new(80.0, -20.0);
    assert!(engine.update_hold(hold.token, 0.125, held).unwrap());
    commit(
        &mut kernel,
        &mut engine,
        2,
        &[Op::SetStyle {
            id: 1,
            patch: translate(20.0, 10.0),
        }],
    );
    commit(
        &mut kernel,
        &mut engine,
        3,
        &[Op::SetStyle {
            id: 1,
            patch: translate(120.0, 40.0),
        }],
    );
    // A declaration-only commit still restates every authored property.
    commit(
        &mut kernel,
        &mut engine,
        4,
        &[Op::SetStyle {
            id: 1,
            patch: transition(linear_all(2.0)),
        }],
    );
    assert_eq!(engine.value(node, Property::Translate), Some(held));
    assert_eq!(
        engine.target(node, Property::Translate),
        Some(Value::new(120.0, 40.0))
    );
    assert!(engine.quiescent());

    assert!(engine
        .end_hold(
            hold.token,
            0.25,
            HoldEnd::Release {
                velocity: Value::ZERO
            }
        )
        .unwrap());
    assert_eq!(engine.value(node, Property::Translate), Some(held));
    assert_eq!(engine.settle_time(), Some(2.25));
    // An unrelated commit during release must not restart the curve either.
    commit(
        &mut kernel,
        &mut engine,
        5,
        &[Op::SetProp {
            id: 1,
            prop: PropId::TestId,
            value: PropValue::Str("released-row".into()),
        }],
    );
    engine.advance(1.25).unwrap();
    assert_eq!(
        engine.value(node, Property::Translate),
        Some(Value::new(100.0, 10.0))
    );
    assert!(!engine.end_hold(hold.token, 1.5, HoldEnd::Cancel).unwrap());
    assert_eq!(engine.now(), 1.25, "a consumed token cannot move the clock");
    engine.advance(2.25).unwrap();
    assert_eq!(
        engine.value(node, Property::Translate),
        Some(Value::new(120.0, 40.0))
    );
    assert!(engine.quiescent());
}

#[test]
fn clearing_or_zeroing_the_kernel_transition_during_hold_snaps_on_end() {
    for clear in [false, true] {
        for cancel in [false, true] {
            let (mut kernel, mut engine, node) = boot(spring_translate());
            let hold = engine
                .begin_hold(node, Property::Translate, 0.0, None)
                .unwrap()
                .unwrap();
            assert!(engine
                .update_hold(hold.token, 0.1, Value::new(80.0, -20.0))
                .unwrap());
            let declaration = if clear {
                let mut mask = exact_kernel::StyleMask::EMPTY;
                mask.set(StyleId::Transition);
                Op::ClearStyle { id: 1, mask }
            } else {
                Op::SetStyle {
                    id: 1,
                    patch: transition(linear_all(0.0)),
                }
            };
            commit(
                &mut kernel,
                &mut engine,
                2,
                &[
                    Op::SetStyle {
                        id: 1,
                        patch: translate(12.0, 4.0),
                    },
                    declaration,
                ],
            );
            assert_eq!(
                engine.value(node, Property::Translate),
                Some(Value::new(80.0, -20.0))
            );
            let end = if cancel {
                HoldEnd::Cancel
            } else {
                HoldEnd::Release {
                    velocity: Value::new(500.0, -100.0),
                }
            };
            assert!(engine.end_hold(hold.token, 0.2, end).unwrap());
            assert_eq!(
                engine.value(node, Property::Translate),
                Some(Value::new(12.0, 4.0))
            );
            assert_eq!(
                engine.target(node, Property::Translate),
                Some(Value::new(12.0, 4.0))
            );
            assert!(engine.quiescent());
            assert_eq!(engine.settle_time(), None);
        }
    }
}

#[test]
fn kernel_spring_release_at_the_authored_target_still_inherits_velocity() {
    let (mut kernel, mut engine, node) = boot(linear_all(1.0));
    let hold = engine
        .begin_hold(node, Property::Translate, 0.0, None)
        .unwrap()
        .unwrap();
    assert_eq!(hold.value, Value::ZERO);
    // The transition chosen at release is this new spring, not the old easing.
    commit(
        &mut kernel,
        &mut engine,
        2,
        &[Op::SetStyle {
            id: 1,
            patch: transition(spring_translate()),
        }],
    );
    assert!(engine
        .end_hold(
            hold.token,
            0.1,
            HoldEnd::Release {
                velocity: Value::new(500.0, -100.0)
            }
        )
        .unwrap());
    assert!(!engine.quiescent());
    let frames = engine.spring_frames(node, Property::Translate).unwrap();
    assert_eq!(frames.values.first(), Some(&Value::ZERO));
    assert_eq!(frames.values.last(), Some(&Value::ZERO));
    assert!(frames
        .values
        .iter()
        .any(|value| value.x > 0.0 && value.y < 0.0));
    engine.advance(0.12).unwrap();
    let moving = engine.value(node, Property::Translate).unwrap();
    assert!(moving.x > 0.0 && moving.y < 0.0);
    engine.advance(engine.settle_time().unwrap()).unwrap();
    assert_eq!(engine.value(node, Property::Translate), Some(Value::ZERO));
    assert!(engine.quiescent());
}

#[test]
fn destroyed_kernel_hold_tokens_cannot_touch_a_reused_node_slot_or_its_new_hold() {
    let (mut kernel, mut engine, node) = boot(linear_all(1.0));
    let old_key = kernel.node(1).unwrap().key;
    let old = engine
        .begin_hold(node, Property::Translate, 0.0, None)
        .unwrap()
        .unwrap();
    assert!(engine
        .update_hold(old.token, 0.25, Value::new(50.0, 10.0))
        .unwrap());
    commit(&mut kernel, &mut engine, 2, &[Op::DestroyView { id: 1 }]);
    assert_eq!(engine.value(node, Property::Translate), None);
    assert!(engine.quiescent());
    commit(
        &mut kernel,
        &mut engine,
        3,
        &[
            Op::CreateView {
                id: 9,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 9,
                patch: translate(7.0, -3.0),
            },
            Op::AttachRoot { id: 9 },
        ],
    );
    let new_key = kernel.node(9).unwrap().key;
    assert_eq!(
        new_key.index, old_key.index,
        "fixture must reuse the arena slot"
    );
    assert_ne!(new_key.generation, old_key.generation);
    let new_node = motion_node(new_key);
    let current = engine
        .begin_hold(new_node, Property::Translate, 0.5, None)
        .unwrap()
        .unwrap();
    assert!(engine
        .update_hold(current.token, 0.75, Value::new(9.0, -4.0))
        .unwrap());
    engine.frame();

    // Stale identity wins over malformed data and backwards/nonfinite clocks.
    assert!(!engine
        .update_hold(old.token, f64::NAN, Value::new(f64::INFINITY, 0.0))
        .unwrap());
    assert!(!engine
        .end_hold(
            old.token,
            0.1,
            HoldEnd::Release {
                velocity: Value::new(f64::NAN, 0.0)
            }
        )
        .unwrap());
    assert!(engine
        .begin_hold(
            node,
            Property::Translate,
            f64::NAN,
            Some(Value::new(f64::NAN, 0.0))
        )
        .unwrap()
        .is_none());
    assert_eq!(engine.now(), 0.75);
    assert!(engine.frame().is_empty());
    assert_eq!(
        engine.value(node, Property::Translate),
        None,
        "no ghost slot"
    );
    assert_eq!(
        engine.value(new_node, Property::Translate),
        Some(Value::new(9.0, -4.0))
    );
    assert_eq!(
        engine.target(new_node, Property::Translate),
        Some(Value::new(7.0, -3.0))
    );
    assert!(engine
        .end_hold(current.token, 1.0, HoldEnd::Cancel)
        .unwrap());
    assert_eq!(
        engine.value(new_node, Property::Translate),
        Some(Value::new(7.0, -3.0))
    );
    assert!(engine.quiescent());
}
