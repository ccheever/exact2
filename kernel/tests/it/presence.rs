//! Exit animation and layout transition rows (LLP 1063): which destroyed
//! nodes leave with an exit, and the seam that hands layout rows to motion.

use crate::keyframed;
use exact_kernel::{
    motion_node, ApplyError, Kernel, KernelError, NodeType, Op, PropId, PropValue, StyleId,
    StyleProps, Transitions,
};
use exact_motion::{AnimationError, Engine, Property, Value};

const FADE: &str = "@keyframes fade{from{opacity:1}to{opacity:0}}";

fn exit(text: &str) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    s.exit_animation = keyframed(&format!("{text} {FADE}"));
    s.mask.set(StyleId::ExitAnimation);
    Box::new(s)
}

fn view(id: u32) -> Op {
    Op::CreateView {
        id,
        node_type: NodeType::View,
    }
}

/// root 1 → [2 (exits) → [3 (exits)], 4]
fn kernel() -> Kernel {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            view(1),
            view(2),
            view(3),
            view(4),
            Op::SetStyle {
                id: 2,
                patch: exit("fade 200ms"),
            },
            Op::SetStyle {
                id: 3,
                patch: exit("fade 300ms"),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 4],
            },
            Op::SetChildren {
                id: 2,
                children: vec![3],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

#[test]
fn only_the_outermost_removed_node_exits_from_its_surviving_parent() {
    let mut k = kernel();
    let (two, root) = (k.node(2).unwrap().key, k.node(1).unwrap().key);
    let r = k.apply(0, 2, &[Op::DestroyView { id: 2 }]).unwrap();
    assert_eq!(r.destroyed.len(), 2);
    assert_eq!(r.exits.len(), 1, "3 goes inside 2");
    assert_eq!(r.exits[0].key, two);
    assert_eq!(r.exits[0].parent, root);
    assert!((r.exits[0].animations.end_time() - 0.2).abs() < 1e-6);
}

#[test]
fn a_producer_that_detaches_before_it_destroys_still_exits_from_the_old_parent() {
    let mut k = kernel();
    let root = k.node(1).unwrap().key;
    let r = k
        .apply(
            0,
            2,
            &[
                Op::SetChildren {
                    id: 1,
                    children: vec![4],
                },
                Op::DestroyView { id: 2 },
            ],
        )
        .unwrap();
    assert_eq!(r.exits.len(), 1);
    assert_eq!(r.exits[0].parent, root);
}

#[test]
fn a_later_destroy_of_the_parent_takes_the_exit_with_it() {
    let mut k = kernel();
    let r = k
        .apply(
            0,
            2,
            &[Op::DestroyView { id: 3 }, Op::DestroyView { id: 2 }],
        )
        .unwrap();
    assert_eq!(r.exits.len(), 1);
    assert_eq!(r.exits[0].animations.0[0].duration as f32, 0.2);
}

#[test]
fn nothing_exits_that_was_never_presented_or_has_no_row() {
    let mut k = kernel();
    // Created and destroyed in one batch.
    let r = k
        .apply(
            0,
            2,
            &[
                view(9),
                Op::SetStyle {
                    id: 9,
                    patch: exit("fade 1s"),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 4, 9],
                },
                Op::DestroyView { id: 9 },
            ],
        )
        .unwrap();
    assert!(r.exits.is_empty());
    let r = k.apply(0, 3, &[Op::DestroyView { id: 4 }]).unwrap();
    assert!(r.exits.is_empty(), "no row");
}

fn key(id: u32, key: &str) -> Op {
    Op::SetProp {
        id,
        prop: PropId::ListItemKey,
        value: PropValue::Str(key.into()),
    }
}

/// A windowed list, 20 → content 21 → wrappers 22 (→ 23) and 24 (→ 25),
/// each wrapper's one root declaring an exit, as the runner's window builds.
fn windowed() -> Kernel {
    let mut k = kernel();
    let mut ops = vec![
        Op::CreateView {
            id: 20,
            node_type: NodeType::List,
        },
        view(21),
    ];
    for (wrapper, root) in [(22, 23), (24, 25)] {
        ops.extend([
            view(wrapper),
            view(root),
            Op::SetStyle {
                id: root,
                patch: exit("fade 1s"),
            },
            Op::SetChildren {
                id: wrapper,
                children: vec![root],
            },
            key(wrapper, &format!("s:{wrapper}")),
        ]);
    }
    ops.extend([
        Op::SetChildren {
            id: 21,
            children: vec![22, 24],
        },
        Op::SetChildren {
            id: 20,
            children: vec![21],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 4, 20],
        },
    ]);
    k.apply(0, 2, &ops).unwrap();
    k
}

#[test]
fn a_windowed_row_exits_when_its_item_left_the_data_not_when_it_scrolled_away() {
    let mut k = windowed();
    let content = k.node(21).unwrap().key;
    // Scrolled away: the window detaches and destroys it, key and all.
    let r = k
        .apply(
            0,
            3,
            &[
                Op::SetChildren {
                    id: 21,
                    children: vec![24],
                },
                Op::DestroyView { id: 22 },
            ],
        )
        .unwrap();
    assert!(r.exits.is_empty(), "{:?}", r.exits);
    // Its item left the data: the window empties the key first. The wrapper
    // leaves (it is where the window placed the row), with its root's exit.
    let wrapper = k.node(24).unwrap().key;
    let r = k
        .apply(
            0,
            4,
            &[
                key(24, ""),
                Op::SetChildren {
                    id: 21,
                    children: vec![],
                },
                Op::DestroyView { id: 24 },
            ],
        )
        .unwrap();
    assert_eq!(r.exits.len(), 1);
    assert_eq!((r.exits[0].key, r.exits[0].parent), (wrapper, content));
    assert!((r.exits[0].animations.end_time() - 1.0).abs() < 1e-6);
}

#[test]
fn a_node_inside_a_list_row_exits_like_any_other() {
    let mut k = windowed();
    let root = k.node(23).unwrap().key;
    k.apply(
        0,
        3,
        &[
            view(30),
            Op::SetStyle {
                id: 30,
                patch: exit("fade 1s"),
            },
            Op::SetChildren {
                id: 23,
                children: vec![30],
            },
        ],
    )
    .unwrap();
    let r = k.apply(0, 4, &[Op::DestroyView { id: 30 }]).unwrap();
    assert_eq!(r.exits.len(), 1);
    assert_eq!(r.exits[0].parent, root);
}

#[test]
fn an_exit_that_never_ends_is_refused_and_applies_nothing() {
    let mut k = kernel();
    for (text, error) in [
        ("fade 1s infinite", AnimationError::Endless),
        ("fade 1s paused", AnimationError::Endless),
    ] {
        let result = k.apply(
            0,
            2,
            &[Op::SetStyle {
                id: 4,
                patch: exit(text),
            }],
        );
        assert!(
            matches!(
                result,
                Err(KernelError::Apply(ApplyError::InvalidAnimation { op_index: 0, error: e })) if e == error
            ),
            "{text}: {result:?}"
        );
    }
    assert!(k.node(4).unwrap().style.exit_animation.0.is_empty());
}

#[test]
fn the_seam_hands_layout_rows_to_the_engine_and_all_never_moves_a_box() {
    let mut k = kernel();
    let mut s = StyleProps::default();
    s.layout_transition = Transitions::parse("1s linear").unwrap();
    s.transition = Transitions::parse("all 5s").unwrap();
    s.mask.set(StyleId::LayoutTransition);
    s.mask.set(StyleId::Transition);
    let r = k
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 4,
                patch: Box::new(s),
            }],
        )
        .unwrap();
    let four = motion_node(k.node(4).unwrap().key);
    let mut engine = Engine::new();
    k.motion_sync(&r).apply(&mut engine).unwrap();
    let at = |engine: &mut Engine, y| {
        engine
            .observe(exact_motion::Change {
                node: four,
                property: Property::Layout,
                value: Value::four(0.0, y, 10.0, 10.0),
                velocity: None,
            })
            .unwrap()
    };
    at(&mut engine, 50.0);
    at(&mut engine, 10.0);
    engine.advance(0.5).unwrap();
    assert_eq!(
        engine.value(four, Property::Layout),
        Some(Value::four(0.0, 30.0, 10.0, 10.0)),
        "the layout row's 1s, not transition's 5s"
    );
}

#[test]
fn shared_layout_observation_seeds_snaps_hides_and_retires() {
    use exact_kernel::motion::{layout_presented, LayoutMotion};
    use exact_kernel::{Offer, StyleValue};

    let rows = |id, rows: &[(StyleId, &str)]| {
        let mut s = StyleProps::default();
        for (row, value) in rows {
            s.set_dynamic(*row, &StyleValue::Text((*value).into()))
                .unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(s),
        }
    };
    let mut k = kernel();
    let receipt = k
        .apply(0, 2, &[rows(4, &[(StyleId::Height, "10px")])])
        .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    let key = k.node(4).unwrap().key;
    let node = motion_node(key);
    let mut engine = Engine::new();
    let mut layout = LayoutMotion::default();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    layout.adopt(&k, [key]);
    let receipt = k
        .apply(
            0,
            3,
            &[
                rows(2, &[(StyleId::Height, "50px")]),
                rows(
                    4,
                    &[
                        (StyleId::Height, "100px"),
                        (StyleId::LayoutTransition, "1s linear"),
                    ],
                ),
            ],
        )
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert!(layout.seed(&k, &receipt, &mut engine).is_empty());
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    layout.observe_all(&k, &mut engine, false);
    assert_eq!(
        layout_presented(&engine, node, engine.value(node, Property::Layout).unwrap()),
        [0.0, -50.0, 1.0, 0.1]
    );
    engine.advance(0.5).unwrap();
    assert_eq!(
        layout_presented(&engine, node, engine.value(node, Property::Layout).unwrap()),
        [0.0, -25.0, 1.0, 0.55]
    );
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    assert_eq!(
        layout.observe_all(&k, &mut engine, true),
        vec![key],
        "a resize resets the host's presentation in the same frame"
    );
    assert!(!engine.is_active(node, Property::Layout), "resize snaps");
    let receipt = k
        .apply(0, 4, &[rows(1, &[(StyleId::Display, "none")])])
        .unwrap();
    layout.seed(&k, &receipt, &mut engine);
    assert_eq!(layout.observe_all(&k, &mut engine, false), vec![key]);
    assert_eq!(engine.target(node, Property::Layout), None);
    let receipt = k
        .apply(0, 5, &[rows(1, &[(StyleId::Display, "block")])])
        .unwrap();
    layout.seed(&k, &receipt, &mut engine);
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    layout.observe_all(&k, &mut engine, false);
    assert_eq!(engine.value(node, Property::Layout), k.layout_box(key));
    assert!(
        !engine.is_active(node, Property::Layout),
        "shown again is first seen"
    );
    let receipt = k
        .apply(0, 6, &[rows(4, &[(StyleId::LayoutTransition, "none")])])
        .unwrap();
    assert_eq!(layout.seed(&k, &receipt, &mut engine), vec![key]);
    assert_eq!(engine.target(node, Property::Layout), None);
}

/// A box a list moved for no author (rows built or measured before it, LLP
/// 1010) takes its place with no transition when idle; one already moving
/// goes on moving toward the new place.
#[test]
fn an_unauthored_move_snaps_an_idle_box_and_retargets_a_moving_one() {
    use exact_kernel::motion::{layout_presented, LayoutMotion};
    use exact_kernel::{Offer, StyleValue};

    let rows = |id, rows: &[(StyleId, &str)]| {
        let mut s = StyleProps::default();
        for (row, value) in rows {
            s.set_dynamic(*row, &StyleValue::Text((*value).into()))
                .unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(s),
        }
    };
    let mut k = kernel();
    let receipt = k
        .apply(
            0,
            2,
            &[
                rows(2, &[(StyleId::Height, "10px")]),
                rows(
                    4,
                    &[
                        (StyleId::Height, "10px"),
                        (StyleId::LayoutTransition, "1s linear"),
                    ],
                ),
            ],
        )
        .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    let key = k.node(4).unwrap().key;
    let node = motion_node(key);
    let mut engine = Engine::new();
    let mut layout = LayoutMotion::default();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    layout.adopt(&k, [key]);
    layout.observe_all(&k, &mut engine, false);
    // The box before it grows (a row measured): idle, it takes the place.
    let receipt = k
        .apply(0, 3, &[rows(2, &[(StyleId::Height, "60px")])])
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    layout.seed(&k, &receipt, &mut engine);
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    assert!(layout.observe_unauthored(&k, key, &mut engine).is_empty());
    assert!(
        !engine.is_active(node, Property::Layout),
        "no transition plays"
    );
    assert_eq!(engine.value(node, Property::Layout), k.layout_box(key));
    assert_eq!(
        layout_presented(&engine, node, engine.value(node, Property::Layout).unwrap()),
        [0.0, 0.0, 1.0, 1.0],
        "nothing moves on screen"
    );
    // An authored move starts its transition; an unauthored one during it
    // retargets the running curve rather than cutting it.
    let receipt = k
        .apply(0, 4, &[rows(2, &[(StyleId::Height, "20px")])])
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    layout.seed(&k, &receipt, &mut engine);
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    layout.observe_all(&k, &mut engine, false);
    assert!(
        engine.is_active(node, Property::Layout),
        "an authored move plays"
    );
    engine.advance(0.25).unwrap();
    let receipt = k
        .apply(0, 5, &[rows(2, &[(StyleId::Height, "30px")])])
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    layout.seed(&k, &receipt, &mut engine);
    k.compute_layout(1, Offer::definite(400.0, 600.0)).unwrap();
    layout.observe_unauthored(&k, key, &mut engine);
    assert!(engine.is_active(node, Property::Layout), "still moving");
    assert_eq!(engine.target(node, Property::Layout), k.layout_box(key));
}
