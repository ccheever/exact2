//! Host-owned UTF8 work, distinct from the kernel's borrowed-run construction.
use super::*;
use crate::paint::{Painter, Presented, Scene};
use crate::raster::Raster;
use exact_kernel::{Dimension, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps};
use std::cell::Cell;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Work {
    copied: usize,
    hashed: usize,
    giant_copied: usize,
    giant_hashed: usize,
    giant_shapes: usize,
}
thread_local! { static WORK: Cell<Work> = const { Cell::new(Work { copied:0, hashed:0, giant_copied:0, giant_hashed:0, giant_shapes:0 }) }; }
pub(super) fn copied(n: usize) {
    WORK.with(|w| {
        let mut v = w.get();
        v.copied += n;
        if n >= 65536 {
            v.giant_copied += n;
        }
        w.set(v);
    });
}
pub(super) fn hashed(n: usize) {
    WORK.with(|w| {
        let mut v = w.get();
        v.hashed += n;
        if n >= 65536 {
            v.giant_hashed += n;
        }
        w.set(v);
    });
}
pub(super) fn shaped(spec: &Spec) {
    if spec.runs.iter().map(|r| r.text.len()).sum::<usize>() >= 65536 {
        WORK.with(|w| {
            let mut v = w.get();
            v.giant_shapes += 1;
            w.set(v);
        });
    }
}
fn reset() {
    WORK.with(|w| w.set(Work::default()));
}
fn work() -> Work {
    WORK.with(Cell::get)
}
fn prop(id: u32, prop: PropId, text: &str) -> Op {
    Op::SetProp {
        id,
        prop,
        value: text.into(),
    }
}
fn apply(k: &mut Kernel, ops: &[Op]) {
    k.apply(0, 0, ops).unwrap();
}
fn tree(engine: Shared, text: &str) -> Kernel {
    let mut k = Kernel::new(Box::new(Measurer(engine)));
    let mut style = StyleProps {
        width: Dimension::Points(320.),
        ..StyleProps::default()
    };
    style.mask.set(StyleId::Width);
    apply(
        &mut k,
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
            prop(2, PropId::Text, text),
            Op::SetStyle {
                id: 2,
                patch: Box::new(style),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::AttachRoot { id: 1 },
        ],
    );
    k.compute_layout(1, Offer::definite(360., 160.)).unwrap();
    k
}
fn request(k: &Kernel, engine: Shared, id: u32, width: AxisOffer) -> TextMetrics {
    let n = k.node(id).unwrap();
    let runs = n.text_runs();
    let req = TextMeasureRequest {
        runs: &runs,
        paragraph: exact_kernel::text::Paragraph::from_style(
            &n.computed_style(exact_kernel::StyleMask::INHERITED),
        ),
        width,
        height: AxisOffer::MaxContent,
    };
    Measurer(engine).measure_identified(&n.paragraph_stamp().unwrap(), &req)
}
fn paint(p: &mut Painter, k: &Kernel) -> crate::paint::Frame {
    p.paint(
        &Scene {
            kernel: k,
            roots: &k.roots(),
            hidden: &|_| false,
            presented: &|_| Presented::IDENTITY,
            scroll: &BTreeMap::new(),
            page: (0., 0.),
            images: &BTreeMap::new(),
            focus: None,
            pointer: None,
        },
        (360., 160.),
    )
    .unwrap()
}
#[test]
fn unchanged_giant_and_sibling_typing_copy_hash_and_shape_no_giant_source() {
    let engine = TextEngine::shared();
    let text = "word é\n".repeat(131072);
    assert!(text.len() >= 1024 * 1024);
    let mut k = tree(engine.clone(), &text);
    let mut p = Painter::new(engine.clone(), 1., Box::new(Raster::new()));
    let first = paint(&mut p, &k);
    let metrics = request(&k, engine.clone(), 2, AxisOffer::Definite(320.));
    reset();
    let before = engine.borrow().shape_calls;
    for _ in 0..3 {
        assert_eq!(
            request(&k, engine.clone(), 2, AxisOffer::Definite(320.)),
            metrics
        );
        assert_eq!(paint(&mut p, &k).pixmap.data(), first.pixmap.data());
    }
    assert_eq!(engine.borrow().shape_calls, before);
    for value in ["a", "ab", "EXACT_é"] {
        apply(&mut k, &[prop(3, PropId::Value, value)]);
        k.compute_layout(1, Offer::definite(360., 160.)).unwrap();
        assert_eq!(
            request(&k, engine.clone(), 2, AxisOffer::Definite(320.)),
            metrics
        );
        let _ = paint(&mut p, &k);
    }
    let w = work();
    eprintln!("giant repeat + sibling typing host work: {w:?}");
    assert_eq!(
        (w.giant_copied, w.giant_hashed, w.giant_shapes),
        (0, 0, 0),
        "host giant source work: {w:?}"
    );
}
#[test]
fn identified_exact_offers_share_anonymous_geometry_without_hot_source_work() {
    let e = TextEngine::shared();
    let k = tree(e.clone(), "alpha beta é中\nnext line");
    for offer in [
        AxisOffer::Definite(320.),
        AxisOffer::Definite(117.25),
        AxisOffer::MinContent,
        AxisOffer::MaxContent,
    ] {
        let got = request(&k, e.clone(), 2, offer);
        let n = k.node(2).unwrap();
        let runs = n.text_runs();
        let req = TextMeasureRequest {
            runs: &runs,
            paragraph: exact_kernel::text::Paragraph::from_style(
                &n.computed_style(exact_kernel::StyleMask::INHERITED),
            ),
            width: offer,
            height: AxisOffer::MaxContent,
        };
        assert_eq!(got, Measurer(e.clone()).measure(&req));
        reset();
        let before = e.borrow().shape_calls;
        assert_eq!(request(&k, e.clone(), 2, offer), got);
        assert_eq!(e.borrow().shape_calls, before);
        assert_eq!(work(), Work::default());
    }
}

#[test]
fn paint_only_revision_refreshes_pixels_without_copy_hash_or_shape() {
    let e = TextEngine::shared();
    let mut k = tree(e.clone(), "MMMM é colored text");
    let mut p = Painter::new(e.clone(), 1., Box::new(Raster::new()));
    let a = paint(&mut p, &k);
    let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
    let mut style = StyleProps {
        text_color: exact_kernel::ColorValue::Fixed(exact_kernel::Color::rgba(210, 20, 30, 255)),
        ..StyleProps::default()
    };
    style.mask.set(StyleId::TextColor);
    apply(
        &mut k,
        &[Op::SetStyle {
            id: 2,
            patch: Box::new(style),
        }],
    );
    let next = k.node(2).unwrap().paragraph_stamp().unwrap();
    assert_ne!(stamp, next);
    assert!(stamp.same_metrics(&next));
    reset();
    let count = e.borrow().shape_calls;
    let b = paint(&mut p, &k);
    assert_ne!(a.pixmap.data(), b.pixmap.data());
    assert_eq!(e.borrow().shape_calls, count);
    assert_eq!(work(), Work::default());
}

#[test]
fn independent_domains_owner_reuse_and_text_revisions_cannot_alias() {
    let e = TextEngine::shared();
    let mut a = tree(e.clone(), "i");
    let b = tree(e.clone(), "WWWWWWWW");
    assert_eq!(a.node(2).unwrap().key, b.node(2).unwrap().key);
    let ma = request(&a, e.clone(), 2, AxisOffer::MaxContent);
    let mb = request(&b, e.clone(), 2, AxisOffer::MaxContent);
    assert!(mb.width > ma.width);
    for _ in 0..3 {
        assert_eq!(request(&a, e.clone(), 2, AxisOffer::MaxContent), ma);
        assert_eq!(request(&b, e.clone(), 2, AxisOffer::MaxContent), mb);
    }
    let old = a.node(2).unwrap().paragraph_stamp().unwrap();
    apply(
        &mut a,
        &[
            Op::DestroyView { id: 2 },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            prop(2, PropId::Text, "WWWWWWWW"),
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
        ],
    );
    assert_ne!(old, a.node(2).unwrap().paragraph_stamp().unwrap());
    assert_eq!(request(&a, e.clone(), 2, AxisOffer::MaxContent), mb);
    apply(&mut a, &[prop(2, PropId::Text, "i")]);
    assert_eq!(request(&a, e, 2, AxisOffer::MaxContent), ma);
}

#[test]
fn evicted_identity_and_replaced_catalog_do_not_reuse_stale_bindings() {
    let e = TextEngine::shared();
    let k = tree(e.clone(), "canonical catalog paragraph");
    let want = request(&k, e.clone(), 2, AxisOffer::Definite(320.));
    e.borrow_mut().paragraphs.clear();
    reset();
    assert_eq!(request(&k, e.clone(), 2, AxisOffer::Definite(320.)), want);
    assert!(work().copied > 0 && work().hashed > 0);
    let plan = contract::compile("component App\n  view\n    text \"font catalog\"\n").unwrap();
    e.borrow_mut().install_plan(&plan, Path::new(""));
    reset();
    assert_eq!(request(&k, e.clone(), 2, AxisOffer::Definite(320.)), want);
    assert!(work().copied > 0 && work().hashed > 0);
    assert!(e.borrow().shape_calls > 0);
    reset();
    assert_eq!(request(&k, e, 2, AxisOffer::Definite(320.)), want);
    assert_eq!(work(), Work::default());
}

#[test]
fn stamp_shortcuts_are_bounded_replace_revisions_and_do_not_pin_cold_storage() {
    let e = TextEngine::shared();
    let mut k = Kernel::with_monospace();
    for id in 1..=300 {
        apply(
            &mut k,
            &[
                Op::CreateView {
                    id,
                    node_type: NodeType::Text,
                },
                prop(id, PropId::Text, "shared source"),
            ],
        );
        request(&k, e.clone(), id, AxisOffer::MaxContent);
    }
    assert_eq!(
        e.borrow().paragraphs.binding_count(),
        cache::COLD_IDENTITIES
    );
    for i in 0..300 {
        apply(&mut k, &[prop(300, PropId::Text, &format!("revision {i}"))]);
        request(&k, e.clone(), 300, AxisOffer::MaxContent);
        assert!(e.borrow().paragraphs.binding_count() <= cache::COLD_IDENTITIES);
    }
    e.borrow_mut().paragraphs.clear();
    assert_eq!(e.borrow().paragraphs.binding_count(), 0);
    assert_eq!(e.borrow().residency().identities, 0);
}

#[test]
fn pixel_oracle_after_appearance_scale_width_and_metric_changes() {
    let e = TextEngine::shared();
    let mut k = tree(e.clone(), "Áj Italic source\nMMMM colored words");
    let mut p = Painter::new(e.clone(), 1., Box::new(Raster::new()));
    let _ = paint(&mut p, &k);
    let mut style = StyleProps {
        text_color: exact_kernel::ColorValue::LightDark(
            exact_kernel::Color::rgba(220, 30, 20, 255),
            exact_kernel::Color::rgba(20, 180, 70, 255),
        ),
        ..StyleProps::default()
    };
    style.mask.set(StyleId::TextColor);
    apply(
        &mut k,
        &[Op::SetStyle {
            id: 2,
            patch: Box::new(style),
        }],
    );
    let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
    let before = e.borrow().shape_calls;
    p.dark = false;
    let light = paint(&mut p, &k);
    p.dark = true;
    let dark = paint(&mut p, &k);
    assert_ne!(light.pixmap.data(), dark.pixmap.data());
    assert_eq!(stamp, k.node(2).unwrap().paragraph_stamp().unwrap());
    assert_eq!(e.borrow().shape_calls, before);
    for (width, size, scale) in [(320., 16., 1.), (173.25, 16., 1.), (173.25, 22., 2.)] {
        let mut style = StyleProps {
            width: Dimension::Points(width),
            font_size: size,
            ..StyleProps::default()
        };
        style.mask.set(StyleId::Width);
        style.mask.set(StyleId::FontSize);
        apply(
            &mut k,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(style),
            }],
        );
        k.compute_layout(1, Offer::definite(360., 160.)).unwrap();
        let mut p = Painter::new(e.clone(), scale, Box::new(Raster::new()));
        p.dark = true;
        let fast = paint(&mut p, &k);
        e.borrow_mut().paragraphs.forget_bindings();
        reset();
        let oracle = paint(&mut p, &k);
        assert!(
            work().copied > 0 && work().hashed > 0,
            "oracle must materialize exact source"
        );
        assert_eq!(fast.pixmap.data(), oracle.pixmap.data());
        reset();
        let repeated = paint(&mut p, &k);
        assert_eq!(repeated.pixmap.data(), oracle.pixmap.data());
        assert_eq!(work(), Work::default());
    }
}

#[test]
fn identified_latest_definite_handoffs_keep_the_existing_64_identity_bound() {
    let e = TextEngine::shared();
    e.borrow_mut().paragraphs.set_target(0);
    let mut k = Kernel::with_monospace();
    for id in 1..=70 {
        apply(
            &mut k,
            &[
                Op::CreateView {
                    id,
                    node_type: NodeType::Text,
                },
                prop(id, PropId::Text, &format!("unique measured text {id}")),
            ],
        );
        request(&k, e.clone(), id, AxisOffer::Definite(120.));
        assert_eq!(
            e.borrow().handoff_residency().identities,
            (id as usize).min(cache::HANDOFF_IDENTITIES)
        );
    }
    let metrics = request(&k, e.clone(), 70, AxisOffer::Definite(120.));
    reset();
    let before = e.borrow().shape_calls;
    assert_eq!(
        request(&k, e.clone(), 70, AxisOffer::Definite(120.)),
        metrics
    );
    assert_eq!(work(), Work::default());
    assert_eq!(e.borrow().shape_calls, before);
    request(&k, e.clone(), 70, AxisOffer::Definite(200.));
    assert_eq!(
        e.borrow().handoff_residency().identities,
        cache::HANDOFF_IDENTITIES
    );
    e.borrow_mut().finish_text_frame();
    assert_eq!(e.borrow().handoff_residency().identities, 0);
    assert_eq!(e.borrow().residency().paragraphs, 0);
    assert_eq!(e.borrow().paragraphs.binding_count(), 0);
}

#[test]
fn new_width_reshapes_from_canonical_spec_without_owned_source_copy_or_hash() {
    let e = TextEngine::shared();
    let k = tree(
        e.clone(),
        "exact canonical metric source repeated over widths",
    );
    let initial = request(&k, e.clone(), 2, AxisOffer::Definite(320.));
    reset();
    let before = e.borrow().shape_calls;
    let narrow = request(&k, e.clone(), 2, AxisOffer::Definite(101.125));
    assert!(narrow.height > initial.height);
    assert!(e.borrow().shape_calls > before);
    assert_eq!(work(), Work::default());
}
