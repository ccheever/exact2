use super::*;
use crate::image::Bitmap;
use crate::paint::{Backend, Painter, Presented, Rect4, Scene, Shape};
use exact_kernel::{Dimension, Kernel, NodeType, Offer, Op, StyleId, StyleProps};
use std::cell::Cell;
use std::collections::BTreeMap;

fn spec(text: &str) -> Spec {
    crate::paint::text_spec(&StyleProps::default(), text)
}

#[test]
fn unique_widths_retire_unpinned_previous_snapshots() {
    let mut engine = TextEngine::new();
    let s = spec(&"abc def é中 ".repeat(500));
    let mut previous = None;
    for width in 160..180 {
        let paragraph = engine.paragraph(&s, Some(width as f32));
        if let Some(previous) = previous {
            assert!(
                std::rc::Weak::upgrade(&previous).is_none(),
                "retained old width {width}"
            );
        }
        previous = Some(Rc::downgrade(&paragraph));
    }
}

#[test]
fn intrinsic_queries_retain_only_scalar_metrics() {
    let mut engine = TextEngine::new();
    let s = spec(&"abc def ".repeat(300));
    for offer in [AxisOffer::MaxContent, AxisOffer::MinContent] {
        let first = engine.measure(&s, offer);
        assert!(first.width > 0.0 && first.height > 0.0);
        assert_eq!(
            engine.paragraphs.len(),
            0,
            "intrinsic query retained a Buffer"
        );
        assert_eq!(engine.measure(&s, offer), first);
    }
}

#[test]
fn exact_text_and_run_boundaries_cannot_alias_formatted_keys() {
    let mut engine = TextEngine::new();
    let mut split = spec("A");
    let mut second = split.runs[0].clone();
    second.text = "B".into();
    split.runs.push(second);
    let r = &split.runs[0];
    let mut joined = split.clone();
    joined.runs.truncate(1);
    joined.runs[0].text = format!(
        "A|{}|{}|{}|{}|{}|{}\u{1}B",
        r.size.to_bits(),
        r.weight,
        r.family,
        r.italic,
        r.line_height.map_or(u32::MAX, f32::to_bits),
        r.letter_spacing.to_bits()
    );
    let a = engine.paragraph(&split, Some(300.0));
    let b = engine.paragraph(&joined, Some(300.0));
    assert!(!Rc::ptr_eq(&a, &b), "distinct UTF8/run boundaries aliased");
}

#[test]
fn independently_pinned_identical_text_keeps_both_owner_widths() {
    let mut engine = TextEngine::new();
    let s = spec(&"words ".repeat(200));
    let a = engine.paragraph(&s, Some(120.0));
    let b = engine.paragraph(&s, Some(280.0));
    for width in 200..205 {
        drop(engine.paragraph(&s, Some(width as f32)));
    }
    assert!(Rc::ptr_eq(&a, &engine.paragraph(&s, Some(120.0))));
    assert!(Rc::ptr_eq(&b, &engine.paragraph(&s, Some(280.0))));
    assert!(a.height > b.height);
}

struct ControlledBackend {
    fail: Rc<Cell<bool>>,
}
impl Backend for ControlledBackend {
    fn name(&self) -> &'static str {
        "test"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {}
    fn fill(&mut self, _: &Shape, _: [u8; 4], _: Transform) {}
    fn stroke(&mut self, _: &Shape, _: f32, _: [u8; 4], _: Transform) {}
    fn image(&mut self, _: &Arc<Bitmap>, _: Rect4, _: &[Shape], _: Transform) {}
    fn text(
        &mut self,
        _: &mut TextEngine,
        _: &Paragraph,
        _: &[RunPaint],
        _: (f32, f32),
        _: Transform,
    ) {
    }
    fn push_clip(&mut self, _: &Shape, _: Transform) {}
    fn pop_clip(&mut self) {}
    fn push_opacity(&mut self, _: f32) {}
    fn pop_opacity(&mut self) {}
    fn pointer(&mut self, _: f32, _: f32) {}
    fn finish(&mut self) -> Result<Pixmap, String> {
        if self.fail.get() {
            Err("injected frame failure".into())
        } else {
            Ok(Pixmap::new(1, 1).unwrap())
        }
    }
}
fn text_tree(text: &str, width: f32) -> Kernel {
    let mut kernel = Kernel::with_monospace();
    kernel
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
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 2,
                    prop: exact_kernel::PropId::Text,
                    value: exact_kernel::PropValue::Str(text.into()),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    resize(&mut kernel, width);
    kernel
}
fn resize(kernel: &mut Kernel, width: f32) {
    let mut style = StyleProps {
        width: Dimension::Points(width),
        ..StyleProps::default()
    };
    style.mask.set(StyleId::Width);
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(style),
            }],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(400., 400.))
        .unwrap();
}
fn paint(painter: &mut Painter, kernel: &Kernel) -> Result<crate::paint::Frame, String> {
    painter.paint(
        &Scene {
            kernel,
            roots: &kernel.roots(),
            hidden: &|_| false,
            presented: &|_| Presented::IDENTITY,
            scroll: &BTreeMap::new(),
            page: (0., 0.),
            images: &BTreeMap::new(),
            focus: None,
            pointer: None,
        },
        (400., 400.),
    )
}

#[test]
fn failed_painter_frame_keeps_previous_accepted_leases_until_success() {
    let text = "An accepted paragraph stays alive while a replacement frame fails.";
    let engine = TextEngine::shared();
    let fail = Rc::new(Cell::new(false));
    let mut painter = Painter::new(
        engine.clone(),
        1.,
        Box::new(ControlledBackend { fail: fail.clone() }),
    );
    let mut kernel = text_tree(text, 120.);
    paint(&mut painter, &kernel).unwrap();
    let old = {
        let p = engine.borrow_mut().paragraph(&spec(text), Some(120.));
        Rc::downgrade(&p)
    };
    // Evict cache ownership: the accepted native frame must be the owner now.
    engine.borrow_mut().paragraphs.clear();
    assert!(old.upgrade().is_some());
    resize(&mut kernel, 240.);
    fail.set(true);
    assert!(paint(&mut painter, &kernel).is_err());
    engine.borrow_mut().paragraphs.clear();
    assert!(
        old.upgrade().is_some(),
        "failed frame retired accepted pixels' layout"
    );
    fail.set(false);
    paint(&mut painter, &kernel).unwrap();
    engine.borrow_mut().paragraphs.clear();
    assert!(
        old.upgrade().is_none(),
        "successful replacement retained history"
    );
    let current = {
        let p = engine.borrow_mut().paragraph(&spec(text), Some(240.));
        Rc::downgrade(&p)
    };
    engine.borrow_mut().paragraphs.clear();
    assert!(current.upgrade().is_some());
    kernel.reset();
    paint(&mut painter, &kernel).unwrap();
    engine.borrow_mut().paragraphs.clear();
    assert!(current.upgrade().is_none(), "reset retained owner leases");
}

#[test]
fn old_unpinned_width_is_destroyed_before_next_buffer_allocation() {
    let mut engine = TextEngine::new();
    let s = spec(&"resize retirement ".repeat(500));
    let old = {
        let p = engine.paragraph(&s, Some(150.));
        Rc::downgrade(&p)
    };
    engine.before_layout = Some(Box::new(move || {
        assert!(
            old.upgrade().is_none(),
            "new shape began before old Buffer dropped"
        );
    }));
    drop(engine.paragraph(&s, Some(151.)));
}

#[test]
fn intrinsic_scratch_does_not_displace_pinned_frame_and_is_not_retained() {
    let mut engine = TextEngine::new();
    let s = spec(&"short words ".repeat(200));
    let displayed = engine.paragraph(&s, Some(240.));
    let shapes = engine.shape_calls;
    engine.measure(&s, AxisOffer::MaxContent);
    engine.measure(&s, AxisOffer::MinContent);
    assert_eq!(engine.residency().paragraphs, 1);
    assert_eq!(engine.residency().pinned_paragraphs, 1);
    assert_eq!(engine.residency().intrinsic_metrics, 2);
    let after = engine.shape_calls;
    assert!(after > shapes);
    for _ in 0..5 {
        engine.measure(&s, AxisOffer::MaxContent);
        engine.measure(&s, AxisOffer::MinContent);
    }
    assert_eq!(engine.shape_calls, after);
    assert!(Rc::ptr_eq(&displayed, &engine.paragraph(&s, Some(240.))));
}

#[test]
fn scalar_identity_cache_and_dead_width_index_are_bounded() {
    let mut engine = TextEngine::new();
    for n in 0..cache::COLD_IDENTITIES + 32 {
        engine.measure(&spec(&format!("item {n}")), AxisOffer::MaxContent);
    }
    assert!(engine.residency().identities <= cache::COLD_IDENTITIES);
    assert!(engine.residency().intrinsic_metrics <= cache::COLD_IDENTITIES * 2);
    assert_eq!(engine.residency().paragraphs, 0);
    let s = spec("same paragraph, many widths");
    for width in 1..100 {
        drop(engine.paragraph(&s, Some(width as f32)));
    }
    assert_eq!(engine.residency().paragraphs, 1);
    assert!(engine.paragraphs.indexed_widths() <= 1);
}

#[test]
fn cold_target_never_rejects_work_or_evicts_painter_owned_snapshots() {
    let mut engine = TextEngine::new();
    engine.paragraphs.set_target(0);
    let s = spec(&"a full paragraph ".repeat(200));
    let pinned = engine.paragraph(&s, Some(150.));
    engine.trim_paragraphs();
    assert_eq!(engine.residency().pinned_paragraphs, 1);
    assert!(engine.residency().owned_capacity_bytes > 0);
    assert!(
        pinned
            .buffer
            .lines
            .iter()
            .map(|l| l.text().len())
            .sum::<usize>()
            >= s.runs[0].text.trim_end().len()
    );
    let weak = Rc::downgrade(&pinned);
    drop(pinned);
    engine.trim_paragraphs();
    assert!(weak.upgrade().is_none());
    assert_eq!(engine.residency().identities, 0);
}

#[test]
fn accepted_small_text_repaints_without_reshaping_under_zero_cold_target() {
    let text = "A small stable label";
    let engine = TextEngine::shared();
    engine.borrow_mut().paragraphs.set_target(0);
    let fail = Rc::new(Cell::new(false));
    let mut painter = Painter::new(engine.clone(), 1., Box::new(ControlledBackend { fail }));
    let kernel = text_tree(text, 160.);
    paint(&mut painter, &kernel).unwrap();
    let shapes = engine.borrow().shape_calls;
    for _ in 0..20 {
        paint(&mut painter, &kernel).unwrap();
    }
    assert_eq!(engine.borrow().shape_calls, shapes);
    assert_eq!(engine.borrow().residency().pinned_paragraphs, 1);
    drop(painter);
    engine.borrow_mut().trim_paragraphs();
    assert_eq!(engine.borrow().residency().paragraphs, 0);
}

#[test]
fn eviction_preserves_metrics_and_all_glyph_positions() {
    let mut engine = TextEngine::new();
    let mut s = spec("Latin é 中 العربية 👩‍💻\nsecond line with words");
    s.runs[0].line_height = Some(22.25);
    let p = engine.paragraph(&s, Some(140.));
    let metrics = paragraph_metrics(&p);
    let glyphs = |p: &Paragraph| {
        p.buffer
            .layout_runs()
            .flat_map(|line| {
                line.glyphs.iter().map(|g| {
                    (
                        g.start,
                        g.end,
                        g.glyph_id,
                        g.x.to_bits(),
                        g.y.to_bits(),
                        g.w.to_bits(),
                        g.metadata,
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    let expected = glyphs(&p);
    let baselines = p.baselines.clone();
    drop(p);
    drop(engine.paragraph(&s, Some(141.)));
    let fresh = engine.paragraph(&s, Some(140.));
    assert_eq!(paragraph_metrics(&fresh), metrics);
    assert_eq!(fresh.baselines, baselines);
    assert_eq!(glyphs(&fresh), expected);
}

#[test]
fn catalog_replacement_and_reused_node_keys_do_not_keep_lease_history() {
    let text = "One owner";
    let engine = TextEngine::shared();
    let fail = Rc::new(Cell::new(false));
    let mut painter = Painter::new(engine.clone(), 1., Box::new(ControlledBackend { fail }));
    let mut kernel = text_tree(text, 140.);
    let before = kernel.node(2).unwrap().key;
    paint(&mut painter, &kernel).unwrap();
    let old = {
        let p = engine.borrow_mut().paragraph(&spec(text), Some(140.));
        Rc::downgrade(&p)
    };
    *engine.borrow_mut() = TextEngine::new();
    assert!(
        old.upgrade().is_some(),
        "previous successful frame still owns its lease"
    );
    paint(&mut painter, &kernel).unwrap();
    assert!(old.upgrade().is_none());
    assert_eq!(
        engine.borrow().shape_calls,
        1,
        "catalog replacement shaped afresh"
    );
    let current = {
        let p = engine.borrow_mut().paragraph(&spec(text), Some(140.));
        Rc::downgrade(&p)
    };
    kernel
        .apply(
            0,
            3,
            &[
                Op::DestroyView { id: 2 },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 2,
                    prop: exact_kernel::PropId::Text,
                    value: exact_kernel::PropValue::Str("Replacement".into()),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
            ],
        )
        .unwrap();
    assert_ne!(kernel.node(2).unwrap().key, before);
    resize(&mut kernel, 180.);
    paint(&mut painter, &kernel).unwrap();
    engine.borrow_mut().paragraphs.clear();
    assert!(current.upgrade().is_none());
}

#[test]
fn oversized_cold_result_reports_overage_after_last_caller_drop_and_hits() {
    let engine = TextEngine::shared();
    // Exercise the oversized path without a large allocation in a unit test.
    engine.borrow_mut().paragraphs.set_target(1);
    let text = "An oversized measurement must survive until its painter takes ownership.";
    let s = spec(text);
    let p = engine.borrow_mut().paragraph(&s, Some(140.));
    let capacities = p.resident_capacity_bytes;
    let private = p.private_text_bytes_estimate;
    let weak = Rc::downgrade(&p);
    let pinned = engine.borrow().residency();
    assert_eq!(pinned.cold_policy_bytes, 0);
    assert_eq!(pinned.cold_overage_bytes, 0);
    drop(p);

    let cold = engine.borrow().residency();
    assert_eq!(cold.cold_paragraphs, 1);
    assert_eq!(cold.cold_owned_capacity_bytes, capacities);
    assert_eq!(cold.cold_target_bytes, 1);
    assert_eq!(
        cold.cold_policy_bytes,
        capacities + private + cold.key_capacity_bytes
    );
    assert_eq!(cold.cold_overage_bytes, cold.cold_policy_bytes - 1);
    let shapes = engine.borrow().shape_calls;
    for _ in 0..5 {
        // Raw lookup remains cold once its caller drops. Definite measurement
        // now has a separate explicit handoff category, tested below.
        drop(engine.borrow_mut().paragraph(&s, Some(140.)));
        let after = engine.borrow().residency();
        assert_eq!(after.cold_policy_bytes, cold.cold_policy_bytes);
        assert_eq!(after.cold_overage_bytes, cold.cold_overage_bytes);
    }
    assert_eq!(engine.borrow().shape_calls, shapes);
    assert!(weak.upgrade().is_some());

    let fail = Rc::new(Cell::new(false));
    let mut painter = Painter::new(engine.clone(), 1., Box::new(ControlledBackend { fail }));
    paint(&mut painter, &text_tree(text, 140.)).unwrap();
    assert_eq!(engine.borrow().shape_calls, shapes, "handoff reshaped text");
    assert_eq!(engine.borrow().residency().cold_policy_bytes, 0);
    drop(painter);
    assert_eq!(
        engine.borrow().residency().cold_overage_bytes,
        cold.cold_overage_bytes
    );
    engine.borrow_mut().trim_paragraphs();
    assert!(weak.upgrade().is_none());
    assert_eq!(engine.borrow().residency().cold_policy_bytes, 0);
    assert_eq!(engine.borrow().residency().cold_overage_bytes, 0);
}

#[test]
fn catalog_swap_failed_frame_reports_deduplicated_retiring_accepted_storage() {
    let text = "Two accepted owners share the same paragraph backing.";
    let engine = TextEngine::shared();
    let fail = Rc::new(Cell::new(false));
    let mut painter = Painter::new(
        engine.clone(),
        1.,
        Box::new(ControlledBackend { fail: fail.clone() }),
    );
    let mut kernel = text_tree(text, 140.);
    let mut style = StyleProps {
        width: Dimension::Points(140.),
        ..StyleProps::default()
    };
    style.mask.set(StyleId::Width);
    kernel
        .apply(
            0,
            3,
            &[
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 3,
                    prop: exact_kernel::PropId::Text,
                    value: exact_kernel::PropValue::Str(text.into()),
                },
                Op::SetStyle {
                    id: 3,
                    patch: Box::new(style),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
            ],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(400., 400.))
        .unwrap();
    paint(&mut painter, &kernel).unwrap();
    assert_eq!(engine.borrow().residency().pinned_paragraphs, 1);
    assert_eq!(
        painter.retiring_text_residency(),
        RetiringResidency::default()
    );
    let (weak, capacities, private) = {
        let p = engine.borrow_mut().paragraph(&spec(text), Some(140.));
        (
            Rc::downgrade(&p),
            p.resident_capacity_bytes,
            p.private_text_bytes_estimate,
        )
    };
    painter.text = TextEngine::shared();
    drop(engine);
    assert_eq!(painter.text.borrow().residency().paragraphs, 0);
    let expected = RetiringResidency {
        owners: 2,
        paragraphs: 1,
        owned_capacity_bytes: capacities,
        private_text_bytes_estimate: private,
    };
    assert_eq!(painter.retiring_text_residency(), expected);
    fail.set(true);
    assert!(paint(&mut painter, &kernel).is_err());
    assert_eq!(painter.retiring_text_residency(), expected);
    assert!(weak.upgrade().is_some());
    // Repeated catalog swaps while presentation fails retain one accepted set.
    painter.text = TextEngine::shared();
    assert!(paint(&mut painter, &kernel).is_err());
    assert_eq!(painter.retiring_text_residency(), expected);
    fail.set(false);
    paint(&mut painter, &kernel).unwrap();
    assert_eq!(
        painter.retiring_text_residency(),
        RetiringResidency::default()
    );
    assert!(weak.upgrade().is_none());
}

#[test]
fn unrelated_identity_trim_preserves_measured_handoff() {
    let mut engine = TextEngine::new();
    engine.paragraphs.set_target(0);
    let paragraph = spec("A measured paragraph awaiting this frame's painter.");
    engine.measure(&paragraph, AxisOffer::Definite(140.));
    let key = engine.paragraphs.identity(&paragraph);
    let measured = engine.paragraphs.get(key, Some(140.).into()).unwrap();
    let weak = Rc::downgrade(&measured);
    drop(measured);

    // This is the precise identity-miss -> trim(None) path. No new Buffer
    // allocation or second width offer is necessary to lose the handoff.
    engine.paragraphs.identity(&spec("new small control"));
    assert!(
        weak.upgrade().is_some(),
        "identity-miss trim(None) destroyed the pending measured paragraph"
    );
}

#[test]
fn taffy_small_control_after_paragraph_preserves_measure_to_paint_handoff() {
    const TEXT: &str = "A full paragraph measured before the later small control.";
    #[derive(Default)]
    struct Observed {
        paragraph: Option<std::rc::Weak<Paragraph>>,
        offers: Vec<(bool, AxisOffer, bool)>,
    }
    struct Probe {
        shared: Shared,
        observed: Rc<RefCell<Observed>>,
    }
    impl TextMeasurer for Probe {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            let metrics = Measurer(self.shared.clone()).measure(request);
            let target = request.runs.iter().any(|run| run.text == TEXT);
            let mut observed = self.observed.borrow_mut();
            if target {
                if let AxisOffer::Definite(width) = request.width {
                    let mut engine = self.shared.borrow_mut();
                    let key = engine.paragraphs.identity(&Spec::from_request(request));
                    let p = engine.paragraphs.get(key, Some(width).into()).unwrap();
                    observed.paragraph = Some(Rc::downgrade(&p));
                }
            }
            let alive = observed
                .paragraph
                .as_ref()
                .is_some_and(|p| p.upgrade().is_some());
            observed.offers.push((target, request.width, alive));
            metrics
        }
    }
    let engine = TextEngine::shared();
    engine.borrow_mut().paragraphs.set_target(0);
    let observed = Rc::new(RefCell::new(Observed::default()));
    let mut kernel = Kernel::new(Box::new(Probe {
        shared: engine.clone(),
        observed: observed.clone(),
    }));
    let mut paragraph_style = StyleProps {
        width: Dimension::Points(140.),
        ..StyleProps::default()
    };
    paragraph_style.mask.set(StyleId::Width);
    kernel
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
                    node_type: NodeType::Text,
                },
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::TextInput,
                },
                Op::SetProp {
                    id: 2,
                    prop: exact_kernel::PropId::Text,
                    value: exact_kernel::PropValue::Str(TEXT.into()),
                },
                Op::SetProp {
                    id: 3,
                    prop: exact_kernel::PropId::Value,
                    value: exact_kernel::PropValue::Str("control".into()),
                },
                Op::SetStyle {
                    id: 2,
                    patch: Box::new(paragraph_style),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(400., 400.))
        .unwrap();
    let measured = observed.borrow().paragraph.clone().unwrap();
    let alive_after_layout = measured.upgrade().is_some();
    let mut painter = Painter::new(
        engine.clone(),
        1.,
        Box::new(ControlledBackend {
            fail: Rc::new(Cell::new(false)),
        }),
    );
    paint(&mut painter, &kernel).unwrap();
    let painted = engine.borrow_mut().paragraph(&spec(TEXT), Some(140.));
    assert!(
        alive_after_layout && std::rc::Weak::ptr_eq(&measured, &Rc::downgrade(&painted)),
        "Taffy/control/paint lost its measured Buffer; offers (target, width, alive): {:?}",
        observed.borrow().offers
    );
}

fn measured_weak(engine: &mut TextEngine, s: &Spec, width: f32) -> std::rc::Weak<Paragraph> {
    engine.measure(s, AxisOffer::Definite(width));
    let key = engine.paragraphs.identity(s);
    Rc::downgrade(&engine.paragraphs.get(key, Some(width).into()).unwrap())
}

#[test]
fn handoff_cap_refreshes_deterministically_and_reports_its_separate_cost() {
    let mut engine = TextEngine::new();
    engine.paragraphs.set_target(0);
    let weak: Vec<_> = (0..64)
        .map(|i| measured_weak(&mut engine, &spec(&format!("paragraph {i}")), 140.))
        .collect();
    let before = engine.handoff_residency();
    assert_eq!(before.identities, 64);
    assert_eq!(before.identity_limit, 64);
    assert_eq!(before.paragraphs, 64);
    assert!(before.owned_capacity_bytes > 0);
    assert_eq!(
        before.policy_bytes,
        before.owned_capacity_bytes + before.private_text_bytes_estimate
    );
    assert_eq!(before.cold_target_reference_bytes, 0);
    assert_eq!(before.above_cold_target_bytes, before.policy_bytes);
    // Repeated use refreshes one identity; it must not grow a pin history.
    let shapes = engine.shape_calls;
    measured_weak(&mut engine, &spec("paragraph 0"), 140.);
    assert_eq!(engine.shape_calls, shapes);
    measured_weak(&mut engine, &spec("paragraph 64"), 140.);
    assert!(weak[0].upgrade().is_some());
    assert!(weak[1].upgrade().is_none());
    assert_eq!(engine.handoff_residency().identities, 64);
    engine.finish_text_frame();
    assert_eq!(engine.handoff_residency().paragraphs, 0);
    assert_eq!(engine.handoff_residency().policy_bytes, 0);
    assert!(weak.iter().all(|p| p.upgrade().is_none()));
}

#[test]
fn replacing_definite_handoff_drops_unaccepted_old_width_before_allocation() {
    let mut engine = TextEngine::new();
    engine.paragraphs.set_target(0);
    let s = spec("A current working width, not a history of width probes.");
    let old = measured_weak(&mut engine, &s, 140.);
    engine.before_layout = Some(Box::new(move || {
        assert!(
            old.upgrade().is_none(),
            "old handoff overlapped new allocation"
        );
    }));
    engine.measure(&s, AxisOffer::Definite(180.));
    assert_eq!(engine.handoff_residency().identities, 1);
}

#[test]
fn intrinsic_miss_retires_handoff_before_scratch_but_scalar_hit_preserves_it() {
    let mut engine = TextEngine::new();
    engine.paragraphs.set_target(0);
    let s = spec("Intrinsic probes must not retain their full scratch Buffers.");
    let old = measured_weak(&mut engine, &s, 140.);
    engine.before_layout = Some(Box::new(move || {
        assert!(old.upgrade().is_none());
    }));
    engine.measure(&s, AxisOffer::MaxContent);
    assert_eq!(engine.handoff_residency().paragraphs, 0);
    assert_eq!(engine.residency().paragraphs, 0);
    engine.before_layout = None;
    let current = measured_weak(&mut engine, &s, 180.);
    let shapes = engine.shape_calls;
    engine.measure(&s, AxisOffer::MaxContent);
    assert_eq!(engine.shape_calls, shapes);
    assert!(current.upgrade().is_some());
    engine.finish_text_frame();
}

#[test]
fn failed_frame_drains_handoff_and_preserves_previous_accepted_owner() {
    let engine = TextEngine::shared();
    engine.borrow_mut().paragraphs.set_target(0);
    let fail = Rc::new(Cell::new(false));
    let mut painter = Painter::new(
        engine.clone(),
        1.,
        Box::new(ControlledBackend { fail: fail.clone() }),
    );
    let text = "Previous accepted pixels survive a failed replacement frame.";
    let s = spec(text);
    let mut kernel = text_tree(text, 140.);
    paint(&mut painter, &kernel).unwrap();
    let accepted = Rc::downgrade(&engine.borrow_mut().paragraph(&s, Some(140.)));
    let candidate = measured_weak(&mut engine.borrow_mut(), &s, 180.);
    assert_eq!(engine.borrow().handoff_residency().paragraphs, 1);
    resize(&mut kernel, 180.);
    fail.set(true);
    assert!(paint(&mut painter, &kernel).is_err());
    assert!(accepted.upgrade().is_some());
    assert!(candidate.upgrade().is_none());
    assert_eq!(engine.borrow().handoff_residency().paragraphs, 0);
    fail.set(false);
    paint(&mut painter, &kernel).unwrap();
    assert!(accepted.upgrade().is_none());
    assert_eq!(engine.borrow().handoff_residency().paragraphs, 0);
    let shapes = engine.borrow().shape_calls;
    paint(&mut painter, &kernel).unwrap();
    assert_eq!(engine.borrow().shape_calls, shapes);
}

#[test]
fn empty_frame_and_catalog_drop_clear_unpresented_handoffs() {
    let engine = TextEngine::shared();
    engine.borrow_mut().paragraphs.set_target(0);
    let s = spec("A removed paragraph must not become retained history.");
    let old = measured_weak(&mut engine.borrow_mut(), &s, 140.);
    let mut painter = Painter::new(
        engine.clone(),
        1.,
        Box::new(ControlledBackend {
            fail: Rc::new(Cell::new(false)),
        }),
    );
    paint(&mut painter, &Kernel::with_monospace()).unwrap();
    assert!(old.upgrade().is_none());
    let old = measured_weak(&mut engine.borrow_mut(), &s, 140.);
    *engine.borrow_mut() = TextEngine::new();
    assert!(old.upgrade().is_none());
    assert_eq!(engine.borrow().handoff_residency().paragraphs, 0);
}

#[test]
fn two_accepted_owner_widths_survive_handoff_drain_and_warm_repaint() {
    let text = "Identical content can belong to two visible owners at different widths.";
    let engine = TextEngine::shared();
    engine.borrow_mut().paragraphs.set_target(0);
    let mut kernel = text_tree(text, 140.);
    let mut style = StyleProps {
        width: Dimension::Points(280.),
        ..StyleProps::default()
    };
    style.mask.set(StyleId::Width);
    kernel
        .apply(
            0,
            3,
            &[
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 3,
                    prop: exact_kernel::PropId::Text,
                    value: exact_kernel::PropValue::Str(text.into()),
                },
                Op::SetStyle {
                    id: 3,
                    patch: Box::new(style),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
            ],
        )
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(400., 400.))
        .unwrap();
    let s = spec(text);
    measured_weak(&mut engine.borrow_mut(), &s, 140.);
    measured_weak(&mut engine.borrow_mut(), &s, 280.);
    assert_eq!(engine.borrow().handoff_residency().identities, 1);
    let mut painter = Painter::new(
        engine.clone(),
        1.,
        Box::new(ControlledBackend {
            fail: Rc::new(Cell::new(false)),
        }),
    );
    paint(&mut painter, &kernel).unwrap();
    assert_eq!(engine.borrow().handoff_residency().paragraphs, 0);
    assert_eq!(engine.borrow().residency().pinned_paragraphs, 2);
    let shapes = engine.borrow().shape_calls;
    for _ in 0..3 {
        paint(&mut painter, &kernel).unwrap();
    }
    assert_eq!(engine.borrow().shape_calls, shapes);
    drop(painter);
    engine.borrow_mut().trim_paragraphs();
    assert_eq!(engine.borrow().residency().paragraphs, 0);
}
