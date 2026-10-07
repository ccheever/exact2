//! UNRUN acceptance tests; original pre-implementation draft remains sealed.
//! Rc sentinels prove kernel ownership only, not native bytes/worker painting.
use super::*;
use exact_kernel::region::{RegionProfile, RegionRequestPurpose};
use std::rc::Weak;

fn split(k: &mut Kernel) {
    k.set_content_region_profile(Some(binding(k)), RegionProfile::SplitFacts)
        .unwrap();
}
fn measured(q: &RegionTextRequest) -> TextMetrics {
    q.with_request(|r| MonospaceMeasurer::default().measure(r))
}
fn publication(r: RegionLayoutReceipt) -> Rc<RegionPublication> {
    assert!(r.current);
    let RegionSelection::Accepted(p) = r.selection else {
        panic!("complete")
    };
    p
}
fn complete(k: &mut Kernel, width: f32) -> Rc<RegionPublication> {
    for _ in 0..=768 + 192 {
        let r = pass(k, width, 1);
        if r.current {
            return publication(r);
        }
        let q = k.region_text_request().expect("one exact miss").clone();
        let value = Rc::new(7_u32);
        let weak = Rc::downgrade(&value);
        let is_measure = q.purpose() == RegionRequestPurpose::Measurement;
        assert!(k.resolve_region_text(&q, measured(&q), value).unwrap());
        if is_measure {
            assert!(weak.upgrade().is_none(), "no payload in scalar fact");
        }
    }
    panic!("finite facts plus final acquisitions");
}
fn until_paint(k: &mut Kernel, width: f32) -> RegionTextRequest {
    for _ in 0..=768 {
        let r = pass(k, width, 1);
        assert!(!r.current, "cannot publish before final owner");
        let q = k.region_text_request().unwrap().clone();
        if q.purpose() == RegionRequestPurpose::FinalPaint {
            return q;
        }
        assert_eq!(q.purpose(), RegionRequestPurpose::Measurement);
        assert!(k
            .resolve_region_text(&q, measured(&q), Rc::new(()))
            .unwrap());
    }
    panic!("bounded scalar discovery");
}
fn changed(k: &mut Kernel, text: &str) {
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: text.into(),
        }],
    )
    .unwrap();
}

#[test]
fn default64_still_retains_each_offer_payload_and_original_refusal_contract() {
    let mut k = fixture();
    register(&mut k);
    let mut leases = Vec::new();
    let p = loop {
        let r = pass(&mut k, 400., 1);
        if r.current {
            break publication(r);
        }
        let q = k.region_text_request().unwrap().clone();
        assert_eq!(q.purpose(), RegionRequestPurpose::RetainedOffer);
        let payload = Rc::new(17_u32);
        leases.push(Rc::downgrade(&payload));
        assert!(k.resolve_region_text(&q, measured(&q), payload).unwrap());
        assert!(leases.iter().all(|w| w.upgrade().is_some()));
        assert!(leases.len() <= 64);
    };
    assert_eq!(p.artifacts().len(), leases.len());
    k.set_content_region(None).unwrap();
    assert!(leases.iter().all(|w| w.upgrade().is_some()));
    drop(p);
    assert!(leases.iter().all(|w| w.upgrade().is_none()));
}

#[test]
fn scalar_facts_release_payload_and_final_same_tuple_requires_fresh_identity() {
    let mut k = fixture();
    split(&mut k);
    let mut measurements = Vec::new();
    let final_q = loop {
        let r = pass(&mut k, 400., 1);
        assert!(!r.current);
        let q = k.region_text_request().unwrap().clone();
        if q.purpose() == RegionRequestPurpose::FinalPaint {
            break q;
        }
        let payload = Rc::new(42_u32);
        let weak = Rc::downgrade(&payload);
        assert!(k.resolve_region_text(&q, measured(&q), payload).unwrap());
        assert!(
            weak.upgrade().is_none(),
            "cold width/ink owner is not a scalar fact"
        );
        measurements.push(q);
        assert!(measurements.len() <= 768);
    };
    let old = measurements
        .iter()
        .find(|q| q.offer() == final_q.offer() && q.stamp() == final_q.stamp())
        .expect("same exact final tuple already measured");
    assert!(!k
        .resolve_region_text(old, measured(old), Rc::new(999_u32))
        .unwrap());
    assert_eq!(
        k.region_text_request().unwrap().purpose(),
        RegionRequestPurpose::FinalPaint
    );
    let payload = Rc::new(88_u32);
    let weak = Rc::downgrade(&payload);
    assert!(k
        .resolve_region_text(&final_q, measured(&final_q), payload)
        .unwrap());
    let p = publication(pass(&mut k, 400., 1));
    assert_eq!(p.artifacts().len(), 1);
    assert_eq!(
        p.paint_artifact(key(&k, 4)).unwrap().payload::<u32>(),
        Some(&88)
    );
    // Taffy 0.14 can finish this block with one offer. Every measured tuple
    // remains a scalar fact even when only one final paint owner is needed.
    assert_eq!(k.region_retention().accepted_facts, measurements.len());
    assert!(!measurements.is_empty());
    drop(measurements);
    drop(final_q);
    k.set_content_region(None).unwrap();
    assert!(weak.upgrade().is_some());
    drop(p);
    assert!(weak.upgrade().is_none());
}

#[test]
fn final_metrics_mismatch_including_baseline_preserves_a_and_pending_identity() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    changed(
        &mut k,
        "different complete source wrapping into more lines than A",
    );
    let q = until_paint(&mut k, 180.);
    let exact = measured(&q);
    let before = k.node(4).unwrap().frame;
    for field in 0..3 {
        let mut wrong = exact;
        match field {
            0 => wrong.width += 1.,
            1 => wrong.height += 1.,
            _ => {
                wrong.first_baseline = if exact.first_baseline.is_some() {
                    None
                } else {
                    Some(0.)
                }
            }
        }
        let payload = Rc::new(1_u32);
        let weak = Rc::downgrade(&payload);
        assert!(k.resolve_region_text(&q, wrong, payload).is_err());
        assert!(weak.upgrade().is_none());
        assert!(k.node(4).unwrap().frame.bits_eq(before));
        let r = pass(&mut k, 180., 1);
        assert!(!r.current);
        let RegionSelection::Accepted(selected) = r.selection else {
            panic!("A")
        };
        assert!(Rc::ptr_eq(&selected, &a));
        assert_eq!(
            k.region_text_request().unwrap().purpose(),
            RegionRequestPurpose::FinalPaint
        );
    }
    assert!(k.resolve_region_text(&q, exact, Rc::new(2_u32)).unwrap());
    let b = publication(pass(&mut k, 180., 1));
    assert!(!Rc::ptr_eq(&a, &b));
}

#[test]
fn changed_ticket_refuses_both_late_measure_and_late_final_before_validation() {
    let mut k = fixture();
    split(&mut k);
    pass(&mut k, 400., 1);
    let measure = k.region_text_request().unwrap().clone();
    changed(&mut k, "replacement one");
    assert!(!k
        .resolve_region_text(
            &measure,
            TextMetrics {
                width: f32::NAN,
                ..TextMetrics::default()
            },
            Rc::new(())
        )
        .unwrap());
    let paint = until_paint(&mut k, 400.);
    drop(measure);
    changed(&mut k, "replacement two");
    let before = k.node(4).unwrap().frame;
    assert!(
        !k.resolve_region_text(
            &paint,
            TextMetrics {
                width: f32::NAN,
                ..TextMetrics::default()
            },
            Rc::new(())
        )
        .unwrap(),
        "stale guard before metric validation"
    );
    assert!(k.node(4).unwrap().frame.bits_eq(before));
    drop(paint);
    let p = complete(&mut k, 400.);
    assert_eq!(p.artifacts().len(), 1);
}

#[test]
fn parked_final_paint_keeps_a_and_unrelated_shell_typing_can_publish() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    let old_frame = k.node(4).unwrap().frame;
    changed(&mut k, "B text B text B text");
    let final_q = until_paint(&mut k, 250.);
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 6,
            prop: PropId::Text,
            value: "latest composer text".into(),
        }],
    )
    .unwrap();
    let r = pass(&mut k, 250., 1);
    assert!(!r.current);
    assert!(r.current_frame(key(&k, 4)).is_none());
    let RegionSelection::Accepted(selected) = r.selection else {
        panic!("A")
    };
    assert!(Rc::ptr_eq(&a, &selected));
    assert!(k.node(4).unwrap().frame.bits_eq(old_frame));
    assert!(k.node(6).unwrap().frame.height > 0.);
    assert!(k
        .resolve_region_text(&final_q, measured(&final_q), Rc::new(3_u32))
        .unwrap());
    assert!(pass(&mut k, 250., 1).current);
}

#[test]
fn retained_artifact_and_request_lease_block_c_even_after_unregister() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    let a_artifact = a.artifacts()[0].clone();
    drop(a);
    changed(&mut k, "B");
    let b = complete(&mut k, 400.);
    let b_request = b.artifacts()[0].request().clone();
    drop(b);
    k.set_content_region(None).unwrap();
    split(&mut k);
    let waiting = k
        .compute_region_layout(
            1,
            Offer::definite(300., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 2,
            },
        )
        .unwrap();
    assert!(!waiting.current);
    assert!(
        matches!(waiting.selection, RegionSelection::Pending(_)),
        "unregistered old A is not re-adopted"
    );
    assert!(k.region_text_request().is_none());
    assert_eq!(k.region_retention().candidate_facts, 0);
    drop(a_artifact);
    let c = complete(&mut k, 300.);
    assert_eq!(c.artifacts().len(), 1);
    drop(b_request);
}

#[test]
fn kernel_reset_does_not_free_external_publication_reservations() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    changed(&mut k, "B");
    let b = complete(&mut k, 400.);
    // Rebuild authored arena in SAME Kernel; no new domain-wide scheduler.
    k.reset();
    let fresh = fixture();
    // Export/import exact authored nodes through existing transaction operations.
    let mut ops = Vec::new();
    for id in 1..=6 {
        let node = fresh.node(id).unwrap();
        ops.push(Op::CreateView {
            id,
            node_type: node.node_type,
        });
        ops.push(Op::SetStyle {
            id,
            patch: Box::new(fresh.arena().style(node.key.index).clone()),
        });
        if id == 4 || id == 5 || id == 6 {
            ops.push(Op::SetProp {
                id,
                prop: PropId::Text,
                value: "new incarnation".into(),
            });
        }
        ops.push(Op::SetChildren {
            id,
            children: node.children(),
        });
    }
    // Children must exist before SetChildren applies.
    ops.sort_by_key(|op| match op {
        Op::CreateView { .. } => 0,
        Op::SetChildren { .. } => 2,
        _ => 1,
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 0, &ops).unwrap();
    split(&mut k);
    let waiting = k
        .compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1,
            },
        )
        .unwrap();
    assert!(!waiting.current);
    assert!(matches!(waiting.selection, RegionSelection::Pending(_)));
    assert!(k.region_text_request().is_none());
    drop(a);
    drop(b);
    assert_eq!(complete(&mut k, 400.).artifacts().len(), 1);
}

fn append_sources(k: &mut Kernel, count: usize, flex: bool) {
    let mut ops = Vec::new();
    let mut children = Vec::new();
    if flex {
        let mut parent = StyleProps::default();
        parent.mask = mask(&[
            StyleId::Display,
            StyleId::FlexDirection,
            StyleId::AlignItems,
        ]);
        parent.display = Display::Flex;
        parent.flex_direction = FlexDirection::Column;
        parent.align_items = AlignItems::FlexStart;
        ops.push(Op::SetStyle {
            id: 3,
            patch: Box::new(parent),
        });
    }
    for i in 0..count {
        let wrap = 100 + i as u32 * 2;
        let text = wrap + 1;
        ops.extend([
            Op::CreateView {
                id: wrap,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: text,
                node_type: NodeType::Text,
            },
        ]);
        let mut style = StyleProps::default();
        style.mask = mask(&[StyleId::Display, StyleId::FlexDirection, StyleId::MaxWidth]);
        style.display = if flex { Display::Flex } else { Display::Block };
        style.flex_direction = FlexDirection::Column;
        style.max_width = Dimension::Percent(88.);
        ops.push(Op::SetStyle {
            id: wrap,
            patch: Box::new(style),
        });
        ops.push(Op::SetProp {
            id: text,
            prop: PropId::Text,
            value: format!(
                "row {i} distinct text repeated to exercise wrapping {}",
                "word ".repeat(i % 7 + 1)
            )
            .into(),
        });
        ops.push(Op::SetChildren {
            id: wrap,
            children: vec![text],
        });
        children.push(wrap);
    }
    ops.push(Op::SetChildren { id: 3, children });
    k.apply(0, 0, &ops).unwrap();
}
fn many_sources(count: usize, flex: bool) -> Kernel {
    many_sources_with(count, flex, Box::<MonospaceMeasurer>::default())
}
fn many_sources_with(count: usize, flex: bool, measurer: Box<dyn TextMeasurer>) -> Kernel {
    let mut k = fixture_with(measurer);
    append_sources(&mut k, count, flex);
    k
}
fn exhausted(mut k: Kernel, expected: &'static str) {
    split(&mut k);
    let mut payloads: Vec<Weak<u32>> = Vec::new();
    for _ in 0..=768 + 192 {
        let before: Vec<_> = k
            .arena()
            .iter_live()
            .map(|s| {
                let n = k.node_by_key(k.arena().key(s)).unwrap();
                (n.key, n.frame, n.content)
            })
            .collect();
        match k.compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 1,
            },
        ) {
            Err(KernelError::Layout(LayoutError::ContentRegion(reason))) => {
                assert_eq!(reason, expected);
                for (key, frame, content) in before {
                    let n = k.node_by_key(key).unwrap();
                    assert!(n.frame.bits_eq(frame));
                    assert_eq!(n.content, content);
                }
                assert_eq!(k.region_retention().accepted_offers, 0);
                assert!(payloads.iter().all(|w| w.upgrade().is_none()));
                return;
            }
            Err(e) => panic!("unexpected error: {e:?}"),
            Ok(r) => {
                assert!(!r.current, "whole overflow must refuse, never truncate");
                let q = k.region_text_request().unwrap().clone();
                let p = Rc::new(0_u32);
                payloads.push(Rc::downgrade(&p));
                assert!(k.resolve_region_text(&q, measured(&q), p).unwrap());
            }
        }
    }
    panic!("finite admission bound");
}
#[test]
fn canonical_source_overflow_refuses_193_without_omitting_a_paragraph() {
    exhausted(
        many_sources(193, false),
        "split canonical source budget exhausted",
    );
}
#[test]
#[ignore = "async lane: 192 content-sized sources, ~14 s warm; bun scripts/async.mjs runs it"]
fn scalar_overflow_is_separate_from_192_source_admission() {
    // This deliberately uses content-dependent flex widths; if it does not
    // exercise M768, retain that fixture failure before changing its shape.
    // A height-bound measurer keeps a fact per height: under a height-free one
    // (monospace) these 192 paragraphs fit in 768 facts.
    exhausted(
        many_sources_with(192, true, Box::new(HeightBound)),
        "split scalar fact budget exhausted",
    );
}
#[test]
fn source_byte_overflow_preserves_default_16mib_limit() {
    let mut k = fixture();
    changed(
        &mut k,
        &"x".repeat(exact_kernel::region::REGION_SOURCE_BYTES + 1),
    );
    split(&mut k);
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
    assert!(k.region_text_request().is_none());
}
#[test]
fn compact_scalar_record_has_a_practical_bounded_layout_price() {
    // Production must independently const-assert its actual ScalarFact <=48B.
    // This is a conservative public-field layout price, not native byte evidence.
    assert!(std::mem::size_of::<(u16, Offer, TextMetrics)>() <= 48);
    assert_eq!(768 * 48, 36 * 1024);
    assert_eq!(2 * 768 * 48, 72 * 1024);
    assert_eq!(exact_kernel::region::REGION_OFFERS, 64);
}

#[test]
fn default_registration_cannot_bypass_live_split_reservations() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    k.set_content_region(None).unwrap();
    let b = binding(&k);
    assert!(
        k.set_content_region(Some(b)).is_err(),
        "no downgrade around externally held split A"
    );
    drop(a);
    assert!(k.set_content_region(Some(b)).unwrap());
    let r = ready(&mut k, 400., 1);
    assert!(r.current);
}

#[test]
fn final_metric_equality_is_bit_exact_including_signed_zero() {
    let mut k = fixture();
    split(&mut k);
    let mut steps = 0;
    let final_q = loop {
        steps += 1;
        assert!(steps <= 768 + 192);
        let r = pass(&mut k, 400., 1);
        assert!(!r.current);
        let q = k.region_text_request().unwrap().clone();
        if q.purpose() == RegionRequestPurpose::FinalPaint {
            break q;
        }
        // Valid synthetic scalar callback oracle, not native/ordinary geometry.
        assert!(k
            .resolve_region_text(&q, TextMetrics::default(), Rc::new(()))
            .unwrap());
    };
    assert!(k
        .resolve_region_text(
            &final_q,
            TextMetrics {
                width: -0.,
                ..TextMetrics::default()
            },
            Rc::new(())
        )
        .is_err());
    assert!(k
        .resolve_region_text(&final_q, TextMetrics::default(), Rc::new(()))
        .unwrap());
    assert!(pass(&mut k, 400., 1).current);
}

#[test]
fn overflow_preserves_complete_a_instead_of_publishing_first_192_sources() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    let a_frames: Vec<_> = a
        .frames()
        .iter()
        .map(|f| (f.node, f.frame, f.content))
        .collect();
    append_sources(&mut k, 193, false);
    for _ in 0..=768 {
        match k.compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 2,
            },
        ) {
            Err(KernelError::Layout(LayoutError::ContentRegion(reason))) => {
                assert_eq!(reason, "split canonical source budget exhausted");
                assert_eq!(k.region_retention().accepted_offers, 1);
                for (f, (key, frame, content)) in a.frames().iter().zip(&a_frames) {
                    assert_eq!(f.node, *key);
                    assert!(f.frame.bits_eq(*frame));
                    assert_eq!(f.content, *content);
                }
                return;
            }
            Err(e) => panic!("unexpected failure {e:?}"),
            Ok(r) => {
                assert!(!r.current);
                let RegionSelection::Accepted(selected) = r.selection else {
                    panic!("A")
                };
                assert!(Rc::ptr_eq(&a, &selected));
                let q = k.region_text_request().unwrap().clone();
                assert!(k
                    .resolve_region_text(&q, measured(&q), Rc::new(()))
                    .unwrap());
            }
        }
    }
    panic!("P193 must refuse");
}

#[test]
fn scalar_import_reacquires_final_wrapper_without_pinning_previous_reservation() {
    let mut k = fixture();
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 7,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 7,
                prop: PropId::Text,
                value: "unchanged second paragraph".into(),
            },
            Op::SetChildren {
                id: 3,
                children: vec![4, 7],
            },
        ],
    )
    .unwrap();
    split(&mut k);
    let a = complete(&mut k, 400.);
    let old = a.paint_artifact(key(&k, 7)).unwrap().request().clone();
    let old_owner = old.stamp().owner();
    let old_offer = old.offer();
    changed(&mut k, "new first paragraph");
    let mut saw_fresh_final = false;
    let mut steps = 0;
    let b = loop {
        steps += 1;
        assert!(steps <= 768 + 192);
        let r = pass(&mut k, 400., 1);
        if r.current {
            break publication(r);
        }
        let q = k.region_text_request().unwrap().clone();
        if q.purpose() == RegionRequestPurpose::FinalPaint && q.stamp().owner() == old_owner {
            assert_eq!(q.offer(), old_offer);
            assert!(!k
                .resolve_region_text(&old, measured(&old), Rc::new(()))
                .unwrap());
            saw_fresh_final = true;
        }
        assert!(k
            .resolve_region_text(&q, measured(&q), Rc::new(()))
            .unwrap());
    };
    assert!(
        saw_fresh_final,
        "opaque old wrapper cannot silently keep A lease inside B"
    );
    drop(old);
    drop(a);
    drop(b);
    changed(&mut k, "C first paragraph");
    assert_eq!(complete(&mut k, 400.).artifacts().len(), 2);
}

#[test]
fn saturated_external_a_and_b_allow_shell_typing_then_release_resumes_c() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    changed(&mut k, "B text");
    let b = complete(&mut k, 400.);
    // A is externally retained; current B keeps the second reservation.
    changed(&mut k, "C text must wait without allocating facts");
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 6,
            prop: PropId::Text,
            value: "independent shell typing while saturated".into(),
        }],
    )
    .unwrap();
    let waiting = pass(&mut k, 300., 1);
    assert!(!waiting.current);
    assert!(k.region_text_request().is_none());
    assert_eq!(k.region_retention().candidate_facts, 0);
    let RegionSelection::Accepted(selected) = &waiting.selection else {
        panic!("B retained")
    };
    assert!(Rc::ptr_eq(selected, &b));
    assert_eq!(k.node(1).unwrap().frame.width, 300.);
    assert!(k.node(6).unwrap().frame.height > 0.);
    let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    // Shell text position may follow retained owner height, whose independent
    // fixed180 box matches ordinary; compare actual current text measurement.
    oracle
        .compute_layout(1, Offer::definite(300., 300.))
        .unwrap();
    assert!(k
        .node(6)
        .unwrap()
        .frame
        .bits_eq(oracle.node(6).unwrap().frame));
    assert!(waiting.current_frame(key(&k, 4)).is_none());
    drop(waiting);
    drop(a);
    let c = complete(&mut k, 300.);
    assert!(!Rc::ptr_eq(&b, &c));
}

#[test]
fn stale_parked_final_reservation_allows_shell_before_request_drop_and_resume() {
    let mut k = fixture();
    split(&mut k);
    let a = complete(&mut k, 400.);
    changed(&mut k, "B parked final");
    let old_q = until_paint(&mut k, 300.);
    changed(
        &mut k,
        "latest C invalidates B but its worker still holds the request",
    );
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 6,
            prop: PropId::Text,
            value: "composer remains independently publishable while A and old B occupy both slots"
                .into(),
        }],
    )
    .unwrap();
    let waiting = pass(&mut k, 280., 1);
    assert!(!waiting.current);
    assert!(k.region_text_request().is_none());
    assert_eq!(k.region_retention().candidate_facts, 0);
    assert_eq!(k.region_retention().candidate_offers, 0);
    let RegionSelection::Accepted(selected) = &waiting.selection else {
        panic!("A")
    };
    assert!(Rc::ptr_eq(&a, selected));
    let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    oracle
        .compute_layout(1, Offer::definite(280., 300.))
        .unwrap();
    assert!(k
        .node(6)
        .unwrap()
        .frame
        .bits_eq(oracle.node(6).unwrap().frame));
    assert!(!k
        .resolve_region_text(
            &old_q,
            TextMetrics {
                width: f32::NAN,
                ..TextMetrics::default()
            },
            Rc::new(())
        )
        .unwrap());
    drop(waiting);
    drop(old_q);
    let c = complete(&mut k, 280.);
    assert!(!Rc::ptr_eq(&a, &c));
}

/// Measures as monospace does, but reads like a measurer whose metrics could
/// depend on the height offered (`height_free` keeps its default, false).
struct HeightBound;
impl TextMeasurer for HeightBound {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(r)
    }
}

/// A flex column with a definite height asks its text at one width under
/// more than one height. Under a height-free measurer (monospace, the Linux
/// and terminal hosts) those are one scalar fact; under one that may read the
/// height, each stays its own exact fact. The paint is the same either way.
#[test]
fn height_free_facts_answer_every_height_at_a_width() {
    let facts = |measurer: Box<dyn TextMeasurer>| {
        let mut k = fixture_with(measurer);
        let mut column = StyleProps::default();
        column.mask = mask(&[StyleId::Display, StyleId::FlexDirection, StyleId::Height]);
        column.display = Display::Flex;
        column.flex_direction = FlexDirection::Column;
        column.height = Dimension::Points(120.);
        k.apply(
            0,
            0,
            &[Op::SetStyle {
                id: 3,
                patch: Box::new(column),
            }],
        )
        .unwrap();
        split(&mut k);
        let p = complete(&mut k, 400.);
        let frame = p.frame(key(&k, 4), Frame::default()).unwrap();
        (k.region_retention().accepted_facts, frame)
    };
    let (free, free_frame) = facts(Box::<MonospaceMeasurer>::default());
    let (bound, bound_frame) = facts(Box::new(HeightBound));
    assert!(free < bound, "height-free {free} facts against {bound}");
    assert_eq!(free_frame, bound_frame, "the same geometry");
}
