//! A sampled CSS height is derived layout, never authored state or a commit.
use std::{cell::Cell, rc::Rc};

use exact_kernel::{
    export, Dimension, Edge, Env, Kernel, KernelError, LayoutError, MonospaceMeasurer, NodeType,
    Offer, Op, PresentedHeight, PropId, StyleId, StyleMask, StyleProps, StyleValue,
    TextMeasureRequest, TextMeasurer, TextMetrics,
};

fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (id, value) in rows {
        patch.set_dynamic(*id, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

fn height(id: u32, px: f64) -> Op {
    style(id, &[(StyleId::Height, StyleValue::Number(px))])
}

fn build(measurer: Box<dyn TextMeasurer>) -> Kernel {
    let mut k = Kernel::new(measurer);
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
            style(
                1,
                &[
                    (StyleId::Width, StyleValue::Number(400.0)),
                    (StyleId::Height, StyleValue::Number(600.0)),
                ],
            ),
            height(2, 180.0),
            height(3, 30.0),
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "sample text".into(),
            },
            Op::SetChildren {
                id: 2,
                children: vec![4],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

fn tree() -> Kernel {
    build(Box::new(MonospaceMeasurer::default()))
}
fn offer() -> Offer {
    Offer::definite(400.0, 600.0)
}
fn sample(k: &Kernel, id: u32, px: f32) -> PresentedHeight {
    PresentedHeight {
        node: k.node(id).unwrap().key,
        epoch: k.epoch(),
        px,
    }
}
fn project(k: &mut Kernel, id: u32, px: f32) {
    let p = sample(k, id, px);
    k.compute_layout_presented(1, offer(), Some(p)).unwrap();
}
fn h(k: &Kernel, id: u32) -> f32 {
    k.node(id).unwrap().frame.height
}

#[test]
fn height_changes_flow_and_frames_without_authoring_or_receipts() {
    let mut k = tree();
    k.compute_layout(1, offer()).unwrap();
    let bytes = k.export(None).unwrap();
    let before = export::decode(&bytes).unwrap();
    let authored = k.node(2).unwrap().style.clone();
    let epoch = k.epoch();
    let receipts = k.receipts().count();
    project(&mut k, 2, 420.25);
    assert_eq!(h(&k, 2), 420.25);
    assert_eq!(k.node(3).unwrap().frame.y, 420.25);
    assert_eq!(k.node(2).unwrap().style, &authored);
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.receipts().count(), receipts);
    let after_bytes = k.export(None).unwrap();
    let after = export::decode(&after_bytes).unwrap();
    assert_ne!(bytes, after_bytes);
    assert_eq!(before.styles, after.styles);
    assert_eq!(before.props, after.props);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&k, 2), 180.0);
    assert_eq!(k.node(3).unwrap().frame.y, 180.0);
}

#[test]
fn css_box_sizing_min_max_and_fractional_zero_samples() {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[style(
            2,
            &[
                (StyleId::PaddingTop, StyleValue::Number(7.0)),
                (StyleId::PaddingBottom, StyleValue::Number(11.0)),
                (StyleId::MinHeight, StyleValue::Number(50.0)),
                (StyleId::MaxHeight, StyleValue::Number(240.0)),
            ],
        )],
    )
    .unwrap();
    project(&mut k, 2, 100.5);
    assert_eq!(h(&k, 2), 118.5);
    project(&mut k, 2, 420.0);
    assert_eq!(h(&k, 2), 258.0);
    project(&mut k, 2, 0.0);
    assert_eq!(h(&k, 2), 68.0);
    k.apply(
        0,
        3,
        &[style(
            2,
            &[(StyleId::BoxSizing, StyleValue::Text("border-box".into()))],
        )],
    )
    .unwrap();
    project(&mut k, 2, 100.5);
    assert_eq!(h(&k, 2), 100.5);
    project(&mut k, 2, 420.0);
    assert_eq!(h(&k, 2), 240.0);
}

#[test]
fn absolute_bottom_panel_retains_its_anchor_and_authored_width() {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[style(
            2,
            &[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Bottom, StyleValue::Number(0.0)),
                (StyleId::Width, StyleValue::Number(300.0)),
            ],
        )],
    )
    .unwrap();
    project(&mut k, 2, 420.0);
    assert_eq!(k.node(2).unwrap().frame.y, 180.0);
    assert_eq!(k.node(2).unwrap().frame.width, 300.0);
    project(&mut k, 2, 0.0);
    assert_eq!(h(&k, 2), 0.0);
    assert_eq!(k.node(2).unwrap().frame.y, 600.0);
}

struct Counted {
    calls: Rc<Cell<usize>>,
    invalid: Rc<Cell<bool>>,
}
impl TextMeasurer for Counted {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        self.calls.set(self.calls.get() + 1);
        if self.invalid.get() {
            TextMetrics {
                width: f32::NAN,
                height: 20.0,
                first_baseline: None,
            }
        } else {
            MonospaceMeasurer::default().measure(request)
        }
    }
}

#[test]
fn identical_samples_reuse_clean_layout_and_text_measurement() {
    let calls = Rc::new(Cell::new(0));
    let mut k = build(Box::new(Counted {
        calls: calls.clone(),
        invalid: Rc::default(),
    }));
    project(&mut k, 2, 256.0);
    let count = calls.get();
    assert!(count > 0);
    let p = sample(&k, 2, 256.0);
    for _ in 0..10 {
        assert!(k
            .compute_layout_presented(1, offer(), Some(p))
            .unwrap()
            .changed
            .is_empty());
    }
    assert_eq!(calls.get(), count);
    // Even an authored target write whose derived style is unchanged stays clean.
    k.apply(0, 2, &[height(2, 360.0)]).unwrap();
    project(&mut k, 2, 256.0);
    assert_eq!(calls.get(), count);
}

#[test]
fn target_and_other_style_commits_survive_hold_switch_and_clear() {
    let mut k = tree();
    project(&mut k, 2, 250.0);
    k.apply(
        0,
        2,
        &[
            height(2, 333.0),
            style(
                2,
                &[
                    (StyleId::PaddingTop, StyleValue::Number(8.0)),
                    (StyleId::Width, StyleValue::Number(222.0)),
                ],
            ),
            height(3, 44.0),
        ],
    )
    .unwrap();
    project(&mut k, 2, 250.0);
    assert_eq!(h(&k, 2), 258.0);
    assert_eq!(k.node(2).unwrap().frame.width, 222.0);
    project(&mut k, 3, 90.0);
    assert_eq!(h(&k, 2), 341.0);
    assert_eq!(h(&k, 3), 90.0);
    k.compute_layout_presented(1, offer(), None).unwrap();
    assert_eq!(h(&k, 3), 44.0);
}

#[test]
fn env_writes_at_same_epoch_keep_projection_and_clear_to_latest_style() {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[style(
            2,
            &[(
                StyleId::PaddingTop,
                StyleValue::Text("env(safe-area-inset-top)".into()),
            )],
        )],
    )
    .unwrap();
    project(&mut k, 2, 250.0);
    let p = sample(&k, 2, 250.0);
    k.set_env(Env::new(17.0, 0.0, 0.0, 0.0)).unwrap();
    assert_eq!(k.epoch(), p.epoch);
    k.compute_layout_presented(1, offer(), Some(p)).unwrap();
    assert_eq!(h(&k, 2), 267.0);
    assert_eq!(k.node(4).unwrap().frame.y, 17.0);
    assert_eq!(
        k.node(2).unwrap().style.padding_top,
        Dimension::Env(Edge::Top, 0.0)
    );
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&k, 2), 197.0);
}

#[test]
fn intrinsic_ratio_updates_at_same_epoch_preserve_sample() {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 5,
                node_type: NodeType::Image,
            },
            height(5, 100.0),
            Op::SetChildren {
                id: 1,
                children: vec![5],
            },
        ],
    )
    .unwrap();
    k.set_intrinsic_size(5, Some((100.0, 50.0))).unwrap();
    project(&mut k, 5, 60.0);
    assert_eq!(k.node(5).unwrap().frame.width, 120.0);
    let p = sample(&k, 5, 60.0);
    k.set_intrinsic_size(5, Some((150.0, 50.0))).unwrap();
    assert_eq!(k.epoch(), p.epoch);
    k.compute_layout_presented(1, offer(), Some(p)).unwrap();
    assert_eq!(k.node(5).unwrap().frame.width, 180.0);
    assert_eq!(h(&k, 5), 60.0);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(k.node(5).unwrap().frame.width, 300.0);
    assert_eq!(h(&k, 5), 100.0);
}

fn rejected_without_publication(k: &mut Kernel, root: u32, offer: Offer, p: PresentedHeight) {
    let bytes = k.export(None).unwrap();
    assert!(k.compute_layout_presented(root, offer, Some(p)).is_err());
    assert_eq!(k.export(None).unwrap(), bytes);
}

#[test]
fn invalid_samples_and_offers_do_not_displace_valid_cached_projection() {
    let mut k = tree();
    project(&mut k, 2, 250.0);
    let p = sample(&k, 2, 250.0);
    for px in [-1.0, f32::NAN, f32::INFINITY] {
        rejected_without_publication(&mut k, 1, offer(), PresentedHeight { px, ..p });
    }
    rejected_without_publication(&mut k, 99, offer(), p);
    rejected_without_publication(&mut k, 2, offer(), p);
    rejected_without_publication(&mut k, 1, Offer::definite(f32::NAN, 600.0), p);
    rejected_without_publication(
        &mut k,
        1,
        offer(),
        PresentedHeight {
            epoch: p.epoch - 1,
            ..p
        },
    );
    assert!(k
        .compute_layout_presented(1, offer(), Some(p))
        .unwrap()
        .changed
        .is_empty());
    assert_eq!(h(&k, 2), 250.0);
}

#[test]
fn stale_epoch_after_unrelated_commit_requires_fresh_sample() {
    let mut k = tree();
    project(&mut k, 2, 250.0);
    let p = sample(&k, 2, 250.0);
    k.apply(0, 2, &[height(3, 55.0)]).unwrap();
    rejected_without_publication(&mut k, 1, offer(), p);
    project(&mut k, 2, 250.0);
    assert_eq!(h(&k, 2), 250.0);
    assert_eq!(h(&k, 3), 55.0);
}

#[test]
fn detached_foreign_hidden_and_inline_nodes_are_not_eligible_boxes() {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 5,
                node_type: NodeType::View,
            },
            height(5, 50.0),
            Op::CreateView {
                id: 6,
                node_type: NodeType::View,
            },
            height(6, 50.0),
            Op::AttachRoot { id: 6 },
            Op::CreateView {
                id: 7,
                node_type: NodeType::Text,
            },
            height(7, 50.0),
            Op::SetChildren {
                id: 4,
                children: vec![7],
            },
            style(3, &[(StyleId::Display, StyleValue::Text("none".into()))]),
        ],
    )
    .unwrap();
    project(&mut k, 2, 250.0);
    for id in [5, 6, 7, 3] {
        let p = sample(&k, id, 60.0);
        rejected_without_publication(&mut k, 1, offer(), p);
    }
    project(&mut k, 2, 250.0);
    assert_eq!(h(&k, 2), 250.0);
}

#[test]
fn deleted_reused_and_reset_keys_cannot_reapply_old_height() {
    let mut k = tree();
    project(&mut k, 2, 250.0);
    let old = sample(&k, 2, 250.0);
    k.apply(
        0,
        2,
        &[
            Op::DestroyView { id: 2 },
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            height(2, 75.0),
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
        ],
    )
    .unwrap();
    let p = PresentedHeight {
        epoch: k.epoch(),
        ..old
    };
    rejected_without_publication(&mut k, 1, offer(), p);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&k, 2), 75.0);
    k.reset();
    k.apply(
        0,
        3,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            height(1, 80.0),
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    let p = PresentedHeight {
        epoch: k.epoch(),
        ..old
    };
    rejected_without_publication(&mut k, 1, offer(), p);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&k, 1), 80.0);
}

#[test]
fn unsupported_authored_height_requires_explicit_clear_not_stale_projection() {
    for value in [
        StyleValue::Auto,
        StyleValue::Percent(50.0),
        StyleValue::Number(-1.0),
        StyleValue::Text("env(safe-area-inset-top)".into()),
    ] {
        let mut k = tree();
        project(&mut k, 2, 250.0);
        k.apply(0, 2, &[style(2, &[(StyleId::Height, value)])])
            .unwrap();
        let p = sample(&k, 2, 250.0);
        rejected_without_publication(&mut k, 1, offer(), p);
        k.compute_layout(1, offer()).unwrap();
        let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
        fresh.compute_layout(1, offer()).unwrap();
        assert_eq!(k.node(2).unwrap().frame, fresh.node(2).unwrap().frame);
    }
    let mut k = tree();
    project(&mut k, 2, 250.0);
    let mut mask = StyleMask::EMPTY;
    mask.set(StyleId::Height);
    k.apply(0, 2, &[Op::ClearStyle { id: 2, mask }]).unwrap();
    let p = sample(&k, 2, 250.0);
    rejected_without_publication(&mut k, 1, offer(), p);
    k.compute_layout(1, offer()).unwrap();
    assert_ne!(h(&k, 2), 250.0);
}

#[test]
fn failed_measurement_preserves_publication_and_retries_same_sample() {
    let calls = Rc::new(Cell::new(0));
    let invalid = Rc::new(Cell::new(false));
    let mut k = build(Box::new(Counted {
        calls: calls.clone(),
        invalid: invalid.clone(),
    }));
    project(&mut k, 2, 250.0);
    k.apply(
        0,
        2,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "changed".into(),
        }],
    )
    .unwrap();
    let before = k.export(None).unwrap();
    let content = k.node(2).unwrap().content;
    let p = sample(&k, 2, 333.0);
    invalid.set(true);
    assert_eq!(
        k.compute_layout_presented(1, offer(), Some(p)),
        Err(KernelError::Layout(LayoutError::InvalidTextMetrics(4)))
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(k.node(2).unwrap().content, content);
    let count = calls.get();
    invalid.set(false);
    k.compute_layout_presented(1, offer(), Some(p)).unwrap();
    assert!(calls.get() > count);
    assert_eq!(h(&k, 2), 333.0);
}

#[test]
fn rehydrate_needs_same_projection_for_equal_presented_geometry() {
    let mut k = tree();
    project(&mut k, 2, 333.25);
    let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    let p = sample(&fresh, 2, 333.25);
    fresh.compute_layout_presented(1, offer(), Some(p)).unwrap();
    for id in 1..=4 {
        assert_eq!(k.node(id).unwrap().frame, fresh.node(id).unwrap().frame);
    }
    fresh.compute_layout(1, offer()).unwrap();
    k.compute_layout(1, offer()).unwrap();
    for id in 1..=4 {
        assert_eq!(k.node(id).unwrap().frame, fresh.node(id).unwrap().frame);
    }
}

fn add_second_root(k: &mut Kernel) {
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 10,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 11,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 12,
                node_type: NodeType::Text,
            },
            height(10, 400.0),
            height(11, 90.0),
            Op::SetProp {
                id: 12,
                prop: PropId::Text,
                value: "second root text".into(),
            },
            Op::SetChildren {
                id: 11,
                children: vec![12],
            },
            Op::SetChildren {
                id: 10,
                children: vec![11],
            },
            Op::AttachRoot { id: 10 },
        ],
    )
    .unwrap();
    k.compute_layout(1, offer()).unwrap();
    k.compute_layout(10, offer()).unwrap();
}

fn root_geometry(k: &Kernel, root: u32) -> Vec<(exact_kernel::Frame, (f32, f32))> {
    k.rows(Some(root))
        .unwrap()
        .into_iter()
        .map(|row| {
            let node = k.node(row.id).unwrap();
            (node.frame, node.content)
        })
        .collect()
}

#[test]
fn two_roots_switch_and_clear_publish_only_addressed_root_after_atomic_preflight() {
    let mut k = tree();
    add_second_root(&mut k);
    project(&mut k, 2, 250.0);
    let a_published = root_geometry(&k, 1);
    let b = sample(&k, 11, 120.0);
    let receipt = k.compute_layout_presented(10, offer(), Some(b)).unwrap();
    assert_eq!(receipt.root, k.node(10).unwrap().key);
    assert!(receipt
        .changed
        .iter()
        .all(|key| [10, 11, 12].contains(&k.node_by_key(*key).unwrap().id)));
    assert_eq!(h(&k, 11), 120.0);
    assert_eq!(
        root_geometry(&k, 1),
        a_published,
        "restoring A's derived style must not publish A in B's receipt"
    );

    // An invalid root/offer/foreign-node sample cannot clear the active B value.
    rejected_without_publication(&mut k, 1, offer(), b);
    let a = sample(&k, 2, 275.0);
    rejected_without_publication(&mut k, 10, offer(), a);
    let before = k.export(None).unwrap();
    assert!(k.compute_layout_presented(99, offer(), None).is_err());
    assert!(k
        .compute_layout_presented(1, Offer::definite(f32::INFINITY, 600.0), None)
        .is_err());
    assert_eq!(k.export(None).unwrap(), before);
    assert!(k
        .compute_layout_presented(10, offer(), Some(b))
        .unwrap()
        .changed
        .is_empty());
    assert_eq!(h(&k, 11), 120.0);

    let b_published = root_geometry(&k, 10);
    let receipt = k.compute_layout_presented(1, offer(), None).unwrap();
    assert_eq!(receipt.root, k.node(1).unwrap().key);
    assert!(receipt
        .changed
        .iter()
        .all(|key| (1..=4).contains(&k.node_by_key(*key).unwrap().id)));
    assert_eq!(h(&k, 2), 180.0);
    assert_eq!(root_geometry(&k, 10), b_published);
    k.compute_layout(10, offer()).unwrap();
    assert_eq!(h(&k, 11), 90.0);
    assert_eq!(h(&k, 2), 180.0);
}

#[test]
fn failed_cross_root_switch_preserves_both_publications_and_recovers_current_targets() {
    let calls = Rc::new(Cell::new(0));
    let invalid = Rc::new(Cell::new(false));
    let mut k = build(Box::new(Counted {
        calls: calls.clone(),
        invalid: invalid.clone(),
    }));
    add_second_root(&mut k);
    project(&mut k, 2, 250.0);
    k.apply(
        0,
        3,
        &[
            height(2, 333.0),
            Op::SetProp {
                id: 12,
                prop: PropId::Text,
                value: "invalidate B measurement".into(),
            },
        ],
    )
    .unwrap();
    let before = k.export(None).unwrap();
    let a_before = root_geometry(&k, 1);
    let b_before = root_geometry(&k, 10);
    let b = sample(&k, 11, 200.0);
    invalid.set(true);
    assert_eq!(
        k.compute_layout_presented(10, offer(), Some(b)),
        Err(KernelError::Layout(LayoutError::InvalidTextMetrics(12)))
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(root_geometry(&k, 1), a_before);
    assert_eq!(root_geometry(&k, 10), b_before);
    let failed_calls = calls.get();
    invalid.set(false);

    project(&mut k, 2, 275.0);
    assert!(
        calls.get() > failed_calls,
        "failed switch must invalidate derived measurement state"
    );
    assert_eq!(h(&k, 2), 275.0);
    assert_eq!(k.node(2).unwrap().style.height, Dimension::Points(333.0));
    assert_eq!(root_geometry(&k, 10), b_before);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(
        h(&k, 2),
        333.0,
        "clear restores the latest authoring, not A's original 180"
    );
    k.compute_layout_presented(10, offer(), Some(b)).unwrap();
    assert_eq!(h(&k, 11), 200.0);
    assert_eq!(h(&k, 2), 333.0);
    k.compute_layout(10, offer()).unwrap();
    assert_eq!(h(&k, 11), 90.0);
}

fn constrained_panel(box_sizing: &str, minimum: f64) -> Kernel {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[
            style(1, &[(StyleId::Height, StyleValue::Percent(100.0))]),
            style(
                2,
                &[
                    (StyleId::PositionType, StyleValue::Text("absolute".into())),
                    (StyleId::Bottom, StyleValue::Number(0.0)),
                    (StyleId::Width, StyleValue::Number(300.0)),
                    (StyleId::BoxSizing, StyleValue::Text(box_sizing.into())),
                    (StyleId::MaxHeight, StyleValue::Percent(100.0)),
                    (StyleId::MinHeight, StyleValue::Number(minimum)),
                    (StyleId::PaddingTop, StyleValue::Number(7.0)),
                    (StyleId::PaddingBottom, StyleValue::Number(11.0)),
                    (StyleId::BorderWidthTop, StyleValue::Number(3.0)),
                    (StyleId::BorderWidthBottom, StyleValue::Number(3.0)),
                    (StyleId::BorderStyleTop, StyleValue::Text("solid".into())),
                    (StyleId::BorderStyleBottom, StyleValue::Text("solid".into())),
                ],
            ),
        ],
    )
    .unwrap();
    k
}

#[test]
fn absolute_panel_percent_max_tracks_same_epoch_offer_and_authored_box_oracle() {
    for sizing in ["border-box", "content-box"] {
        let mut projected = constrained_panel(sizing, 0.0);
        let mut authored = constrained_panel(sizing, 0.0);
        authored.apply(0, 3, &[height(2, 420.0)]).unwrap();
        let p = sample(&projected, 2, 420.0);
        let original_style = projected.node(2).unwrap().style.clone();
        for available in [600.0, 256.0, 105.0, 600.0] {
            let offer = Offer::definite(400.0, available);
            authored.compute_layout(1, offer).unwrap();
            projected
                .compute_layout_presented(1, offer, Some(p))
                .unwrap();
            assert_eq!(
                projected.epoch(),
                p.epoch,
                "offer changes do not refresh the authored epoch"
            );
            assert_eq!(projected.node(2).unwrap().style, &original_style);
            assert_eq!(
                root_geometry(&projected, 1),
                root_geometry(&authored, 1),
                "{sizing}, offer {available}"
            );
            let outer = 420.0_f32.min(available) + if sizing == "content-box" { 24.0 } else { 0.0 };
            assert_eq!(
                h(&projected, 2),
                outer,
                "padding18 + solid borders6 follow current box sizing"
            );
            assert_eq!(projected.node(2).unwrap().frame.y, available - outer);
            assert!(projected
                .compute_layout_presented(1, offer, Some(p))
                .unwrap()
                .changed
                .is_empty());
        }
    }
}

#[test]
fn content_box_minimum_wins_over_percentage_maximum_like_authored_height() {
    let mut projected = constrained_panel("content-box", 300.0);
    let mut authored = constrained_panel("content-box", 300.0);
    let offer = Offer::definite(400.0, 200.0);
    for px in [100.5, 420.0] {
        authored.apply(0, 3, &[height(2, px as f64)]).unwrap();
        authored.compute_layout(1, offer).unwrap();
        let p = sample(&projected, 2, px);
        projected
            .compute_layout_presented(1, offer, Some(p))
            .unwrap();
        assert_eq!(root_geometry(&projected, 1), root_geometry(&authored, 1));
        assert_eq!(h(&projected, 2), 324.0);
        assert_eq!(projected.node(2).unwrap().frame.y, -124.0);
        assert_eq!(
            projected.node(2).unwrap().style.height,
            Dimension::Points(180.0)
        );
    }
}

fn auto_width_root() -> Kernel {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            style(
                1,
                &[
                    (StyleId::Height, StyleValue::Number(180.0)),
                    (StyleId::BoxSizing, StyleValue::Text("content-box".into())),
                    (StyleId::PaddingTop, StyleValue::Number(7.0)),
                    (StyleId::PaddingBottom, StyleValue::Number(11.0)),
                    (StyleId::MaxHeight, StyleValue::Number(240.0)),
                ],
            ),
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

#[test]
fn auto_width_root_retains_ordinary_lowerings_border_box_exception() {
    let mut projected = auto_width_root();
    let mut authored = auto_width_root();
    let original_style = projected.node(1).unwrap().style.clone();
    assert_eq!(original_style.width, Dimension::Auto);
    assert_eq!(
        original_style.box_sizing,
        exact_kernel::BoxSizing::ContentBox
    );
    for (px, expected) in [(100.5, 100.5), (420.0, 240.0)] {
        authored.apply(0, 2, &[height(1, px as f64)]).unwrap();
        authored.compute_layout(1, offer()).unwrap();
        project(&mut projected, 1, px);
        assert_eq!(root_geometry(&projected, 1), root_geometry(&authored, 1));
        assert_eq!(h(&projected, 1), expected);
        assert_eq!(projected.node(1).unwrap().style, &original_style);
    }
    projected.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&projected, 1), 180.0);
}
