//! Old full Buffer route is an independent geometry/selection oracle.
use super::*;
use cosmic_text::{Cursor, LayoutRun};
use std::mem::size_of;

struct Legacy {
    buffer: Buffer,
    width: f32,
    height: f32,
    baselines: Vec<f32>,
}
fn legacy(catalog: &mut catalog::Catalog, spec: &Spec, width: Option<f32>) -> Legacy {
    let minimum = catalog.line_height(&spec.strut);
    let (ascent, descent, leading) = catalog.font_metrics(&spec.strut);
    let half = (minimum - ascent - descent - leading) / 2.0;
    let strut = (ascent + half, descent + leading + half);
    let line_heights: Vec<f32> = spec.runs.iter().map(|r| catalog.line_height(r)).collect();
    let run_metrics: Vec<_> = spec.runs.iter().map(|r| catalog.font_metrics(r)).collect();
    let base = spec.runs.first().map_or(16.0, |r| r.size.max(0.5));
    let tallest = line_heights.iter().copied().fold(minimum, f32::max);
    let mut buffer = Buffer::new(
        &mut catalog.fonts,
        Metrics::new(base, tallest.max(f32::EPSILON)),
    );
    // CSS `overflow-wrap: normal`: lines break between words; a word
    // longer than the line overflows it, never breaks.
    buffer.set_wrap(
        if spec.overflow_wrap == exact_kernel::OverflowWrap::Normal {
            Wrap::Word
        } else {
            Wrap::WordOrGlyph
        },
    );
    buffer.set_size(width.map(|w| w.max(0.0)), None);
    if spec.line_clamp > 0 {
        buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(
            spec.line_clamp as usize,
        )));
    }
    let align = match spec.align.physical(spec.direction) {
        // Physical: cosmic-text's `None` would align by each line's own bidi
        // direction, not the paragraph's CSS `direction` (LLP 1053).
        TextAlign::Left | TextAlign::Start => Some(Align::Left),
        TextAlign::Center => Some(Align::Center),
        TextAlign::Right | TextAlign::End => Some(Align::Right),
        TextAlign::Justify => Some(Align::Justified),
    };
    let weights: Vec<u16> = spec
        .runs
        .iter()
        .map(|r| catalog.snap_weight(r.family, r.weight, r.italic))
        .collect();
    let families: Vec<FamilyChoice> = spec
        .runs
        .iter()
        .map(|r| {
            catalog
                .families
                .get(r.family as usize)
                .cloned()
                .unwrap_or(FamilyChoice::SansSerif)
        })
        .collect();
    let spans: Vec<(&str, Attrs<'_>)> = spec
        .runs
        .iter()
        .zip(line_heights.iter())
        .zip(weights.iter())
        .zip(families.iter())
        .enumerate()
        .map(|(index, (((r, lh), w), family))| {
            (
                r.text.as_str(),
                // cosmic-text's scrolling loop requires positive pitch.
                // This shaping-only pitch never escapes: CSS placement below
                // uses the original lengths, including zero, by run metadata.
                catalog::Catalog::attrs(r, *w, family.cosmic())
                    .metadata(index)
                    .metrics(Metrics::new(r.size.max(0.5), lh.max(1.0))),
            )
        })
        .collect();
    let default = spec
        .runs
        .first()
        .zip(weights.first())
        .zip(families.first())
        .map(|((r, w), family)| catalog::Catalog::attrs(r, *w, family.cosmic()))
        .unwrap_or_else(Attrs::new);
    buffer.set_rich_text(spans, &default, Shaping::Advanced, align);
    buffer.shape_until_scroll(&mut catalog.fonts, false);
    let mut w = 0.0f32;
    let mut h = 0.0f32;
    let mut baselines = Vec::new();
    let mut explicit = false;
    for run in buffer.layout_runs() {
        w = w.max(run.line_w);
        let (mut above, mut below) = strut;
        let mut above_explicit = spec.strut.line_height.is_some();
        let mut below_explicit = above_explicit;
        for glyph in run.glyphs {
            if let Some(font) = catalog.fonts.get_font(glyph.font_id, glyph.font_weight) {
                let m = font.metrics();
                let scale = glyph.font_size / m.units_per_em as f32;
                // Explicit lengths size the authored inline box; only
                // normal expands to the actual fallback glyph font.
                let (ascent, descent, leading) = if spec.runs[glyph.metadata].line_height.is_some()
                {
                    run_metrics[glyph.metadata]
                } else {
                    (m.ascent * scale, m.descent.abs() * scale, m.leading * scale)
                };
                let height = spec.runs[glyph.metadata]
                    .line_height
                    .unwrap_or(ascent + descent + leading);
                let half = (height - ascent - descent - leading) / 2.0;
                let run_explicit = spec.runs[glyph.metadata].line_height.is_some();
                let (a, b) = (ascent + half, descent + leading + half);
                if a > above {
                    above = a;
                    above_explicit = run_explicit;
                } else if a == above {
                    above_explicit &= run_explicit;
                }
                if b > below {
                    below = b;
                    below_explicit = run_explicit;
                } else if b == below {
                    below_explicit &= run_explicit;
                }
            }
        }
        explicit |= above_explicit || below_explicit;
        baselines.push(h + above);
        h += above + below;
    }
    Legacy {
        buffer,
        width: w.ceil(),
        height: if explicit { h } else { h.ceil() },
        baselines,
    }
}

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

// Compare every glyph's float bits, including signed zero, and all source/font data.
fn glyph(g: &LayoutGlyph) -> String {
    format!(
        "{:?}:{:?}",
        (
            g.start,
            g.end,
            g.font_id,
            g.font_weight,
            g.glyph_id,
            g.level,
            g.color_opt,
            g.metadata,
            g.cache_key_flags
        ),
        [g.font_size, g.x, g.y, g.w, g.x_offset, g.y_offset].map(f32::to_bits)
    ) + &format!("{:?}", g.line_height_opt.map(f32::to_bits))
}
fn signature(run: LayoutRun<'_>) -> String {
    let mut s = format!(
        "{}:{:?}:{}:{:?}:{:?}",
        run.line_i,
        run.text,
        run.rtl,
        [run.line_y, run.line_top, run.line_height, run.line_w].map(f32::to_bits),
        run.decorations
    );
    for g in run.glyphs {
        s.push_str(&glyph(g));
    }
    // Actual cosmic selection helpers over every UTF8 source boundary, not a new hit tester.
    for offset in run
        .text
        .char_indices()
        .map(|(i, _)| i)
        .chain([run.text.len()])
    {
        let cursor = Cursor::new(run.line_i, offset);
        s.push_str(&format!(
            "{:?}:{:?}",
            run.cursor_position(&cursor),
            run.cursor_glyph(&cursor)
        ));
    }
    let highlights: Vec<_> = run
        .highlight(
            Cursor::new(run.line_i, 0),
            Cursor::new(run.line_i, run.text.len()),
        )
        .map(|(x, w)| (x.to_bits(), w.to_bits()))
        .collect();
    s + &format!("{highlights:?}")
}

#[test]
fn shared_layout_matches_full_buffer_geometry_source_and_selection() {
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
                let expected = legacy(&mut engine.catalog.borrow_mut(), &s, width);
                assert_eq!(
                    p.layout_runs().map(signature).collect::<Vec<_>>(),
                    expected
                        .buffer
                        .layout_runs()
                        .map(signature)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    [p.width, p.height].map(f32::to_bits),
                    [expected.width, expected.height].map(f32::to_bits)
                );
                assert_eq!(
                    p.baselines.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    expected
                        .baselines
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    p.first_baseline.to_bits(),
                    expected.baselines.first().copied().unwrap_or(0.).to_bits()
                );
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
    let original = a.layout_runs().map(signature).collect::<Vec<_>>();
    for width in [121., 300., 80., 121.] {
        let b = engine.paragraph(&s, Some(width));
        assert!(Rc::ptr_eq(&a.source, &b.source));
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(shaping::shape_line_calls(), calls);
    }
    engine.measure(&s, AxisOffer::MinContent);
    engine.measure(&s, AxisOffer::MaxContent);
    assert_eq!(shaping::shape_line_calls(), calls);
    assert_eq!(a.layout_runs().map(signature).collect::<Vec<_>>(), original);
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
    // Exercise the public install boundary, then make the CURRENT catalog empty:
    // old font ids cannot accidentally succeed against a coincidentally equal DB.
    let plan = caltrain::compile().unwrap();
    engine.install_plan(&plan, Path::new("/nonexistent"));
    engine.catalog.borrow_mut().fonts =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
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

#[derive(Debug)]
struct GlyphStorage {
    glyphs: usize,
    glyph_capacity: usize,
    lines: usize,
    line_capacity: usize,
    largest_line_bytes: usize,
}
fn glyph_storage(p: &Paragraph) -> GlyphStorage {
    let lines = p.layouts.iter().flatten();
    GlyphStorage {
        glyphs: lines.clone().map(|l| l.glyphs.len()).sum(),
        glyph_capacity: lines.clone().map(|l| l.glyphs.capacity()).sum(),
        lines: lines.clone().count(),
        line_capacity: p.layouts.iter().map(Vec::capacity).sum(),
        largest_line_bytes: lines
            .map(|l| l.glyphs.len() * size_of::<LayoutGlyph>())
            .max()
            .unwrap_or(0),
    }
}
pub(super) fn assert_tight_glyph_storage(p: &Paragraph) {
    let actual = glyph_storage(p);
    assert_eq!(actual.glyph_capacity, actual.glyphs, "{actual:?}");
}
fn compact_fixture_engine() -> TextEngine {
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(
        include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").to_vec(),
    )));
    db.set_sans_serif_family("DejaVu Sans");
    TextEngine::with_catalog(catalog::Catalog::with_fonts(
        FontSystem::new_with_locale_and_db("en-US".into(), db),
    ))
}
fn uncompacted_oracle(engine: &mut TextEngine, p: &Paragraph, width: Option<f32>) -> Paragraph {
    // The existing independent Buffer route stays untouched. Copy its numeric
    // output only in this test so the ordinary CPU/GPU consumers can paint it.
    let old = legacy(&mut engine.catalog.borrow_mut(), &p.source.spec, width);
    let mut oracle = Paragraph {
        flow: None,
        source: p.source.clone(),
        layouts: Arc::new(
            old.buffer
                .lines
                .iter()
                .map(|l| {
                    let Some(lines) = l.layout_opt() else {
                        return Vec::new();
                    };
                    let mut copy = Vec::with_capacity(lines.capacity());
                    copy.extend(lines.iter().map(|line| {
                        // Vec::clone tightens capacity itself, which would hide
                        // the behavioral RED. Preserve the reference capacities.
                        let mut value = line.clone();
                        let mut glyphs = Vec::with_capacity(line.glyphs.capacity());
                        glyphs.extend(line.glyphs.iter().cloned());
                        value.glyphs = glyphs;
                        value
                    }));
                    copy
                })
                .collect(),
        ),
        layout_lifetime: Arc::new(()),
        width: old.width,
        height: old.height,
        first_baseline: old.baselines.first().copied().unwrap_or(0.),
        baselines: Arc::new(old.baselines),
        ink: RefCell::new(ink::Cache::default()),
        resident_capacity_bytes: 0,
        private_text_bytes_estimate: 0,
    };
    oracle.resident_capacity_bytes = cache::capacities(&oracle);
    oracle
}
fn assert_compacted_parity(engine: &mut TextEngine, actual: &Paragraph, old: &Paragraph) {
    assert_eq!(
        [actual.width, actual.height, actual.first_baseline].map(f32::to_bits),
        [old.width, old.height, old.first_baseline].map(f32::to_bits)
    );
    assert_eq!(
        actual
            .baselines
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        old.baselines
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(actual.layout_runs().count(), old.layout_runs().count());
    let mut previous_source = None;
    for (a, b) in actual.layout_runs().zip(old.layout_runs()) {
        assert_eq!((a.line_i, a.rtl), (b.line_i, b.rtl));
        if previous_source != Some(a.line_i) {
            assert_eq!(a.text, b.text);
            previous_source = Some(a.line_i);
        }
        assert_eq!(
            [a.line_y, a.line_top, a.line_height, a.line_w].map(f32::to_bits),
            [b.line_y, b.line_top, b.line_height, b.line_w].map(f32::to_bits)
        );
        assert_eq!(
            format!("{:?}", a.decorations),
            format!("{:?}", b.decorations)
        );
        assert_eq!(a.glyphs.len(), b.glyphs.len());
        for (a, b) in a.glyphs.iter().zip(b.glyphs) {
            assert_eq!(glyph(a), glyph(b));
        }
        // Full selection span for every wrapped line; representative cursor
        // queries avoid quadratic work for the deliberately unwrapped giant.
        // The existing small mixed-script oracle covers every UTF-8 boundary.
        let selection = |r: &LayoutRun<'_>| {
            r.highlight(
                Cursor::new(r.line_i, 0),
                Cursor::new(r.line_i, r.text.len()),
            )
            .map(|(x, w)| (x.to_bits(), w.to_bits()))
            .collect::<Vec<_>>()
        };
        assert_eq!(selection(&a), selection(&b));
        for g in [
            a.glyphs.first(),
            a.glyphs.get(a.glyphs.len() / 2),
            a.glyphs.last(),
        ]
        .into_iter()
        .flatten()
        {
            for offset in [g.start, g.end] {
                let cursor = Cursor::new(a.line_i, offset);
                assert_eq!(
                    format!("{:?}", a.cursor_position(&cursor)),
                    format!("{:?}", b.cursor_position(&cursor))
                );
                assert_eq!(
                    format!("{:?}", a.cursor_glyph(&cursor)),
                    format!("{:?}", b.cursor_glyph(&cursor))
                );
            }
        }
    }
    let palette: Vec<_> = actual
        .source
        .spec
        .runs
        .iter()
        .enumerate()
        .map(|(i, _)| RunPaint {
            color: if i == 0 {
                [30, 70, 160, 255]
            } else {
                [150, 30, 45, 255]
            },
            source: 7 + i as u32,
        })
        .collect();
    let paint = |engine: &mut TextEngine, p: &Paragraph, y| {
        let mut image = Pixmap::new(400, 180).unwrap();
        engine.paint(
            &mut image,
            p,
            &palette,
            (0., y),
            1.,
            Transform::identity(),
            None,
        );
        image
    };
    for y in [0., -actual.height / 2., -(actual.height - 100.).max(0.)] {
        assert_eq!(
            paint(engine, actual, y).data(),
            paint(engine, old, y).data()
        );
    }
    let a = engine.glyph_runs(actual, &palette);
    let b = engine.glyph_runs(old, &palette);
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(&b) {
        assert_eq!(a.font.index, b.font.index);
        assert_eq!(a.font.data.data(), b.font.data.data());
        assert_eq!(
            (a.size.to_bits(), a.run_index, a.paint, a.synthetic_italic),
            (b.size.to_bits(), b.run_index, b.paint, b.synthetic_italic)
        );
        assert_eq!(
            a.glyphs
                .iter()
                .map(|(g, x, y)| (*g, x.to_bits(), y.to_bits()))
                .collect::<Vec<_>>(),
            b.glyphs
                .iter()
                .map(|(g, x, y)| (*g, x.to_bits(), y.to_bits()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn glyph_capacity_compaction_reduces_wrapped_storage_with_exact_oracle() {
    let mut engine = compact_fixture_engine();
    let mut s = spec();
    s.runs[0].text = format!(
        "{}\n{}",
        "office ffi café e\u{301} words 12345 ".repeat(2500),
        s.runs[0].text
    );
    assert!(s.runs[0].text.len() >= 65536);
    for width in [600., 984.] {
        let p = engine.paragraph(&s, Some(width));
        let old = uncompacted_oracle(&mut engine, &p, Some(width));
        let a = glyph_storage(&p);
        let b = glyph_storage(&old);
        eprintln!(
            "wrapped width={width}: LayoutGlyph={} LayoutLine={} actual={a:?} oracle={b:?}",
            size_of::<LayoutGlyph>(),
            size_of::<cosmic_text::LayoutLine>()
        );
        assert_compacted_parity(&mut engine, &p, &old);
        assert_eq!((a.glyphs, a.lines), (b.glyphs, b.lines));
        assert!(
            (b.glyph_capacity - b.glyphs) * size_of::<LayoutGlyph>() >= 65536,
            "fixture must actually contain substantial spare capacity: {b:?}"
        );
        assert_tight_glyph_storage(&p);
        assert!(
            a.glyph_capacity < b.glyph_capacity,
            "retained capacity did not fall"
        );
    }
}

#[test]
fn glyph_capacity_compaction_unwrapped_line_exposes_large_transient() {
    let mut engine = compact_fixture_engine();
    let s = crate::paint::text_spec(
        &exact_kernel::StyleProps::default(),
        &"office ffi café words 12345 ".repeat(2500),
    );
    assert!(s.runs[0].text.len() >= 65536);
    let p = engine.paragraph(&s, None);
    let old = uncompacted_oracle(&mut engine, &p, None);
    let a = glyph_storage(&p);
    let b = glyph_storage(&old);
    assert_eq!(a.lines, 1);
    assert!(a.largest_line_bytes > 65536);
    eprintln!("unwrapped LayoutGlyph={} LayoutLine={} actual={a:?} oracle={b:?}; a moving shrink may temporarily retain old capacity plus {} bytes, not viewport-bounded scratch",
        size_of::<LayoutGlyph>(), size_of::<cosmic_text::LayoutLine>(), a.largest_line_bytes);
    assert_compacted_parity(&mut engine, &p, &old);
    assert_tight_glyph_storage(&p);
    assert!(a.glyph_capacity < b.glyph_capacity);
}

#[test]
fn glyph_capacity_compaction_keeps_ordinary_small_storage_unchanged() {
    let mut engine = compact_fixture_engine();
    let s = spec();
    let p = engine.paragraph(&s, Some(200.));
    let old = uncompacted_oracle(&mut engine, &p, Some(200.));
    let a = glyph_storage(&p);
    let b = glyph_storage(&old);
    assert!((b.glyph_capacity - b.glyphs) * size_of::<LayoutGlyph>() < 65536);
    // The existing Buffer route starts its outer line vector at capacity 1;
    // ShapedSource starts empty (ordinary baseline was 9 versus 24 slots).
    // This change only compacts glyph vectors, whose capacities agree here.
    assert_eq!(a.glyph_capacity, b.glyph_capacity);
    assert_compacted_parity(&mut engine, &p, &old);
    eprintln!("ordinary unchanged glyph capacity: {a:?}; line-header bytes={}; this is not a timing measurement",
        a.line_capacity * size_of::<cosmic_text::LayoutLine>());
}

// Local width-pass lookup reuse. The reference method is the pinned original
// production method, not a second implementation of the candidate slot.
mod last_font_metrics {
    use super::*;

    fn engine() -> TextEngine {
        let mut db = fontdb::Database::new();
        for bytes in [
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice(),
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans-Bold.ttf")
                .as_slice(),
            include_bytes!("../../../../vendor/cosmic-text/fonts/Inter-Regular.ttf").as_slice(),
            include_bytes!("../../../../vendor/cosmic-text/fonts/NotoSansArabic.ttf").as_slice(),
        ] {
            db.load_font_source(fontdb::Source::Binary(Arc::new(bytes.to_vec())));
        }
        db.set_sans_serif_family("DejaVu Sans");
        TextEngine::with_catalog(catalog::Catalog::with_fonts(
            FontSystem::new_with_locale_and_db("en-US".into(), db),
        ))
    }

    fn source(engine: &TextEngine, spec: Spec) -> Rc<ShapedSource> {
        Rc::new(ShapedSource::new(engine.catalog.clone(), Arc::new(spec)))
    }

    fn transitions(p: &Paragraph) -> (usize, usize) {
        let mut last = None;
        let mut calls = 0;
        let mut glyphs = 0;
        for g in p.layout_runs().flat_map(|r| r.glyphs) {
            let key = (g.font_id, g.font_weight);
            if last != Some(key) {
                calls += 1;
                last = Some(key);
            }
            glyphs += 1;
        }
        (calls, glyphs)
    }

    fn pixels(engine: &mut TextEngine, p: &Paragraph, scale: f32, y: f32) -> Vec<u8> {
        let palette: Vec<_> = p
            .source
            .spec
            .runs
            .iter()
            .enumerate()
            .map(|(i, _)| RunPaint {
                color: [30 + i as u8, 70, 160, 231],
                source: i as u32,
            })
            .collect();
        let mut image = Pixmap::new(320, 180).unwrap();
        let mut mask = Mask::new(320, 180).unwrap();
        for row in 7..165 {
            mask.data_mut()[row * 320 + 11..row * 320 + 297].fill(255);
        }
        engine.paint_clipped(
            &mut image,
            p,
            &palette,
            (7.375, y),
            scale,
            Transform::from_scale(scale, scale),
            Some(&mask),
            (11., 7., 286., 158.),
        );
        image.data().to_vec()
    }

    fn parity(engine: &mut TextEngine, p: &Paragraph, old: &Paragraph) {
        assert_eq!(
            [p.width, p.height, p.first_baseline].map(f32::to_bits),
            [old.width, old.height, old.first_baseline].map(f32::to_bits),
        );
        assert_eq!(
            p.baselines.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            old.baselines
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            p.layout_runs().map(signature).collect::<Vec<_>>(),
            old.layout_runs().map(signature).collect::<Vec<_>>()
        );
        for (scale, y) in [(1., 0.625), (1.25, -13.375), (2., -31.125)] {
            // Both cold and subsequently warmed raster/index paths compare to
            // the unchanged loop's independently placed Paragraph.
            for _ in 0..2 {
                assert_eq!(pixels(engine, p, scale, y), pixels(engine, old, scale, y));
            }
        }
    }

    #[test]
    fn repetitive_width_pass_counts_actual_font_lookups() {
        let mut engine = engine();
        let s = crate::paint::text_spec(
            &exact_kernel::StyleProps::default(),
            &"office café words 1234 ".repeat(24),
        );
        let source = source(&engine, s);
        for width in [193.25, 241.75, 193.25] {
            let before = shaping::width_font_lookups();
            let p = source.layout(Some(width), None);
            let actual = shaping::width_font_lookups() - before;
            let (expected, glyphs) = transitions(&p);
            assert!(glyphs > 100 && expected < glyphs / 2);
            let old = source.layout_reference(Some(width), None);
            parity(&mut engine, &p, &old);
            assert!(pixels(&mut engine, &p, 1., 0.625)
                .chunks_exact(4)
                .any(|p| p[3] != 0));
            eprintln!(
                "width={width} glyphs={glyphs} transitions={expected} actual_get_font={actual}"
            );
            assert_eq!(actual, expected, "per-glyph font lookup was not eliminated");
        }
    }

    #[test]
    fn fallback_weights_sizes_and_metadata_keep_exact_results() {
        let mut engine = engine();
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
            // Inter fixture is regular-only: use the already loaded bold face
            // for the weight control; keep the first run's actual fallback.
            if weight == 700 {
                r.family = 1;
            }
            r.size = size;
            r.line_height = height;
            s.runs.push(r);
        }
        let source = source(&engine, s);
        let before = shaping::width_font_lookups();
        let p = source.layout(Some(211.125), None);
        let calls = shaping::width_font_lookups() - before;
        let keys: Vec<_> = p
            .layout_runs()
            .flat_map(|r| r.glyphs)
            .map(|g| (g.font_id, g.font_weight, g.font_size.to_bits(), g.metadata))
            .collect();
        assert!(keys.iter().any(|k| k.1 == Weight(700)));
        assert!(keys.iter().any(|k| k.1 == Weight(400)));
        assert!(keys.iter().any(|k| k.2 == 12.5f32.to_bits()));
        assert!(keys.iter().any(|k| k.2 == 23.5f32.to_bits()));
        assert!(keys
            .iter()
            .all(|k| engine.catalog.borrow().fonts.db().face(k.0).is_some()));
        let first_run_fonts: std::collections::HashSet<_> =
            keys.iter().filter(|k| k.3 == 0).map(|k| k.0).collect();
        assert!(
            first_run_fonts.len() >= 2,
            "Latin/Arabic must exercise actual fallback faces"
        );
        let old = source.layout_reference(Some(211.125), None);
        parity(&mut engine, &p, &old);
        assert_eq!(calls, transitions(&p).0);
    }

    #[test]
    fn explicit_normal_equal_ties_and_zero_height_keep_bits() {
        let mut engine = engine();
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
                let p = source.layout(Some(270.5), None);
                let old = source.layout_reference(Some(270.5), None);
                parity(&mut engine, &p, &old);
                if height == Some(normal) {
                    // This fixture's strut and normal run use the SAME font and
                    // size, so equal ascent/descent ties must preserve the old
                    // explicit &= normal decision, including final ceil.
                    assert_eq!(p.height.to_bits(), normal.ceil().to_bits());
                }
            }
        }
    }

    fn raw_bits(m: shaping::RawFontMetrics) -> [u32; 4] {
        [
            u32::from(m.units_per_em),
            m.ascent.to_bits(),
            m.descent.to_bits(),
            m.leading.to_bits(),
        ]
    }

    #[test]
    fn slot_keys_include_weight_and_missing_none_is_a_hit() {
        let engine = engine();
        let mut catalog = engine.catalog.borrow_mut();
        let id = catalog.fonts.db().faces().next().unwrap().id;
        let missing = fontdb::ID::dummy();
        let keys = [
            (missing, Weight(400)),
            (missing, Weight(400)),
            (id, Weight(400)),
            (id, Weight(400)),
            (id, Weight(700)),
            (id, Weight(700)),
            (id, Weight(400)),
            (missing, Weight(400)),
        ];
        let expected: Vec<_> = keys
            .iter()
            .map(|&(id, weight)| {
                catalog.fonts.get_font(id, weight).map(|font| {
                    let m = font.metrics();
                    [
                        u32::from(m.units_per_em),
                        m.ascent.to_bits(),
                        m.descent.to_bits(),
                        m.leading.to_bits(),
                    ]
                })
            })
            .collect();
        assert!(expected[0].is_none() && expected[2].is_some() && expected[4].is_some());
        let mut slot = shaping::LastFontMetrics::default();
        let before = shaping::width_font_lookups();
        let actual: Vec<_> = keys
            .iter()
            .map(|&(id, weight)| slot.get(&mut catalog.fonts, id, weight).map(raw_bits))
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(shaping::width_font_lookups() - before, 5);
    }

    #[test]
    fn copied_slot_does_not_retain_font_arc_or_catalog_owner() {
        let engine = engine();
        let mut catalog = engine.catalog.borrow_mut();
        let id = catalog.fonts.db().faces().next().unwrap().id;
        let font = catalog.fonts.get_font(id, Weight(400)).unwrap();
        let weak = Arc::downgrade(&font);
        let count = Arc::strong_count(&font);
        let mut slot = shaping::LastFontMetrics::default();
        assert!(slot.get(&mut catalog.fonts, id, Weight(400)).is_some());
        assert_eq!(Arc::strong_count(&font), count);
        drop(font);
        drop(catalog);
        drop(engine);
        assert!(
            weak.upgrade().is_none(),
            "scalar slot retained a font owner"
        );
        // Keep the slot live past the final owner assertion.
        std::hint::black_box(slot);
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
        let p = source.layout(Some(160.25), None);
        let expected = pixels(&mut engine, &p, 1.25, 0.625);
        engine.catalog = Rc::new(RefCell::new(catalog::Catalog::with_fonts(
            FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new()),
        )));
        assert_eq!(pixels(&mut engine, &p, 1.25, 0.625), expected);
        let before = shaping::width_font_lookups();
        let b = source.layout(Some(201.75), None);
        let calls = shaping::width_font_lookups() - before;
        assert!(calls > 0, "slot must not outlive one width layout");
        let old = source.layout_reference(Some(201.75), None);
        parity(&mut engine, &b, &old);
        assert_eq!(calls, transitions(&b).0);
        drop((p, b, old, source));
        assert!(old_catalog.upgrade().is_none());
    }

    #[test]
    fn empty_paragraph_has_no_width_font_lookup() {
        let mut engine = engine();
        let s = crate::paint::text_spec(&exact_kernel::StyleProps::default(), "");
        let source = source(&engine, s);
        let before = shaping::width_font_lookups();
        let p = source.layout(Some(0.), None);
        assert_eq!(shaping::width_font_lookups(), before);
        assert_eq!(transitions(&p), (0, 0));
        let old = source.layout_reference(Some(0.), None);
        parity(&mut engine, &p, &old);
    }
}

/// LLP 1053: `direction: rtl` is the paragraph's base direction, so a Latin
/// word that starts it sits at the right; `text-align: start` (the initial
/// value) is then the right edge, and `end` the left.
#[test]
fn rtl_direction_sets_the_base_direction_and_start_alignment() {
    let mut engine = compact_fixture_engine();
    let x_of = |p: &Paragraph, byte: usize| {
        p.layout_runs()
            .flat_map(|r| r.glyphs.iter())
            .find(|g| g.start == byte)
            .map(|g| g.x)
            .unwrap()
    };
    let text = "abc \u{5d0}\u{5d1}\u{5d2}";
    let hebrew = "abc ".len();
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
