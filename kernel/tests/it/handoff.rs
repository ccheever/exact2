//! Shared-element handoffs (LLP 1013.000 D3): a name leaving a destroyed
//! node and arriving at a created one pairs, with the curve of either end.

use exact_kernel::{Kernel, NodeType, Op, PropId, PropValue, StyleId, StyleProps, Transitions};

fn view(id: u32) -> Op {
    Op::CreateView {
        id,
        node_type: NodeType::View,
    }
}

fn name(id: u32, name: &str) -> Op {
    Op::SetProp {
        id,
        prop: PropId::SharedElement,
        value: PropValue::Str(name.into()),
    }
}

fn curve(id: u32, text: &str) -> Op {
    let mut s = StyleProps::default();
    s.rare.layout_transition = Transitions::parse(text).unwrap();
    s.mask.set(StyleId::LayoutTransition);
    Op::SetStyle {
        id,
        patch: Box::new(s),
    }
}

/// root 1 → [grid 2 → [thumb 3 "photo"]]
fn kernel(thumb_curve: bool) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![view(1), view(2), view(3), name(3, "photo")];
    if thumb_curve {
        ops.push(curve(3, "300ms ease"));
    }
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 1, &ops).unwrap();
    k
}

/// The grid goes and a viewer with the named image comes, in one commit.
fn open(k: &mut Kernel, viewer_curve: bool) -> exact_kernel::CommitReceipt {
    let mut ops = vec![
        Op::SetChildren {
            id: 1,
            children: vec![],
        },
        Op::DestroyView { id: 2 },
        view(4),
        view(5),
        name(5, "photo"),
    ];
    if viewer_curve {
        ops.push(curve(5, "spring(300, 30, 1)"));
    }
    ops.extend([
        Op::SetChildren {
            id: 4,
            children: vec![5],
        },
        Op::SetChildren {
            id: 1,
            children: vec![4],
        },
    ]);
    k.apply(0, 2, &ops).unwrap()
}

#[test]
fn a_name_leaving_with_a_destroyed_ancestor_pairs_with_the_created_holder() {
    let mut k = kernel(false);
    let thumb = k.node(3).unwrap().key;
    let r = open(&mut k, true);
    assert_eq!(r.handoffs.len(), 1);
    let h = &r.handoffs[0];
    assert_eq!(h.name, "photo");
    assert_eq!(h.from, thumb);
    assert_eq!(h.to, k.node(5).unwrap().key);
    assert!(k.node_by_key(h.from).is_none(), "the leaver is gone");
}

#[test]
fn the_curve_is_the_arrivers_else_the_leavers_else_none() {
    let mut k = kernel(true);
    let r = open(&mut k, true);
    assert!(
        matches!(
            r.handoffs[0].transition.timing,
            exact_motion::TimingFunction::Spring(_)
        ),
        "the arriver's row governs"
    );
    let mut k = kernel(true);
    let r = open(&mut k, false);
    assert_eq!(r.handoffs.len(), 1, "the leaver's row suffices");
    assert!((r.handoffs[0].transition.duration - 0.3).abs() < 1e-9);
    let mut k = kernel(false);
    assert!(
        open(&mut k, false).handoffs.is_empty(),
        "no curve, no flight"
    );
}

#[test]
fn two_arrivers_or_a_name_with_no_arriver_pair_nothing() {
    let mut k = kernel(true);
    let r = k
        .apply(
            0,
            2,
            &[
                Op::DestroyView { id: 2 },
                view(4),
                view(5),
                name(4, "photo"),
                name(5, "photo"),
                Op::SetChildren {
                    id: 1,
                    children: vec![4, 5],
                },
            ],
        )
        .unwrap();
    assert!(r.handoffs.is_empty());
    let mut k = kernel(true);
    let r = k.apply(0, 2, &[Op::DestroyView { id: 2 }]).unwrap();
    assert!(r.handoffs.is_empty());
}

#[test]
fn a_name_already_held_elsewhere_does_not_block_a_pair() {
    // A mounted route keeps "photo" (6) while the grid's leaves and a
    // viewer's arrives: the pair is the change, not a lookup.
    let mut k = kernel(true);
    k.apply(
        0,
        2,
        &[
            view(6),
            name(6, "photo"),
            Op::SetChildren {
                id: 1,
                children: vec![2, 6],
            },
        ],
    )
    .unwrap();
    let r = open(&mut k, false);
    assert_eq!(r.handoffs.len(), 1);
}

#[test]
fn a_node_created_and_destroyed_in_one_commit_hands_off_nothing() {
    let mut k = kernel(true);
    let r = k
        .apply(
            0,
            2,
            &[
                view(7),
                name(7, "other"),
                curve(7, "1s linear"),
                Op::DestroyView { id: 7 },
                view(8),
                name(8, "other"),
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 8],
                },
            ],
        )
        .unwrap();
    assert!(r.handoffs.is_empty());
}

#[test]
fn two_leavers_of_one_name_pair_nothing() {
    let mut k = kernel(true);
    k.apply(
        0,
        2,
        &[
            view(6),
            name(6, "photo"),
            Op::SetChildren {
                id: 2,
                children: vec![3, 6],
            },
        ],
    )
    .unwrap();
    assert!(open(&mut k, true).handoffs.is_empty());
}
