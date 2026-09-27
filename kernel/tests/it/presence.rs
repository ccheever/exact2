//! Exit animation and layout transition rows (LLP 1063): which destroyed
//! nodes leave with an exit, and the seam that hands layout rows to motion.

use exact_kernel::{
    motion_node, Animations, ApplyError, Kernel, KernelError, NodeType, Op, StyleId, StyleProps,
    Transitions,
};
use exact_motion::{AnimationError, Engine, Property, Value};

const FADE: &str = "@keyframes fade{from{opacity:1}to{opacity:0}}";

fn exit(text: &str) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    s.exit_animation = Animations::parse(&format!("{text} {FADE}")).unwrap();
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
fn nothing_exits_that_was_never_presented_or_has_no_row_or_lives_in_a_list() {
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

    k.apply(
        0,
        4,
        &[
            Op::CreateView {
                id: 20,
                node_type: NodeType::List,
            },
            view(21),
            Op::SetStyle {
                id: 21,
                patch: exit("fade 1s"),
            },
            Op::SetChildren {
                id: 20,
                children: vec![21],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 20],
            },
        ],
    )
    .unwrap();
    let r = k.apply(0, 5, &[Op::DestroyView { id: 21 }]).unwrap();
    assert!(r.exits.is_empty(), "a windowed row");
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
                value: Value::new(0.0, y),
                velocity: None,
            })
            .unwrap()
    };
    at(&mut engine, 50.0);
    at(&mut engine, 10.0);
    engine.advance(0.5).unwrap();
    assert_eq!(
        engine.value(four, Property::Layout),
        Some(Value::new(0.0, 30.0)),
        "the layout row's 1s, not transition's 5s"
    );
}
