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
