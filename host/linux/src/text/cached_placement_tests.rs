use super::*;
use cosmic_text::{CacheKey, SubpixelBin};
use std::collections::{BTreeMap, BTreeSet};

const BINS: [SubpixelBin; 4] = [
    SubpixelBin::Zero,
    SubpixelBin::One,
    SubpixelBin::Two,
    SubpixelBin::Three,
];
#[derive(Debug, PartialEq, Eq)]
struct ImageCard {
    placement: (i32, i32, u32, u32),
    content: u8,
    data: Vec<u8>,
    pointer: usize,
    capacity: usize,
}
fn cache_card(p: &Paragraph) -> BTreeMap<CacheKey, Option<ImageCard>> {
    p.source
        .catalog
        .borrow()
        .swash
        .image_cache
        .iter()
        .map(|(key, image)| {
            (
                *key,
                image.as_ref().map(|image| ImageCard {
                    placement: (
                        image.placement.left,
                        image.placement.top,
                        image.placement.width,
                        image.placement.height,
                    ),
                    content: match image.content {
                        SwashContent::Mask => 0,
                        SwashContent::SubpixelMask => 1,
                        SwashContent::Color => 2,
                    },
                    data: image.data.clone(),
                    pointer: image.data.as_ptr() as usize,
                    capacity: image.data.capacity(),
                }),
            )
        })
        .collect()
}
fn keys(p: &Paragraph, scale: f32) -> BTreeSet<CacheKey> {
    p.layout_runs()
        .flat_map(|line| {
            line.glyphs.iter().flat_map(move |glyph| {
                let mut key = glyph.physical((0.0, 0.0), scale).cache_key;
                key.y_bin = SubpixelBin::Zero;
                BINS.map(|bin| {
                    let mut phase = key;
                    phase.x_bin = bin;
                    phase
                })
            })
        })
        .collect()
}
fn warm(p: &Paragraph, keys: impl IntoIterator<Item = CacheKey>) {
    let mut borrow = p.source.catalog.borrow_mut();
    let c = &mut *borrow;
    for key in keys {
        // Only test preparation populates the existing ordinary cache.
        let _ = c.swash.get_image(&mut c.fonts, key);
    }
}
fn delta(before: ink::BuildWork) -> ink::BuildWork {
    let after = ink::build_work();
    ink::BuildWork {
        attempts: after.attempts - before.attempts,
        glyphs: after.glyphs - before.glyphs,
        raster_phases: after.raster_phases - before.raster_phases,
        uncached_calls: after.uncached_calls - before.uncached_calls,
        cached_placements: after.cached_placements - before.cached_placements,
    }
}
fn checked_index(p: &Paragraph, scale: f32) -> (ink::Index, ink::BuildWork) {
    let before = cache_card(p);
    let work = ink::build_work();
    let index = ink::Index::build(&mut p.source.catalog.borrow_mut(), p, scale).unwrap();
    let work = delta(work);
    assert_eq!(
        cache_card(p),
        before,
        "index mutated cached keys/payload/owners"
    );
    // This is the copied pre-change formula, not the candidate lookup helper.
    let oracle =
        ink::Index::uncached_oracle(&mut p.source.catalog.borrow_mut(), p, scale, ink::MAX_BYTES)
            .unwrap();
    index.assert_same_numeric(&oracle);
    assert_eq!(cache_card(p), before, "uncached oracle changed image cache");
    for origin in [
        (7.375, -0.625),
        (-9.125, -37.875),
        (0.125, -p.height + 60.25),
    ] {
        for ts in [
            Transform::identity(),
            Transform::from_rotate(7.0),
            Transform::from_row(-1.0, 0.125, -0.25, 1.0, 260.25, -5.125),
        ] {
            for clip in [(0., 0., 320., 128.), (10.125, 20.25, 210.5, 9.25)] {
                let mut actual = Vec::new();
                let mut expected = Vec::new();
                let a = index.viewport(origin, scale, ts, clip).unwrap();
                let b = oracle.viewport(origin, scale, ts, clip).unwrap();
                let visits = index.visit(a, |line| actual.push(line));
                assert_eq!(visits, oracle.visit(b, |line| expected.push(line)));
                assert_eq!(actual, expected, "query/order differs");
            }
        }
    }
    (index, work)
}
fn pixels(engine: &mut TextEngine, p: &Paragraph, index: ink::Index, scale: f32) {
    {
        let token = p.source.catalog.borrow().ink_catalog.clone();
        let mut cache = p.ink.borrow_mut();
        cache.reset(&token, scale);
        cache.index = Some(index.into());
    }
    let mut mask = Mask::new(320, 128).unwrap();
    let path = PathBuilder::from_rect(Rect::from_xywh(10.125, 7.25, 275.5, 110.5).unwrap());
    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
    for y in [-0.625, -37.875, -p.height + 70.125] {
        let mut view = View::at(y, scale);
        compare(engine, p, &palette(), view, Some(&mask));
        view.transform = view.transform.pre_rotate(-7.0);
        compare(engine, p, &palette(), view, Some(&mask));
    }
}

#[test]
fn messages_32_body_revisions_reuse_missing_phase_envelopes() {
    use super::messages_envelope_model::{history, Controls};

    const COUNT: usize = 10_000;
    const BATCH: usize = 32;
    let rows_a = history(Controls::new(COUNT, 1, BATCH).unwrap(), "").unwrap();
    let rows_b = history(Controls::new(COUNT, 2, BATCH).unwrap(), "").unwrap();
    assert_eq!((rows_a.len(), rows_b.len()), (COUNT, COUNT));
    assert_eq!(rows_a[..COUNT - BATCH], rows_b[..COUNT - BATCH]);
    for (a, b) in rows_a[COUNT - BATCH..].iter().zip(&rows_b[COUNT - BATCH..]) {
        assert_eq!(a.id, b.id);
        assert_ne!(a.body, b.body);
    }
    let mut engine = engine();
    let mut make = |rows: &[super::messages_envelope_model::Row]| {
        rows[COUNT - BATCH..]
            .iter()
            .map(|row| {
                let mut s = spec(&row.body);
                s.strut.size = 14.0;
                s.strut.line_height = Some(14.0 * 1.45);
                s.runs[0].size = 14.0;
                s.runs[0].line_height = s.strut.line_height;
                s.white_space = exact_kernel::WhiteSpace::PreWrap;
                engine.paragraph(&s, Some(280.0))
            })
            .collect::<Vec<_>>()
    };
    let a = make(&rows_a);
    let b = make(&rows_b);
    assert_eq!((a.len(), b.len()), (BATCH, BATCH));
    for (a, b) in a.iter().zip(&b) {
        assert!(!Rc::ptr_eq(&a.source, &b.source));
        assert!(Rc::ptr_eq(&a.source.catalog, &b.source.catalog));
    }
    // Keep the real paragraph cache: equal bodies may share one paragraph.
    // Thirty-two rows must not be turned into 32 artificial index builds.
    let distinct_b = b.iter().map(Rc::as_ptr).collect::<BTreeSet<_>>().len();
    let keys_a = a.iter().flat_map(|p| keys(p, 1.0)).collect::<BTreeSet<_>>();
    let keys_b = b.iter().flat_map(|p| keys(p, 1.0)).collect::<BTreeSet<_>>();
    let canonical = keys_a
        .union(&keys_b)
        .map(|key| {
            let mut key = *key;
            key.x_bin = SubpixelBin::Zero;
            key
        })
        .collect::<BTreeSet<_>>();
    assert!(
        canonical.len() <= 256,
        "fixture exceeds proposed envelope bound"
    );
    let mut mask = Mask::new(320, 128).unwrap();
    let path = PathBuilder::from_rect(Rect::from_xywh(10.125, 7.25, 275.5, 110.5).unwrap());
    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
    let views = |paragraphs: &[Rc<Paragraph>]| {
        paragraphs
            .iter()
            .enumerate()
            .map(|(i, p)| View::at([-0.625, -37.875, -p.height + 70.125][i % 3], 1.0))
            .collect::<Vec<_>>()
    };
    let views_a = views(&a);
    let views_b = views(&b);
    let metrics = |paragraphs: &[Rc<Paragraph>]| {
        paragraphs
            .iter()
            .map(|p| {
                (
                    p.width.to_bits(),
                    p.height.to_bits(),
                    p.first_baseline.to_bits(),
                    p.baselines.iter().map(|n| n.to_bits()).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    let before_metrics = (metrics(&a), metrics(&b));
    let paint_batch = |engine: &mut TextEngine, paragraphs: &[Rc<Paragraph>], views: &[View]| {
        paragraphs
            .iter()
            .zip(views)
            .map(|(p, view)| {
                let mut pixmap = Pixmap::new(320, 128).unwrap();
                pixmap.fill(Color::WHITE);
                engine.paint(
                    &mut pixmap,
                    p,
                    &palette(),
                    view.origin,
                    view.scale,
                    view.transform,
                    Some(&mask),
                );
                pixmap
            })
            .collect::<Vec<_>>()
    };
    let pixels_a = paint_batch(&mut engine, &a, &views_a);
    let warmed = cache_card(&a[0]);
    let shared_missing = keys_b
        .intersection(&keys_a)
        .filter(|key| !warmed.contains_key(*key))
        .count();
    assert!(
        shared_missing > 0,
        "STOP: ordinary A drawing already cached every shared phase; no new target"
    );
    // Landed placement reuse already covers every key in `warmed`.
    // New work is permitted for B-only phases, never for A's numeric envelopes.
    let new_missing = keys_b
        .difference(&keys_a)
        .filter(|key| !warmed.contains_key(*key))
        .count();
    let start = ink::build_work();
    let pixels_b = paint_batch(&mut engine, &b, &views_b);
    let work = delta(start);
    assert_eq!(work.attempts, distinct_b, "actual lazy index builds only");
    assert_eq!((pixels_a.len(), pixels_b.len()), (BATCH, BATCH));

    // Only now may the independent full-glyph oracle warm additional images.
    // Every one of the 64 full RGBA buffers is checked before the work RED.
    for (paragraphs, views, captured) in [(&a, &views_a, &pixels_a), (&b, &views_b, &pixels_b)] {
        for ((p, view), actual) in paragraphs.iter().zip(views).zip(captured) {
            let expected = full(&mut engine, p, &palette(), *view, Some(&mask));
            assert_eq!(actual.data(), expected.data(), "full RGBA changed");
        }
    }
    let mut checked = BTreeSet::new();
    for p in a.iter().chain(&b) {
        if checked.insert(Rc::as_ptr(p)) {
            // Existing oracle also checks numeric bounds, fractional queries,
            // transformed selection/order and no image-cache owner mutation.
            let (index, _) = checked_index(p, 1.0);
            pixels(&mut engine, p, index, 1.0);
        }
    }
    assert_eq!((metrics(&a), metrics(&b)), before_metrics);
    assert!(
        work.uncached_calls <= new_missing,
        "shared missing phases recomputed: shared_missing={shared_missing}, new_missing={new_missing}, actual_uncached={}",
        work.uncached_calls
    );
}

#[test]
fn numeric_envelopes_bound_generation_and_clear_on_cap() {
    let mut engine = engine();
    let p = engine.layout(&spec("f"), Some(100.0));
    let (first, cold) = checked_index(&p, 1.0);
    assert_eq!(cold.uncached_calls, 4);
    let (again, hot) = checked_index(&p, 1.0);
    first.assert_same_numeric(&again);
    assert_eq!(hot.raster_phases, 0);
    assert_eq!(hot.uncached_calls, 0);
    let old = Rc::downgrade(&p.source.catalog.borrow().ink_catalog);
    p.source.catalog.borrow_mut().ink_catalog = Rc::new(());
    assert!(
        old.upgrade().is_none(),
        "numeric cache must not own generation"
    );
    let (fresh, reset) = checked_index(&p, 1.0);
    first.assert_same_numeric(&fresh);
    assert_eq!(reset.uncached_calls, cold.uncached_calls);
    p.source.catalog.borrow_mut().ink_catalog = Rc::new(());
    let mut first_key = None;
    for i in 0..257 {
        let mut s = spec("f");
        s.runs[0].size = 14.0 + i as f32 / 1024.0;
        let next = engine.layout(&s, Some(100.0));
        let (_, work) = checked_index(&next, 1.0);
        assert_eq!(work.uncached_calls, 4, "full font-size key at {i}");
        let c = next.source.catalog.borrow();
        let entries = &c.envelopes.entries;
        assert_eq!(entries.len(), i % 256 + 1);
        assert_eq!(entries.capacity(), 256);
        assert!(entries
            .iter()
            .all(|(k, _)| k.x_bin == SubpixelBin::Zero && k.y_bin == SubpixelBin::Zero));
        if i == 0 {
            first_key = Some(entries[0].0);
        }
        if i == 256 {
            assert!(entries.iter().all(|(k, _)| Some(*k) != first_key));
        }
    }
    let mut s = spec("f");
    s.runs[0].size = 14.0;
    let evicted = engine.layout(&s, Some(100.0));
    assert_eq!(checked_index(&evicted, 1.0).1.uncached_calls, 4);
    assert_eq!(checked_index(&evicted, 1.0).1.uncached_calls, 0);
    let c = p.source.catalog.borrow();
    eprintln!(
        "numeric envelope bytes: struct={} entry={} capacity={} payload={}",
        std::mem::size_of::<ink::Envelopes>(),
        std::mem::size_of::<(CacheKey, ink::Bounds)>(),
        c.envelopes.entries.capacity(),
        c.envelopes.entries.capacity() * std::mem::size_of::<(CacheKey, ink::Bounds)>()
    );
}

#[test]
fn warm_accepted_a_different_source_b_reuses_existing_phase_placements() {
    let mut engine = engine();
    let a = Rc::new(engine.layout(&spec(&"fj café repeat\n".repeat(8)), Some(190.0)));
    let a_weak = Rc::downgrade(&a);
    let before_a = full(&mut engine, &a, &palette(), View::at(-0.625, 1.0), None);
    let b = engine.layout(&spec(&"repeat café fj fj\n".repeat(9)), Some(190.0));
    assert!(!Rc::ptr_eq(&a.source, &b.source));
    let required = keys(&b, 1.0);
    let card = cache_card(&b);
    let present = required.iter().filter(|k| card.contains_key(k)).count();
    assert!(present > 0, "real drawing of A must warm shared B keys");
    let (index, work) = checked_index(&b, 1.0);
    assert_eq!(
        work.raster_phases,
        required.len(),
        "four phases still considered"
    );
    assert_eq!(
        work.uncached_calls,
        required.len() - present,
        "old formula redundantly rasterizes already-cached phases"
    );
    assert_eq!(work.cached_placements, present);
    pixels(&mut engine, &b, index, 1.0);
    assert_eq!(
        full(&mut engine, &a, &palette(), View::at(-0.625, 1.0), None).data(),
        before_a.data()
    );
    assert!(a_weak.upgrade().is_some());
    drop(a);
    assert!(a_weak.upgrade().is_none());
}

#[test]
fn cold_partial_full_warm_keep_exact_index_queries_pixels_and_cache() {
    let mut engine = engine();
    for scale in [0.75, 1.0, 1.25, 2.0] {
        let p = engine.layout(
            &spec(&"fj Áe\u{301} words and spaces\n".repeat(9)),
            Some(190.0),
        );
        let required = keys(&p, scale);
        assert!(!required.is_empty());
        for regime in 0..3 {
            // Each placement-count regime starts with cold numeric envelopes.
            p.source.catalog.borrow_mut().ink_catalog = Rc::new(());
            p.source.catalog.borrow_mut().swash.image_cache.clear();
            match regime {
                0 => {}
                1 => warm(
                    &p,
                    required
                        .iter()
                        .copied()
                        .filter(|k| k.x_bin == SubpixelBin::One),
                ),
                _ => warm(&p, required.iter().copied()),
            }
            let count = cache_card(&p).len();
            let (index, work) = checked_index(&p, scale);
            assert_eq!(work.raster_phases, required.len());
            assert_eq!(work.uncached_calls, required.len() - count);
            assert_eq!(work.cached_placements, count);
            pixels(&mut engine, &p, index, scale);
        }
    }
}

#[test]
fn cached_none_is_no_ink_but_absent_still_calls_uncached() {
    let mut engine = engine();
    let mut p = engine.layout(&spec("fj"), Some(100.0));
    // A real out-of-face glyph produces None through the ordinary Swash API.
    for line in Arc::get_mut(&mut p.layouts).unwrap() {
        for layout in line {
            for glyph in &mut layout.glyphs {
                glyph.glyph_id = u16::MAX;
            }
        }
    }
    let required = keys(&p, 1.0);
    p.source.catalog.borrow_mut().swash.image_cache.clear();
    let (_, absent) = checked_index(&p, 1.0);
    assert_eq!(absent.uncached_calls, required.len());
    warm(&p, required.iter().copied());
    assert!(
        cache_card(&p).values().all(Option::is_none),
        "actual missing-glyph fixture"
    );
    p.source.catalog.borrow_mut().ink_catalog = Rc::new(());
    let (index, cached) = checked_index(&p, 1.0);
    assert_eq!(cached.uncached_calls, 0);
    assert_eq!(cached.cached_placements, required.len());
    pixels(&mut engine, &p, index, 1.0);
    // Controlled cached Some with zero width must also remain no ink.
    // The real missing glyph's uncached oracle is still empty.
    for key in &required {
        let mut empty = cosmic_text::SwashImage::new();
        empty.placement.left = -23;
        empty.placement.top = 17;
        empty.placement.height = 12;
        p.source
            .catalog
            .borrow_mut()
            .swash
            .image_cache
            .insert(*key, Some(empty));
    }
    p.source.catalog.borrow_mut().ink_catalog = Rc::new(());
    let (zero, work) = checked_index(&p, 1.0);
    assert_eq!(work.uncached_calls, 0);
    assert_eq!(work.cached_placements, required.len());
    pixels(&mut engine, &p, zero, 1.0);
}

#[test]
fn size_weight_italic_and_scale_use_full_keys_without_cross_aliasing() {
    let mut engine = engine();
    let a = engine.layout(&spec("fj Café"), Some(180.0));
    warm(&a, keys(&a, 1.0));
    let initial = cache_card(&a);
    for (size, weight, italic, scale) in [
        (21.25, 400, false, 1.0),
        (16.0, 700, false, 1.0),
        (16.0, 400, true, 1.0),
        (16.0, 400, false, 1.25),
    ] {
        let mut s = spec("fj Café");
        s.runs[0].size = size;
        s.runs[0].weight = weight;
        s.runs[0].italic = italic;
        let p = engine.layout(&s, Some(180.0));
        // Keep precisely A's original entries; painting below can add ordinary ones.
        p.source
            .catalog
            .borrow_mut()
            .swash
            .image_cache
            .retain(|k, _| initial.contains_key(k));
        let required = keys(&p, scale);
        let present = required.iter().filter(|k| initial.contains_key(k)).count();
        assert!(
            present < required.len(),
            "fixture must differ in full raster key"
        );
        let (index, work) = checked_index(&p, scale);
        assert_eq!(work.uncached_calls, required.len() - present);
        assert_eq!(work.cached_placements, present);
        pixels(&mut engine, &p, index, scale);
    }
}

#[test]
fn catalog_replacement_reset_and_last_owner_preserve_old_pixels() {
    let mut engine = engine();
    let a = Rc::new(engine.layout(&spec(&"old face café\n".repeat(7)), Some(185.0)));
    let old_catalog = Rc::downgrade(&a.source.catalog);
    let expected = full(&mut engine, &a, &palette(), View::at(-0.625, 1.0), None);
    let (index, _) = checked_index(&a, 1.0);
    let old_index = Arc::downgrade(&index.lifetime);
    pixels(&mut engine, &a, index, 1.0);
    // A fresh engine owns a different font catalog, never translated old IDs.
    engine = super::engine();
    let b = engine.layout(&spec("new face fj café"), Some(185.0));
    assert!(!Rc::ptr_eq(&a.source.catalog, &b.source.catalog));
    assert!(old_catalog.upgrade().is_some());
    let (next, _) = checked_index(&b, 1.0);
    pixels(&mut engine, &b, next, 1.0);
    assert_eq!(
        full(&mut engine, &a, &palette(), View::at(-0.625, 1.0), None).data(),
        expected.data()
    );
    // Rebuild after scale/token reset, releasing the old numeric index first.
    a.source.catalog.borrow_mut().ink_catalog = Rc::new(());
    let (replacement, _) = checked_index(&a, 1.25);
    pixels(&mut engine, &a, replacement, 1.25);
    assert!(old_index.upgrade().is_none());
    drop(a);
    assert!(old_catalog.upgrade().is_none());
}

#[test]
fn refused_b_keeps_a_and_adds_no_phase_cache_or_index_history() {
    let mut engine = engine();
    let a = Rc::new(engine.layout(&spec("accepted A fj"), Some(180.0)));
    let expected = full(&mut engine, &a, &palette(), View::at(0.0, 1.0), None);
    let (index, _) = checked_index(&a, 1.0);
    let retained = Arc::downgrade(&index.lifetime);
    pixels(&mut engine, &a, index, 1.0);
    let b = engine.layout(&spec(&"different B fj\n".repeat(20)), Some(100.0));
    let card = cache_card(&b);
    let start = ink::build_work();
    assert!(ink::Index::with_limit(&mut b.source.catalog.borrow_mut(), &b, 1.0, 64).is_none());
    let work = delta(start);
    assert_eq!(work.attempts, 1);
    assert_eq!(work.raster_phases, 0);
    assert_eq!(work.uncached_calls, 0);
    assert_eq!(cache_card(&b), card);
    drop(b);
    assert!(retained.upgrade().is_some());
    assert_eq!(
        full(&mut engine, &a, &palette(), View::at(0.0, 1.0), None).data(),
        expected.data()
    );
    drop(a);
    assert!(retained.upgrade().is_none());
}

#[test]
fn warm_repaint_reuses_one_index_and_preserves_metrics_source_and_runs() {
    let mut engine = engine();
    let p = engine.layout(&spec(&"fj café unchanged\n".repeat(8)), Some(180.0));
    let before = (
        p.width.to_bits(),
        p.height.to_bits(),
        p.first_baseline.to_bits(),
        p.baselines.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
    );
    let source = Rc::as_ptr(&p.source);
    let runs: Vec<_> = p
        .paint_glyphs(&palette())
        .map(|(g, b, ink)| {
            (
                g.glyph_id,
                g.font_id,
                g.font_size.to_bits(),
                g.start,
                g.end,
                b.to_bits(),
                ink,
            )
        })
        .collect();
    let (index, _) = checked_index(&p, 1.0);
    pixels(&mut engine, &p, index, 1.0);
    let work = ink::build_work();
    let builds = engine.ink_builds;
    compare(&mut engine, &p, &palette(), View::at(-17.375, 1.0), None);
    assert_eq!(delta(work), ink::BuildWork::default());
    assert_eq!(engine.ink_builds, builds);
    assert_eq!(Rc::as_ptr(&p.source), source);
    assert_eq!(
        before,
        (
            p.width.to_bits(),
            p.height.to_bits(),
            p.first_baseline.to_bits(),
            p.baselines.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        )
    );
    assert_eq!(
        runs,
        p.paint_glyphs(&palette())
            .map(|(g, b, ink)| (
                g.glyph_id,
                g.font_id,
                g.font_size.to_bits(),
                g.start,
                g.end,
                b.to_bits(),
                ink
            ))
            .collect::<Vec<_>>()
    );
}
