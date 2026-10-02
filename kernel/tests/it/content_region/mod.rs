#![allow(clippy::field_reassign_with_default)]
//! Kernel publication only: manual paragraph completion, no host/worker claim.
mod support;
use exact_kernel::*;
use std::rc::Rc;
use support::*;

#[test]
fn first_miss_publishes_shell_and_real_pending_branch() {
    let mut k = fixture();
    let before_handles = handles(&k);
    register(&mut k);
    let r = pass(&mut k, 400., 1);
    assert!(!r.current);
    assert!(matches!(r.selection, RegionSelection::Pending(_)));
    assert_eq!(k.node(1).unwrap().frame.width, 400.);
    assert!(k.node(5).unwrap().frame.height > 0.);
    assert_eq!(k.node(4).unwrap().frame, Frame::default());
    assert!(k.region_text_request().is_some());
    assert_eq!(
        handles(&k),
        before_handles,
        "derived map must not replace arena handles"
    );
}

#[test]
fn offer_discovery_retries_discard_all_candidate_caches_and_pin_ready() {
    let mut k = fixture();
    register(&mut k);
    for delivered in 0..64 {
        let r = pass(&mut k, 400., 1);
        if r.current {
            assert!(delivered > 0);
            assert!(r.current_frame(key(&k, 4)).is_some());
            return;
        }
        let request = k.region_text_request().unwrap().clone();
        let clone = request.clone();
        let m = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        assert!(k
            .resolve_region_text(&request, m, Rc::new(delivered))
            .unwrap());
        assert!(
            !k.resolve_region_text(&clone, m, Rc::new(999)).unwrap(),
            "duplicate completion"
        );
    }
    panic!("discard/retry must discover finite offers without losing earlier answers");
}

#[test]
fn pending_resize_keeps_old_local_width_artifact_and_new_shell_origin() {
    let mut k = fixture();
    register(&mut k);
    let old = ready(&mut k, 400., 1);
    let old_frame = old.current_frame(key(&k, 4)).unwrap();
    let request = k.region_text_request().cloned();
    assert!(request.is_none());
    let r = pass(&mut k, 300., 1);
    assert!(!r.current);
    assert!(r.current_frame(key(&k, 4)).is_none());
    let RegionSelection::Accepted(p) = &r.selection else {
        panic!("retain accepted")
    };
    assert_eq!(
        p.frame(key(&k, 4), r.origin).unwrap().width,
        old_frame.width
    );
    assert!(!p.artifacts().is_empty());
    assert_eq!(k.node(1).unwrap().frame.width, 300.);
    assert_eq!(
        p.ticket(),
        match &old.selection {
            RegionSelection::Accepted(p) => p.ticket(),
            _ => unreachable!(),
        }
    );
}

#[test]
fn unrelated_typing_does_not_invalidate_request_but_source_and_catalog_do() {
    let mut k = fixture();
    register(&mut k);
    pass(&mut k, 400., 1);
    let old = k.region_text_request().unwrap().clone();
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 6,
            prop: PropId::Text,
            value: "typed".into(),
        }],
    )
    .unwrap();
    pass(&mut k, 400., 1);
    assert_eq!(k.region_text_request().unwrap().ticket(), old.ticket());
    let m = old.with_request(|r| MonospaceMeasurer::default().measure(r));
    assert!(k.resolve_region_text(&old, m, Rc::new(())).unwrap());
    pass(&mut k, 400., 2);
    assert!(!k.resolve_region_text(&old, m, Rc::new(())).unwrap());
    let next = k.region_text_request().unwrap().clone();
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "replacement source".into(),
        }],
    )
    .unwrap();
    assert!(!k.resolve_region_text(&next, m, Rc::new(())).unwrap());
}

#[test]
fn inherited_change_and_destruction_reject_stale_delivery_consumer_revision_republishes() {
    let mut k = fixture();
    register(&mut k);
    pass(&mut k, 400., 1);
    let first = k.region_text_request().unwrap().clone();
    let m = first.with_request(|r| MonospaceMeasurer::default().measure(r));
    let mut s = StyleProps::default();
    s.mask = mask(&[StyleId::TextAlign]);
    s.text_align = TextAlign::Right;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(s),
        }],
    )
    .unwrap();
    assert!(!k.resolve_region_text(&first, m, Rc::new(())).unwrap());
    pass(&mut k, 400., 1);
    let second = k.region_text_request().unwrap().clone();
    k.compute_region_layout(
        1,
        Offer::definite(400., 300.),
        RegionInputs {
            catalog: 1,
            consumer_revision: 2,
        },
    )
    .unwrap();
    assert_eq!(k.region_text_request().unwrap().ticket(), second.ticket());
    let third = k.region_text_request().unwrap().clone();
    k.apply(0, 0, &[Op::DestroyView { id: 2 }]).unwrap();
    assert!(!k.resolve_region_text(&third, m, Rc::new(())).unwrap());
    assert!(k.region_text_request().is_none());
}

#[test]
fn ready_matches_unsplit_geometry_and_preserves_inherited_styles() {
    let mut ordinary = fixture();
    let mut region = fixture();
    ordinary
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    register(&mut region);
    let r = ready(&mut region, 400., 1);
    for id in [1, 2, 3, 4, 6] {
        assert!(
            ordinary
                .node(id)
                .unwrap()
                .frame
                .bits_eq(region.node(id).unwrap().frame),
            "id {id}: {:?} {:?}",
            ordinary.node(id).unwrap().frame,
            region.node(id).unwrap().frame
        );
    }
    assert!(r.current);
}

#[test]
fn malformed_ready_metrics_do_not_consume_request_and_old_tokens_are_stale_first() {
    let mut k = fixture();
    register(&mut k);
    pass(&mut k, 400., 1);
    let req = k.region_text_request().unwrap().clone();
    let bad = TextMetrics {
        width: f32::NAN,
        ..Default::default()
    };
    assert!(k.resolve_region_text(&req, bad, Rc::new(())).is_err());
    assert_eq!(k.region_text_request().unwrap().ticket(), req.ticket());
    k.reset();
    assert!(!k.resolve_region_text(&req, bad, Rc::new(())).unwrap());
}

#[test]
fn registration_refuses_auto_sized_outer_box_and_ordinary_layout_cannot_bypass_gate() {
    let mut k = fixture();
    register(&mut k);
    assert!(k.compute_layout(1, Offer::definite(400., 300.)).is_err());
    k.set_content_region(None).unwrap();
    let mut s = StyleProps::default();
    s.mask = mask(&[StyleId::Height]);
    s.height = Dimension::Auto;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 2,
            patch: Box::new(s),
        }],
    )
    .unwrap();
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_err());
    // LLP 1074 T1: a static owner is refused too. A trial lays the owner
    // out as the top of its own tree, where it contains every absolutely
    // positioned descendant; the ordinary tree must agree.
    let mut k = fixture();
    let mut s = StyleProps::default();
    s.mask = mask(&[StyleId::PositionType]);
    s.position_type = PositionType::Static;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 2,
            patch: Box::new(s),
        }],
    )
    .unwrap();
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_err());
}

#[test]
fn immutable_request_round_trips_thread_and_all_offers_share_four_mib_source() {
    use std::sync::Arc;
    let mut k = fixture();
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "word ".repeat(840_000).into(),
        }],
    )
    .unwrap();
    let mut flex = StyleProps::default();
    flex.mask = mask(&[StyleId::Display]);
    flex.display = Display::Flex;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 3,
            patch: Box::new(flex),
        }],
    )
    .unwrap();
    register(&mut k);
    let mut canonical = None;
    for count in 0..64 {
        let r = pass(&mut k, 400., 1);
        if r.current {
            assert!(count > 1, "fixture must discover multiple offers");
            let RegionSelection::Accepted(p) = r.selection else {
                unreachable!()
            };
            assert!(p
                .artifacts()
                .iter()
                .all(|a| Arc::ptr_eq(a.request().source(), canonical.as_ref().unwrap())));
            return;
        }
        let request = k.region_text_request().unwrap().clone();
        if let Some(source) = &canonical {
            assert!(Arc::ptr_eq(source, request.source()))
        } else {
            canonical = Some(request.source().clone())
        }
        let (request, metrics) = std::thread::spawn(move || {
            // A fast deterministic manual answer, not a glyph-shaping claim.
            let metrics = request.with_request(|r| TextMetrics {
                width: match r.width {
                    AxisOffer::Definite(w) => w,
                    _ => 800.,
                },
                height: 1200.,
                first_baseline: Some(12.),
            });
            (request, metrics)
        })
        .join()
        .unwrap();
        assert!(request.source().bytes() > 4 * 1024 * 1024);
        assert!(k
            .resolve_region_text(&request, metrics, Rc::new(()))
            .unwrap());
    }
    panic!("four MiB must not be charged once per exact offer")
}

#[test]
fn accepted_source_outlives_destroyed_keys_without_reviving_dispatch() {
    use std::sync::Arc;
    let mut k = fixture();
    register(&mut k);
    let r = ready(&mut k, 400., 1);
    let RegionSelection::Accepted(p) = r.selection else {
        panic!()
    };
    let old = key(&k, 4);
    let weak = Arc::downgrade(p.artifacts()[0].request().source());
    k.apply(0, 0, &[Op::DestroyView { id: 4 }]).unwrap();
    assert!(k.node_by_key(old).is_none());
    assert!(weak.upgrade().is_some());
    k.reset();
    assert!(weak.upgrade().is_some());
    drop(p);
    assert!(weak.upgrade().is_none());
}

#[test]
fn source_budget_refusal_preserves_published_frames_and_accepted_publication() {
    let mut k = fixture();
    register(&mut k);
    let r = ready(&mut k, 400., 1);
    let before = frames(&k);
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "x"
                .repeat(exact_kernel::region::REGION_SOURCE_BYTES + 1)
                .into(),
        }],
    )
    .unwrap();
    assert!(k
        .compute_region_layout(
            1,
            Offer::definite(310., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1
            }
        )
        .is_err());
    assert_eq!(frames(&k), before);
    let RegionSelection::Accepted(p) = r.selection else {
        panic!()
    };
    assert!(p.frame(key(&k, 4), r.origin).is_some());
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "recovered".into(),
        }],
    )
    .unwrap();
    assert!(ready(&mut k, 310., 1).current);
}

#[test]
fn invalid_placeholder_metrics_publish_nothing_and_retry_recovers() {
    use std::cell::Cell;
    struct Toggle(Rc<Cell<bool>>);
    impl TextMeasurer for Toggle {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            if self.0.get() && r.runs.iter().any(|r| r.text == "Loading content") {
                TextMetrics {
                    width: f32::NAN,
                    ..Default::default()
                }
            } else {
                MonospaceMeasurer::default().measure(r)
            }
        }
    }
    let bad = Rc::new(Cell::new(true));
    let mut k = fixture_with(Box::new(Toggle(bad.clone())));
    register(&mut k);
    let before = frames(&k);
    assert!(k
        .compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1
            }
        )
        .is_err());
    assert_eq!(frames(&k), before);
    let request = k.region_text_request().unwrap().clone();
    bad.set(false);
    pass(&mut k, 400., 1);
    assert_eq!(k.region_text_request().unwrap().ticket(), request.ticket());
    assert!(ready(&mut k, 400., 1).current);
}

fn frames(k: &Kernel) -> Vec<(NodeKey, Frame, (f32, f32))> {
    k.arena()
        .iter_live()
        .map(|s| {
            let key = k.arena().key(s);
            let n = k.node_by_key(key).unwrap();
            (key, n.frame, n.content)
        })
        .collect()
}

#[test]
fn final_paint_offer_is_retained_and_old_plus_candidate_retention_is_explicit() {
    let mut k = fixture();
    register(&mut k);
    let first = ready(&mut k, 400., 1);
    let RegionSelection::Accepted(p) = &first.selection else {
        panic!()
    };
    let node = key(&k, 4);
    let artifact = p.paint_artifact(node).unwrap();
    assert_eq!(
        artifact.request().offer().width,
        AxisOffer::Definite(first.current_frame(node).unwrap().width)
    );
    let bytes = artifact.request().source().bytes();
    assert_eq!(k.region_retention().accepted_source_bytes, bytes);
    pass(&mut k, 300., 1);
    let retained = k.region_retention();
    assert_eq!(retained.accepted_source_bytes, bytes);
    assert_eq!(retained.candidate_source_bytes, bytes);
    assert_eq!(retained.shared_source_bytes, bytes);
    assert_eq!(retained.total_source_bytes, bytes);
    assert!(retained.accepted_offers > 0);
    assert!(retained.candidate_offers > 0);
}

#[test]
fn owner_layout_change_at_same_offer_invalidates_and_unusable_binding_keeps_gate() {
    let mut k = fixture();
    register(&mut k);
    pass(&mut k, 400., 1);
    let req = k.region_text_request().unwrap().clone();
    let mut s = StyleProps::default();
    s.mask = mask(&[StyleId::Display]);
    s.display = Display::Flex;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 2,
            patch: Box::new(s),
        }],
    )
    .unwrap();
    assert!(!k
        .resolve_region_text(&req, TextMetrics::default(), Rc::new(()))
        .unwrap());
    let mut s = StyleProps::default();
    s.mask = mask(&[StyleId::Height]);
    s.height = Dimension::Auto;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 2,
            patch: Box::new(s),
        }],
    )
    .unwrap();
    assert!(k.compute_layout(1, Offer::definite(400., 300.)).is_err());
    assert!(k
        .compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1
            }
        )
        .is_err());
}

#[test]
fn offer_count_refusal_keeps_publication_atomic_and_recovers_after_source_change() {
    let mut k = fixture();
    let mut ops = Vec::new();
    let mut children = Vec::new();
    for id in 10..80 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Text,
        });
        ops.push(Op::SetProp {
            id,
            prop: PropId::Text,
            value: "distinct paragraph".into(),
        });
        children.push(id);
    }
    ops.push(Op::SetChildren { id: 3, children });
    k.apply(0, 0, &ops).unwrap();
    register(&mut k);
    let mut refused = false;
    for _ in 0..=exact_kernel::region::REGION_OFFERS {
        let before = frames(&k);
        match k.compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1,
            },
        ) {
            Err(KernelError::Layout(LayoutError::ContentRegion(
                "exact-offer/source budget exhausted",
            ))) => {
                assert_eq!(before, frames(&k));
                refused = true;
                break;
            }
            Ok(r) => {
                assert!(!r.current);
                let q = k.region_text_request().unwrap().clone();
                let m = q.with_request(|r| MonospaceMeasurer::default().measure(r));
                k.resolve_region_text(&q, m, Rc::new(())).unwrap();
            }
            _ => panic!("unexpected result"),
        }
    }
    assert!(refused);
    assert_eq!(
        k.region_retention().candidate_offers,
        exact_kernel::region::REGION_OFFERS
    );
    k.apply(
        0,
        0,
        &[Op::SetChildren {
            id: 3,
            children: vec![4],
        }],
    )
    .unwrap();
    assert!(ready(&mut k, 400., 1).current);
}

#[test]
fn malformed_candidate_answer_preserves_existing_publication_and_valid_answer_recovers() {
    let mut k = fixture();
    register(&mut k);
    let accepted = ready(&mut k, 400., 1);
    pass(&mut k, 300., 1);
    let before = frames(&k);
    let q = k.region_text_request().unwrap().clone();
    assert!(k
        .resolve_region_text(
            &q,
            TextMetrics {
                height: -1.,
                ..Default::default()
            },
            Rc::new(())
        )
        .is_err());
    assert_eq!(frames(&k), before);
    let RegionSelection::Accepted(old) = &accepted.selection else {
        panic!()
    };
    let after = pass(&mut k, 300., 1);
    let RegionSelection::Accepted(still) = &after.selection else {
        panic!()
    };
    assert!(Rc::ptr_eq(old, still));
    let metrics = q.with_request(|r| MonospaceMeasurer::default().measure(r));
    assert!(k.resolve_region_text(&q, metrics, Rc::new(())).unwrap());
    assert!(ready(&mut k, 300., 1).current);
}

#[test]
fn consumer_epoch_recomputes_geometry_using_retained_answers_without_new_request() {
    let mut k = fixture();
    register(&mut k);
    let old = ready(&mut k, 400., 1);
    let r = k
        .compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 2,
            },
        )
        .unwrap();
    assert!(r.current);
    assert!(k.region_text_request().is_none());
    let RegionSelection::Accepted(before) = old.selection else {
        panic!()
    };
    let RegionSelection::Accepted(after) = r.selection else {
        panic!()
    };
    assert!(!Rc::ptr_eq(&before, &after));
    assert_eq!(before.ticket(), after.ticket());
    assert_eq!(before.inputs().consumer_revision, 1);
    assert_eq!(after.inputs().consumer_revision, 2);
    assert_eq!(k.region_retention().candidate_offers, 0);
}

#[test]
fn accepted_to_new_width_shares_canonical_source_without_offer_history() {
    use std::sync::Arc;
    let mut k = fixture();
    register(&mut k);
    let old = ready(&mut k, 400., 1);
    let RegionSelection::Accepted(p) = old.selection else {
        panic!()
    };
    let source = p.artifacts()[0].request().source().clone();
    pass(&mut k, 310., 1);
    let q = k.region_text_request().unwrap();
    assert!(
        Arc::ptr_eq(&source, q.source()),
        "width changes must share the exact immutable source"
    );
    for width in 310..380 {
        let r = ready(&mut k, width as f32, 1);
        let RegionSelection::Accepted(p) = r.selection else {
            panic!()
        };
        assert!(
            p.artifacts().len() < 8,
            "do not retain visited-width history"
        );
        assert!(p
            .artifacts()
            .iter()
            .all(|a| Arc::ptr_eq(&source, a.request().source())));
    }
}

#[test]
fn indefinite_percentage_height_is_refused_before_any_shell_publication() {
    let mut k = fixture();
    let mut root = StyleProps::default();
    root.mask = mask(&[StyleId::Height]);
    root.height = Dimension::Auto;
    let mut owner = StyleProps::default();
    owner.mask = mask(&[StyleId::Height]);
    owner.height = Dimension::Percent(100.);
    k.apply(
        0,
        0,
        &[
            Op::SetStyle {
                id: 1,
                patch: Box::new(root),
            },
            Op::SetStyle {
                id: 2,
                patch: Box::new(owner),
            },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
    let ordinary = frames(&k);
    assert!(k.node(4).unwrap().frame.height > 0.);
    let binding = binding(&k);
    assert!(
        k.set_content_region(Some(binding)).is_err(),
        "percent is not a definite containing-block certificate"
    );
    assert_eq!(ordinary, frames(&k));
}

#[test]
fn prior_height_projection_is_refused_without_clearing_its_publication() {
    let mut k = fixture();
    let owner = key(&k, 2);
    let sample = PresentedHeight {
        node: owner,
        epoch: k.epoch(),
        px: 120.,
    };
    k.compute_layout_presented(1, Offer::definite(400., 300.), &[sample])
        .unwrap();
    let before = frames(&k);
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_err());
    assert_eq!(frames(&k), before);
    k.compute_layout_presented(1, Offer::definite(400., 300.), &[sample])
        .unwrap();
    assert_eq!(frames(&k), before);
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
    register(&mut k);
    ready(&mut k, 400., 1);
    let before = frames(&k);
    assert!(k
        .compute_layout_presented(1, Offer::definite(400., 300.), &[sample])
        .is_err());
    assert_eq!(frames(&k), before);
}

fn two_regions() -> Kernel {
    let mut k = fixture();
    let owner = k.node(2).unwrap().style.clone();
    let pending = k.node(5).unwrap().style.clone();
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 8,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 9,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 10,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 11,
                node_type: NodeType::Text,
            },
            Op::SetStyle {
                id: 8,
                patch: Box::new(owner),
            },
            Op::SetStyle {
                id: 11,
                patch: Box::new(pending),
            },
            Op::SetProp {
                id: 10,
                prop: PropId::Text,
                value: "second candidate".into(),
            },
            Op::SetProp {
                id: 11,
                prop: PropId::Text,
                value: "pending B".into(),
            },
            Op::SetChildren {
                id: 9,
                children: vec![10],
            },
            Op::SetChildren {
                id: 8,
                children: vec![9, 11],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 8, 6],
            },
        ],
    )
    .unwrap();
    k
}
#[test]
fn replacing_region_restores_previous_owner_authored_children_before_shell_pass() {
    let mut ordinary = two_regions();
    ordinary
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    let expected: Vec<_> = [2, 3, 4, 5]
        .map(|id| (id, ordinary.node(id).unwrap().frame))
        .into();
    let mut k = two_regions();
    register(&mut k);
    ready(&mut k, 400., 1);
    let b = ContentRegion {
        owner: key(&k, 8),
        content: key(&k, 9),
        pending: key(&k, 11),
    };
    // Deliberately no intervening authored receipt to rebuild A's child edge.
    k.set_content_region(Some(b)).unwrap();
    let epoch = k.epoch();
    let r = pass(&mut k, 400., 1);
    assert!(!r.current);
    assert_eq!(epoch, k.epoch());
    for (id, frame) in expected {
        assert!(
            k.node(id).unwrap().frame.bits_eq(frame),
            "ordinary A id {id}: actual {:?}, expected {frame:?}",
            k.node(id).unwrap().frame
        );
    }
}
#[test]
fn invalid_region_replacement_preserves_previous_registration_cache_and_publication() {
    let mut k = two_regions();
    register(&mut k);
    let accepted = ready(&mut k, 400., 1);
    let before = frames(&k);
    let handles = handles(&k);
    let invalid = ContentRegion {
        owner: key(&k, 8),
        content: key(&k, 3),
        pending: key(&k, 11),
    };
    assert!(k.set_content_region(Some(invalid)).is_err());
    assert_eq!(before, frames(&k));
    assert_eq!(handles, support::handles(&k));
    let r = pass(&mut k, 400., 1);
    assert!(r.current);
    let RegionSelection::Accepted(old) = accepted.selection else {
        panic!()
    };
    let RegionSelection::Accepted(still) = r.selection else {
        panic!()
    };
    assert!(Rc::ptr_eq(&old, &still));
}

struct CatalogMetrics {
    height: Rc<std::cell::Cell<f32>>,
    calls: Rc<std::cell::Cell<usize>>,
}
impl TextMeasurer for CatalogMetrics {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        if r.runs.iter().any(|run| run.text == "shell") {
            self.calls.set(self.calls.get() + 1);
        }
        TextMetrics {
            width: 32.,
            height: self.height.get(),
            first_baseline: Some(self.height.get() * 0.8),
        }
    }
}

#[test]
fn review_catalog_change_remeasures_shell_before_publication_without_authored_commit() {
    let height = Rc::new(std::cell::Cell::new(20.));
    let calls = Rc::new(std::cell::Cell::new(0));
    let mut k = fixture_with(Box::new(CatalogMetrics {
        height: height.clone(),
        calls: calls.clone(),
    }));
    register(&mut k);
    pass(&mut k, 400., 1);
    assert_eq!(k.node(6).unwrap().frame.height, 20.);
    let first_calls = calls.get();
    pass(&mut k, 400., 1);
    assert_eq!(calls.get(), first_calls, "unchanged shell stays cached");
    let epoch = k.epoch();
    let old = k.region_text_request().unwrap().clone();
    height.set(40.);
    let mut oracle = fixture_with(Box::new(CatalogMetrics {
        height,
        calls: Rc::new(std::cell::Cell::new(0)),
    }));
    oracle
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    pass(&mut k, 400., 2);
    assert_eq!(k.epoch(), epoch);
    for id in [1, 2, 6] {
        assert_eq!(
            k.node(id).unwrap().frame,
            oracle.node(id).unwrap().frame,
            "shell id {id} must use the new catalog"
        );
    }
    assert!(calls.get() > first_calls);
    assert!(!k
        .resolve_region_text(&old, TextMetrics::default(), Rc::new(()))
        .unwrap());
    let next_calls = calls.get();
    pass(&mut k, 400., 2);
    assert_eq!(calls.get(), next_calls);
}

fn baseline_fixture() -> Kernel {
    let mut k = fixture();
    let mut root = StyleProps::default();
    root.mask = mask(&[StyleId::Display, StyleId::AlignItems]);
    root.display = Display::Flex;
    root.align_items = AlignItems::Baseline;
    let mut owner = StyleProps::default();
    owner.mask = mask(&[StyleId::Display, StyleId::Width]);
    owner.display = Display::Flex;
    owner.width = Dimension::Points(200.);
    k.apply(
        0,
        0,
        &[
            Op::SetStyle {
                id: 1,
                patch: Box::new(root),
            },
            Op::SetStyle {
                id: 2,
                patch: Box::new(owner),
            },
            Op::SetChildren {
                id: 3,
                children: vec![],
            },
            Op::SetChildren {
                id: 2,
                children: vec![4, 5],
            },
        ],
    )
    .unwrap();
    k
}

#[test]
fn review_fixed_size_baseline_dependency_requires_refusal_or_ordinary_shell_parity() {
    let mut k = baseline_fixture();
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
    let before = frames(&k);
    let expected: Vec<_> = [1, 2, 6].map(|id| (id, k.node(id).unwrap().frame)).into();
    let b = ContentRegion {
        owner: key(&k, 2),
        content: key(&k, 4),
        pending: key(&k, 5),
    };
    if k.set_content_region(Some(b)).is_err() {
        assert_eq!(before, frames(&k), "unsupported admission must be atomic");
        // An explicitly cleared ordinary pass retains the baseline oracle.
        k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
        assert_eq!(before, frames(&k));
        return;
    }
    pass(&mut k, 400., 1);
    for (id, frame) in expected {
        assert_eq!(k.node(id).unwrap().frame, frame, "baseline shell id {id}");
    }
}

#[test]
fn initial_region_catalog_invalidates_prior_ordinary_measurement_cache() {
    let height = Rc::new(std::cell::Cell::new(20.));
    let mut k = fixture_with(Box::new(CatalogMetrics {
        height: height.clone(),
        calls: Rc::new(std::cell::Cell::new(0)),
    }));
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
    height.set(40.);
    register(&mut k);
    pass(&mut k, 400., 2);
    assert_eq!(k.node(6).unwrap().frame.height, 40.);
}

#[test]
fn baseline_dependency_refuses_explicit_self_and_nested_ancestor_participation() {
    for nested in [false, true] {
        let mut k = baseline_fixture();
        if nested {
            let mut wrapper = StyleProps::default();
            wrapper.mask = mask(&[StyleId::Display]);
            wrapper.display = Display::Flex;
            k.apply(
                0,
                0,
                &[
                    Op::CreateView {
                        id: 8,
                        node_type: NodeType::View,
                    },
                    Op::SetStyle {
                        id: 8,
                        patch: Box::new(wrapper),
                    },
                    Op::SetChildren {
                        id: 1,
                        children: vec![6],
                    },
                    Op::SetChildren {
                        id: 8,
                        children: vec![2],
                    },
                    Op::SetChildren {
                        id: 1,
                        children: vec![8, 6],
                    },
                ],
            )
            .unwrap();
        } else {
            let mut parent = StyleProps::default();
            parent.mask = mask(&[StyleId::AlignItems]);
            parent.align_items = AlignItems::FlexStart;
            let mut child = StyleProps::default();
            child.mask = mask(&[StyleId::AlignSelf]);
            child.align_self = AlignSelf::Baseline;
            k.apply(
                0,
                0,
                &[
                    Op::SetStyle {
                        id: 1,
                        patch: Box::new(parent),
                    },
                    Op::SetStyle {
                        id: 2,
                        patch: Box::new(child.clone()),
                    },
                    Op::SetStyle {
                        id: 6,
                        patch: Box::new(child),
                    },
                ],
            )
            .unwrap();
        }
        k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
        let before = frames(&k);
        let b = ContentRegion {
            owner: key(&k, 2),
            content: key(&k, 4),
            pending: key(&k, 5),
        };
        assert!(k.set_content_region(Some(b)).is_err(), "nested={nested}");
        assert_eq!(before, frames(&k));
    }
}

#[test]
fn new_baseline_dependency_refuses_pass_without_changing_prior_publication() {
    let mut k = fixture();
    register(&mut k);
    ready(&mut k, 400., 1);
    let before = frames(&k);
    let mut style = StyleProps::default();
    style.mask = mask(&[StyleId::Display, StyleId::AlignItems]);
    style.display = Display::Flex;
    style.align_items = AlignItems::Baseline;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }],
    )
    .unwrap();
    assert!(k
        .compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1
            }
        )
        .is_err());
    assert_eq!(before, frames(&k));
    // Explicit ordinary restoration is still available.
    k.set_content_region(None).unwrap();
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
}

#[test]
fn review_grid_baseline_dependency_requires_refusal_or_ordinary_shell_parity() {
    let mut k = baseline_fixture();
    let mut root = StyleProps::default();
    root.mask = mask(&[StyleId::Display, StyleId::GridTemplateColumns]);
    root.display = Display::Grid;
    root.grid_template_columns =
        GridTracks::from_tracks(vec![GridTrack::Points(200.), GridTrack::Points(200.)]);
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
    let before = frames(&k);
    let expected: Vec<_> = [1, 2, 6].map(|id| (id, k.node(id).unwrap().frame)).into();
    let b = ContentRegion {
        owner: key(&k, 2),
        content: key(&k, 4),
        pending: key(&k, 5),
    };
    if k.set_content_region(Some(b)).is_err() {
        assert_eq!(before, frames(&k));
        k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
        assert_eq!(before, frames(&k));
        return;
    }
    pass(&mut k, 400., 1);
    for (id, frame) in expected {
        assert_eq!(
            k.node(id).unwrap().frame,
            frame,
            "grid baseline shell id {id}"
        );
    }
}

// @ref LLP 1043.000 §3 D4 — the selected region carries flow-only changes.
#[test]
fn region_receipt_carries_resolved_exclusions_with_selected_frames() {
    let mut k = fixture();
    let mut paragraph = StyleProps::default();
    paragraph.width = Dimension::Points(400.);
    paragraph.height = Dimension::Points(180.);
    paragraph.mask = mask(&[StyleId::Width, StyleId::Height]);
    let mut ball = StyleProps::default();
    ball.width = Dimension::Points(120.);
    ball.height = Dimension::Points(120.);
    ball.top = Dimension::Points(0.);
    ball.left = Dimension::Points(60.);
    ball.position_type = PositionType::Absolute;
    ball.wrap_flow = WrapFlow::Both;
    ball.shape_outside = ShapeOutside::parse("circle()").unwrap();
    ball.mask = mask(&[
        StyleId::Width,
        StyleId::Height,
        StyleId::Top,
        StyleId::Left,
        StyleId::PositionType,
        StyleId::WrapFlow,
        StyleId::ShapeOutside,
    ]);
    k.apply(
        0,
        0,
        &[
            Op::SetStyle {
                id: 4,
                patch: Box::new(paragraph),
            },
            Op::CreateView {
                id: 7,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 7,
                patch: Box::new(ball),
            },
            Op::SetChildren {
                id: 3,
                children: vec![4, 7],
            },
        ],
    )
    .unwrap();
    register(&mut k);
    let r = ready(&mut k, 400., 1);
    assert_eq!(
        k.node(4).unwrap().flow_shapes(),
        &[FlowShape::Circle {
            cx: 120.,
            cy: 60.,
            r: 60.
        }]
    );
    assert_eq!(r.shell.flow_changed, vec![key(&k, 4)]);
    let mut moved = StyleProps::default();
    moved.left = Dimension::Points(80.);
    moved.mask = mask(&[StyleId::Left]);
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 7,
            patch: Box::new(moved),
        }],
    )
    .unwrap();
    let r = ready(&mut k, 400., 1);
    assert_eq!(r.shell.flow_changed, vec![key(&k, 4)]);
    assert!(!r.shell.changed.contains(&key(&k, 4)));
}
mod flex;
mod projection;

// Split-profile acceptance tests; baseline uses a separate existing-API fragment.
mod split_facts;
