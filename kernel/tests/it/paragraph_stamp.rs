//! Kernel-issued paragraph identities: invalidation, lifetime and request agreement.
use exact_kernel::{
    Color, ColorValue, Env, FieldSizing, Kernel, MonospaceMeasurer, NodeType, Offer, Op,
    ParagraphStamp, PropId, StyleId, StyleMask, StyleProps, TextAlign, TextMeasureRequest,
    TextMeasurer, TextMetrics, TextOverflow,
};
use std::{cell::RefCell, rc::Rc};

fn prop(id: u32, prop: PropId, value: &str) -> Op {
    Op::SetProp {
        id,
        prop,
        value: value.into(),
    }
}
fn children(id: u32, children: &[u32]) -> Op {
    Op::SetChildren {
        id,
        children: children.to_vec(),
    }
}
fn style(id: u32, f: impl FnOnce(&mut StyleProps)) -> Op {
    let mut patch = StyleProps::default();
    f(&mut patch);
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn font(id: u32, size: f32) -> Op {
    style(id, |s| {
        s.font_size = size;
        s.mask.set(StyleId::FontSize);
    })
}
fn color(id: u32) -> Op {
    style(id, |s| {
        s.text_color = ColorValue::Fixed(Color::rgba(200, 20, 30, 255));
        s.mask.set(StyleId::TextColor);
    })
}
fn apply(k: &mut Kernel, ops: &[Op]) {
    k.apply(0, 0, ops).unwrap();
}
fn stamp(k: &Kernel, id: u32) -> ParagraphStamp {
    k.node(id)
        .unwrap()
        .paragraph_stamp()
        .expect("independent paragraph")
}
fn fixture_with(measurer: Box<dyn TextMeasurer>) -> Kernel {
    let mut k = Kernel::new(measurer);
    let mut ops: Vec<_> = [
        (1, NodeType::View),
        (2, NodeType::Text),
        (3, NodeType::Text),
        (4, NodeType::Text),
        (5, NodeType::Text),
        (6, NodeType::TextInput),
        (7, NodeType::View),
        (8, NodeType::Text),
        (9, NodeType::Text),
    ]
    .into_iter()
    .map(|(id, node_type)| Op::CreateView { id, node_type })
    .collect();
    ops.extend([
        prop(3, PropId::Text, "left "),
        prop(4, PropId::Text, "right"),
        prop(5, PropId::Text, "other"),
        prop(8, PropId::Text, "nested"),
        prop(9, PropId::Text, "fixed"),
        font(7, 32.0),
        font(9, 18.0),
        children(2, &[3, 4]),
        children(7, &[8, 9]),
        children(1, &[2, 5, 6, 7]),
        Op::AttachRoot { id: 1 },
    ]);
    apply(&mut k, &ops);
    k
}
fn fixture() -> Kernel {
    fixture_with(Box::new(MonospaceMeasurer::default()))
}

#[test]
fn unrelated_typing_and_offer_changes_leave_paragraph_identity_stable() {
    let mut k = fixture();
    apply(&mut k, &[prop(3, PropId::Text, &"giant ".repeat(25_000))]);
    let original = stamp(&k, 2);
    for text in ["a", "EXACT_🧪漢字", "e\u{301}", ""] {
        apply(&mut k, &[prop(6, PropId::Value, text)]);
        assert_eq!(stamp(&k, 2), original);
    }
    for width in [300.0, 500.0, 301.0] {
        k.compute_layout(1, Offer::definite(width, 500.0)).unwrap();
        assert_eq!(stamp(&k, 2), original, "offers are separate inputs");
    }
}

#[test]
fn inline_nodes_do_not_stamp_their_different_payloads_as_the_same_paragraph() {
    let k = fixture();
    assert_ne!(
        k.node(3).unwrap().text_runs(),
        k.node(4).unwrap().text_runs()
    );
    for id in [1, 3, 4, 7] {
        assert!(k.node(id).unwrap().paragraph_stamp().is_none());
    }
    assert_eq!(stamp(&k, 2).owner(), k.node(2).unwrap().key);
    assert!(k.node(6).unwrap().paragraph_stamp().is_some());
}

#[test]
fn metric_changes_invalidate_only_affected_owners_and_propagate_inheritance() {
    let mut k = fixture();
    let p = stamp(&k, 2);
    let q = stamp(&k, 5);
    apply(&mut k, &[prop(3, PropId::Text, "longer left")]);
    assert!(!p.same_metrics(&stamp(&k, 2)));
    assert_ne!(
        p.paint_source_revision(),
        stamp(&k, 2).paint_source_revision()
    );
    assert_eq!(q, stamp(&k, 5));
    let inherited = stamp(&k, 8);
    let overridden = stamp(&k, 9);
    apply(&mut k, &[font(7, 40.0)]);
    assert!(!inherited.same_metrics(&stamp(&k, 8)));
    assert_eq!(overridden, stamp(&k, 9));
    let inherited = stamp(&k, 8);
    apply(
        &mut k,
        &[Op::ClearStyle {
            id: 7,
            mask: StyleMask::of(StyleId::FontSize),
        }],
    );
    assert!(!inherited.same_metrics(&stamp(&k, 8)));
    assert_eq!(k.node(8).unwrap().text_style().font_size, 16.0);
}

#[test]
fn paint_changes_keep_metric_proof_and_update_source_mapping_revision() {
    let mut k = fixture();
    for op in [
        color(3),
        color(1),
        prop(4, PropId::Href, "/new"),
        Op::ClearProp {
            id: 4,
            prop: PropId::Href,
        },
    ] {
        let old = stamp(&k, 2);
        apply(&mut k, &[op]);
        let new = stamp(&k, 2);
        assert!(old.same_metrics(&new));
        assert_ne!(old, new);
    }
    let old = stamp(&k, 2);
    apply(
        &mut k,
        &[Op::ClearStyle {
            id: 1,
            mask: StyleMask::of(StyleId::TextColor),
        }],
    );
    assert!(old.same_metrics(&stamp(&k, 2)));
    assert_ne!(old, stamp(&k, 2));
}

#[test]
fn noops_and_rejected_mixed_batches_preserve_every_stamp() {
    let mut k = fixture();
    let old = [stamp(&k, 2), stamp(&k, 5), stamp(&k, 8), stamp(&k, 9)];
    apply(
        &mut k,
        &[
            prop(3, PropId::Text, "left "),
            children(2, &[3, 4]),
            font(7, 32.0),
            Op::ClearProp {
                id: 2,
                prop: PropId::Href,
            },
            Op::ClearStyle {
                id: 2,
                mask: StyleMask::of(StyleId::TextColor),
            },
        ],
    );
    assert!(k
        .apply(
            0,
            8,
            &[
                prop(3, PropId::Text, "must not publish"),
                font(7, 90.0),
                color(4),
                children(2, &[3, 3])
            ]
        )
        .is_err());
    assert_eq!(
        old,
        [stamp(&k, 2), stamp(&k, 5), stamp(&k, 8), stamp(&k, 9)]
    );
    assert_eq!(k.node(3).unwrap().props.str(PropId::Text), Some("left "));
}

#[test]
fn implicit_reparent_invalidates_both_paragraphs_and_destroy_invalidates_old_owner() {
    let mut k = fixture();
    // Clearing own text makes the second paragraph consume the moved run.
    apply(
        &mut k,
        &[Op::ClearProp {
            id: 5,
            prop: PropId::Text,
        }],
    );
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let p = stamp(&k, 2);
    let q = stamp(&k, 5);
    apply(&mut k, &[children(5, &[3])]);
    assert!(!p.same_metrics(&stamp(&k, 2)));
    assert!(!q.same_metrics(&stamp(&k, 5)));
    assert_eq!(k.node(2).unwrap().text_runs()[0].text, "right");
    let q = stamp(&k, 5);
    apply(&mut k, &[Op::DestroyView { id: 3 }]);
    assert!(!q.same_metrics(&stamp(&k, 5)));
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    rebuilt.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    for id in [2, 5] {
        assert_eq!(k.node(id).unwrap().frame, rebuilt.node(id).unwrap().frame);
    }
}

#[test]
fn nested_topology_own_text_mask_and_clearing_are_safe() {
    let mut k = fixture();
    apply(
        &mut k,
        &[
            children(3, &[4]),
            Op::ClearProp {
                id: 3,
                prop: PropId::Text,
            },
        ],
    );
    let p = stamp(&k, 2);
    apply(&mut k, &[prop(4, PropId::Text, "deep change")]);
    assert!(!p.same_metrics(&stamp(&k, 2)));
    apply(&mut k, &[prop(2, PropId::Text, "owner masks descendants")]);
    let masked = stamp(&k, 2);
    apply(&mut k, &[prop(4, PropId::Text, "hidden change")]);
    assert_eq!(
        k.node(2).unwrap().text_runs()[0].text,
        "owner masks descendants"
    );
    // Conservative churn under masking is allowed; clearing must never reuse the masked proof.
    apply(
        &mut k,
        &[Op::ClearProp {
            id: 2,
            prop: PropId::Text,
        }],
    );
    assert!(!masked.same_metrics(&stamp(&k, 2)));
    assert_eq!(k.node(2).unwrap().text_runs()[0].text, "hidden change");
}

#[test]
fn detaching_a_container_refreshes_descendant_inheritance_before_becoming_root() {
    let mut k = fixture();
    apply(
        &mut k,
        &[
            font(1, 24.0),
            Op::ClearStyle {
                id: 7,
                mask: StyleMask::of(StyleId::FontSize),
            },
        ],
    );
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let inherited = stamp(&k, 8);
    let overridden = stamp(&k, 9);
    apply(&mut k, &[children(1, &[2, 5, 6])]);
    assert!(!inherited.same_metrics(&stamp(&k, 8)));
    assert_eq!(k.node(8).unwrap().text_style().font_size, 16.0);
    // Topology can conservatively invalidate fixed descendants too, but cannot leave wrong geometry.
    assert_eq!(k.node(9).unwrap().text_style().font_size, 18.0);
    let _ = overridden;
    apply(&mut k, &[Op::AttachRoot { id: 7 }]);
    k.compute_layout(7, Offer::MAX_CONTENT).unwrap();
    let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    rebuilt.compute_layout(7, Offer::MAX_CONTENT).unwrap();
    assert_eq!(k.node(8).unwrap().frame, rebuilt.node(8).unwrap().frame);
}

#[test]
fn attaching_an_orphan_container_only_remeasures_changed_inherited_values() {
    let mut k = fixture();
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    apply(&mut k, &[children(1, &[2, 5, 6])]);
    let before = [stamp(&k, 8), stamp(&k, 9)];
    apply(&mut k, &[children(1, &[2, 5, 6, 7])]);
    for (id, old) in [8, 9].into_iter().zip(before) {
        let now = stamp(&k, id);
        assert!(old.same_metrics(&now), "same inherited values at {id}");
        assert_ne!(old.paint_source_revision(), now.paint_source_revision());
    }
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    rebuilt.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    for id in [7, 8, 9] {
        assert_eq!(k.node(id).unwrap().frame, rebuilt.node(id).unwrap().frame);
    }

    // A detached subtree still follows its new parent's changed values, while
    // a descendant's authored override continues to stop propagation.
    apply(
        &mut k,
        &[
            children(1, &[2, 5, 6]),
            Op::ClearStyle {
                id: 7,
                mask: StyleMask::of(StyleId::FontSize),
            },
            font(1, 24.0),
        ],
    );
    let inherited = stamp(&k, 8);
    let overridden = stamp(&k, 9);
    assert_eq!(k.node(8).unwrap().text_style().font_size, 16.0);
    apply(&mut k, &[children(1, &[2, 5, 6, 7])]);
    assert!(!inherited.same_metrics(&stamp(&k, 8)));
    assert!(overridden.same_metrics(&stamp(&k, 9)));
    assert_eq!(k.node(8).unwrap().text_style().font_size, 24.0);
    assert_eq!(k.node(9).unwrap().text_style().font_size, 18.0);
    assert_eq!(k.node(8).unwrap().source_of(StyleId::FontSize), Some(1));
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    rebuilt.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    for id in [7, 8, 9] {
        assert_eq!(k.node(id).unwrap().frame, rebuilt.node(id).unwrap().frame);
    }
}

#[test]
fn reset_reuse_independent_kernels_and_rehydrate_do_not_alias() {
    let mut k = fixture();
    let first = stamp(&k, 5);
    let another = fixture();
    assert_eq!(first.owner(), stamp(&another, 5).owner());
    assert_ne!(first, stamp(&another, 5));
    let rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    assert_ne!(first, stamp(&rebuilt, 5));
    assert!(!first.same_metrics(&stamp(&rebuilt, 5)));
    assert_eq!(first, stamp(&k, 5), "rehydrate does not alter the original");
    apply(
        &mut k,
        &[
            Op::DestroyView { id: 5 },
            Op::CreateView {
                id: 5,
                node_type: NodeType::Text,
            },
            prop(5, PropId::Text, "other"),
        ],
    );
    assert_ne!(first, stamp(&k, 5));
    let before_reset = stamp(&k, 5);
    k.reset();
    apply(
        &mut k,
        &[
            Op::CreateView {
                id: 5,
                node_type: NodeType::Text,
            },
            prop(5, PropId::Text, "other"),
        ],
    );
    assert!(!before_reset.same_metrics(&stamp(&k, 5)));
}

#[test]
fn align_overflow_and_textarea_semantics_invalidate_real_measurement() {
    let mut k = fixture();
    for op in [
        style(1, |s| {
            s.text_align = TextAlign::Right;
            s.mask.set(StyleId::TextAlign);
        }),
        style(2, |s| {
            s.text_overflow = TextOverflow::Ellipsis;
            s.mask.set(StyleId::TextOverflow);
        }),
    ] {
        let old = stamp(&k, 2);
        apply(&mut k, &[op]);
        assert!(!old.same_metrics(&stamp(&k, 2)));
    }
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    let old = stamp(&k, 6);
    let h = k.node(6).unwrap().frame.height;
    apply(&mut k, &[prop(6, PropId::SemanticTag, "textarea")]);
    assert!(!old.same_metrics(&stamp(&k, 6)));
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    assert!(k.node(6).unwrap().frame.height > h);
    apply(
        &mut k,
        &[
            style(6, |s| {
                s.field_sizing = FieldSizing::Content;
                s.mask.set(StyleId::FieldSizing);
            }),
            prop(6, PropId::Value, "last\n"),
        ],
    );
    assert_eq!(
        k.node(6).unwrap().text_runs().last().unwrap().text,
        "\u{200b}"
    );
    let old = stamp(&k, 6);
    apply(
        &mut k,
        &[Op::ClearProp {
            id: 6,
            prop: PropId::SemanticTag,
        }],
    );
    assert!(!old.same_metrics(&stamp(&k, 6)));
    assert_eq!(k.node(6).unwrap().text_runs().len(), 1);
}

#[test]
fn environment_changes_offer_without_revising_source() {
    let mut k = fixture();
    apply(
        &mut k,
        &[style(1, |s| {
            s.set_dynamic(
                StyleId::PaddingLeft,
                &exact_kernel::StyleValue::Text("env(safe-area-inset-left)".into()),
            )
            .unwrap();
        })],
    );
    let old = stamp(&k, 2);
    k.set_env(Env::new(0.0, 30.0, 0.0, 12.0)).unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert_eq!(old, stamp(&k, 2));
}

struct Recorder(Rc<RefCell<Vec<ParagraphStamp>>>);
impl TextMeasurer for Recorder {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(r)
    }
    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        r: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        self.0.borrow_mut().push(stamp.clone());
        self.measure(r)
    }
}
#[test]
fn identified_callback_and_node_reads_agree_without_changing_legacy_measurers() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut k = fixture_with(Box::new(Recorder(seen.clone())));
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert!(!seen.borrow().is_empty());
    for request_stamp in seen.borrow().iter() {
        assert_eq!(
            *request_stamp,
            k.node_by_key(request_stamp.owner())
                .unwrap()
                .paragraph_stamp()
                .unwrap()
        );
    }
    assert!(seen.borrow().iter().any(|s| *s == stamp(&k, 2)));
    assert!(seen
        .borrow()
        .iter()
        .all(|s| ![k.node(3).unwrap().key, k.node(4).unwrap().key].contains(&s.owner())));
    let mut legacy = fixture();
    legacy
        .compute_layout(1, Offer::definite(300.0, 500.0))
        .unwrap();
    for id in [2, 5, 6, 8, 9] {
        assert_eq!(k.node(id).unwrap().frame, legacy.node(id).unwrap().frame);
    }
}

#[test]
fn public_arena_clone_forks_cannot_stamp_different_canonical_runs_identically() {
    use exact_kernel::{layout::LayoutTree, selector::SelectorIndex, txn};
    let original = fixture();
    let old = stamp(&original, 2);
    let mut left = original.arena().clone();
    let mut right = original.arena().clone();
    let mut left_layout = LayoutTree::rebuild(&mut left);
    let mut right_layout = LayoutTree::rebuild(&mut right);
    let mut left_selectors = SelectorIndex::new();
    let mut right_selectors = SelectorIndex::new();
    for (arena, layout, selectors, text) in [
        (
            &mut left,
            &mut left_layout,
            &mut left_selectors,
            "left fork",
        ),
        (
            &mut right,
            &mut right_layout,
            &mut right_selectors,
            "right fork",
        ),
    ] {
        txn::apply(
            txn::Target {
                arena,
                layout,
                mirrored: true,
                selectors,
            },
            &[prop(3, PropId::Text, text)],
            0,
            0,
            0,
        )
        .unwrap();
    }
    let owner = old.owner().index;
    let mut left_runs = Vec::new();
    let mut right_runs = Vec::new();
    left.text_runs(owner, &mut left_runs);
    right.text_runs(owner, &mut right_runs);
    assert_ne!(
        left_runs, right_runs,
        "public forks have different measured payloads"
    );
    let left_stamp = left.paragraph_stamp(owner).unwrap();
    let right_stamp = right.paragraph_stamp(owner).unwrap();
    assert_ne!(left_stamp, right_stamp, "forked namespaces must not alias");
    assert!(!left_stamp.same_metrics(&right_stamp));
    assert!(!old.same_metrics(&left_stamp));
    assert!(!old.same_metrics(&right_stamp));
    assert_eq!(
        old,
        stamp(&original, 2),
        "fork mutations leave original unchanged"
    );
    assert_eq!(
        original.node(3).unwrap().props.str(PropId::Text),
        Some("left ")
    );
}
