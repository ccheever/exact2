//! Old full Buffer route is an independent geometry/selection oracle.
use super::*;
use cosmic_text::{Cursor, LayoutRun};

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
    let align = match spec.align {
        TextAlign::Left => None,
        TextAlign::Center => Some(Align::Center),
        TextAlign::Right => Some(Align::Right),
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
