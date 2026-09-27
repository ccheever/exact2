use crate::{
    Frame, Kernel, MonospaceMeasurer, NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue,
};

fn kernel(ops: &[Op]) -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    k.apply(0, 1, ops).unwrap();
    k
}

fn create(ids: impl IntoIterator<Item = u32>, node_type: NodeType) -> Vec<Op> {
    ids.into_iter()
        .map(|id| Op::CreateView { id, node_type })
        .collect()
}

fn children(id: u32, children: &[u32]) -> Op {
    Op::SetChildren {
        id,
        children: children.to_vec(),
    }
}

fn tall(id: u32, height: f64) -> Op {
    let mut patch = StyleProps::default();
    patch
        .set_dynamic(StyleId::Height, &StyleValue::Number(height))
        .unwrap();
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

fn prop(id: u32, prop: PropId, value: &str) -> Op {
    Op::SetProp {
        id,
        prop,
        value: value.into(),
    }
}

/// Child-list work and attached engine removals during `f`.
fn work(f: impl FnOnce()) -> (usize, usize) {
    super::CHILD_WORK.with(|w| w.set((0, 0)));
    f();
    super::CHILD_WORK.with(|w| w.get())
}

/// Frames of `ids` after one layout, and whether the engine holds exactly
/// the live nodes.
fn laid_out(k: &mut Kernel, ids: &[u32]) -> (Vec<(u32, Frame)>, bool) {
    k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    let frames = ids
        .iter()
        .map(|&id| (id, k.node(id).unwrap().frame))
        .collect();
    (frames, k.engine_nodes() == k.live_count())
}

/// Moving N children in one op and destroying N children one op each
/// once cost O(N²): every moved or destroyed child cloned and filtered
/// its siblings in validation, then pruned and re-sent them to the
/// engine (8,000 moves: 685 ms; 8,000 destroys: 802 ms, release). Each
/// old parent is now pruned once per op or run of destroys.
#[test]
fn moving_or_destroying_many_children_is_linear_in_them() {
    for n in [1_000u32, 8_000] {
        let kids: Vec<u32> = (10..10 + n).collect();
        let mut ops = create([1, 2, 3], NodeType::View);
        ops.extend(create(kids.iter().copied(), NodeType::View));
        for &id in &kids {
            ops.push(tall(id, 1.0));
            ops.push(prop(id, PropId::TestId, "row"));
        }
        ops.extend([
            children(2, &kids),
            children(1, &[2, 3]),
            Op::AttachRoot { id: 1 },
        ]);
        let mut k = kernel(&ops);
        let (entries, attached) = work(|| {
            k.apply(0, 2, &[children(3, &kids)]).unwrap();
        });
        assert!(entries <= 4 * n as usize, "moving {n}: {entries} entries");
        assert_eq!(attached, 0);
        assert!(k.node(2).unwrap().children().is_empty());
        assert_eq!(k.node(3).unwrap().children(), kids);
        let (frames, engine_matches) = laid_out(&mut k, &[3]);
        assert_eq!(frames[0].1.height, n as f32);
        assert!(engine_matches);

        let destroys: Vec<Op> = kids.iter().map(|&id| Op::DestroyView { id }).collect();
        let (entries, attached) = work(|| {
            let receipt = k.apply(0, 3, &destroys).unwrap();
            assert_eq!(receipt.destroyed.len(), n as usize);
        });
        assert!(
            entries <= 4 * n as usize,
            "destroying {n}: {entries} entries"
        );
        assert_eq!(attached, 0);
        assert!(k.node(3).unwrap().children().is_empty());
        assert!(k.find_by_test_id("row").is_empty());
        let (frames, engine_matches) = laid_out(&mut k, &[3]);
        assert_eq!(frames[0].1.height, 0.0);
        assert!(engine_matches);
        assert_eq!(k.live_count(), 3);
    }
}

/// The batched detach against what one op at a time built: destroys
/// that later ops read, a parent destroyed after its children, a slot
/// reused within the batch, moves from several parents, inline runs.
#[test]
fn batched_detaches_leave_the_tree_a_fresh_build_has() {
    let base = || {
        let mut ops = create([1, 2, 3, 4], NodeType::View);
        ops.extend(create(10..15, NodeType::View));
        ops.extend((10..15).map(|id| tall(id, id as f64)));
        ops.extend([
            prop(12, PropId::TestId, "twelve"),
            children(2, &[10, 11, 12]),
            children(3, &[13, 14]),
            children(1, &[2, 3, 4]),
            Op::AttachRoot { id: 1 },
        ]);
        ops
    };
    let ids = [1, 2, 3, 4];
    let cases: Vec<(&str, Vec<Op>, Vec<Op>)> = vec![
        (
            "children then their parent",
            vec![
                Op::DestroyView { id: 10 },
                Op::DestroyView { id: 11 },
                Op::DestroyView { id: 2 },
            ],
            vec![Op::DestroyView { id: 2 }],
        ),
        (
            "a destroy then a reorder of the survivors",
            vec![Op::DestroyView { id: 10 }, children(2, &[12, 11])],
            vec![Op::DestroyView { id: 10 }, children(2, &[12, 11])],
        ),
        (
            "a destroyed id made again in the same batch",
            vec![
                Op::DestroyView { id: 10 },
                Op::DestroyView { id: 11 },
                Op::CreateView {
                    id: 10,
                    node_type: NodeType::View,
                },
                tall(10, 7.0),
                children(2, &[10, 12]),
            ],
            vec![
                Op::DestroyView { id: 11 },
                tall(10, 7.0),
                children(2, &[10, 12]),
            ],
        ),
        (
            "moves from two parents at once",
            vec![children(4, &[11, 13])],
            vec![
                children(2, &[10, 12]),
                children(3, &[14]),
                children(4, &[11, 13]),
            ],
        ),
        (
            "a destroy, a move, a destroy",
            vec![
                Op::DestroyView { id: 13 },
                children(4, &[12]),
                Op::DestroyView { id: 10 },
            ],
            vec![
                Op::DestroyView { id: 10 },
                Op::DestroyView { id: 13 },
                children(4, &[12]),
            ],
        ),
    ];
    for (name, batch, one_by_one) in cases {
        let mut batched = kernel(&base());
        batched.apply(0, 2, &batch).unwrap();
        let mut expected = kernel(&base());
        for op in &one_by_one {
            expected.apply(0, 2, std::slice::from_ref(op)).unwrap();
        }
        for id in ids {
            if let (Some(a), Some(b)) = (batched.node(id), expected.node(id)) {
                assert_eq!(a.children(), b.children(), "{name}: children of {id}");
            }
        }
        let (a, engine_matches) = laid_out(&mut batched, &[1]);
        let (b, _) = laid_out(&mut expected, &[1]);
        assert_eq!(a, b, "{name}");
        assert!(engine_matches, "{name}: engine nodes");
        assert_eq!(batched.live_count(), expected.live_count(), "{name}");
        assert_eq!(
            batched.find_by_test_id("twelve").len(),
            expected.find_by_test_id("twelve").len(),
            "{name}"
        );
    }
    // Inline runs destroyed one op each re-measure their paragraph.
    let mut ops = create([1], NodeType::View);
    ops.extend(create(5..9, NodeType::Text));
    ops.extend([
        prop(6, PropId::Text, "aaaa"),
        prop(7, PropId::Text, "bb"),
        prop(8, PropId::Text, "c"),
        children(5, &[6, 7, 8]),
        children(1, &[5]),
        Op::AttachRoot { id: 1 },
    ]);
    let mut k = kernel(&ops);
    let (before, _) = laid_out(&mut k, &[5]);
    k.apply(
        0,
        2,
        &[Op::DestroyView { id: 6 }, Op::DestroyView { id: 8 }],
    )
    .unwrap();
    let (after, engine_matches) = laid_out(&mut k, &[5]);
    assert_eq!(k.node(5).unwrap().children(), vec![7]);
    assert!(after[0].1.width < before[0].1.width || before[0].1.width == 400.0);
    assert!(engine_matches);
}

/// A chain of `levels` nodes below root 1 (ids 2..): each the only child
/// of the one before, linked top-down or bottom-up.
fn chain(levels: u32, node_type: NodeType, bottom_up: bool) -> Vec<Op> {
    let mut ops = create([1], NodeType::View);
    ops.extend(create(2..levels + 2, node_type));
    let mut links: Vec<Op> = (1..levels + 1).map(|id| children(id, &[id + 1])).collect();
    if bottom_up {
        links.reverse();
    }
    ops.extend(links);
    ops.push(Op::AttachRoot { id: 1 });
    ops
}

fn too_deep(k: &mut Kernel, ops: &[Op]) -> bool {
    let before = k.export(None).unwrap();
    let refused = matches!(
        k.apply(0, 9, ops),
        Err(crate::KernelError::Apply(super::ApplyError::TooDeep { depth, .. }))
            if depth == super::MAX_DEPTH + 1
    );
    refused && k.export(None).unwrap() == before
}

/// Taffy recurses once per level, so a 20,000-deep tree overflowed the
/// stack and aborted the process. Past `MAX_DEPTH` a batch is refused
/// whole, however it builds the tree; at the bound every layout mode and
/// nested inline runs lay out (on a debug build's larger frames, so on a
/// roomy thread here).
#[test]
fn trees_past_the_depth_bound_are_refused_whole() {
    let max = super::MAX_DEPTH;
    for node_type in [NodeType::View, NodeType::Text] {
        for bottom_up in [false, true] {
            let mut k = kernel(&[]);
            assert!(too_deep(&mut k, &chain(max + 1, node_type, bottom_up)));
            assert_eq!(k.live_count(), 0);
            k.apply(0, 1, &chain(max, node_type, bottom_up)).unwrap();
        }
    }
    // Onto an existing tree: one more level under the deepest node.
    let mut k = kernel(&chain(max, NodeType::View, false));
    let deeper = [
        Op::CreateView {
            id: 900,
            node_type: NodeType::View,
        },
        children(max + 1, &[900]),
    ];
    assert!(too_deep(&mut k, &deeper));
    // Moving a subtree deeper: 10 levels (ids 500..=509) under root 1.
    let mut ops = create(500..510, NodeType::View);
    ops.extend((500..509).map(|id| children(id, &[id + 1])));
    ops.push(children(1, &[2, 500]));
    k.apply(0, 2, &ops).unwrap();
    // Under the node at depth max - 9 its last level would be max + 1;
    // one level up it fits exactly.
    let at = |depth: u32| depth + 1;
    assert!(too_deep(
        &mut k,
        &[children(at(max - 9), &[at(max - 8), 500])]
    ));
    k.apply(0, 3, &[children(at(max - 10), &[at(max - 9), 500])])
        .unwrap();
    // Reordering under the deepest parent moves nothing deeper.
    k.apply(0, 4, &[children(at(max - 10), &[500, at(max - 9)])])
        .unwrap();

    for display in ["block", "flex", "grid", "inline runs"] {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let text = display == "inline runs";
                let mut ops = chain(
                    max,
                    if text { NodeType::Text } else { NodeType::View },
                    false,
                );
                ops.push(prop(max + 1, PropId::Text, "leaf"));
                if !text {
                    for id in 1..max + 2 {
                        let mut patch = StyleProps::default();
                        patch
                            .set_dynamic(StyleId::Display, &StyleValue::Text(display.into()))
                            .unwrap();
                        ops.push(Op::SetStyle {
                            id,
                            patch: Box::new(patch),
                        });
                    }
                }
                if !text {
                    ops.push(tall(max + 1, 10.0));
                }
                let mut k = kernel(&ops);
                k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
                let height = k.node(1).unwrap().frame.height;
                assert!(
                    if text { height > 0.0 } else { height == 10.0 },
                    "{display}: {height}"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
