//! Regressions from the 2026-08-28 review: each test is a defect that existed.

use exact_kernel::{
    export, wire, ApplyError, DecodeError, Dimension, Kernel, KernelError, MonospaceMeasurer,
    NodeFlags, NodeType, Offer, Op, PropId, StyleId, StyleProps,
};

fn size(w: f32, h: f32) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    s.width = Dimension::Points(w);
    s.mask.set(StyleId::Width);
    s.height = Dimension::Points(h);
    s.mask.set(StyleId::Height);
    Box::new(s)
}

#[test]
fn a_text_node_only_holds_text_children() {
    // Finding 2: a View under a Text was accepted, orphaned from the engine
    // tree, and then published its stale cached layout — breaking the
    // result-equality gate. Now it is a rejection.
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
                node_type: NodeType::Text,
            },
            Op::SetStyle {
                id: 2,
                patch: size(50.0, 50.0),
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
    k.compute_layout(1, Offer::definite(100.0, 100.0)).unwrap();
    let before = k.export(None).unwrap();

    let err = k
        .apply(
            0,
            2,
            &[Op::SetChildren {
                id: 4,
                children: vec![2],
            }],
        )
        .unwrap_err();
    assert_eq!(
        err,
        KernelError::Apply(ApplyError::InlineRunNotText {
            op_index: 0,
            parent: 4,
            child: 2,
            node_type: NodeType::View
        })
    );
    assert_eq!(k.export(None).unwrap(), before);

    // And the incremental frames still equal a rehydrated kernel's.
    k.compute_layout(1, Offer::definite(100.0, 100.0)).unwrap();
    let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    fresh
        .compute_layout(1, Offer::definite(100.0, 100.0))
        .unwrap();
    let a: Vec<_> = k
        .rows(None)
        .unwrap()
        .into_iter()
        .map(|r| (r.id, r.frame))
        .collect();
    let b: Vec<_> = fresh
        .rows(None)
        .unwrap()
        .into_iter()
        .map(|r| (r.id, r.frame))
        .collect();
    assert_eq!(a, b);
}

#[test]
fn non_finite_style_numbers_are_refused_on_both_ingress_paths() {
    // Finding 3: NaN in an f32 row reached published frames.
    let mut k = Kernel::with_monospace();
    k.apply(
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

    let mut nan = StyleProps::default();
    nan.border_width_top = f32::NAN;
    nan.mask.set(StyleId::BorderWidthTop);
    let op = Op::SetStyle {
        id: 1,
        patch: Box::new(nan),
    };
    assert_eq!(
        k.apply(0, 2, std::slice::from_ref(&op)).unwrap_err(),
        KernelError::Apply(ApplyError::NonFiniteStyle {
            op_index: 0,
            style: StyleId::BorderWidthTop
        })
    );
    let bytes = wire::encode(0, 3, &[op]);
    assert_eq!(
        k.apply_frame(&bytes).unwrap_err(),
        KernelError::Decode(DecodeError::NonFinite(StyleId::BorderWidthTop))
    );

    let mut inf = StyleProps::default();
    inf.row_gap = f32::INFINITY;
    inf.mask.set(StyleId::RowGap);
    assert!(matches!(
        k.apply(
            0,
            4,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(inf)
            }]
        ),
        Err(KernelError::Apply(ApplyError::NonFiniteStyle {
            style: StyleId::RowGap,
            ..
        }))
    ));

    let mut shadow = StyleProps::default();
    shadow.shadow_offset.y = f32::NAN;
    shadow.mask.set(StyleId::ShadowOffset);
    assert!(matches!(
        k.apply(
            0,
            5,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(shadow)
            }]
        ),
        Err(KernelError::Apply(ApplyError::NonFiniteStyle {
            style: StyleId::ShadowOffset,
            ..
        }))
    ));

    k.compute_layout(1, Offer::definite(100.0, 100.0)).unwrap();
    let f = k.node(1).unwrap().frame;
    assert!(f.x.is_finite() && f.y.is_finite() && f.width.is_finite() && f.height.is_finite());
}

#[test]
fn nested_inline_runs_are_visited_and_carry_no_geometry() {
    // Finding 7: a run under a run was never visited, so its CREATED bit and
    // any stale frame survived every layout pass.
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
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "deep".into(),
            },
            Op::SetChildren {
                id: 3,
                children: vec![4],
            },
            Op::SetChildren {
                id: 2,
                children: vec![3],
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
    assert!(
        k.node(2).unwrap().frame.width > 0.0,
        "the paragraph measured its nested run"
    );
    let deep = k.arena().slot_of(4).unwrap();
    assert!(!k.arena().flags(deep).has(NodeFlags::CREATED));
    assert!(!k.arena().flags(deep).has(NodeFlags::GEOMETRY_CHANGED));
    assert_eq!(k.node(4).unwrap().frame, Default::default());
    let rows = k.rows(Some(1)).unwrap();
    assert_eq!(
        rows[3].flags & export::ROW_INLINE_RUN,
        export::ROW_INLINE_RUN
    );
    assert_eq!(rows[3].flags & export::ROW_GEOMETRY_CHANGED, 0);
}

#[test]
fn a_node_once_laid_out_then_made_a_run_reports_no_geometry() {
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
            Op::SetProp {
                id: 3,
                prop: PropId::Text,
                value: "moves".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    assert!(k.node(3).unwrap().frame.width > 0.0);
    k.apply(
        0,
        2,
        &[
            Op::SetChildren {
                id: 2,
                children: vec![3],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
        ],
    )
    .unwrap();
    let receipt = k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let changed: Vec<u32> = receipt
        .changed
        .iter()
        .map(|key| k.node_by_key(*key).unwrap().id)
        .collect();
    assert!(
        !changed.contains(&3),
        "runs are never in the changed-geometry receipt"
    );
    assert_eq!(k.node(3).unwrap().frame, Default::default());
    let rows = k.rows(Some(1)).unwrap();
    assert_eq!(rows[2].flags & export::ROW_GEOMETRY_CHANGED, 0);
}

#[test]
fn created_receipt_excludes_nodes_destroyed_in_the_same_batch() {
    // Finding 8: `created` kept keys that `destroyed` also listed.
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
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::DestroyView { id: 1 },
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::View,
                },
            ],
        )
        .unwrap();
    assert_eq!(receipt.created.len(), 1);
    assert_eq!(k.node_by_key(receipt.created[0]).unwrap().id, 3);
    assert_eq!(receipt.destroyed.len(), 2);
    assert!(receipt
        .destroyed
        .iter()
        .all(|key| k.node_by_key(*key).is_none()));
    assert_eq!(k.live_count(), 1);
}

#[test]
fn a_huge_node_count_in_an_envelope_is_refused_before_allocation() {
    let mut k = Kernel::with_monospace();
    k.apply(
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
    let mut bytes = k.export(None).unwrap();
    bytes[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        export::decode(&bytes),
        Err(DecodeError::SectionOverrun { declared: u32::MAX })
    );
}

#[test]
fn a_payload_length_near_u32_max_is_refused() {
    // Finding 4: `align8(payload_len as usize)` overflowed 32-bit targets.
    let mut k = Kernel::with_monospace();
    let mut bytes = wire::encode(
        0,
        1,
        &[Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        }],
    );
    let pos = wire::frame::HEADER_LEN + 8;
    bytes[pos..pos + 4].copy_from_slice(&0xffff_fffcu32.to_le_bytes());
    assert!(matches!(
        k.apply_frame(&bytes),
        Err(KernelError::Decode(DecodeError::PayloadOverrun { .. }))
    ));
    assert_eq!(k.live_count(), 0);
}
