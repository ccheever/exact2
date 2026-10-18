use super::*;
use crate::{layout::LayoutTree, selector::SelectorIndex, txn, wire::Op};
use std::sync::Arc;

#[test]
fn rollover_is_atomic_and_invalid_or_noop_batches_do_not_rotate() {
    let mut arena = NodeArena::new();
    let mut layout = LayoutTree::new();
    let mut selectors = SelectorIndex::new();
    let commit = |a: &mut NodeArena, l: &mut LayoutTree, s: &mut SelectorIndex, ops: &[Op]| {
        txn::apply(
            txn::Target {
                arena: a,
                layout: l,
                selectors: s,
            },
            ops,
            0,
            0,
            0,
        )
    };
    commit(
        &mut arena,
        &mut layout,
        &mut selectors,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
        ],
    )
    .unwrap();
    let a = arena.slot_of(1).unwrap();
    let b = arena.slot_of(2).unwrap();
    let old = [arena.paragraph_stamp(a), arena.paragraph_stamp(b)];
    arena.text_serial = u64::MAX - 1;
    let set = |id| Op::SetProp {
        id,
        prop: PropId::Text,
        value: "new".into(),
    };
    assert!(commit(&mut arena, &mut layout, &mut selectors, &[set(1), set(99)]).is_err());
    assert_eq!(old, [arena.paragraph_stamp(a), arena.paragraph_stamp(b)]);
    assert_eq!(arena.text_serial, u64::MAX - 1);
    commit(
        &mut arena,
        &mut layout,
        &mut selectors,
        &[Op::ClearProp {
            id: 1,
            prop: PropId::Text,
        }],
    )
    .unwrap();
    assert_eq!(old, [arena.paragraph_stamp(a), arena.paragraph_stamp(b)]);
    commit(&mut arena, &mut layout, &mut selectors, &[set(1), set(2)]).unwrap();
    for (slot, previous) in [a, b].into_iter().zip(old) {
        assert!(!previous
            .unwrap()
            .same_metrics(&arena.paragraph_stamp(slot).unwrap()));
        assert_eq!(arena.props(slot).str(PropId::Text), Some("new"));
    }
    assert_eq!(arena.text_serial, 1);
}

#[test]
fn current_slot_storage_and_namespace_lifetimes_do_not_retain_history() {
    let mut arena = NodeArena::new();
    let slot = arena.alloc(1, NodeType::Text).unwrap();
    let old = arena.paragraph_stamp(slot).unwrap();
    let weak = Arc::downgrade(&arena.text_domain.0);
    let fork = arena.clone();
    assert!(!old.same_metrics(&fork.paragraph_stamp(slot).unwrap()));
    arena.reset();
    assert!(
        weak.upgrade().is_some(),
        "the caller still holds a payload-free stamp"
    );
    drop(old);
    assert!(
        weak.upgrade().is_none(),
        "neither reset nor a live public fork retains the retired namespace"
    );
    drop(fork);
    for _ in 0..500 {
        let slot = arena.alloc(1, NodeType::Text).unwrap();
        arena.free_slot(slot);
        assert_eq!(arena.text_revisions[slot as usize], Default::default());
    }
    assert_eq!(arena.text_revisions.len(), arena.slot_count());
    assert_eq!(
        arena.slot_count(),
        1,
        "high-water storage, not allocation history"
    );
    let slot = arena.alloc(1, NodeType::Text).unwrap();
    let held = arena.paragraph_stamp(slot).unwrap();
    let weak = Arc::downgrade(&arena.text_domain.0);
    drop(arena);
    assert!(weak.upgrade().is_some());
    drop(held);
    assert!(weak.upgrade().is_none());
}

#[test]
fn node_generation_wrap_does_not_reuse_a_paragraph_proof() {
    let mut arena = NodeArena::new();
    let slot = arena.alloc(1, NodeType::Text).unwrap();
    let old = arena.paragraph_stamp(slot).unwrap();
    arena.free_slot(slot);
    arena.generations[slot as usize] = u32::MAX;
    let reused = arena.alloc(1, NodeType::Text).unwrap();
    let new = arena.paragraph_stamp(reused).unwrap();
    assert_eq!(
        old.owner(),
        new.owner(),
        "exercise existing u32 generation wrap"
    );
    assert!(
        !old.same_metrics(&new),
        "allocation serial must still differ"
    );
}
