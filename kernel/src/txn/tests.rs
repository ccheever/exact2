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

/// `color` on `id`, written by its author or implied by the compiler.
fn colour(id: u32, value: &str, written: bool) -> Op {
    let mut patch = StyleProps::default();
    patch
        .set_dynamic(StyleId::TextColor, &StyleValue::Text(value.into()))
        .unwrap();
    if written {
        patch.written.set(StyleId::TextColor);
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

/// LLP 1115.000 D2: "written" is a fact each row carries. A patch says it
/// for each row it sets, the node keeps it beside the row, and a child that
/// only inherits the value has no row and so no bit.
#[test]
fn a_node_keeps_which_of_its_rows_its_author_wrote() {
    let mut ops = create([1, 2, 3], NodeType::View);
    ops.extend([children(1, &[2, 3]), Op::AttachRoot { id: 1 }]);
    ops.extend([colour(1, "#111111", true), colour(2, "#111111", false)]);
    let mut k = kernel(&ops);
    let wrote = |k: &Kernel, id: u32| k.node(id).unwrap().style.written;
    let row = crate::StyleMask::of(StyleId::TextColor);
    assert_eq!(wrote(&k, 1), row, "the author's row");
    assert!(
        wrote(&k, 2).is_empty(),
        "the compiler's row: set, not written"
    );
    assert!(k.node(2).unwrap().style.mask.has(StyleId::TextColor));
    // Inherited: the value arrives, the bit does not.
    let child = k.node(3).unwrap();
    let computed = child.computed_style(crate::StyleMask::INHERITED);
    assert_eq!(computed.text_color, k.node(1).unwrap().style.text_color);
    assert!(computed.written.is_empty() && child.style.written.is_empty());
    // A written value equal to the parent's is still written; the same
    // value with another bit is a change the node's hosts hear of.
    let touched = k.apply(0, 2, &[colour(2, "#111111", true)]).unwrap();
    assert_eq!(wrote(&k, 2), row);
    assert!(touched.touched.contains(&k.node(2).unwrap().key));
    // Another row's patch leaves the bit; the row's own patch replaces it.
    k.apply(0, 3, &[tall(1, 40.0)]).unwrap();
    assert_eq!(wrote(&k, 1), row);
    k.apply(0, 4, &[colour(1, "#222222", false)]).unwrap();
    assert!(wrote(&k, 1).is_empty(), "the winner's bit");
    // A cleared row has no bit.
    k.apply(
        0,
        5,
        &[Op::ClearStyle {
            id: 2,
            mask: row,
            written: false,
        }],
    )
    .unwrap();
    assert!(wrote(&k, 2).is_empty() && !k.node(2).unwrap().style.mask.has(StyleId::TextColor));
}

#[test]
fn explicit_clear_intent_changes_without_an_own_value() {
    let color = StyleId::TextColor;
    let row = crate::StyleMask::of(color);
    let mut ops = create([1, 2, 3], NodeType::View);
    ops.extend([children(1, &[2]), Op::AttachRoot { id: 1 }]);
    ops.push(colour(1, "#123456", true));
    let mut k = kernel(&ops);
    let clear = |id| {
        let mut patch = StyleProps::default();
        patch.written = row;
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    };
    let initial = k.node(3).unwrap().text_color();
    for id in [2, 3] {
        let before = k.node(id).unwrap().text_color();
        let receipt = k.apply(0, 2, &[clear(id)]).unwrap();
        let node = k.node(id).unwrap();
        assert!(!node.style.mask.has(color));
        assert!(node.style.written.has(color));
        assert_eq!(node.text_color(), before);
        assert!(
            receipt.touched.contains(&node.key),
            "intent is observable even at the same color"
        );
        assert!(k.apply(0, 3, &[clear(id)]).unwrap().touched.is_empty());
    }
    k.apply(0, 3, &[tall(2, 40.0)]).unwrap();
    assert!(!k.node(2).unwrap().style.mask.has(color));
    assert!(k.node(2).unwrap().style.written.has(color));
    k.apply(0, 4, &[colour(1, "#654321", true)]).unwrap();
    assert_eq!(
        k.node(2).unwrap().text_color(),
        k.node(1).unwrap().text_color()
    );
    k.apply(0, 5, &[children(1, &[])]).unwrap();
    assert_eq!(
        k.node(2).unwrap().text_color(),
        initial,
        "detaching does not freeze inherited ink"
    );
    k.apply(0, 6, &[children(1, &[2])]).unwrap();
    assert_eq!(
        k.node(2).unwrap().text_color(),
        k.node(1).unwrap().text_color()
    );
    k.apply(0, 7, &[colour(2, "#654321", true)]).unwrap();
    assert!(k.node(2).unwrap().style.mask.has(color));
    k.apply(0, 8, &[clear(2)]).unwrap();
    assert!(!k.node(2).unwrap().style.mask.has(color));
    assert!(k.node(2).unwrap().style.written.has(color));
    let removed = k
        .apply(
            0,
            9,
            &[Op::ClearStyle {
                id: 2,
                mask: row,
                written: false,
            }],
        )
        .unwrap();
    assert!(removed.touched.contains(&k.node(2).unwrap().key));
    assert!(!k.node(2).unwrap().style.written.has(color));
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
/// the live layout boxes (inline runs belong to their paragraph).
fn laid_out(k: &mut Kernel, ids: &[u32]) -> (Vec<(u32, Frame)>, bool) {
    k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    let frames = ids
        .iter()
        .map(|&id| (id, k.node(id).unwrap().frame))
        .collect();
    let boxes = k
        .arena()
        .iter_live()
        .filter(|s| !k.arena().is_inline_run(*s))
        .count();
    (frames, k.engine_nodes() == boxes)
}

#[test]
fn inline_runs_need_no_engine_nodes_and_promote_with_current_style() {
    let mut first = create([1, 5], NodeType::View);
    first.extend(create(2..5, NodeType::Text));
    first.extend([
        prop(3, PropId::Text, "alpha"),
        prop(4, PropId::Text, " beta"),
        children(3, &[4]),
        children(2, &[3]),
        children(1, &[2, 5]),
        Op::AttachRoot { id: 1 },
    ]);
    let batches = [
        first,
        vec![tall(3, 37.0), prop(4, PropId::Text, " changed inline")],
        vec![children(5, &[3])],
        vec![children(2, &[3])],
        vec![children(2, &[])],
        vec![children(5, &[3]), children(2, &[3]), children(5, &[3])],
        vec![
            Op::DestroyView { id: 3 },
            Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            },
            prop(3, PropId::Text, "reused"),
            children(2, &[3]),
        ],
        vec![children(2, &[]), Op::AttachRoot { id: 3 }],
    ];
    for on_demand in [false, true] {
        let mut k = if on_demand {
            Kernel::with_monospace_on_demand()
        } else {
            Kernel::with_monospace()
        };
        for (round, batch) in batches.iter().enumerate() {
            k.apply(0, round as u64, batch).unwrap();
            let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
            let mut replay = Kernel::with_monospace();
            for (i, ops) in batches[..=round].iter().enumerate() {
                replay.apply(0, i as u64, ops).unwrap();
            }
            for root in k.roots() {
                for tree in [&mut k, &mut rebuilt, &mut replay] {
                    tree.compute_layout(root, Offer::definite(400.0, 800.0))
                        .unwrap();
                }
            }
            for slot in k.arena().iter_live() {
                let arena = k.arena();
                assert_eq!(arena.taffy(slot).is_none(), arena.is_inline_run(slot));
                let id = arena.local_id(slot);
                let frame = k.node(id).unwrap().frame;
                assert_eq!(
                    frame,
                    rebuilt.node(id).unwrap().frame,
                    "round {round}: rebuild {id}"
                );
                assert_eq!(
                    frame,
                    replay.node(id).unwrap().frame,
                    "round {round}: replay {id}"
                );
            }
            if round == 0 {
                assert_eq!(k.engine_nodes(), 3);
                assert_eq!(k.live_count(), 5);
            }
            if round == 2 || round == 5 {
                assert_eq!(k.node(3).unwrap().frame.height, 37.0);
                assert_eq!(k.node(4).unwrap().frame, Frame::default());
            }
        }
    }
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

#[test]
fn continuation_rejects_misuse_and_never_publishes_a_failed_commit() {
    use crate::{ApplyError, KernelError};
    let mut k = kernel(&create([1], NodeType::View));
    let (epoch, history) = (k.epoch(), k.receipts().count());
    assert!(matches!(
        k.continue_document_commit(&[]),
        Err(KernelError::DocumentCommit(_))
    ));
    assert!(matches!(
        k.finish_document_commit(),
        Err(KernelError::DocumentCommit(_))
    ));
    let invalid = [tall(1, 20.0), Op::DestroyView { id: 99 }];
    assert!(matches!(
        k.begin_document_commit(0, 2, &invalid, None),
        Err(KernelError::Apply(_))
    ));
    assert_eq!(k.node(1).unwrap().style.height, crate::Dimension::Auto);
    // Rejected initial validation did not reserve a continuation.
    k.begin_document_commit(0, 2, &[tall(1, 30.0)], None)
        .unwrap();
    assert!(matches!(
        k.begin_document_commit(0, 3, &[], None),
        Err(KernelError::DocumentCommit(_))
    ));
    assert!(matches!(
        k.apply(0, 3, &[]),
        Err(KernelError::DocumentCommit(_))
    ));
    assert!(matches!(
        k.apply_document(0, 3, &[], None),
        Err(KernelError::DocumentCommit(_))
    ));
    assert!(matches!(
        k.continue_document_commit(&invalid),
        Err(KernelError::Apply(ApplyError::UnknownView { .. }))
    ));
    assert_eq!(
        k.node(1).unwrap().style.height,
        crate::Dimension::Points(30.0)
    );
    assert_eq!((k.epoch(), k.receipts().count()), (epoch, history));
    assert!(matches!(
        k.finish_document_commit(),
        Err(KernelError::DocumentCommit(_))
    ));
    assert!(matches!(
        k.continue_document_commit(&[]),
        Err(KernelError::DocumentCommit(_))
    ));
    assert!(matches!(
        k.apply(0, 3, &[]),
        Err(KernelError::DocumentCommit(_))
    ));
    assert_eq!((k.epoch(), k.receipts().count()), (epoch, history));
    k.reset();
    k.apply(0, 3, &create([1], NodeType::View)).unwrap();
}

#[test]
fn continuation_preserves_detached_and_unchanged_commit_behavior() {
    let mut detached = Kernel::detached();
    let invalid = [Op::DestroyView { id: 99 }];
    let expected = detached.apply_document(7, 2, &invalid, None).unwrap();
    detached
        .begin_document_commit(7, 2, &invalid, None)
        .unwrap();
    detached.continue_document_commit(&invalid).unwrap();
    assert!(detached.apply(0, 3, &[]).is_err());
    assert_eq!(detached.finish_document_commit().unwrap(), expected);
    assert_eq!(detached.live_count(), 0);
    assert_eq!(detached.receipts().count(), 0);

    let mut k = kernel(&create([1], NodeType::View));
    let (epoch, history) = (k.epoch(), k.receipts().count());
    k.begin_document_commit(7, 2, &[], None).unwrap();
    k.continue_document_commit(&[]).unwrap();
    let receipt = k.finish_document_commit().unwrap();
    assert_eq!(
        receipt,
        crate::CommitReceipt {
            batch: 2,
            root_id: 7,
            epoch,
            ..Default::default()
        }
    );
    assert_eq!((k.epoch(), k.receipts().count()), (epoch, history + 1));
}

#[test]
fn continuation_filters_and_deduplicates_final_live_display_keys() {
    let display = |id, value: &str| {
        let mut patch = StyleProps::default();
        patch
            .set_dynamic(StyleId::Display, &StyleValue::Text(value.into()))
            .unwrap();
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    };
    let mut k = kernel(&create([1, 2], NodeType::View));
    let old = k.node(2).unwrap().key;
    k.begin_document_commit(0, 2, &[display(1, "none"), display(2, "none")], None)
        .unwrap();
    k.continue_document_commit(&[display(1, "flex"), Op::DestroyView { id: 2 }])
        .unwrap();
    k.continue_document_commit(&create([2], NodeType::View))
        .unwrap();
    k.continue_document_commit(&[display(2, "none")]).unwrap();
    let fresh = k.node(2).unwrap().key;
    assert_ne!(old, fresh);
    let receipt = k.finish_document_commit().unwrap();
    assert_eq!(receipt.display_changed, vec![k.node(1).unwrap().key, fresh]);
    assert_eq!(receipt.created, vec![fresh]);
    assert_eq!(receipt.destroyed, vec![old]);
    assert_eq!(receipt.touched, vec![k.node(1).unwrap().key]);
}

#[test]
fn continuation_resynchronizes_text_created_in_an_earlier_phase() {
    for on_demand in [false, true] {
        for promote in [true, false] {
            let mut k = if on_demand {
                Kernel::with_monospace_on_demand()
            } else {
                Kernel::with_monospace()
            };
            let mut initial = create([1], NodeType::View);
            initial.extend(create([2], NodeType::Text));
            initial.extend([children(1, &[2]), Op::AttachRoot { id: 1 }]);
            k.apply(0, 1, &initial).unwrap();
            let mut height = StyleProps::default();
            height
                .set_dynamic(StyleId::Height, &StyleValue::Text("2em".into()))
                .unwrap();
            let mut first = create([3], NodeType::Text);
            first.extend([
                Op::SetStyle {
                    id: 3,
                    patch: Box::new(height),
                },
                prop(3, PropId::Text, "alpha"),
            ]);
            first.push(if promote {
                children(2, &[3])
            } else {
                children(1, &[2, 3])
            });
            k.begin_document_commit(0, 2, &first, None).unwrap();
            // Move directly: the source parent's children are pruned by the op.
            // Lifecycle creation identity persists, but text ownership must sync
            // again when a later phase promotes or demotes this same allocation.
            let mut font = StyleProps::default();
            font.set_dynamic(StyleId::FontSize, &StyleValue::Number(20.0))
                .unwrap();
            k.continue_document_commit(&[
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(font),
                },
                if promote {
                    children(1, &[2, 3])
                } else {
                    children(2, &[3])
                },
            ])
            .unwrap();
            let receipt = k.finish_document_commit().unwrap();
            let key = k.node(3).unwrap().key;
            assert_eq!(receipt.created, vec![key]);
            assert_eq!(k.receipts().last(), Some(receipt));
            k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
            let slot = k.arena().slot_of(3).unwrap();
            assert_eq!(
                k.arena().taffy(slot).is_some(),
                promote,
                "on_demand={on_demand}, promote={promote}: final text ownership"
            );
            let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
            rebuilt
                .compute_layout(1, Offer::definite(400.0, 800.0))
                .unwrap();
            assert_eq!(k.node(3).unwrap().frame, rebuilt.node(3).unwrap().frame);
            assert_eq!(
                k.node(3).unwrap().style.height,
                crate::Dimension::Points(40.0)
            );
            if promote {
                assert_eq!(k.node(3).unwrap().frame.height, 40.0);
            }
        }
    }
}

/// @ref LLP 1121 D8 — a node's looks hold the look rows only, set or
/// cleared (a written-only `width` is refused, as a set one is), each state
/// once; they go with the node.
#[test]
fn looks_take_the_look_rows_only_and_go_with_the_node() {
    use crate::{LOOK_ACTIVE, LOOK_HOVER};
    let mut k = kernel(&create([1], NodeType::View));
    let mut fill = StyleProps::default();
    fill.set_dynamic(
        StyleId::BackgroundColor,
        &StyleValue::Text("#ff0000".into()),
    )
    .unwrap();
    let mut width = StyleProps::default();
    width.written.set(StyleId::Width);
    let mut opacity = StyleProps::default();
    opacity.written.set(StyleId::Opacity);
    for (looks, ok) in [
        (vec![(LOOK_HOVER, width)], false),
        (
            vec![(LOOK_HOVER, fill.clone()), (LOOK_ACTIVE, fill.clone())],
            false,
        ),
        (
            vec![(LOOK_ACTIVE, fill.clone()), (LOOK_HOVER, opacity)],
            true,
        ),
    ] {
        let applied = k.apply(1, 2, &[Op::SetLooks { id: 1, looks }]).is_ok();
        assert_eq!(applied, ok);
    }
    assert_eq!(k.node(1).unwrap().looks().len(), 2);
    k.apply(2, 3, &[Op::DestroyView { id: 1 }]).unwrap();
    assert!(k.node(1).is_none());
}
