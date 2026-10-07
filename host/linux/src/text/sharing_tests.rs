//! A width broken from a shared shape is the width a fresh shape gives: the
//! independent oracle is the same spec shaped by a new engine over the same
//! fonts and broken once at that width (Parley re-breaks one layout in place;
//! nothing a previous width did — justification, hyphens, a clamp — may
//! leak into the next).
use super::*;
use std::mem::size_of;

fn spec() -> Spec {
    let mut s = crate::paint::text_spec(
        &exact_kernel::StyleProps::default(),
        "office café e\u{301} العربية 漢字 🧪\t words\r\n\r\nשלום second paragraph\n",
    );
    let mut bold = s.runs[0].clone();
    bold.text = "bold italic tail 👩‍💻".into();
    bold.weight = 700;
    bold.italic = true;
    bold.size = 21.;
    s.runs.push(bold);
    s
}

// Every glyph's float bits, including signed zero, and its face's data.
fn glyph(g: &LayoutGlyph, faces: &[Face]) -> String {
    let face = &faces[g.face as usize];
    format!(
        "{:?}:{:?}",
        (
            g.start,
            g.end,
            face.font.index,
            face.font.data.len(),
            &face.coords,
            face.skew,
            g.glyph_id,
            g.level,
            g.metadata,
        ),
        [g.font_size, g.x, g.y, g.w].map(f32::to_bits)
    )
}
fn signature(p: &Paragraph) -> Vec<String> {
    p.layout_runs()
        .map(|run| {
            let mut s = format!(
                "{}:{:?}:{}:{:?}",
                run.line_i,
                run.text,
                run.rtl,
                [run.line_y, run.line_top, run.line_height, run.line_w].map(f32::to_bits),
            );
            for g in run.glyphs {
                s.push_str(&glyph(g, &p.lines().faces));
            }
            s
        })
        .collect()
}
fn metrics(p: &Paragraph) -> (Vec<u32>, Vec<u32>) {
    (
        [p.width, p.height, p.first_baseline]
            .map(f32::to_bits)
            .to_vec(),
        p.baselines.iter().map(|v| v.to_bits()).collect(),
    )
}

#[test]
fn shared_layout_matches_a_fresh_shape_at_every_width() {
    let mut engine = TextEngine::new();
    for align in [
        TextAlign::Left,
        TextAlign::Center,
        TextAlign::Right,
        TextAlign::Justify,
    ] {
        for (height, clamp, wrap) in [
            (None, 0, exact_kernel::OverflowWrap::Normal),
            (Some(0.), 0, exact_kernel::OverflowWrap::Anywhere),
            (Some(13.25), 2, exact_kernel::OverflowWrap::BreakWord),
        ] {
            let mut s = spec();
            s.align = align;
            s.line_clamp = clamp;
            s.overflow_wrap = wrap;
            s.strut.line_height = height;
            for r in &mut s.runs {
                r.line_height = height;
                r.letter_spacing = 0.3;
            }
            let mut pinned = Vec::new();
            for width in [Some(0.), Some(97.125), Some(203.5), None] {
                let p = engine.paragraph(&s, width);
                let mut fresh = TextEngine::new();
                let expected = fresh.layout(&s, width);
                assert_eq!(signature(&p), signature(&expected), "{align:?} {width:?}");
                assert_eq!(metrics(&p), metrics(&expected));
                pinned.push(p);
            }
            assert!(pinned
                .windows(2)
                .all(|w| Rc::ptr_eq(&w[0].source, &w[1].source)));
        }
    }
}

#[test]
fn width_and_intrinsic_queries_reuse_actual_shape_calls() {
    let mut engine = TextEngine::new();
    let s = spec();
    let a = engine.paragraph(&s, Some(120.));
    let calls = shaping::shape_line_calls();
    let original = signature(&a);
    for width in [121., 300., 80., 121.] {
        let b = engine.paragraph(&s, Some(width));
        assert!(Rc::ptr_eq(&a.source, &b.source));
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(shaping::shape_line_calls(), calls);
    }
    engine.measure(&s, AxisOffer::MinContent);
    engine.measure(&s, AxisOffer::MaxContent);
    assert_eq!(shaping::shape_line_calls(), calls);
    assert_eq!(signature(&a), original);
    let mut changed = s.clone();
    changed.runs[0].size += 1.;
    let c = engine.paragraph(&changed, Some(120.));
    assert!(!Rc::ptr_eq(&a.source, &c.source));
    assert!(shaping::shape_line_calls() > calls);
}

#[test]
fn retiring_catalog_outlives_engine_and_supplies_cpu_and_gpu_fonts() {
    let mut engine = TextEngine::new();
    let a = engine.paragraph(&spec(), Some(200.));
    let old = Rc::downgrade(&a.source.catalog);
    let source = Rc::downgrade(&a.source);
    let palette = [
        RunPaint {
            color: [30, 70, 160, 255],
            source: 7,
        },
        RunPaint {
            color: [150, 30, 45, 255],
            source: 8,
        },
    ];
    let render = |engine: &mut TextEngine| {
        let mut image = Pixmap::new(300, 300).unwrap();
        engine.paint(
            &mut image,
            &a,
            &palette,
            (0., 0.),
            1.,
            Transform::identity(),
            None,
        );
        image
    };
    let expected = render(&mut engine);
    assert!(expected.data().chunks_exact(4).any(|p| p[3] != 0));
    let batches = engine.glyph_runs(&a, &palette);
    assert!(!batches.is_empty());
    // Exercise the public install boundary, then make the CURRENT catalog
    // empty: an old paragraph paints from its own catalog's faces.
    let plan = caltrain::compile().unwrap();
    engine.install_plan(&plan, Path::new("/nonexistent"));
    engine.catalog.borrow_mut().fonts.collection = fonts::empty_collection();
    assert!(old.upgrade().is_some());
    assert_eq!(render(&mut engine).data(), expected.data());
    let actual = engine.glyph_runs(&a, &palette);
    assert_eq!(actual.len(), batches.len());
    for (a, b) in actual.iter().zip(&batches) {
        assert_eq!(a.font.index, b.font.index);
        assert_eq!(a.font.data.data(), b.font.data.data());
    }
    assert_eq!(
        actual
            .iter()
            .map(|r| (&r.glyphs, r.paint))
            .collect::<Vec<_>>(),
        batches
            .iter()
            .map(|r| (&r.glyphs, r.paint))
            .collect::<Vec<_>>()
    );
    drop(engine);
    assert!(old.upgrade().is_some());
    drop(a);
    assert!(source.upgrade().is_none());
    assert!(
        old.upgrade().is_none(),
        "font batches must retain bytes, not the catalog"
    );
    assert!(actual.iter().all(|r| !r.font.data.data().is_empty()));
}

#[test]
fn two_widths_count_shared_source_once_and_release_without_history() {
    let mut engine = TextEngine::new();
    let s = spec();
    let a = engine.paragraph(&s, Some(150.));
    let b = engine.paragraph(&s, Some(210.));
    let source = Rc::downgrade(&a.source);
    assert!(Rc::ptr_eq(&a.source, &b.source));
    let report = engine.residency();
    assert_eq!(report.paragraphs, 2);
    assert_eq!(
        report.owned_capacity_bytes,
        report.key_capacity_bytes + a.owned_capacity_bytes() + b.owned_capacity_bytes()
            - a.source.accessible_capacity_bytes
    );
    let retiring = cache::Cache::default().retiring([&a, &a, &b].into_iter());
    assert_eq!(retiring.owners, 3);
    assert_eq!(retiring.paragraphs, 2);
    assert_eq!(
        retiring.owned_capacity_bytes,
        report.owned_capacity_bytes - report.key_capacity_bytes
    );
    engine.paragraphs.clear();
    assert!(source.upgrade().is_some());
    let wa = Rc::downgrade(&a);
    drop(a);
    assert!(wa.upgrade().is_none());
    assert!(source.upgrade().is_some());
    drop(b);
    engine.paragraphs.set_target(0);
    assert!(source.upgrade().is_none());
    assert_eq!(engine.residency().identities, 0);
    assert_eq!(engine.paragraphs.indexed_widths(), 0);
}

pub(super) fn assert_tight_glyph_storage(p: &Paragraph) {
    let lines = p.lines();
    let spare = (lines.glyphs.capacity() - lines.glyphs.len()) * size_of::<LayoutGlyph>();
    assert!(spare < 4 * 1024, "{spare} spare glyph bytes");
}
fn compact_fixture_engine() -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_bytes(
        &[include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice()],
        "DejaVu Sans",
    ))
}

/// A width's storage is the glyphs it shows and its lines, in a compact
/// record (36 bytes a glyph; cosmic-text's `LayoutGlyph` was 104), with no
/// spare capacity worth keeping, wrapped or not.
#[test]
fn a_width_keeps_bounded_bytes_per_glyph() {
    assert!(
        size_of::<LayoutGlyph>() <= 36,
        "{}",
        size_of::<LayoutGlyph>()
    );
    let mut engine = compact_fixture_engine();
    let mut s = spec();
    s.runs[0].text = format!(
        "{}\n{}",
        "office ffi café e\u{301} words 12345 ".repeat(2500),
        s.runs[0].text
    );
    assert!(s.runs[0].text.len() >= 65536);
    for width in [Some(600.), Some(984.), None] {
        let p = engine.paragraph(&s, width);
        let lines = p.lines();
        assert_tight_glyph_storage(&p);
        let glyphs = lines.glyphs.len();
        assert!(glyphs > 50_000, "{glyphs}");
        let bytes = lines.capacity_bytes();
        assert!(
            bytes
                <= glyphs * size_of::<LayoutGlyph>()
                    + lines.lines.len() * size_of::<LayoutLine>()
                    + 4096
                    + 64 * lines.faces.len(),
            "{bytes} bytes for {glyphs} glyphs"
        );
        // The shape is the source's alone: two widths share it.
        assert!(p.source.accessible_capacity_bytes > 0);
    }
}

#[test]
fn a_small_paragraph_keeps_exactly_its_glyphs() {
    let mut engine = compact_fixture_engine();
    let p = engine.paragraph(&spec(), Some(200.));
    let shown: usize = p.layout_runs().map(|r| r.glyphs.len()).sum();
    assert_eq!(p.lines().glyphs.len(), shown);
    assert_tight_glyph_storage(&p);
}

// A face's metrics are read from its data once per catalog, however many
// widths and glyphs use it.
mod face_metrics {
    use super::*;

    fn engine() -> TextEngine {
        TextEngine::with_catalog(catalog::Catalog::from_bytes(
            &[
                include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf")
                    .as_slice(),
                include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans-Bold.ttf")
                    .as_slice(),
                include_bytes!("../../tests/fonts/Inter-Regular.ttf").as_slice(),
                include_bytes!("../../tests/fonts/NotoSansArabic.ttf").as_slice(),
            ],
            "DejaVu Sans",
        ))
    }

    fn source(engine: &TextEngine, spec: Spec) -> Rc<ShapedSource> {
        Rc::new(ShapedSource::new(engine.catalog.clone(), Arc::new(spec)))
    }

    fn faces(p: &Paragraph) -> std::collections::HashSet<FaceId> {
        p.layout_runs()
            .flat_map(|r| r.glyphs)
            .map(|g| p.lines().faces[g.face as usize].id())
            .collect()
    }

    #[test]
    fn widths_read_each_faces_metrics_once() {
        let engine = engine();
        let s = crate::paint::text_spec(
            &exact_kernel::StyleProps::default(),
            &"office café words 1234 ".repeat(24),
        );
        let source = source(&engine, s);
        let mut seen = std::collections::HashSet::new();
        for width in [193.25, 241.75, 193.25] {
            let before = shaping::width_font_lookups();
            let p = source.layout(Some(width));
            let actual = shaping::width_font_lookups() - before;
            let new: Vec<_> = faces(&p).into_iter().filter(|f| seen.insert(*f)).collect();
            assert!(
                actual <= new.len(),
                "width={width}: {actual} reads, {new:?} new"
            );
        }
    }

    #[test]
    fn fallback_weights_sizes_and_metadata_are_kept_per_glyph() {
        let engine = engine();
        engine.catalog.borrow_mut().families[0] = FamilyChoice::Declared("Inter".into());
        let mut s = crate::paint::text_spec(
            &exact_kernel::StyleProps::default(),
            "café العربية abc abc ",
        );
        for (weight, size, height) in [
            (700, 21.25, None),
            (400, 12.5, Some(7.25)),
            (400, 23.5, None),
        ] {
            let mut r = s.runs[0].clone();
            r.text = "tail שלום xyz xyz ".into();
            r.weight = weight;
            // Inter fixture is regular-only: the bold run uses the sans
            // family, whose bold face is loaded.
            if weight == 700 {
                r.family = 1;
            }
            r.size = size;
            r.line_height = height;
            s.runs.push(r);
        }
        let source = source(&engine, s);
        let p = source.layout(Some(211.125));
        let keys: Vec<_> = p
            .layout_runs()
            .flat_map(|r| r.glyphs)
            .map(|g| {
                (
                    p.lines().faces[g.face as usize].weight,
                    g.font_size.to_bits(),
                    g.metadata,
                    g.face,
                )
            })
            .collect();
        assert!(keys.iter().any(|k| k.0 == 700));
        assert!(keys.iter().any(|k| k.0 == 400));
        assert!(keys.iter().any(|k| k.1 == 12.5f32.to_bits()));
        assert!(keys.iter().any(|k| k.1 == 23.5f32.to_bits()));
        let first_run_faces: std::collections::HashSet<_> =
            keys.iter().filter(|k| k.2 == 0).map(|k| k.3).collect();
        assert!(
            first_run_faces.len() >= 2,
            "Latin/Arabic must exercise actual fallback faces"
        );
        // Shaped afresh, the same.
        let again = Rc::new(ShapedSource::new(
            engine.catalog.clone(),
            source.spec.clone(),
        ));
        assert_eq!(signature(&p), signature(&again.layout(Some(211.125))));
    }

    #[test]
    fn explicit_normal_equal_ties_and_zero_height_keep_bits() {
        let engine = engine();
        let mut s = crate::paint::text_spec(&exact_kernel::StyleProps::default(), "aaa ");
        s.strut.size = 17.25;
        s.runs[0].size = 17.25;
        let (a, d, l) = engine.catalog.borrow_mut().font_metrics(&s.strut);
        let normal = a + d + l;
        for height in [Some(normal), Some(0.), Some(7.25), None] {
            s.strut.line_height = height;
            s.runs[0].line_height = height;
            let mut normal_run = s.runs[0].clone();
            normal_run.line_height = None;
            normal_run.text = "bbb ".into();
            s.runs.truncate(1);
            s.runs.push(normal_run);
            for reverse in [false, true] {
                let mut current = s.clone();
                if reverse {
                    current.runs.reverse();
                }
                let source = source(&engine, current);
                let p = source.layout(Some(270.5));
                if height == Some(normal) {
                    // This fixture's strut and normal run use the SAME font and
                    // size, so equal ascent/descent ties must preserve the
                    // explicit &= normal decision, including final ceil.
                    assert_eq!(p.height.to_bits(), normal.ceil().to_bits());
                }
            }
        }
    }

    #[test]
    fn catalog_reset_keeps_old_paragraph_and_new_width_exact() {
        let mut engine = engine();
        let s = crate::paint::text_spec(
            &exact_kernel::StyleProps::default(),
            "old catalog café words ",
        );
        let source = source(&engine, s);
        let old_catalog = Rc::downgrade(&source.catalog);
        let p = source.layout(Some(160.25));
        let palette = [RunPaint {
            color: [30, 70, 160, 231],
            source: 0,
        }];
        let pixels = |engine: &mut TextEngine, p: &Paragraph| {
            let mut image = Pixmap::new(320, 180).unwrap();
            engine.paint(
                &mut image,
                p,
                &palette,
                (7.375, 0.625),
                1.25,
                Transform::from_scale(1.25, 1.25),
                None,
            );
            image.data().to_vec()
        };
        let expected = pixels(&mut engine, &p);
        assert!(expected.chunks_exact(4).any(|p| p[3] != 0));
        engine.catalog = Rc::new(RefCell::new(catalog::Catalog::from_bytes(&[], "none")));
        assert_eq!(pixels(&mut engine, &p), expected);
        let b = source.layout(Some(201.75));
        assert_eq!(
            signature(&b),
            signature(
                &Rc::new(ShapedSource::new(
                    source.catalog.clone(),
                    source.spec.clone()
                ))
                .layout(Some(201.75))
            )
        );
        drop((p, b, source));
        assert!(old_catalog.upgrade().is_none());
    }

    #[test]
    fn empty_paragraph_has_no_width_font_lookup() {
        let engine = engine();
        let s = crate::paint::text_spec(&exact_kernel::StyleProps::default(), "");
        let source = source(&engine, s);
        let before = shaping::width_font_lookups();
        let p = source.layout(Some(0.));
        assert_eq!(shaping::width_font_lookups(), before);
        assert_eq!(p.layout_runs().map(|r| r.glyphs.len()).sum::<usize>(), 0);
    }
}

/// LLP 1053: `direction: rtl` is the paragraph's base direction, so a Latin
/// word that starts it sits at the right; `text-align: start` (the initial
/// value) is then the right edge, and `end` the left.
#[test]
fn rtl_direction_sets_the_base_direction_and_start_alignment() {
    let mut engine = compact_fixture_engine();
    let x_of = |p: &Paragraph, byte: u32| {
        p.layout_runs()
            .flat_map(|r| r.glyphs.iter())
            .find(|g| g.start == byte)
            .map(|g| g.x)
            .unwrap()
    };
    let text = "abc \u{5d0}\u{5d1}\u{5d2}";
    let hebrew = "abc ".len() as u32;
    let mut s = crate::paint::text_spec(&exact_kernel::StyleProps::default(), text);
    let ltr = engine.paragraph(&s, Some(300.));
    assert!(x_of(&ltr, 0) < x_of(&ltr, hebrew), "ltr: Latin first");
    assert!(x_of(&ltr, 0) < 1.0, "ltr start is the left edge");
    let mut style = exact_kernel::StyleProps::default();
    style
        .set_dynamic(
            exact_kernel::StyleId::Direction,
            &exact_kernel::StyleValue::Text("rtl".into()),
        )
        .unwrap();
    s = crate::paint::text_spec(&style, text);
    assert_eq!(s.align, TextAlign::Right, "start under rtl");
    let rtl = engine.paragraph(&s, Some(300.));
    assert!(
        x_of(&rtl, 0) > x_of(&rtl, hebrew),
        "rtl: Latin at the right"
    );
    let right = rtl
        .layout_runs()
        .flat_map(|r| r.glyphs.iter())
        .map(|g| g.x + g.w)
        .fold(0.0f32, f32::max);
    assert!(
        (right - 300.).abs() < 0.5,
        "rtl start is the right edge: {right}"
    );
    style
        .set_dynamic(
            exact_kernel::StyleId::TextAlign,
            &exact_kernel::StyleValue::Text("end".into()),
        )
        .unwrap();
    assert_eq!(crate::paint::text_spec(&style, text).align, TextAlign::Left);
}

/// Under `ltr` the first strong character still sets the bidi base (LLP 1001
/// §1), but the line box keeps the CSS direction: a wrapped line's trailing
/// space hangs at its right end, where Chrome puts it, not at the left before
/// the visible text (LLP 1085.000 host parity, `bidi-ltr-starts-hebrew`).
#[test]
fn ltr_text_that_starts_rtl_hangs_its_trailing_spaces_at_the_right() {
    let mut engine = compact_fixture_engine();
    let text = "\u{5d0}\u{5d1}\u{5d2} \u{5d3}\u{5d4}\u{5d5} and then English words follow here";
    let s = crate::paint::text_spec(&exact_kernel::StyleProps::default(), text);
    let p = engine.paragraph(&s, Some(120.));
    let runs: Vec<_> = p.layout_runs().collect();
    assert!(runs.len() > 2, "wraps");
    for (i, run) in runs.iter().enumerate() {
        let last = run.glyphs.iter().max_by_key(|g| g.start).unwrap();
        let space = &run.text[last.start as usize..last.end as usize] == " ";
        let rightmost = run.glyphs.iter().all(|g| g.x <= last.x);
        let left = run.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        if i + 1 < runs.len() {
            assert!(space && rightmost, "line {i}: trailing space at the right");
        }
        assert!(
            left.abs() < 0.01,
            "line {i} starts at the left edge: {left}"
        );
    }
}
