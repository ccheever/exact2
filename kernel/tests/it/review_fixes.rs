//! Regressions from the 2026-08-28 review: each test is a defect that existed.

use exact_kernel::{
    export, wire, ApplyError, DecodeError, Dimension, GridLine, GridPlacement, GridTrack,
    GridTracks, Kernel, KernelError, LayoutError, MonospaceMeasurer, NodeFlags, NodeType, Offer,
    Op, PropId, StyleId, StyleProps, TextMeasureRequest, TextMeasurer, TextMetrics,
};
use std::cell::Cell;
use std::rc::Rc;

fn size(w: f32, h: f32) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    s.width = Dimension::Points(w);
    s.mask.set(StyleId::Width);
    s.height = Dimension::Points(h);
    s.mask.set(StyleId::Height);
    Box::new(s)
}

#[test]
fn intrinsic_flex_items_do_not_include_their_containers_padding() {
    // Messages' nested text column made a 48-point bubble 56 points wide.
    // The parent inset was flooring every item's intrinsic contribution,
    // then being added again. Row/column cases also cover the height bug.
    use exact_kernel::{StyleId::*, StyleValue};
    fn style(rows: &[(exact_kernel::StyleId, StyleValue)]) -> Box<StyleProps> {
        let mut result = StyleProps::default();
        for (id, value) in rows {
            result.set_dynamic(*id, value).unwrap();
        }
        Box::new(result)
    }
    let number = StyleValue::Number;
    let text = |value: &str| StyleValue::Text(value.into());
    for parent_direction in ["row", "column"] {
        for direction in ["row", "column"] {
            for inset in [0.0, 5.0, 14.0, 24.0] {
                for child_padding in [0.0, 3.0, 18.0] {
                    let mut k = Kernel::new(Box::new(MonospaceMeasurer {
                        advance_em: 0.65,
                        ..Default::default()
                    }));
                    let mut ops: Vec<_> = (1..=4)
                        .map(|id| Op::CreateView {
                            id,
                            node_type: if id == 4 {
                                NodeType::Text
                            } else {
                                NodeType::View
                            },
                        })
                        .collect();
                    ops.extend([
                        Op::SetStyle {
                            id: 1,
                            patch: style(&[
                                (Display, text("flex")),
                                (FlexDirection, text(parent_direction)),
                                (AlignItems, text("flex-start")),
                                (Width, number(402.0)),
                                (Height, number(874.0)),
                                (FontSize, number(20.0)),
                                (LineHeight, text("20px")),
                            ]),
                        },
                        Op::SetStyle {
                            id: 2,
                            patch: style(&[
                                (Display, text("flex")),
                                (FlexDirection, text(direction)),
                                (BoxSizing, text("border-box")),
                                (MinWidth, number(48.0)),
                                (MinHeight, number(40.0)),
                                (AlignItems, text("center")),
                                (JustifyContent, text("center")),
                                (PaddingLeft, number(inset)),
                                (PaddingRight, number(inset)),
                                (PaddingTop, number(inset)),
                                (PaddingBottom, number(inset)),
                            ]),
                        },
                        Op::SetStyle {
                            id: 3,
                            patch: style(&[
                                (Display, text("flex")),
                                (FlexDirection, text("column")),
                                (MinWidth, number(0.0)),
                                (MinHeight, number(0.0)),
                                (PaddingLeft, number(child_padding)),
                                (PaddingRight, number(child_padding)),
                                (PaddingTop, number(child_padding)),
                                (PaddingBottom, number(child_padding)),
                            ]),
                        },
                        Op::SetProp {
                            id: 4,
                            prop: PropId::Text,
                            value: "H".into(),
                        },
                        Op::SetChildren {
                            id: 1,
                            children: vec![2],
                        },
                        Op::SetChildren {
                            id: 2,
                            children: vec![3],
                        },
                        Op::SetChildren {
                            id: 3,
                            children: vec![4],
                        },
                        Op::AttachRoot { id: 1 },
                    ]);
                    k.apply(0, 1, &ops).unwrap();
                    k.compute_layout(1, Offer::definite(402.0, 874.0)).unwrap();
                    let frame = k.node(2).unwrap().frame;
                    let padding = 2.0 * (inset + child_padding) as f32;
                    assert_eq!((frame.width, frame.height),
                        ((13.0 + padding).max(48.0), (20.0 + padding).max(40.0)),
                        "parent={parent_direction}, direction={direction}, inset={inset}, child_padding={child_padding}");
                }
            }
        }
    }
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
fn non_finite_layout_offers_are_refused_without_publishing_frames() {
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
    k.compute_layout(1, Offer::definite(100.0, 100.0)).unwrap();
    let before = k.node(1).unwrap().frame;

    for offer in [
        Offer::definite(f32::NAN, 100.0),
        Offer::definite(100.0, f32::INFINITY),
    ] {
        assert_eq!(
            k.compute_layout(1, offer),
            Err(KernelError::Layout(LayoutError::InvalidOffer))
        );
        assert_eq!(k.node(1).unwrap().frame, before);
    }
    assert!(before.x.is_finite());
    assert!(before.y.is_finite());
    assert!(before.width.is_finite());
    assert!(before.height.is_finite());
}

#[test]
fn zero_grid_spans_are_refused_on_both_ingress_paths() {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        }],
    )
    .unwrap();
    let mut patch = StyleProps::default();
    patch.grid_column = GridPlacement {
        start: GridLine::Auto,
        end: GridLine::Span(0),
    };
    patch.mask.set(StyleId::GridColumn);
    let op = Op::SetStyle {
        id: 1,
        patch: Box::new(patch),
    };
    assert_eq!(
        k.apply(0, 2, std::slice::from_ref(&op)),
        Err(KernelError::Apply(ApplyError::InvalidGridSpan {
            op_index: 0,
            style: StyleId::GridColumn,
        }))
    );
    assert_eq!(
        k.apply_frame(&wire::encode(0, 3, &[op])),
        Err(KernelError::Decode(DecodeError::InvalidGridSpan))
    );
}

#[test]
fn structured_style_domain_matches_wire_and_export() {
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
    let before = k.export(None).unwrap();
    let epoch = k.epoch();

    let mut too_many = StyleProps::default();
    too_many.grid_template_columns = GridTracks(vec![GridTrack::Fr(1.0); 33]);
    too_many.mask.set(StyleId::GridTemplateColumns);
    assert_eq!(
        k.apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(too_many),
            }],
        ),
        Err(KernelError::Apply(ApplyError::TooManyTracks {
            op_index: 0,
            style: StyleId::GridTemplateColumns,
            count: 33,
        }))
    );

    let mut auto_padding = StyleProps::default();
    auto_padding.padding_top = Dimension::Auto;
    auto_padding.mask.set(StyleId::PaddingTop);
    let invalid_auto = Op::SetStyle {
        id: 1,
        patch: Box::new(auto_padding),
    };
    assert_eq!(
        k.apply(0, 3, std::slice::from_ref(&invalid_auto)),
        Err(KernelError::Apply(ApplyError::AutoNotAdmitted {
            op_index: 0,
            style: StyleId::PaddingTop,
        }))
    );
    assert_eq!(
        k.apply_frame(&wire::encode(0, 4, &[invalid_auto])),
        Err(KernelError::Decode(DecodeError::AutoNotAdmitted {
            style: StyleId::PaddingTop,
        }))
    );
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.export(None).unwrap(), before);

    let mut maximum = StyleProps::default();
    maximum.grid_template_columns = GridTracks(vec![GridTrack::Fr(1.0); 32]);
    maximum.mask.set(StyleId::GridTemplateColumns);
    maximum.padding_top = Dimension::Points(4.0);
    maximum.mask.set(StyleId::PaddingTop);
    maximum.width = Dimension::Auto;
    maximum.mask.set(StyleId::Width);
    let valid_style = Op::SetStyle {
        id: 1,
        patch: Box::new(maximum),
    };
    k.apply(0, 5, std::slice::from_ref(&valid_style)).unwrap();
    let snapshot = export::decode(&k.export(None).unwrap()).unwrap();
    assert_eq!(snapshot.styles[0].grid_template_columns.0.len(), 32);
    assert_eq!(snapshot.styles[0].padding_top, Dimension::Points(4.0));
    assert_eq!(snapshot.styles[0].width, Dimension::Auto);

    let mut from_wire = Kernel::with_monospace();
    let wire_ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        valid_style,
        Op::AttachRoot { id: 1 },
    ];
    from_wire
        .apply_frame(&wire::encode(0, 1, &wire_ops))
        .unwrap();
    let wire_snapshot = export::decode(&from_wire.export(None).unwrap()).unwrap();
    assert_eq!(wire_snapshot.styles, snapshot.styles);
}

#[derive(Clone)]
struct ControlledMeasurer(Rc<Cell<TextMetrics>>);

impl TextMeasurer for ControlledMeasurer {
    fn measure(&mut self, _request: &TextMeasureRequest<'_>) -> TextMetrics {
        self.0.get()
    }
}

#[test]
fn invalid_text_metrics_are_typed_and_never_publish_frames() {
    let valid = TextMetrics {
        width: 20.0,
        height: 10.0,
        first_baseline: Some(7.0),
    };
    let invalid = [
        TextMetrics {
            width: f32::NAN,
            ..valid
        },
        TextMetrics {
            width: f32::INFINITY,
            ..valid
        },
        TextMetrics {
            width: -1.0,
            ..valid
        },
        TextMetrics {
            height: f32::NAN,
            ..valid
        },
        TextMetrics {
            height: f32::INFINITY,
            ..valid
        },
        TextMetrics {
            height: -1.0,
            ..valid
        },
        TextMetrics {
            first_baseline: Some(f32::NAN),
            ..valid
        },
        TextMetrics {
            first_baseline: Some(f32::INFINITY),
            ..valid
        },
        TextMetrics {
            first_baseline: Some(-1.0),
            ..valid
        },
    ];

    for (index, bad) in invalid.into_iter().enumerate() {
        let metrics = Rc::new(Cell::new(valid));
        let mut k = Kernel::new(Box::new(ControlledMeasurer(metrics.clone())));
        k.apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 1,
                    prop: PropId::Text,
                    value: "valid".into(),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
        k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
        let before = k.node(1).unwrap().frame;

        metrics.set(bad);
        k.apply(
            0,
            2,
            &[Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: format!("invalid-{index}").into(),
            }],
        )
        .unwrap();
        assert_eq!(
            k.compute_layout(1, Offer::MAX_CONTENT),
            Err(KernelError::Layout(LayoutError::InvalidTextMetrics(1)))
        );
        assert_eq!(k.node(1).unwrap().frame, before);

        metrics.set(TextMetrics {
            width: 30.0,
            ..valid
        });
        k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
        assert!(k.node(1).unwrap().frame.width.is_finite());
    }
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

#[test]
fn content_sized_textarea_keeps_the_caret_line_after_return() {
    // CoreText omits a paragraph's terminal empty line. The field must still
    // grow for its caret; an ordinary paragraph keeps the shaper's behavior.
    struct ParagraphMeasurer;
    impl TextMeasurer for ParagraphMeasurer {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            let text: String = request.runs.iter().map(|run| &*run.text).collect();
            TextMetrics {
                width: 80.0,
                height: 20.0 * text.lines().count().max(1) as f32,
                first_baseline: Some(16.0),
            }
        }
    }
    for (node_type, semantic) in [(NodeType::TextInput, "textarea"), (NodeType::Text, "")] {
        let mut kernel = Kernel::new(Box::new(ParagraphMeasurer));
        let mut style = StyleProps::default();
        style.field_sizing = exact_kernel::FieldSizing::Content;
        style.mask.set(StyleId::FieldSizing);
        kernel
            .apply(
                0,
                1,
                &[
                    Op::CreateView { id: 1, node_type },
                    Op::SetStyle {
                        id: 1,
                        patch: Box::new(style),
                    },
                    Op::SetProp {
                        id: 1,
                        prop: PropId::SemanticTag,
                        value: semantic.into(),
                    },
                    Op::AttachRoot { id: 1 },
                ],
            )
            .unwrap();
        let prop = if node_type == NodeType::TextInput {
            PropId::Value
        } else {
            PropId::Text
        };
        for (index, (text, field_lines, paragraph_lines)) in [
            ("Line", 1, 1),
            ("Line\n", 2, 1),
            ("Line\n\n", 3, 2),
            ("Line\nLast", 2, 2),
            ("Short", 1, 1),
            ("\n", 2, 1),
        ]
        .into_iter()
        .enumerate()
        {
            kernel
                .apply(
                    0,
                    index as u64 + 2,
                    &[Op::SetProp {
                        id: 1,
                        prop,
                        value: text.into(),
                    }],
                )
                .unwrap();
            kernel
                .compute_layout(1, Offer::definite(300.0, 200.0))
                .unwrap();
            let lines = if node_type == NodeType::TextInput {
                field_lines
            } else {
                paragraph_lines
            };
            assert_eq!(
                kernel.node(1).unwrap().frame.height,
                20.0 * lines as f32,
                "{semantic}: {text:?}"
            );
            assert_eq!(kernel.node(1).unwrap().props.str(prop), Some(text));
        }
    }
}
