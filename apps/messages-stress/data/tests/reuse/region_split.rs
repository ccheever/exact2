// UNRUN tests-first draft, included inside existing region_admission helper.
// Uses full app/data and selected publication feedback. No native payload proof.
mod split_facts {
    use super::*;
    use exact_kernel::{RegionInputs, RegionLayoutReceipt, RegionPublication, RegionSelection};
    use exact_kernel::region::{RegionProfile, RegionRequestPurpose};

    fn register(r: &mut Live) {
        let b = binding(r);
        r.kernel_mut()
            .set_content_region_profile(Some(b), RegionProfile::SplitFacts)
            .unwrap();
    }
    fn ordinary(r: &Live, width: f32) -> Kernel {
        let mut k = r.kernel().rehydrate(Box::new(MonospaceMeasurer::default()));
        for root in k.roots() {
            k.compute_layout(root, Offer::definite(width, 820.))
                .unwrap();
        }
        k
    }
    fn paragraphs(r: &Live) -> Vec<ParagraphStamp> {
        let c = r.collections().remove(0);
        assert_eq!(c.count, 10000);
        let mut result = Vec::new();
        for row in c.rows {
            let entries: Vec<_> = descendants(r.kernel(), row.root)
                .into_iter()
                .filter_map(|id| r.kernel().node(id).unwrap().paragraph_stamp())
                .collect();
            assert_eq!(entries.len(), 4, "sender/body/meta/Reply are all required");
            result.extend(entries);
        }
        result
    }
    fn current(r: &mut Live, width: f32) -> RegionLayoutReceipt {
        let root = r.roots()[0];
        let revision = r.collections()[0].revision;
        r.kernel_mut()
            .compute_region_layout(
                root,
                Offer::definite(width, 820.),
                RegionInputs {
                    catalog: 1,
                    consumer_revision: revision,
                },
            )
            .unwrap()
    }
    fn verify(r: &Live, width: f32, receipt: &RegionLayoutReceipt, p: &RegionPublication) {
        let oracle = ordinary(r, width);
        let b = binding(r);
        let content = r.kernel().node_by_key(b.content).unwrap().id;
        let nodes = descendants(r.kernel(), content);
        assert_eq!(p.frames().len(), nodes.len());
        assert_frame(
            receipt.origin,
            oracle.node(view(r, "region-owner")).unwrap().frame,
            "owner",
        );
        for id in nodes {
            let node = r.kernel().node(id).unwrap();
            let expected = oracle.node(id).unwrap();
            assert_frame(
                p.frame(node.key, receipt.origin).unwrap(),
                expected.frame,
                "all descendant geometry",
            );
            assert_eq!(
                p.frames()
                    .iter()
                    .find(|f| f.node == node.key)
                    .unwrap()
                    .content,
                expected.content
            );
        }
        for id in descendants(r.kernel(), composer(r)) {
            let node = r.kernel().node(id).unwrap();
            let expected = oracle.node(id).unwrap();
            assert_frame(node.frame, expected.frame, "complete composer");
            assert_eq!(node.content, expected.content);
        }
        let stamps = paragraphs(r);
        assert_eq!(
            p.artifacts().len(),
            stamps.len(),
            "only final owners, not speculative offers"
        );
        for stamp in stamps {
            let a = p.paint_artifact(stamp.owner()).unwrap();
            let q = a.request();
            assert_eq!(q.stamp(), &stamp);
            assert_eq!(q.purpose(), RegionRequestPurpose::FinalPaint);
            assert!(matches!(q.offer().width, AxisOffer::Definite(_)));
            assert_eq!(q.offer().height, AxisOffer::MaxContent);
            let mut runs = Vec::new();
            r.kernel().arena().text_runs(stamp.owner().index, &mut runs);
            q.with_request(|req| {
                assert_eq!(req.runs.len(), runs.len());
                for (a, b) in req.runs.iter().zip(&runs) {
                    assert_eq!(a.text, b.text);
                    assert_eq!(a.style, b.style);
                }
                assert_eq!(a.metrics(), MonospaceMeasurer::default().measure(req));
            });
        }
        let keep = r.kernel().region_retention();
        assert!(keep.accepted_facts <= 768);
        assert!(keep.accepted_offers <= 192);
        assert!(keep.total_source_bytes <= 2 * exact_kernel::region::REGION_SOURCE_BYTES);
    }
    fn finish(
        r: &mut Live,
        width: f32,
        prior: &mut Option<Rc<RegionPublication>>,
        park_and_type: bool,
    ) -> RegionLayoutReceipt {
        r.set_viewport(width as f64, 820.).unwrap();
        let old = prior.clone();
        let mut parked = false;
        let mut measured = 0;
        let mut painted = 0;
        // One extra generation is permitted ONLY for the deliberate shell edit.
        for _ in 0..=2 * (768 + 192) {
            let receipt = current(r, width);
            if receipt.current {
                let RegionSelection::Accepted(p) = &receipt.selection else {
                    panic!("complete")
                };
                verify(r, width, &receipt, p);
                if park_and_type {
                    assert!(parked, "test must actually park final paint");
                }
                eprintln!("SPLIT rows={} paragraphs={} measurements={measured} final_requests={painted} retention={:?}",
                    r.collections()[0].rows.len(),paragraphs(r).len(),r.kernel().region_retention());
                *prior = Some(p.clone());
                return receipt;
            }
            if let Some(a) = &old {
                let RegionSelection::Accepted(selected) = &receipt.selection else {
                    panic!("all A while B pending")
                };
                assert!(Rc::ptr_eq(a, selected));
                assert!(receipt.current_frame(binding(r).content).is_none());
                for f in a.projected_frames(receipt.origin).unwrap() {
                    if let Some(n) = r.kernel().node_by_key(f.node) {
                        assert_frame(n.frame, f.frame, "selected A frames remain coherent");
                        assert_eq!(n.content, f.content);
                    }
                }
            }
            let q = r.kernel().region_text_request().unwrap().clone();
            if park_and_type && !parked && q.purpose() == RegionRequestPurpose::FinalPaint {
                parked = true;
                r.act(
                    "editDraft",
                    vec![Value::str(
                        "Latest composer input while all B final paint is parked.",
                    )],
                )
                .unwrap();
                assert_eq!(
                    r.slot("draft"),
                    Some(&Value::str(
                        "Latest composer input while all B final paint is parked."
                    ))
                );
                // Do not apply feedback from A, nor complete a possibly invalidated
                // q: latest shell height/offer discovery decides the new request.
                let after = current(r, width);
                assert!(!after.current);
                if let Some(a) = &old {
                    let RegionSelection::Accepted(selected) = after.selection else {
                        panic!("A")
                    };
                    assert!(Rc::ptr_eq(a, &selected));
                }
                let oracle = ordinary(r, width);
                for id in descendants(r.kernel(), composer(r)) {
                    assert_frame(
                        r.kernel().node(id).unwrap().frame,
                        oracle.node(id).unwrap().frame,
                        "latest shell while parked",
                    );
                }
                continue;
            }
            let payload = Rc::new(42_u32);
            let weak = Rc::downgrade(&payload);
            let m = q.with_request(|req| MonospaceMeasurer::default().measure(req));
            assert!(r.kernel_mut().resolve_region_text(&q, m, payload).unwrap());
            match q.purpose() {
                RegionRequestPurpose::Measurement => {
                    measured += 1;
                    assert!(weak.upgrade().is_none());
                }
                RegionRequestPurpose::FinalPaint => {
                    painted += 1;
                    assert!(weak.upgrade().is_some());
                }
                _ => panic!("opt-in never uses legacy retention"),
            }
        }
        panic!("bounded full viewport completion");
    }
    fn feedback(r: &mut Live, receipt: &RegionLayoutReceipt, pin: bool) {
        assert!(receipt.current);
        let RegionSelection::Accepted(p) = &receipt.selection else {
            panic!("current publication required")
        };
        let c = r.collections().remove(0);
        assert_eq!(p.inputs().consumer_revision, c.revision);
        let port = receipt
            .current_frame(r.kernel().node(c.view).unwrap().key)
            .unwrap();
        let row_width = receipt
            .current_frame(r.kernel().node(c.rows[0].view).unwrap().key)
            .unwrap()
            .width;
        let measurements = c
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                height: receipt
                    .current_frame(r.kernel().node(row.view).unwrap().key)
                    .unwrap()
                    .height as f64,
            })
            .collect();
        let selected = pin.then_some(c.rows[0].view);
        r.collection_feedback(CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            scroll_top: (c.total_extent - port.height as f64).max(0.),
            port_width: port.width as f64,
            port_height: port.height as f64,
            row_width: row_width as f64,
            measurements,
            focus_view: selected,
            interaction_view: selected,
        })
        .unwrap();
    }

    #[test]
    fn full_messages_split_completes_all132_tail_paragraphs_and_latest_batch32() {
        let mut r = boot(&envelope(false));
        register(&mut r);
        let mut prior = None;
        let a = finish(&mut r, 980., &mut prior, false);
        assert_eq!(r.collections()[0].rows.len(), 16);
        assert_eq!(paragraphs(&r).len(), 64);
        // @ref LLP 1043.000 §8 — Taffy 0.14 discovers 19 scalar offers per
        // row here (0.9 discovered 18); final owners and the 768-fact cap stay
        // unchanged. Keep an exact count so lost reuse cannot hide in the cap.
        assert_eq!(r.kernel().region_retention().accepted_facts, 304);
        feedback(&mut r, &a, false);
        drop(a);
        let tail = finish(&mut r, 980., &mut prior, false);
        assert_eq!(r.collections()[0].rows.len(), 33);
        assert_eq!(paragraphs(&r).len(), 132);
        assert_eq!(r.kernel().region_retention().accepted_facts, 627);
        feedback(&mut r, &tail, false);
        drop(tail);
        let settled = finish(&mut r, 980., &mut prior, false);
        drop(settled);
        let resized = finish(&mut r, 896., &mut prior, false);
        feedback(&mut r, &resized, false);
        drop(resized);
        let settled = finish(&mut r, 896., &mut prior, false);
        feedback(&mut r, &settled, false);
        drop(settled);
        let settled = finish(&mut r, 896., &mut prior, false);
        drop(settled);
        configure_composer(
            &mut r,
            true,
            &"Real multiline draft with reply strip and unchanged transcript. ".repeat(6),
        );
        let composed = finish(&mut r, 896., &mut prior, false);
        drop(composed);
        let old_history = r.resource("history").unwrap().clone();
        r.act("step", vec![]).unwrap();
        let new_history = r.resource("history").unwrap();
        let old_rows = super::super::rows(&old_history);
        let new_rows = super::super::rows(new_history);
        assert_eq!(old_rows.len(), 10000);
        assert_eq!(new_rows.len(), 10000);
        assert_eq!(
            old_rows
                .iter()
                .zip(new_rows.iter())
                .filter(|(a, b)| !super::super::shared_record(a, b))
                .count(),
            32
        );
        let latest = finish(&mut r, 896., &mut prior, true);
        assert_eq!(r.kernel().find_by_test_id("reply-target").len(), 1);
        feedback(&mut r, &latest, false);
        drop(latest);
        let last = finish(&mut r, 896., &mut prior, false);
        drop(last);
        let repeated = finish(&mut r, 896., &mut prior, false);
        assert!(r.kernel().region_text_request().is_none());
        drop(repeated);
    }

    #[test]
    fn selected_row_heights_and_live_focus_interaction_pins_survive_pending_reflow() {
        let mut r = boot(&envelope(false));
        register(&mut r);
        let mut prior = None;
        let first = finish(&mut r, 980., &mut prior, false);
        feedback(&mut r, &first, false);
        drop(first);
        let tail = finish(&mut r, 980., &mut prior, false);
        feedback(&mut r, &tail, false);
        drop(tail);
        let settled = finish(&mut r, 980., &mut prior, false);
        let pinned = r.collections()[0].rows[0].view;
        feedback(&mut r, &settled, true);
        drop(settled);
        let pinned_ready = finish(&mut r, 980., &mut prior, false);
        drop(pinned_ready);
        assert!(r.kernel().node(pinned).is_some());
        let resized = finish(&mut r, 896., &mut prior, true);
        assert!(
            r.kernel().node(pinned).is_some(),
            "collection pin must not disappear during pending B"
        );
        let c = r.collections().remove(0);
        assert_eq!(c.count, 10000);
        // Existing default64/overflow tests remain unchanged in the parent module.
        verify(&r, 896., &resized, prior.as_ref().unwrap());
    }
}
