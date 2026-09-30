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
    k.compute_layout_presented(1, offer(), &[p]).unwrap();
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
            .compute_layout_presented(1, offer(), &[p])
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
    k.compute_layout_presented(1, offer(), &[]).unwrap();
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
    k.compute_layout_presented(1, offer(), &[p]).unwrap();
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
    k.compute_layout_presented(1, offer(), &[p]).unwrap();
    assert_eq!(k.node(5).unwrap().frame.width, 180.0);
    assert_eq!(h(&k, 5), 60.0);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(k.node(5).unwrap().frame.width, 300.0);
    assert_eq!(h(&k, 5), 100.0);
}

fn rejected_without_publication(k: &mut Kernel, root: u32, offer: Offer, p: PresentedHeight) {
    let bytes = k.export(None).unwrap();
    assert!(k.compute_layout_presented(root, offer, &[p]).is_err());
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
        .compute_layout_presented(1, offer(), &[p])
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
    let samples = [sample(&k, 2, 333.0), sample(&k, 3, 70.0)];
    invalid.set(true);
    assert_eq!(
        k.compute_layout_presented(1, offer(), &samples),
        Err(KernelError::Layout(LayoutError::InvalidTextMetrics(4)))
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(k.node(2).unwrap().content, content);
    let count = calls.get();
    invalid.set(false);
    k.compute_layout_presented(1, offer(), &samples).unwrap();
    assert!(calls.get() > count);
    assert_eq!(h(&k, 2), 333.0);
    assert_eq!(h(&k, 3), 70.0);
    assert_eq!(k.node(3).unwrap().frame.y, 333.0);
}

#[test]
fn rehydrate_needs_same_projection_for_equal_presented_geometry() {
    let mut k = tree();
    project(&mut k, 2, 333.25);
    let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    let p = sample(&fresh, 2, 333.25);
    fresh.compute_layout_presented(1, offer(), &[p]).unwrap();
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
    let receipt = k.compute_layout_presented(10, offer(), &[b]).unwrap();
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
    assert!(k.compute_layout_presented(99, offer(), &[]).is_err());
    assert!(k
        .compute_layout_presented(1, Offer::definite(f32::INFINITY, 600.0), &[])
        .is_err());
    assert_eq!(k.export(None).unwrap(), before);
    assert!(k
        .compute_layout_presented(10, offer(), &[b])
        .unwrap()
        .changed
        .is_empty());
    assert_eq!(h(&k, 11), 120.0);

    let b_published = root_geometry(&k, 10);
    let receipt = k.compute_layout_presented(1, offer(), &[]).unwrap();
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
        k.compute_layout_presented(10, offer(), &[b]),
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
    k.compute_layout_presented(10, offer(), &[b]).unwrap();
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
            projected.compute_layout_presented(1, offer, &[p]).unwrap();
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
                .compute_layout_presented(1, offer, &[p])
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
        projected.compute_layout_presented(1, offer, &[p]).unwrap();
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
fn auto_width_root_keeps_its_authored_box_sizing() {
    let mut projected = auto_width_root();
    let mut authored = auto_width_root();
    let original_style = projected.node(1).unwrap().style.clone();
    assert_eq!(original_style.width, Dimension::Auto);
    assert_eq!(
        original_style.box_sizing,
        exact_kernel::BoxSizing::ContentBox
    );
    // Content-box heights under 18 px of padding; `max-height: 240px` clamps the second.
    for (px, expected) in [(100.5, 118.5), (420.0, 258.0)] {
        authored.apply(0, 2, &[height(1, px as f64)]).unwrap();
        authored.compute_layout(1, offer()).unwrap();
        project(&mut projected, 1, px);
        assert_eq!(root_geometry(&projected, 1), root_geometry(&authored, 1));
        assert_eq!(h(&projected, 1), expected);
        assert_eq!(projected.node(1).unwrap().style, &original_style);
    }
    projected.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&projected, 1), 198.0);
}

#[test]
fn concurrent_sections_move_following_content_and_retire_independently() {
    let mut k = tree();
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 5,
                node_type: NodeType::View,
            },
            height(5, 20.0),
            Op::SetChildren {
                id: 1,
                children: vec![2, 3, 5],
            },
        ],
    )
    .unwrap();
    let epoch = k.epoch();
    let receipts = k.receipts().count();
    // Opposing section trajectories share one published layout at each step.
    for step in 0..=120 {
        let a = 240.0 - step as f32;
        let b = 30.0 + 2.0 * step as f32;
        let samples = [sample(&k, 2, a), sample(&k, 3, b)];
        k.compute_layout_presented(1, offer(), &samples).unwrap();
        assert_eq!((h(&k, 2), h(&k, 3)), (a, b));
        assert_eq!(k.node(3).unwrap().frame.y, a);
        assert_eq!(k.node(5).unwrap().frame.y, a + b);
        assert_eq!(k.epoch(), epoch);
        assert_eq!(k.receipts().count(), receipts);
    }
    // A target change during presentation does not snap either section.
    k.apply(0, 3, &[height(2, 320.0), height(3, 90.0)]).unwrap();
    let samples = [sample(&k, 2, 120.0), sample(&k, 3, 270.0)];
    assert!(k
        .compute_layout_presented(1, offer(), &samples)
        .unwrap()
        .changed
        .is_empty());
    k.compute_layout_presented(1, offer(), &samples[1..])
        .unwrap();
    assert_eq!((h(&k, 2), h(&k, 3)), (320.0, 270.0));
    assert_eq!(k.node(5).unwrap().frame.y, 590.0);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!((h(&k, 2), h(&k, 3)), (320.0, 90.0));
    assert_eq!(k.node(5).unwrap().frame.y, 410.0);
}

#[test]
fn reordered_equal_sections_do_not_remeasure_or_republish() {
    let calls = Rc::new(Cell::new(0));
    let mut k = build(Box::new(Counted {
        calls: calls.clone(),
        invalid: Rc::default(),
    }));
    let samples = [sample(&k, 2, 220.0), sample(&k, 3, 110.0)];
    k.compute_layout_presented(1, offer(), &samples).unwrap();
    let count = calls.get();
    assert!(count > 0);
    for samples in [samples, [samples[1], samples[0]]].iter().cycle().take(120) {
        assert!(k
            .compute_layout_presented(1, offer(), samples)
            .unwrap()
            .changed
            .is_empty());
    }
    assert_eq!(calls.get(), count);
}

#[test]
fn one_invalid_section_rejects_entire_sample_set_without_revoking_prior_owners() {
    let mut k = tree();
    let samples = [sample(&k, 2, 220.0), sample(&k, 3, 110.0)];
    k.compute_layout_presented(1, offer(), &samples).unwrap();
    for bad in [
        PresentedHeight {
            px: f32::NAN,
            ..samples[1]
        },
        PresentedHeight {
            epoch: samples[1].epoch - 1,
            ..samples[1]
        },
        PresentedHeight {
            px: 90.0,
            ..samples[0]
        }, // Duplicate owner, different sample.
    ] {
        let before = k.export(None).unwrap();
        let replacement = [
            PresentedHeight {
                px: 999.0,
                ..samples[0]
            },
            bad,
        ];
        assert!(k
            .compute_layout_presented(1, offer(), &replacement)
            .is_err());
        assert_eq!(k.export(None).unwrap(), before);
        assert!(k
            .compute_layout_presented(1, offer(), &samples)
            .unwrap()
            .changed
            .is_empty());
    }
    let before = k.export(None).unwrap();
    assert_eq!(
        k.compute_layout_presented(1, offer(), &[samples[0], samples[0]]),
        Err(KernelError::Layout(LayoutError::DuplicatePresentedHeight(
            samples[0].node
        )))
    );
    assert_eq!(k.export(None).unwrap(), before);
}

#[test]
fn removing_one_projected_section_keeps_the_other_sample_and_current_authored_target() {
    let mut k = tree();
    let samples = [sample(&k, 2, 220.0), sample(&k, 3, 110.0)];
    k.compute_layout_presented(1, offer(), &samples).unwrap();
    k.apply(
        0,
        2,
        &[
            Op::SetChildren {
                id: 1,
                children: vec![3],
            },
            Op::DestroyView { id: 2 },
            height(3, 60.0),
        ],
    )
    .unwrap();
    let remaining = sample(&k, 3, 130.0);
    k.compute_layout_presented(1, offer(), &[remaining])
        .unwrap();
    assert_eq!(h(&k, 3), 130.0);
    assert_eq!(k.node(3).unwrap().frame.y, 0.0);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(h(&k, 3), 60.0);
}

fn automatic_sections(k: &mut Kernel) {
    k.apply(0, 2, &[
        style(2, &[
            (StyleId::Height, StyleValue::Auto),
            (StyleId::BoxSizing, StyleValue::Text("border-box".into())),
            (StyleId::PaddingTop, StyleValue::Number(6.0)),
            (StyleId::PaddingBottom, StyleValue::Number(8.0)),
        ]),
        style(3, &[(StyleId::BoxSizing, StyleValue::Text("border-box".into()))]),
        Op::SetProp { id: 4, prop: PropId::Text, value: "Content sized accordion text that wraps as the product viewport becomes narrower.".into() },
    ]).unwrap();
}

#[test]
fn authored_auto_targets_measure_without_publishing_or_displacing_concurrent_samples() {
    let mut k = tree();
    automatic_sections(&mut k);
    let samples = [sample(&k, 2, 14.0), sample(&k, 3, 15.0)];
    k.compute_layout_presented(1, offer(), &samples).unwrap();
    let before = k.export(None).unwrap();
    let content = k.node(2).unwrap().content;
    let epoch = k.epoch();
    let receipts = k.receipts().count();
    let owners = samples.map(|p| p.node);
    let measured = k.measure_height_targets(1, offer(), &owners).unwrap();
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(k.node(2).unwrap().content, content);
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.receipts().count(), receipts);
    assert_eq!(k.node(2).unwrap().style.height, Dimension::Auto);
    // Authoring stays untouched and the old numeric-only host admission remains explicit.
    assert!(k.height_target(owners[0]).is_none());
    let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    oracle.compute_layout(1, offer()).unwrap();
    assert_eq!(
        measured,
        [sample(&k, 2, h(&oracle, 2)), sample(&k, 3, h(&oracle, 3))]
    );
    assert!(measured[0].px > samples[0].px);
    assert!(k
        .compute_layout_presented(1, offer(), &samples)
        .unwrap()
        .changed
        .is_empty());
    // The measured auto target is directly usable as an explicit projection.
    k.compute_layout_presented(1, offer(), &measured).unwrap();
    for id in 1..=4 {
        assert_eq!(k.node(id).unwrap().frame, oracle.node(id).unwrap().frame);
    }
}

#[test]
fn height_target_measurement_tracks_text_and_width_without_publishing_them() {
    let calls = Rc::new(Cell::new(0));
    let mut k = build(Box::new(Counted {
        calls: calls.clone(),
        invalid: Rc::default(),
    }));
    automatic_sections(&mut k);
    let owner = k.node(2).unwrap().key;
    project(&mut k, 2, 14.0);
    let wide = k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px;
    let count = calls.get();
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px,
        wide
    );
    assert_eq!(calls.get(), count, "same text offers reuse measured leaves");
    k.apply(0, 3, &[style(1, &[(StyleId::Width, StyleValue::Auto)])])
        .unwrap();
    let epoch = k.epoch();
    let before_resize = k.export(None).unwrap();
    let narrow_offer = k
        .measure_height_targets(1, Offer::definite(100.0, 600.0), &[owner])
        .unwrap()[0]
        .px;
    assert!(narrow_offer > wide);
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.export(None).unwrap(), before_resize);
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px,
        wide
    );
    k.apply(
        0,
        4,
        &[style(1, &[(StyleId::Width, StyleValue::Number(100.0))])],
    )
    .unwrap();
    let before = k.export(None).unwrap();
    let narrow = k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px;
    assert!(narrow > wide);
    assert_eq!(k.export(None).unwrap(), before);
    k.apply(
        0,
        5,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "Short".into(),
        }],
    )
    .unwrap();
    let before = k.export(None).unwrap();
    let short = k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px;
    assert!(short < narrow);
    assert_eq!(k.export(None).unwrap(), before);
    let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    oracle.compute_layout(1, offer()).unwrap();
    assert_eq!(short, h(&oracle, 2));
}

#[test]
fn failed_target_measurement_preserves_publication_and_retries_host_metrics() {
    let calls = Rc::new(Cell::new(0));
    let invalid = Rc::new(Cell::new(false));
    let mut k = build(Box::new(Counted {
        calls: calls.clone(),
        invalid: invalid.clone(),
    }));
    automatic_sections(&mut k);
    let owner = k.node(2).unwrap().key;
    project(&mut k, 2, 14.0);
    k.apply(
        0,
        3,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "A new description forces fresh measurement.".into(),
        }],
    )
    .unwrap();
    let before = k.export(None).unwrap();
    invalid.set(true);
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner]),
        Err(KernelError::Layout(LayoutError::InvalidTextMetrics(4)))
    );
    assert_eq!(k.export(None).unwrap(), before);
    let count = calls.get();
    invalid.set(false);
    let target = k.measure_height_targets(1, offer(), &[owner]).unwrap();
    assert!(calls.get() > count);
    assert!(target[0].px > 14.0);
    assert_eq!(k.export(None).unwrap(), before);
    project(&mut k, 2, 14.0);
    assert_eq!(h(&k, 2), 14.0);
}

#[test]
fn height_measurement_preflight_is_atomic_and_refuses_unsupported_box_models() {
    let mut k = tree();
    automatic_sections(&mut k);
    project(&mut k, 2, 14.0);
    let owner = k.node(2).unwrap().key;
    let unsupported = k.node(1).unwrap().key;
    let before = k.export(None).unwrap();
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner, unsupported]),
        Err(KernelError::Layout(
            LayoutError::UnsupportedHeightMeasurement(unsupported)
        ))
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner, owner]),
        Err(KernelError::Layout(LayoutError::DuplicatePresentedHeight(
            owner
        )))
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert!(k
        .measure_height_targets(1, Offer::definite(f32::NAN, 600.0), &[owner])
        .is_err());
    assert_eq!(k.export(None).unwrap(), before);
    assert!(k.measure_height_targets(99, offer(), &[owner]).is_err());
    assert_eq!(k.export(None).unwrap(), before);
    assert!(k
        .measure_height_targets(1, offer(), &[])
        .unwrap()
        .is_empty());
    assert_eq!(k.export(None).unwrap(), before);
    let p = sample(&k, 2, 14.0);
    assert!(k
        .compute_layout_presented(1, offer(), &[p])
        .unwrap()
        .changed
        .is_empty());
}

#[test]
fn measured_targets_include_current_constraints_and_same_epoch_image_size() {
    let mut k = tree();
    automatic_sections(&mut k);
    k.apply(
        0,
        3,
        &[
            Op::CreateView {
                id: 5,
                node_type: NodeType::Image,
            },
            style(5, &[(StyleId::Width, StyleValue::Number(100.0))]),
            Op::SetChildren {
                id: 2,
                children: vec![5],
            },
            style(2, &[(StyleId::MaxHeight, StyleValue::Number(150.0))]),
        ],
    )
    .unwrap();
    k.set_intrinsic_size(5, Some((100.0, 50.0))).unwrap();
    project(&mut k, 2, 14.0);
    let owner = k.node(2).unwrap().key;
    let epoch = k.epoch();
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px,
        64.0
    );
    k.set_intrinsic_size(5, Some((100.0, 300.0))).unwrap();
    assert_eq!(k.epoch(), epoch);
    let before = k.export(None).unwrap();
    assert_eq!(
        k.measure_height_targets(1, offer(), &[owner]).unwrap()[0].px,
        150.0
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(h(&k, 2), 14.0);
}
