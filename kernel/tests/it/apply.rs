//! Behavior of the one write path: structured apply, receipts, rejection
//! atomicity, identity, layout publication.

use exact_kernel::{
    ApplyError, Dimension, Display, FlexDirection, Kernel, KernelError, NodeType, Offer, Op,
    PropId, PropValue, StyleId, StyleProps,
};

fn style(f: impl FnOnce(&mut StyleProps)) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    f(&mut s);
    Box::new(s)
}

fn size(w: f32, h: f32) -> Box<StyleProps> {
    style(|s| {
        s.width = Dimension::Points(w);
        s.mask.set(StyleId::Width);
        s.height = Dimension::Points(h);
        s.mask.set(StyleId::Height);
    })
}

fn height(h: f32) -> Box<StyleProps> {
    style(|s| {
        s.height = Dimension::Points(h);
        s.mask.set(StyleId::Height);
    })
}

/// root(1) 200×300 → a(2) h50, b(3) h70 → b holds c(4) h20.
fn build() -> Kernel {
    let mut k = Kernel::with_monospace();
    k.apply(
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
                id: 1,
                patch: size(200.0, 300.0),
            },
            Op::SetStyle {
                id: 2,
                patch: height(50.0),
            },
            Op::SetStyle {
                id: 3,
                patch: height(70.0),
            },
            Op::SetStyle {
                id: 4,
                patch: height(20.0),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::SetChildren {
                id: 3,
                children: vec![4],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

fn frame(k: &Kernel, id: u32) -> (f32, f32, f32, f32) {
    let f = k.node(id).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn column_layout_stacks_children_and_stretches_width() {
    let mut k = build();
    let receipt = k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(receipt.changed.len(), 4, "first layout reports every node");
    assert_eq!(frame(&k, 1), (0.0, 0.0, 200.0, 300.0));
    assert_eq!(frame(&k, 2), (0.0, 0.0, 200.0, 50.0));
    assert_eq!(frame(&k, 3), (0.0, 50.0, 200.0, 70.0));
    assert_eq!(
        frame(&k, 4),
        (0.0, 50.0, 200.0, 20.0),
        "frames are absolute"
    );
}

#[test]
fn fractional_frames_and_updates_survive_publication_and_rehydration() {
    // Browser DOM boxes retain these binary-exact CSS pixel fractions. A
    // native point-grid round lost the inset and turned a half-point edit
    // into a whole-point move of the following sibling.
    let mut k = build();
    let mut root = size(200.5, 400.75);
    root.display = Display::Flex;
    root.mask.set(StyleId::Display);
    root.flex_direction = FlexDirection::Column;
    root.mask.set(StyleId::FlexDirection);
    root.padding_left = Dimension::Points(0.25);
    root.mask.set(StyleId::PaddingLeft);
    root.padding_top = Dimension::Points(80.125);
    root.mask.set(StyleId::PaddingTop);
    root.row_gap = 0.375;
    root.mask.set(StyleId::RowGap);
    let mut positioned = size(33.25, 14.75);
    positioned.position_type = exact_kernel::PositionType::Absolute;
    positioned.mask.set(StyleId::PositionType);
    positioned.left = Dimension::Points(0.375);
    positioned.mask.set(StyleId::Left);
    positioned.top = Dimension::Points(0.125);
    positioned.mask.set(StyleId::Top);
    k.apply(
        0,
        2,
        &[
            Op::SetStyle { id: 1, patch: root },
            Op::SetStyle {
                id: 2,
                patch: {
                    // The absolute box's containing block.
                    let mut holder = size(120.5, 86.625);
                    holder.position_type = exact_kernel::PositionType::Relative;
                    holder.mask.set(StyleId::PositionType);
                    holder
                },
            },
            Op::SetStyle {
                id: 3,
                patch: size(120.5, 86.625),
            },
            Op::SetStyle {
                id: 4,
                patch: positioned,
            },
            Op::SetChildren {
                id: 3,
                children: vec![],
            },
            Op::SetChildren {
                id: 2,
                children: vec![4],
            },
        ],
    )
    .unwrap();
    let offer = Offer::definite(402.0, 874.0);
    k.compute_layout(1, offer).unwrap();
    assert_eq!(frame(&k, 2), (0.25, 80.125, 120.5, 86.625));
    assert_eq!(frame(&k, 3), (0.25, 167.125, 120.5, 86.625));
    assert_eq!(frame(&k, 4), (0.625, 80.25, 33.25, 14.75));
    k.apply(
        0,
        3,
        &[Op::SetStyle {
            id: 2,
            patch: height(87.125),
        }],
    )
    .unwrap();
    let receipt = k.compute_layout(1, offer).unwrap();
    assert!(!receipt.changed.is_empty());
    assert_eq!(frame(&k, 3), (0.25, 167.625, 120.5, 86.625));
    assert!(k.compute_layout(1, offer).unwrap().changed.is_empty());
    let mut fresh = k.rehydrate(Box::new(exact_kernel::MonospaceMeasurer::default()));
    fresh.compute_layout(1, offer).unwrap();
    for id in 1..=4 {
        assert_eq!(frame(&fresh, id), frame(&k, id));
    }
}

#[test]
fn layout_receipt_names_only_the_nodes_that_moved() {
    let mut k = build();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 2,
            patch: height(60.0),
        }],
    )
    .unwrap();
    let receipt = k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    let changed: Vec<u32> = receipt
        .changed
        .iter()
        .map(|key| k.node_by_key(*key).unwrap().id)
        .collect();
    assert_eq!(
        changed,
        vec![2, 3, 4],
        "a grew; b and c shifted; the root did not move"
    );
    assert_eq!(frame(&k, 3), (0.0, 60.0, 200.0, 70.0));
    let receipt = k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert!(
        receipt.changed.is_empty(),
        "a second pass with no mutation changes nothing"
    );
}

#[test]
fn row_direction_and_flex_grow() {
    let mut k = build();
    k.apply(
        0,
        2,
        &[
            Op::SetStyle {
                id: 1,
                patch: style(|s| {
                    s.display = Display::Flex;
                    s.mask.set(StyleId::Display);
                    s.flex_direction = FlexDirection::Row;
                    s.mask.set(StyleId::FlexDirection);
                }),
            },
            Op::SetStyle {
                id: 2,
                patch: style(|s| {
                    s.flex_grow = 1.0;
                    s.mask.set(StyleId::FlexGrow);
                }),
            },
            Op::SetStyle {
                id: 3,
                patch: style(|s| {
                    s.width = Dimension::Percent(25.0);
                    s.mask.set(StyleId::Width);
                }),
            },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(
        frame(&k, 3),
        (150.0, 0.0, 50.0, 70.0),
        "25% of 200 is 50, placed after the grown sibling"
    );
    assert_eq!(frame(&k, 2), (0.0, 0.0, 150.0, 50.0));
}

#[test]
fn every_rejection_class_leaves_the_kernel_untouched() {
    let base = build();
    let before = base.export(None).unwrap();
    let before_epoch = base.epoch();
    let before_receipts = base.receipts().count();

    let cases: Vec<(Vec<Op>, ApplyError)> = vec![
        (
            vec![
                Op::SetProp {
                    id: 2,
                    prop: PropId::TestId,
                    value: "x".into(),
                },
                Op::SetProp {
                    id: 99,
                    prop: PropId::TestId,
                    value: "y".into(),
                },
            ],
            ApplyError::UnknownView {
                op_index: 1,
                id: 99,
            },
        ),
        (
            vec![Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            }],
            ApplyError::TypeMismatch {
                op_index: 0,
                id: 2,
                existing: NodeType::View,
                requested: NodeType::Text,
            },
        ),
        (
            vec![
                Op::DestroyView { id: 3 },
                Op::SetStyle {
                    id: 4,
                    patch: height(1.0),
                },
            ],
            ApplyError::DestroyedInBatch { op_index: 1, id: 4 },
        ),
        (
            vec![Op::SetChildren {
                id: 1,
                children: vec![2, 2],
            }],
            ApplyError::DuplicateChild {
                op_index: 0,
                parent: 1,
                child: 2,
            },
        ),
        (
            vec![Op::SetChildren {
                id: 2,
                children: vec![2],
            }],
            ApplyError::SelfChild { op_index: 0, id: 2 },
        ),
        (
            vec![Op::SetChildren {
                id: 4,
                children: vec![1],
            }],
            ApplyError::RootAsChild {
                op_index: 0,
                parent: 4,
                child: 1,
            },
        ),
        (
            vec![Op::SetChildren {
                id: 4,
                children: vec![3],
            }],
            ApplyError::Cycle {
                op_index: 0,
                parent: 4,
                child: 3,
            },
        ),
        (
            vec![Op::AttachRoot { id: 4 }],
            ApplyError::RootHasParent { op_index: 0, id: 4 },
        ),
        (
            vec![Op::SetProp {
                id: 2,
                prop: PropId::Disabled,
                value: PropValue::Str("true".into()),
            }],
            ApplyError::PropKindMismatch {
                op_index: 0,
                prop: PropId::Disabled,
                expected: exact_kernel::PropKind::Bool,
                actual: exact_kernel::PropKind::Str,
            },
        ),
        (
            vec![
                Op::CreateView {
                    id: 5,
                    node_type: NodeType::Image,
                },
                Op::SetChildren {
                    id: 5,
                    children: vec![2],
                },
            ],
            ApplyError::LeafCannotHoldChildren {
                op_index: 1,
                id: 5,
                node_type: NodeType::Image,
            },
        ),
        (
            // A chain of 127 views under node 4 (depth 2) would put 226 at
            // depth 129; the op that places it there is refused.
            (100..227)
                .map(|id| Op::CreateView {
                    id,
                    node_type: NodeType::View,
                })
                .chain((99..226).map(|id| Op::SetChildren {
                    id: if id == 99 { 4 } else { id },
                    children: vec![id + 1],
                }))
                .collect(),
            ApplyError::TooDeep {
                op_index: 253,
                id: 226,
                depth: exact_kernel::MAX_DEPTH + 1,
            },
        ),
    ];

    for (ops, expected) in cases {
        let mut k = build();
        let err = k.apply(0, 7, &ops).unwrap_err();
        assert_eq!(err, KernelError::Apply(expected.clone()), "ops: {ops:?}");
        assert_eq!(
            k.export(None).unwrap(),
            before,
            "rejected batch changed the tree: {expected:?}"
        );
        assert_eq!(k.epoch(), before_epoch);
        assert_eq!(
            k.receipts().count(),
            before_receipts,
            "a rejected batch leaves no receipt"
        );
        assert_eq!(k.live_count(), 4);
        // The engine is still healthy: layout runs and matches the untouched tree.
        k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
        assert_eq!(frame(&k, 4), (0.0, 50.0, 200.0, 20.0));
    }
}

#[test]
fn destroy_is_subtree_and_stale_keys_fail_closed() {
    let mut k = build();
    let key_b = k.node(3).unwrap().key;
    let key_c = k.node(4).unwrap().key;
    let receipt = k.apply(0, 2, &[Op::DestroyView { id: 3 }]).unwrap();
    assert_eq!(receipt.destroyed, vec![key_b, key_c]);
    assert!(k.node(3).is_none());
    assert!(k.node(4).is_none());
    assert!(k.node_by_key(key_b).is_none());
    assert!(k.node_by_key(key_c).is_none());
    assert_eq!(k.node(1).unwrap().children(), vec![2]);
    assert_eq!(k.live_count(), 2);

    // Recreating the id mints a new generation; the old key still fails.
    let receipt = k
        .apply(
            0,
            3,
            &[Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            }],
        )
        .unwrap();
    assert_eq!(receipt.created.len(), 1);
    let key_b2 = k.node(3).unwrap().key;
    assert_ne!(key_b2, key_b);
    assert!(k.node_by_key(key_b).is_none());
    assert_eq!(k.node_by_key(key_b2).unwrap().node_type, NodeType::Text);

    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(frame(&k, 2), (0.0, 0.0, 200.0, 50.0));
}

#[test]
fn create_on_a_live_id_of_the_same_type_is_a_no_op() {
    let mut k = build();
    let key = k.node(2).unwrap().key;
    let epoch = k.epoch();
    let receipt = k
        .apply(
            0,
            2,
            &[Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            }],
        )
        .unwrap();
    assert!(receipt.created.is_empty());
    assert!(receipt.touched.is_empty());
    assert_eq!(k.node(2).unwrap().key, key);
    assert_eq!(
        k.epoch(),
        epoch,
        "a batch that changes nothing does not publish an epoch"
    );
}

#[test]
fn set_children_reparents_and_orphans() {
    let mut k = build();
    // Move c from b to a; b is left empty.
    let receipt = k
        .apply(
            0,
            2,
            &[Op::SetChildren {
                id: 2,
                children: vec![4],
            }],
        )
        .unwrap();
    assert_eq!(k.node(4).unwrap().parent, Some(2));
    assert_eq!(k.node(3).unwrap().children(), Vec::<u32>::new());
    assert_eq!(k.node(2).unwrap().children(), vec![4]);
    let touched: Vec<u32> = receipt
        .touched
        .iter()
        .map(|key| k.node_by_key(*key).unwrap().id)
        .collect();
    assert_eq!(
        touched,
        vec![2, 3],
        "both the new and the old parent are touched"
    );
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(frame(&k, 4), (0.0, 0.0, 200.0, 20.0));

    // Orphan c entirely: it stays live but leaves every root's layout.
    k.apply(
        0,
        3,
        &[Op::SetChildren {
            id: 2,
            children: vec![],
        }],
    )
    .unwrap();
    assert!(k.node(4).is_some());
    assert_eq!(k.node(4).unwrap().parent, None);
    assert_eq!(k.rows(Some(1)).unwrap().len(), 3);
}

#[test]
fn test_id_is_a_multimap_in_tree_order() {
    let mut k = build();
    k.apply(
        0,
        2,
        &[
            Op::SetProp {
                id: 4,
                prop: PropId::TestId,
                value: "row".into(),
            },
            Op::SetProp {
                id: 2,
                prop: PropId::TestId,
                value: "row".into(),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::TestId,
                value: "other".into(),
            },
        ],
    )
    .unwrap();
    let hits: Vec<u32> = k
        .find_by_test_id("row")
        .iter()
        .map(|key| k.node_by_key(*key).unwrap().id)
        .collect();
    assert_eq!(hits, vec![2, 4], "structural order, not insertion order");
    k.apply(
        0,
        3,
        &[Op::ClearProp {
            id: 2,
            prop: PropId::TestId,
        }],
    )
    .unwrap();
    assert_eq!(k.find_by_test_id("row").len(), 1);
    k.apply(0, 4, &[Op::DestroyView { id: 3 }]).unwrap();
    assert!(
        k.find_by_test_id("row").is_empty(),
        "destroying b's subtree drops c from the index"
    );
    assert!(k.find_by_test_id("other").is_empty());
}

#[test]
fn receipts_carry_batch_root_and_epoch() {
    let mut k = Kernel::with_monospace();
    assert_eq!(k.epoch(), 0);
    let r1 = k
        .apply(
            9,
            100,
            &[Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            }],
        )
        .unwrap();
    assert_eq!((r1.batch, r1.root_id, r1.epoch), (100, 9, 1));
    assert_eq!(r1.created.len(), 1);
    assert!(r1.layout_invalidated);
    let r2 = k
        .apply(
            9,
            101,
            &[Op::SetProp {
                id: 1,
                prop: PropId::TestId,
                value: "t".into(),
            }],
        )
        .unwrap();
    assert_eq!(r2.epoch, 2);
    assert!(
        !r2.layout_invalidated,
        "a non-measure prop does not invalidate layout"
    );
    assert_eq!(r2.touched, vec![k.node(1).unwrap().key]);
    let kept: Vec<u64> = k.receipts().map(|r| r.batch).collect();
    assert_eq!(kept, vec![100, 101]);
}

#[test]
fn text_measures_wraps_and_relayouts_on_change() {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::SetStyle {
                id: 1,
                patch: size(40.0, 200.0),
            },
            Op::SetStyle {
                id: 2,
                patch: style(|s| {
                    s.font_size = 10.0;
                    s.mask.set(StyleId::FontSize);
                }),
            },
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "hello world".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(
        frame(&k, 2),
        (0.0, 0.0, 40.0, 24.0),
        "two lines of 12pt at 40pt wide"
    );

    let receipt = k
        .apply(
            0,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "hi".into(),
            }],
        )
        .unwrap();
    assert!(receipt.layout_invalidated);
    let layout = k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    let changed: Vec<u32> = layout
        .changed
        .iter()
        .map(|key| k.node_by_key(*key).unwrap().id)
        .collect();
    assert_eq!(changed, vec![2]);
    assert_eq!(frame(&k, 2), (0.0, 0.0, 40.0, 12.0));

    // A text-affecting style row re-measures too.
    k.apply(
        0,
        3,
        &[Op::SetStyle {
            id: 2,
            patch: style(|s| {
                s.font_size = 20.0;
                s.mask.set(StyleId::FontSize);
            }),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(
        frame(&k, 2),
        (0.0, 0.0, 40.0, 24.0),
        "'hi' at 20pt is 24pt wide: one line, 24pt tall"
    );
}

#[test]
fn inline_runs_measure_with_their_parent() {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::Text,
            },
            Op::SetStyle {
                id: 1,
                patch: style(|s| {
                    s.flex_direction = FlexDirection::Row;
                    s.mask.set(StyleId::FlexDirection);
                }),
            },
            Op::SetStyle {
                id: 2,
                patch: style(|s| {
                    s.font_size = 10.0;
                    s.mask.set(StyleId::FontSize);
                }),
            },
            Op::SetStyle {
                id: 3,
                patch: style(|s| {
                    s.font_size = 10.0;
                    s.mask.set(StyleId::FontSize);
                }),
            },
            Op::SetStyle {
                id: 4,
                patch: style(|s| {
                    s.font_size = 20.0;
                    s.mask.set(StyleId::FontSize);
                }),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::Text,
                value: "ab".into(),
            },
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "cd".into(),
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
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    assert_eq!(frame(&k, 2), (0.0, 0.0, 36.0, 24.0));
    assert_eq!(
        k.rows(Some(1)).unwrap().len(),
        4,
        "runs are exported as rows"
    );
    assert_eq!(
        k.rows(Some(1)).unwrap()[2].flags & exact_kernel::export::ROW_INLINE_RUN,
        exact_kernel::export::ROW_INLINE_RUN
    );

    // Editing a run re-measures the owning paragraph.
    k.apply(
        0,
        2,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "cdef".into(),
        }],
    )
    .unwrap();
    let receipt = k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let changed: Vec<u32> = receipt
        .changed
        .iter()
        .map(|key| k.node_by_key(*key).unwrap().id)
        .collect();
    assert_eq!(changed, vec![1, 2]);
    assert_eq!(frame(&k, 2), (0.0, 0.0, 60.0, 24.0));
}

#[test]
fn reset_bumps_incarnation_and_forgets_everything() {
    let mut k = build();
    let key = k.node(1).unwrap().key;
    let mut retained = std::collections::BTreeMap::from([(key, "old host state")]);
    assert_eq!(k.incarnation(), 1);
    k.reset();
    assert_eq!(k.incarnation(), 2);
    assert_eq!(k.live_count(), 0);
    assert!(k.node(1).is_none());
    assert!(k.node_by_key(key).is_none());
    assert!(k.roots().is_empty());
    assert!(k.find_by_test_id("row").is_empty());
    assert_eq!(k.receipts().count(), 0);
    assert!(matches!(
        k.compute_layout(1, Offer::MAX_CONTENT),
        Err(KernelError::Layout(exact_kernel::LayoutError::UnknownView(
            1
        )))
    ));

    k.apply(
        0,
        2,
        &[Op::CreateView {
            id: 99,
            node_type: NodeType::View,
        }],
    )
    .unwrap();
    let replacement = k.node(99).unwrap().key;
    assert_eq!(replacement.index, key.index, "reset reuses the first slot");
    assert_ne!(replacement.generation, key.generation);
    assert!(k.node_by_key(key).is_none());
    assert!(
        !retained.contains_key(&replacement),
        "a retained host cache cannot alias"
    );
    retained.insert(replacement, "new host state");
    assert_eq!(retained.len(), 2);
}

#[test]
fn identical_writes_do_not_touch_nodes_or_advance_the_epoch() {
    let mut k = build();
    k.apply(
        0,
        2,
        &[Op::SetProp {
            id: 2,
            prop: PropId::TestId,
            value: "same".into(),
        }],
    )
    .unwrap();
    let epoch = k.epoch();
    let mut unset = exact_kernel::StyleMask::EMPTY;
    unset.set(StyleId::Width);

    let receipt = k
        .apply(
            0,
            3,
            &[
                Op::SetProp {
                    id: 2,
                    prop: PropId::TestId,
                    value: "same".into(),
                },
                Op::SetStyle {
                    id: 3,
                    patch: height(70.0),
                },
                Op::ClearStyle { id: 2, mask: unset },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
            ],
        )
        .unwrap();

    assert_eq!(k.epoch(), epoch);
    assert_eq!(receipt.epoch, epoch);
    assert!(receipt.created.is_empty());
    assert!(receipt.destroyed.is_empty());
    assert!(receipt.touched.is_empty());
    assert!(!receipt.layout_invalidated);
}

#[test]
fn layout_needs_a_root() {
    let mut k = build();
    assert!(matches!(
        k.compute_layout(2, Offer::MAX_CONTENT),
        Err(KernelError::Layout(exact_kernel::LayoutError::NotARoot(2)))
    ));
}

#[test]
fn clear_style_restores_defaults() {
    let mut k = build();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    let mut mask = exact_kernel::StyleMask::EMPTY;
    mask.set(StyleId::Height);
    let receipt = k.apply(0, 2, &[Op::ClearStyle { id: 3, mask }]).unwrap();
    assert!(receipt.layout_invalidated);
    assert_eq!(k.node(3).unwrap().style.height, Dimension::Auto);
    assert!(!k.node(3).unwrap().style.mask.has(StyleId::Height));
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(
        frame(&k, 3),
        (0.0, 50.0, 200.0, 20.0),
        "b now sizes to its child"
    );
}

#[test]
fn text_color_inherits_through_reparenting_and_cleared_overrides() {
    use exact_kernel::{Color, ColorValue, StyleMask};
    let mut k = build();
    let pair = ColorValue::LightDark(Color(0x112233ff), Color(0xeeddccff));
    let red = ColorValue::Fixed(Color(0xff0000ff));
    let ink = |value| {
        style(|s| {
            s.text_color = value;
            s.mask.set(StyleId::TextColor);
        })
    };
    k.apply(
        0,
        2,
        &[
            Op::SetStyle {
                id: 1,
                patch: ink(pair),
            },
            Op::SetStyle {
                id: 2,
                patch: ink(red),
            },
        ],
    )
    .unwrap();
    assert_eq!(k.node(4).unwrap().text_color(), pair);
    assert!(!k.node(4).unwrap().style.mask.has(StyleId::TextColor));
    k.apply(
        0,
        3,
        &[
            Op::SetChildren {
                id: 3,
                children: vec![],
            },
            Op::SetChildren {
                id: 2,
                children: vec![4],
            },
        ],
    )
    .unwrap();
    assert_eq!(k.node(4).unwrap().text_color(), red);
    let mut mask = StyleMask::EMPTY;
    mask.set(StyleId::TextColor);
    k.apply(0, 4, &[Op::ClearStyle { id: 2, mask }]).unwrap();
    assert_eq!(k.node(4).unwrap().text_color(), pair);
    k.apply(0, 5, &[Op::ClearStyle { id: 1, mask }]).unwrap();
    assert_eq!(
        k.node(4).unwrap().text_color(),
        StyleProps::default().text_color
    );
}

#[test]
fn spelling_hint_inherits_without_losing_authored_values() {
    let mut k = build();
    let hint = |id, value: &str| Op::SetProp {
        id,
        prop: PropId::Spellcheck,
        value: value.into(),
    };
    assert_eq!(k.node(4).unwrap().spellcheck(), None);
    k.apply(0, 2, &[hint(1, "FaLsE"), hint(2, ""), hint(4, " true")])
        .unwrap();
    assert_eq!(k.node(4).unwrap().spellcheck(), Some(false));
    assert_eq!(
        k.node(4).unwrap().props.str(PropId::Spellcheck),
        Some(" true")
    );
    k.apply(
        0,
        3,
        &[
            Op::SetChildren {
                id: 3,
                children: vec![],
            },
            Op::SetChildren {
                id: 2,
                children: vec![4],
            },
        ],
    )
    .unwrap();
    assert_eq!(k.node(4).unwrap().spellcheck(), Some(true));
    k.apply(0, 4, &[hint(4, "false")]).unwrap();
    assert_eq!(k.node(4).unwrap().spellcheck(), Some(false));
    k.apply(
        0,
        5,
        &[Op::ClearProp {
            id: 4,
            prop: PropId::Spellcheck,
        }],
    )
    .unwrap();
    assert_eq!(k.node(4).unwrap().spellcheck(), Some(true));
    k.apply(
        0,
        6,
        &[
            Op::ClearProp {
                id: 2,
                prop: PropId::Spellcheck,
            },
            Op::ClearProp {
                id: 1,
                prop: PropId::Spellcheck,
            },
        ],
    )
    .unwrap();
    assert_eq!(k.node(4).unwrap().spellcheck(), None);
}

#[test]
fn text_rows_inherit_into_runs_and_a_change_touches_only_the_runs_that_follow_it() {
    use exact_kernel::{StyleMask, TextStyle};
    let mut k = Kernel::with_monospace();
    let text = |id, s: &str| Op::SetProp {
        id,
        prop: PropId::Text,
        value: s.into(),
    };
    let font_size = |id, size: f32| Op::SetStyle {
        id,
        patch: style(|s| {
            s.font_size = size;
            s.mask.set(StyleId::FontSize);
        }),
    };
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            // A paragraph with three runs: bare, bold, small.
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 5,
                node_type: NodeType::Text,
            },
            // A second, 14-point paragraph.
            Op::CreateView {
                id: 6,
                node_type: NodeType::Text,
            },
            Op::SetStyle {
                id: 1,
                patch: style(|s| {
                    s.flex_direction = FlexDirection::Row;
                    s.mask.set(StyleId::FlexDirection);
                }),
            },
            Op::SetStyle {
                id: 2,
                patch: style(|s| {
                    s.font_size = 20.0;
                    s.mask.set(StyleId::FontSize);
                    s.line_height = exact_kernel::LineHeight::Length(24.0);
                    s.mask.set(StyleId::LineHeight);
                }),
            },
            Op::SetStyle {
                id: 4,
                patch: style(|s| {
                    s.font_weight = 700;
                    s.mask.set(StyleId::FontWeight);
                }),
            },
            font_size(5, 12.0),
            font_size(6, 14.0),
            text(3, "aa"),
            text(4, "bb"),
            text(5, "cc"),
            Op::SetChildren {
                id: 2,
                children: vec![3, 4, 5],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 6],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    // Computed: the bare run takes its paragraph's size and line height, the
    // bold run keeps its weight and takes the size, the small run keeps its
    // size and takes the line height. Presence stays the author's.
    let initial = TextStyle::from_style(&StyleProps::default());
    assert_eq!(
        k.node(3).unwrap().text_style(),
        TextStyle {
            font_size: 20.0,
            line_height: Some(24.0),
            ..initial
        }
    );
    assert_eq!(k.node(4).unwrap().text_style().font_weight, 700);
    assert_eq!(k.node(4).unwrap().text_style().font_size, 20.0);
    assert_eq!(k.node(5).unwrap().text_style().font_size, 12.0);
    assert_eq!(k.node(5).unwrap().text_style().line_height, Some(24.0));
    assert!(!k.node(3).unwrap().style.mask.has(StyleId::FontSize));
    assert_eq!(k.node(3).unwrap().source_of(StyleId::FontSize), Some(2));
    assert_eq!(k.node(5).unwrap().source_of(StyleId::FontSize), Some(5));
    assert_eq!(k.node(3).unwrap().source_of(StyleId::LetterSpacing), None);
    assert_eq!(
        k.node(3).unwrap().source_of(StyleId::Width),
        None,
        "a box row never inherits"
    );
    // Measured with the computed styles: 0.6 em per glyph, so
    // 2×12 + 2×12 + 2×7.2 = 62.4 wide, in the authored 24-point line box.
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let f = frame(&k, 2);
    assert!(
        (f.2 - 62.4).abs() < 1e-3 && (f.3 - 24.0).abs() < 1e-3,
        "{f:?}"
    );
    // A changed ancestor touches the runs that follow it and none that
    // override it: the receipt names them, no host re-derives per frame.
    let ids = |k: &Kernel, keys: &[exact_kernel::NodeKey]| -> Vec<u32> {
        keys.iter()
            .map(|key| k.node_by_key(*key).unwrap().id)
            .collect()
    };
    let receipt = k.apply(0, 2, &[font_size(2, 24.0)]).unwrap();
    assert_eq!(ids(&k, &receipt.touched), vec![2, 3, 4]);
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    assert!((frame(&k, 2).2 - 72.0).abs() < 1e-3, "{:?}", frame(&k, 2));
    // A reparented run follows its new paragraph.
    let receipt = k
        .apply(
            0,
            3,
            &[
                Op::SetChildren {
                    id: 2,
                    children: vec![4, 5],
                },
                Op::SetChildren {
                    id: 6,
                    children: vec![3],
                },
            ],
        )
        .unwrap();
    assert!(ids(&k, &receipt.touched).contains(&3));
    assert_eq!(k.node(3).unwrap().text_style().font_size, 14.0);
    assert_eq!(k.node(3).unwrap().text_style().line_height, None);
    assert_eq!(k.node(3).unwrap().source_of(StyleId::FontSize), Some(6));
    // Clearing falls back to the initial value, and touches only followers.
    let mut mask = StyleMask::EMPTY;
    mask.set(StyleId::FontSize);
    let receipt = k.apply(0, 4, &[Op::ClearStyle { id: 2, mask }]).unwrap();
    assert_eq!(ids(&k, &receipt.touched), vec![2, 4]);
    assert_eq!(k.node(4).unwrap().text_style().font_size, 16.0);
    assert_eq!(k.node(4).unwrap().source_of(StyleId::FontSize), None);
    // An identical write is no change at all.
    let receipt = k.apply(0, 5, &[font_size(6, 14.0)]).unwrap();
    assert!(receipt.touched.is_empty());
}

#[test]
fn inherited_line_height_resolves_per_font_and_invalidates_through_tree_changes() {
    use exact_kernel::{LineHeight, StyleMask};
    let mut k = Kernel::with_monospace();
    let set = |id, font, lh| Op::SetStyle {
        id,
        patch: style(|s| {
            s.font_size = font;
            s.mask.set(StyleId::FontSize);
            if let Some(lh) = lh {
                s.line_height = lh;
                s.mask.set(StyleId::LineHeight);
            }
        }),
    };
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::AttachRoot { id: 1 },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::View,
            },
            set(1, 16.0, Some(LineHeight::Number(1.5))),
            set(2, 20.0, None),
            set(3, 18.0, Some(LineHeight::Length(24.0))),
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "hello".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
        ],
    )
    .unwrap();
    let check = |k: &mut Kernel, height: f32| {
        k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
        assert!((frame(k, 2).3 - height).abs() < 0.02, "{:?}", frame(k, 2));
    };
    check(&mut k, 30.0);
    k.apply(0, 2, &[set(1, 30.0, None), set(2, 24.0, None)])
        .unwrap();
    check(&mut k, 36.0);
    for (epoch, lh, height) in [
        (3, LineHeight::Normal, 28.8),
        (4, LineHeight::Number(0.0), 0.0),
        (5, LineHeight::Length(0.0), 0.0),
    ] {
        k.apply(0, epoch, &[set(2, 24.0, Some(lh))]).unwrap();
        check(&mut k, height);
    }
    k.apply(
        0,
        6,
        &[Op::ClearStyle {
            id: 2,
            mask: StyleMask::of(StyleId::LineHeight),
        }],
    )
    .unwrap();
    check(&mut k, 36.0);
    k.apply(
        0,
        7,
        &[
            Op::SetChildren {
                id: 1,
                children: vec![3],
            },
            Op::SetChildren {
                id: 3,
                children: vec![2],
            },
        ],
    )
    .unwrap();
    check(&mut k, 24.0);
    k.apply(0, 8, &[set(2, 40.0, None)]).unwrap();
    check(&mut k, 24.0);
    k.apply(0, 9, &[Op::DestroyView { id: 2 }]).unwrap();
    k.apply(
        0,
        10,
        &[
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            set(2, 20.0, None),
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "new".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![3, 2],
            },
        ],
    )
    .unwrap();
    check(&mut k, 30.0);
}

#[test]
fn touched_receipt_is_unique_ordered_and_excludes_destroyed_or_created_generations() {
    let mut k = build();
    let before: Vec<_> = (1..=4).map(|id| k.node(id).unwrap().key).collect();
    let receipt = k
        .apply(
            0,
            2,
            &[
                Op::SetStyle {
                    id: 4,
                    patch: height(21.),
                },
                Op::SetStyle {
                    id: 2,
                    patch: height(51.),
                },
                Op::SetStyle {
                    id: 3,
                    patch: height(71.),
                },
                Op::SetStyle {
                    id: 2,
                    patch: height(52.),
                },
                Op::DestroyView { id: 3 },
                Op::CreateView {
                    id: 5,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 5,
                    patch: height(53.),
                },
                Op::CreateView {
                    id: 6,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 6,
                    patch: height(54.),
                },
                Op::DestroyView { id: 5 },
                Op::CreateView {
                    id: 7,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 7,
                    patch: height(55.),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 6, 7],
                },
            ],
        )
        .unwrap();
    assert_eq!(receipt.touched, before[..2]);
    assert!(
        receipt.touched.capacity() <= 4,
        "retained receipt kept per-operation scratch"
    );
    let created = [k.node(6).unwrap().key, k.node(7).unwrap().key];
    assert_eq!(receipt.created, created);
    assert_eq!(receipt.destroyed.len(), 3);
    assert_eq!(receipt.destroyed[..2], before[2..]);
    for key in &receipt.destroyed {
        assert!(k.node_by_key(*key).is_none());
        assert!(
            created.iter().any(|new| new.index == key.index),
            "exercise reused slots"
        );
    }
    // In a later batch those same allocations are ordinary touched nodes.
    let next = k
        .apply(
            0,
            3,
            &[
                Op::SetStyle {
                    id: 7,
                    patch: height(60.),
                },
                Op::SetStyle {
                    id: 6,
                    patch: height(61.),
                },
                Op::SetStyle {
                    id: 7,
                    patch: height(62.),
                },
            ],
        )
        .unwrap();
    let mut expected = created.to_vec();
    expected.sort_unstable();
    assert_eq!(next.touched, expected);
    assert!(next.created.is_empty());
}
