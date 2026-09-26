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
thread_local! { static SPEC_LOOKUPS: Cell<usize> = const { Cell::new(0) }; }
pub(super) fn spec_looked_up() {
    SPEC_LOOKUPS.with(|v| v.set(v.get() + 1));
}
fn spec_lookups() -> usize {
    SPEC_LOOKUPS.with(Cell::get)
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
        exclusions: &[],
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
            exclusions: &[],
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
fn new_width_reuses_shape_without_owned_source_copy_or_hash() {
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
    assert_eq!(e.borrow().shape_calls, before);
    assert_eq!(work(), Work::default());
}

#[test]
fn retained_identified_600_to_632_reuses_giant_shape() {
    let e = TextEngine::shared();
    let text = "office café e\u{301} العربية 漢字 🧪 word ".repeat(2048);
    assert!(text.len() >= 65536);
    let mut k = tree(e.clone(), "");
    let mut bold = StyleProps {
        font_weight: 700,
        ..StyleProps::default()
    };
    bold.mask.set(StyleId::FontWeight);
    apply(
        &mut k,
        &[
            Op::ClearProp {
                id: 2,
                prop: PropId::Text,
            },
            Op::CreateView {
                id: 4,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 5,
                node_type: NodeType::Text,
            },
            prop(4, PropId::Text, &text),
            prop(5, PropId::Text, " styled tail e\u{301} 🧪"),
            Op::SetStyle {
                id: 5,
                patch: Box::new(bold),
            },
            Op::SetChildren {
                id: 2,
                children: vec![4, 5],
            },
        ],
    );
    let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
    request(&k, e.clone(), 2, AxisOffer::Definite(600.));
    let a = e
        .borrow_mut()
        .paragraph_identified(&stamp, Some(600.), || panic!("warm source"))
        .unwrap();
    let signature = |p: &Paragraph| {
        // Wrapped rows borrow the same complete source line. Snapshot its bytes
        // once, while retaining exact text equality for every row below.
        let mut texts = HashMap::new();
        p.layout_runs()
            .map(|r| {
                let text = texts
                    .entry((r.text.as_ptr(), r.text.len()))
                    .or_insert_with(|| Rc::<str>::from(r.text))
                    .clone();
                (
                    text,
                    format!("{:?}", r.glyphs),
                    r.line_y.to_bits(),
                    r.line_w.to_bits(),
                )
            })
            .collect::<Vec<_>>()
    };
    let original = signature(&a);
    let metrics = paragraph_metrics(&a);
    let baselines = a.baselines.clone();
    let calls = e.borrow().shape_calls;
    let actual_calls = shaping::shape_line_calls();
    reset();
    let b = e
        .borrow_mut()
        .paragraph_identified(&stamp, Some(632.), || panic!("width copied source"))
        .unwrap();
    assert_ne!(a.height, b.height, "fixture must change wrapping");
    assert_eq!(signature(&a), original);
    assert_eq!(paragraph_metrics(&a), metrics);
    assert_eq!(a.baselines, baselines);
    assert!(Rc::ptr_eq(
        &a,
        &e.borrow_mut()
            .paragraph_identified(&stamp, Some(600.), || panic!("A lost identity"))
            .unwrap()
    ));
    assert_eq!(work().copied, 0);
    assert_eq!(work().hashed, 0);
    assert_eq!(
        e.borrow().shape_calls,
        calls,
        "new width reshaped retained source: {:?}",
        work()
    );
    assert_eq!(work().giant_shapes, 0);
    assert_eq!(shaping::shape_line_calls(), actual_calls);
    assert!(Rc::ptr_eq(&a.source, &b.source));
}

// Both arms use existing identified_spec/measure APIs. Counters live at actual
// Run construction/clone sites; the reference always uses borrowed identity.
mod owned_spec {
    use super::*;

    fn engine() -> TextEngine {
        let mut db = fontdb::Database::new();
        db.load_font_source(fontdb::Source::Binary(Arc::new(
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").to_vec(),
        )));
        db.set_sans_serif_family("DejaVu Sans");
        TextEngine::with_catalog(catalog::Catalog::with_fonts(
            FontSystem::new_with_locale_and_db("en-US".into(), db),
        ))
    }
    fn run(text: &str) -> Run {
        Run {
            text: text.into(),
            size: 16.,
            weight: 400,
            family: 0,
            italic: false,
            line_height: None,
            letter_spacing: 0.,
            font_variant_numeric: 0,
        }
    }
    fn spec(text: &str) -> Spec {
        Spec {
            strut: run(""),
            runs: vec![run(text)],
            align: TextAlign::Left,
            line_clamp: 0,
            overflow_wrap: exact_kernel::OverflowWrap::Normal,
            white_space: exact_kernel::WhiteSpace::Normal,
            direction: exact_kernel::Direction::Ltr,
        }
    }
    fn stamp_tree(text: &str) -> Kernel {
        let mut k = Kernel::with_monospace();
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
                prop(2, PropId::Text, text),
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::AttachRoot { id: 1 },
            ],
        );
        k
    }
    fn capacities(s: &Spec) -> Vec<usize> {
        let mut result = vec![s.runs.capacity(), s.strut.text.capacity()];
        result.extend(s.runs.iter().map(|r| r.text.capacity()));
        result
    }
    fn canonical(s: &Spec) -> bool {
        s.runs.capacity() == s.runs.len()
            && std::iter::once(&s.strut)
                .chain(&s.runs)
                .all(|r| r.text.capacity() == r.text.len())
    }
    fn pixels(e: &mut TextEngine, p: &Paragraph) -> Vec<u8> {
        let palette: Vec<_> = p
            .source
            .spec
            .runs
            .iter()
            .enumerate()
            .map(|(i, _)| RunPaint {
                color: if i == 0 {
                    [20, 80, 150, 255]
                } else {
                    [160, 20, 40, 220]
                },
                source: i as u32 + 2,
            })
            .collect();
        let mut pix = Pixmap::new(360, 180).unwrap();
        e.paint(
            &mut pix,
            p,
            &palette,
            (3.25, 4.5),
            1.25,
            Transform::from_scale(1.25, 1.25),
            None,
        );
        pix.data().to_vec()
    }
    fn geometry(p: &Paragraph) -> (TextMetrics, Vec<u32>, Vec<String>) {
        (
            paragraph_metrics(p),
            p.baselines.iter().map(|v| v.to_bits()).collect(),
            p.layout_runs()
                .flat_map(|r| r.glyphs.iter().map(|g| format!("{g:?}")))
                .collect(),
        )
    }

    #[test]
    fn cold_real_request_copies_unicode_once_with_exact_borrowed_pixels() {
        let text = "café e\u{301} العربية\nsecond UTF8 line";
        let k = stamp_tree(text);
        let n = k.node(2).unwrap();
        let stamp = n.paragraph_stamp().unwrap();
        let runs = n.text_runs();
        let req = TextMeasureRequest {
            exclusions: &[],
            runs: &runs,
            paragraph: exact_kernel::text::Paragraph::from_style(
                &n.computed_style(exact_kernel::StyleMask::INHERITED),
            ),
            width: AxisOffer::Definite(117.25),
            height: AxisOffer::MaxContent,
        };
        let expected_bytes: usize = runs.iter().map(|r| r.text.len()).sum();
        let old_spec = Spec::from_request(&req);
        assert!(canonical(&old_spec));
        assert_eq!(capacities(&old_spec), capacities(&old_spec.clone()));
        let mut a = engine();
        let mut b = engine();
        reset();
        let measured = a.measure_identified(&stamp, &req);
        let actual = work();
        let reference = b.measure(&old_spec, req.width);
        assert_eq!(measured, reference);
        let p = a
            .paragraph_identified(&stamp, Some(117.25), || panic!("warm spec"))
            .unwrap();
        let q = b.paragraph(&old_spec, Some(117.25));
        assert_eq!(geometry(&p), geometry(&q));
        assert_eq!(pixels(&mut a, &p), pixels(&mut b, &q));
        assert_eq!(
            a.residency().cold_policy_bytes,
            b.residency().cold_policy_bytes
        );
        eprintln!(
            "identified UTF8 copied={} expected={} hashed={}",
            actual.copied, expected_bytes, actual.hashed
        );
        assert!(actual.hashed >= expected_bytes);
        assert_eq!(actual.copied, expected_bytes, "second owned source copy");
    }

    #[test]
    fn canonical_builder_moves_allocations_and_preserves_exact_key_state() {
        let k = stamp_tree("styled source");
        let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
        let mut s = spec("α styled first ");
        let mut second = run("second 日本語\nline");
        second.size = 19.25;
        second.weight = 700;
        second.italic = true;
        second.line_height = Some(23.5);
        second.letter_spacing = 0.25;
        s.runs = vec![s.runs.remove(0), second];
        s.align = TextAlign::Center;
        assert!(canonical(&s));
        let original = s.clone();
        assert_eq!(capacities(&s), capacities(&original));
        let ptrs: Vec<_> = s.runs.iter().map(|r| r.text.as_ptr()).collect();
        let vector = s.runs.as_ptr();
        let mut a = engine();
        let mut b = engine();
        reset();
        let (key, stored) = a.identified_spec(&stamp, || s);
        let copied = work().copied;
        let old_key = b.paragraphs.identity(&original);
        b.paragraphs.bind(&stamp, old_key);
        assert_eq!(key, old_key);
        assert_eq!(*stored, original);
        assert_eq!(capacities(&stored), capacities(&original));
        assert_eq!(
            a.paragraphs.trim_test_state(),
            b.paragraphs.trim_test_state()
        );
        let p = a.paragraph_for(&stored, Some(120.25), key);
        let q = b.paragraph(&original, Some(120.25));
        assert_eq!(geometry(&p), geometry(&q));
        assert_eq!(pixels(&mut a, &p), pixels(&mut b, &q));
        assert_eq!(copied, 0, "canonical builder copied owned text");
        assert_eq!(stored.runs.as_ptr(), vector);
        assert_eq!(
            stored
                .runs
                .iter()
                .map(|r| r.text.as_ptr())
                .collect::<Vec<_>>(),
            ptrs
        );
    }

    #[test]
    fn spare_capacity_falls_back_and_keeps_original_eviction_accounting() {
        for spare in 0..3 {
            let k = stamp_tree("capacity fallback");
            let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
            let mut s = spec("spare-capacity UTF8 é");
            match spare {
                0 => s.runs.reserve(64),
                1 => s.strut.text.reserve(64),
                _ => s.runs[0].text.reserve(128),
            }
            assert!(!canonical(&s));
            let expected = s.clone();
            let bytes: usize = s.runs.iter().map(|r| r.text.len()).sum();
            let mut a = engine();
            let mut b = engine();
            reset();
            let (key, stored) = a.identified_spec(&stamp, || s);
            assert_eq!(work().copied, bytes);
            let old_key = b.paragraphs.identity(&expected);
            b.paragraphs.bind(&stamp, old_key);
            assert_eq!(key, old_key);
            assert_eq!(capacities(&stored), capacities(&expected));
            assert_eq!(
                a.paragraphs.trim_test_state(),
                b.paragraphs.trim_test_state()
            );
            drop(stored);
            let exact = a.residency().cold_policy_bytes;
            for target in [exact, exact.saturating_sub(1), 0] {
                a.paragraphs.set_target(target);
                b.paragraphs.set_target(target);
                assert_eq!(
                    a.paragraphs.trim_test_state(),
                    b.paragraphs.trim_test_state()
                );
            }
        }
    }

    #[test]
    fn equal_content_new_owner_keeps_canonical_allocation_and_borrowed_path() {
        let a_tree = stamp_tree("same key");
        let b_tree = stamp_tree("same key");
        let sa = a_tree.node(2).unwrap().paragraph_stamp().unwrap();
        let sb = b_tree.node(2).unwrap().paragraph_stamp().unwrap();
        assert!(!sa.same_metrics(&sb));
        let mut e = engine();
        let original = spec("same canonical text");
        reset();
        let key = e.paragraphs.identity(&original);
        assert_eq!(work().copied, original.runs[0].text.len());
        let canonical = e.paragraphs.spec(key).unwrap();
        assert_ne!(
            canonical.runs[0].text.as_ptr(),
            original.runs[0].text.as_ptr()
        );
        for stamp in [&sa, &sb] {
            let owned = spec("same canonical text");
            reset();
            let (again, stored) = e.identified_spec(stamp, || owned);
            assert_eq!(again, key);
            assert!(Arc::ptr_eq(&stored, &canonical));
            assert_eq!(work().copied, 0);
        }
        reset();
        assert_eq!(e.paragraphs.identity(&original), key);
        assert_eq!(work().copied, 0);
    }

    #[test]
    fn empty_canonical_specs_keep_scalar_and_capacity_semantics() {
        for no_runs in [false, true] {
            let k = stamp_tree("");
            let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
            let mut s = spec("");
            if no_runs {
                s.runs = Vec::new();
            }
            let old = s.clone();
            assert!(canonical(&s));
            assert_eq!(capacities(&s), capacities(&old));
            let mut a = engine();
            let mut b = engine();
            let (key, stored) = a.identified_spec(&stamp, || s);
            let old_key = b.paragraphs.identity(&old);
            b.paragraphs.bind(&stamp, old_key);
            assert_eq!(
                a.measure_for(&stored, AxisOffer::MinContent, key),
                b.measure_for(&old, AxisOffer::MinContent, old_key)
            );
            assert_eq!(
                a.paragraphs.trim_test_state(),
                b.paragraphs.trim_test_state()
            );
            assert_eq!(a.shape_calls, 0);
            assert_eq!(b.shape_calls, 0);
        }
    }

    #[test]
    fn moved_source_retains_a_while_b_replaces_then_dies_at_last_owner() {
        let mut k = stamp_tree("old accepted");
        let mut e = engine();
        let sa = k.node(2).unwrap().paragraph_stamp().unwrap();
        let a = e
            .paragraph_identified(&sa, Some(120.), || spec("old accepted"))
            .unwrap();
        let old_source = Arc::downgrade(&a.source.spec);
        let old_paragraph = Rc::downgrade(&a);
        apply(&mut k, &[prop(2, PropId::Text, "new accepted")]);
        let sb = k.node(2).unwrap().paragraph_stamp().unwrap();
        assert!(!sa.same_metrics(&sb));
        let b = e
            .paragraph_identified(&sb, Some(117.25), || spec("new accepted"))
            .unwrap();
        let new_source = Arc::downgrade(&b.source.spec);
        assert!(!Rc::ptr_eq(&a.source, &b.source));
        e.paragraphs.clear();
        assert!(old_source.upgrade().is_some());
        assert!(new_source.upgrade().is_some());
        // clear retains pinned identities; remove that owner before last-owner checks.
        drop(e);
        assert!(old_source.upgrade().is_some());
        assert!(new_source.upgrade().is_some());
        drop(a);
        assert!(old_paragraph.upgrade().is_none());
        assert!(old_source.upgrade().is_none());
        assert!(new_source.upgrade().is_some());
        drop(b);
        assert!(new_source.upgrade().is_none());
    }

    mod warm_identity {
        use super::*;

        fn original(
            e: &mut TextEngine,
            stamp: &ParagraphStamp,
            text: &str,
        ) -> ((u64, u64), Arc<Spec>) {
            if let Some(key) = e.paragraphs.identified_reference(stamp) {
                return (key, e.paragraphs.spec(key).expect("checked identity"));
            }
            let key = e.paragraphs.identity_owned(spec(text));
            e.paragraphs.bind(stamp, key);
            (key, e.paragraphs.spec(key).expect("new identity"))
        }

        #[test]
        fn warm_painter_performs_one_spec_lookup_with_identical_pixels() {
            let e = Rc::new(std::cell::RefCell::new(engine()));
            let mut k = tree(e.clone(), "café e\u{301} 日本語 warm paint");
            apply(
                &mut k,
                &[Op::SetChildren {
                    id: 1,
                    children: vec![2],
                }],
            );
            k.compute_layout(1, Offer::definite(360., 160.)).unwrap();
            let mut painter = Painter::new(e.clone(), 1., Box::new(Raster::new()));
            let first = paint(&mut painter, &k);
            let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
            let p = e
                .borrow_mut()
                .paragraph_identified(&stamp, Some(320.), || panic!("warm"))
                .unwrap();
            let shape_before = e.borrow().shape_calls;
            reset();
            let before = spec_lookups();
            let next = paint(&mut painter, &k);
            let actual = spec_lookups() - before;
            assert_eq!(next.pixmap.data(), first.pixmap.data());
            assert_eq!(e.borrow().shape_calls, shape_before);
            assert_eq!(work(), Work::default());
            let again = e
                .borrow_mut()
                .paragraph_identified(&stamp, Some(320.), || panic!("warm"))
                .unwrap();
            assert!(Rc::ptr_eq(&p, &again));
            eprintln!("warm actual Painter Cache::spec calls={actual}, expected=1");
            assert_eq!(actual, 1, "duplicate warm Spec lookup");
        }

        #[test]
        fn warm_sequence_matches_original_state_geometry_palette_and_stale_refusal() {
            let mut k = stamp_tree("styled α日本語");
            apply(
                &mut k,
                &[
                    Op::CreateView {
                        id: 3,
                        node_type: NodeType::Text,
                    },
                    prop(3, PropId::Text, "other owner"),
                    Op::SetChildren {
                        id: 1,
                        children: vec![2, 3],
                    },
                ],
            );
            let mut a = engine();
            let mut b = engine();
            let mut held_a = Vec::new();
            let mut held_b = Vec::new();
            for (id, text, width) in [
                (2, "styled α日本語", 120.25),
                (3, "other owner", 93.5),
                (2, "styled α日本語", 120.25),
                (3, "other owner", 93.5),
                (2, "styled α日本語", 81.25),
            ] {
                let stamp = k.node(id).unwrap().paragraph_stamp().unwrap();
                let (key, s) = a.identified_spec(&stamp, || spec(text));
                let (old_key, old_s) = original(&mut b, &stamp, text);
                assert_eq!(key, old_key);
                assert_eq!(*s, *old_s);
                assert!(!Arc::ptr_eq(&s, &old_s));
                let pa = a.paragraph_for(&s, Some(width), key);
                let pb = b.paragraph_for(&old_s, Some(width), old_key);
                assert_eq!(geometry(&pa), geometry(&pb));
                assert_eq!(pixels(&mut a, &pa), pixels(&mut b, &pb));
                assert_eq!(
                    a.paragraphs.trim_test_state(),
                    b.paragraphs.trim_test_state()
                );
                held_a.push(pa);
                held_b.push(pb);
            }
            let prior = k.node(2).unwrap().paragraph_stamp().unwrap();
            let mut style = StyleProps {
                text_color: exact_kernel::ColorValue::Fixed(exact_kernel::Color::rgba(
                    220, 15, 25, 255,
                )),
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
            let painted = k.node(2).unwrap().paragraph_stamp().unwrap();
            assert_ne!(prior, painted);
            assert!(prior.same_metrics(&painted));
            let (key, sa) = a.identified_spec(&painted, || panic!("paint-only build"));
            let (old_key, sb) = original(&mut b, &painted, "must not replace");
            assert_eq!(key, old_key);
            assert_eq!(*sa, *sb);
            assert_eq!(
                a.paragraphs.trim_test_state(),
                b.paragraphs.trim_test_state()
            );
            drop(sa);
            drop(sb);
            // Invalid identity is removed before ordinary cold fallback, with no hit clock.
            a.paragraphs.bind(&painted, (u64::MAX, u64::MAX));
            b.paragraphs.bind(&painted, (u64::MAX, u64::MAX));
            let (key, sa) = a.identified_spec(&painted, || spec("styled α日本語"));
            let (old_key, sb) = original(&mut b, &painted, "styled α日本語");
            assert_eq!(key, old_key);
            assert_eq!(*sa, *sb);
            assert_eq!(
                a.paragraphs.trim_test_state(),
                b.paragraphs.trim_test_state()
            );
            drop(sa);
            drop(sb);
            drop(held_a);
            drop(held_b);
            a.paragraphs.clear();
            b.paragraphs.clear();
            assert_eq!(
                a.paragraphs.trim_test_state(),
                b.paragraphs.trim_test_state()
            );
            assert_eq!(a.residency().identities, 0);
        }

        #[test]
        fn warm_empty_and_returned_spec_add_no_persistent_owner() {
            for text in ["", "one owner é"] {
                let k = stamp_tree(text);
                let stamp = k.node(2).unwrap().paragraph_stamp().unwrap();
                let mut e = engine();
                let (key, first) = e.identified_spec(&stamp, || spec(text));
                let weak = Arc::downgrade(&first);
                assert_eq!(Arc::strong_count(&first), 2);
                let (again, second) = e.identified_spec(&stamp, || panic!("warm build"));
                assert_eq!(key, again);
                assert!(Arc::ptr_eq(&first, &second));
                assert_eq!(Arc::strong_count(&first), 3);
                drop(second);
                assert_eq!(Arc::strong_count(&first), 2);
                e.paragraphs.clear();
                assert_eq!(e.residency().identities, 0);
                assert_eq!(Arc::strong_count(&first), 1);
                assert!(weak.upgrade().is_some());
                drop(first);
                assert!(weak.upgrade().is_none());
                if text.is_empty() {
                    assert!(e
                        .paragraph_identified(&stamp, Some(100.), || spec(""))
                        .is_none());
                }
            }
        }
    }
}
