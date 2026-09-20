//! Compare selected CPU paint with the pre-index full glyph loop, byte for byte.
use super::*;
use exact_kernel::StyleProps;
use tiny_skia::{Color, FillRule, PathBuilder, Rect};

#[allow(dead_code)]
#[path = "../../../../apps/messages-stress/data/src/model.rs"]
pub(super) mod messages_envelope_model;

fn engine() -> TextEngine {
    let engine = TextEngine::new();
    engine
        .catalog
        .borrow_mut()
        .fonts
        .db_mut()
        .load_fonts_dir(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets"
        ));
    engine
        .catalog
        .borrow_mut()
        .fonts
        .db_mut()
        .set_sans_serif_family("DejaVu Sans");
    engine
}

fn spec(text: &str) -> Spec {
    crate::paint::text_spec(&StyleProps::default(), text)
}

fn palette() -> [RunPaint; 2] {
    [
        RunPaint {
            color: [180, 15, 30, 230],
            source: 7,
        },
        RunPaint {
            color: [15, 50, 190, 200],
            source: 9,
        },
    ]
}

#[derive(Clone, Copy)]
struct View {
    origin: (f32, f32),
    scale: f32,
    transform: Transform,
}
impl View {
    fn at(y: f32, scale: f32) -> Self {
        Self {
            origin: (7.375, y),
            scale,
            transform: Transform::from_scale(scale, scale),
        }
    }
}

// The old renderer, intentionally retaining its complete paint-order traversal.
// This is the pixel oracle, not an alternate implementation of ink selection.
fn full(
    _engine: &mut TextEngine,
    paragraph: &Paragraph,
    palette: &[RunPaint],
    view: View,
    mask: Option<&Mask>,
) -> Pixmap {
    let mut target = Pixmap::new(320, 128).unwrap();
    target.fill(Color::WHITE);
    let glyph_ts = view.transform.pre_scale(1.0 / view.scale, 1.0 / view.scale);
    let mut catalog = paragraph.source.catalog.borrow_mut();
    for (g, baseline, ink) in paragraph.paint_glyphs(palette) {
        if ink.color[3] == 0 {
            continue;
        }
        let phys = g.physical(
            (
                view.origin.0 * view.scale,
                (view.origin.1 + baseline) * view.scale,
            ),
            view.scale,
        );
        let Some(glyph) = catalog.glyph(phys.cache_key, ink.color) else {
            continue;
        };
        target.draw_pixmap(
            phys.x + glyph.left,
            phys.y - glyph.top,
            glyph.pixmap.as_ref(),
            &PixmapPaint::default(),
            glyph_ts,
            mask,
        );
    }
    target
}

fn compare(
    engine: &mut TextEngine,
    paragraph: &Paragraph,
    palette: &[RunPaint],
    view: View,
    mask: Option<&Mask>,
) -> usize {
    let expected = full(engine, paragraph, palette, view, mask);
    let mut actual = Pixmap::new(320, 128).unwrap();
    actual.fill(Color::WHITE);
    engine.ink_visits = 0;
    engine.ink_nodes = 0;
    engine.paint(
        &mut actual,
        paragraph,
        palette,
        view.origin,
        view.scale,
        view.transform,
        mask,
    );
    assert_eq!(
        actual.data(),
        expected.data(),
        "origin={:?}, scale={}",
        view.origin,
        view.scale
    );
    engine.ink_visits
}

#[test]
fn wrapped_top_middle_and_end_match_pixels_without_prefix_glyph_visits() {
    let mut engine = engine();
    let p = engine.layout(
        &spec(&"f j café words on this line\n".repeat(1200)),
        Some(240.0),
    );
    let all = p.paint_glyphs(&palette()).count();
    let metrics = (p.width, p.height, p.first_baseline, p.baselines.clone());
    let builds = engine.ink_builds;
    for y in [0.375, -p.height / 2.0 + 0.625, -p.height + 100.125] {
        let visits = compare(&mut engine, &p, &palette(), View::at(y, 1.0), None);
        assert!(visits > 0 && visits < all / 20, "visited {visits}/{all}");
        assert!(
            engine.ink_nodes < 200,
            "walked {} index nodes",
            engine.ink_nodes
        );
    }
    assert_eq!(engine.ink_builds, builds + 1, "scroll rebuilt the index");
    assert!(p.ink_capacity_bytes() > 0);
    assert!(p.ink_capacity_bytes() <= 8 * 1024 * 1024);
    assert_eq!(
        (p.width, p.height, p.first_baseline, p.baselines.clone()),
        metrics
    );
    assert_eq!(p.paint_glyphs(&palette()).count(), all);
    assert_eq!(
        engine
            .glyph_runs(&p, &palette())
            .iter()
            .map(|r| r.glyphs.len())
            .sum::<usize>(),
        all
    );
}

#[test]
fn fractional_origins_scales_and_transforms_preserve_overhanging_ink() {
    let mut engine = engine();
    let mut s = spec(&"fjy Áé W\n".repeat(80));
    s.runs[0].italic = true;
    s.runs[0].size = 35.0;
    s.runs[0].line_height = Some(7.25);
    s.strut.line_height = Some(0.0);
    let p = engine.layout(&s, Some(170.0));
    for scale in [0.75, 1.0, 1.25, 2.0] {
        for y in [-0.875, -79.625, -320.125] {
            let mut view = View::at(y, scale);
            compare(&mut engine, &p, &palette(), view, None);
            view.transform = view.transform.pre_rotate(19.0).pre_translate(12.25, -6.75);
            compare(&mut engine, &p, &palette(), view, None);
        }
    }
}

#[test]
fn zero_and_backwards_baselines_keep_every_overlapping_line_in_paint_order() {
    let mut engine = engine();
    let mut s = spec(&"Áf\n".repeat(30));
    s.runs.push(s.runs[0].clone());
    let mut p = engine.layout(&s, Some(180.0));
    for (i, baseline) in Arc::get_mut(&mut p.baselines)
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        *baseline = [40.0, -10.0, 100.0, 15.0, 35.0][i % 5];
    }
    compare(&mut engine, &p, &palette(), View::at(0.375, 1.25), None);
    let mut zero = engine.layout(&s, Some(180.0));
    Arc::get_mut(&mut zero.baselines).unwrap().fill(18.0);
    zero.height = 0.0;
    let all = zero.paint_glyphs(&palette()).count();
    assert_eq!(
        compare(&mut engine, &zero, &palette(), View::at(0.0, 1.0), None),
        all
    );
}

#[test]
fn palette_changes_do_not_freeze_transparency_or_change_shape() {
    let mut engine = engine();
    let mut s = spec(&"zero alpha then colored\n".repeat(60));
    s.runs.push(s.runs[0].clone());
    let p = engine.layout(&s, Some(230.0));
    let shapes = engine.shape_calls;
    let mut ink = palette();
    ink[0].color[3] = 0;
    compare(&mut engine, &p, &ink, View::at(-33.875, 1.0), None);
    let capacity = p.ink_capacity_bytes();
    let builds = engine.ink_builds;
    ink[0].color = [0, 120, 60, 255];
    ink[1].source = 100;
    compare(&mut engine, &p, &ink, View::at(-33.875, 1.0), None);
    assert_eq!(engine.shape_calls, shapes);
    assert_eq!(p.ink_capacity_bytes(), capacity);
    assert_eq!(engine.ink_builds, builds);
}

#[test]
fn nested_mask_and_fractional_clip_edges_keep_exact_pixels() {
    let mut engine = engine();
    let p = engine.layout(&spec(&"clip accented é fj\n".repeat(70)), Some(210.0));
    let mut mask = Mask::new(320, 128).unwrap();
    mask.fill_path(
        &PathBuilder::from_rect(Rect::from_xywh(9.25, 17.75, 265.5, 85.25).unwrap()),
        FillRule::Winding,
        true,
        Transform::identity(),
    );
    mask.intersect_path(
        &PathBuilder::from_rect(Rect::from_xywh(27.5, 23.125, 185.25, 43.25).unwrap()),
        FillRule::Winding,
        true,
        Transform::from_rotate(3.0),
    );
    compare(
        &mut engine,
        &p,
        &palette(),
        View::at(-198.375, 1.25),
        Some(&mask),
    );
}

#[test]
fn large_fractional_scroll_and_narrow_clip_do_not_lose_end_ink() {
    let mut engine = engine();
    let mut p = engine.layout(&spec(&"large extent\n".repeat(90)), Some(200.0));
    for baseline in Arc::get_mut(&mut p.baselines).unwrap() {
        *baseline += 16_777_216.0;
    }
    compare(
        &mut engine,
        &p,
        &palette(),
        View::at(-16_777_216.0, 1.25),
        None,
    );
}

#[test]
fn fallback_cjk_combining_and_emoji_match_full_paint() {
    let mut engine = engine();
    let s = spec(&"Latin e\u{301} 中日 🦀 🌈\n".repeat(80));
    let p = engine.layout(&s, Some(230.0));
    for y in [0.0, -100.375, -p.height + 80.0] {
        compare(&mut engine, &p, &palette(), View::at(y, 1.25), None);
    }
}

#[test]
fn scale_changes_replace_one_index_and_do_not_retain_history() {
    let mut engine = engine();
    let p = engine.layout(&spec(&"scale changes\n".repeat(60)), Some(220.0));
    let mut first_capacity = 0;
    let builds = engine.ink_builds;
    for scale in [1.0, 2.0, 0.75, 1.25, 1.0] {
        compare(&mut engine, &p, &palette(), View::at(-120.5, scale), None);
        let bytes = p.ink_capacity_bytes();
        assert!(bytes > 0 && bytes <= 8 * 1024 * 1024);
        if first_capacity == 0 {
            first_capacity = bytes;
        } else {
            assert_eq!(bytes, first_capacity);
        }
    }
    assert_eq!(engine.ink_builds, builds + 5);
}

#[test]
fn physical_cpu_placement_uses_all_x_bins_and_only_zero_y_bin() {
    use cosmic_text::SubpixelBin;
    let mut engine = engine();
    let p = engine.layout(&spec("f"), Some(100.0));
    let mut g = p.layout_runs().next().unwrap().glyphs[0].clone();
    g.x = 0.0;
    g.x_offset = 0.0;
    g.y = 0.375;
    g.y_offset = 0.013;
    let mut x_bins = std::collections::HashSet::new();
    for scale in [0.75, 1.0, 1.25, 2.0] {
        for i in -32..32 {
            let offset = (i as f32 / 16.0, i as f32 / 13.0);
            let physical = g.physical(offset, scale);
            x_bins.insert(physical.cache_key.x_bin);
            assert_eq!(physical.cache_key.y_bin, SubpixelBin::Zero);
            assert_eq!(
                physical.y,
                (g.y - g.y_offset * g.font_size)
                    .mul_add(scale, offset.1)
                    .trunc() as i32
            );
        }
    }
    assert_eq!(x_bins.len(), 4);
}

#[test]
fn actual_color_bitmap_placement_matches_full_paint() {
    let mut engine = engine();
    let mut s = spec(&"😀 🌈 🦀\n".repeat(48));
    s.runs[0].size = 28.0;
    let p = engine.layout(&s, Some(230.0));
    let color = p.paint_glyphs(&palette()).any(|(g, _, _)| {
        let mut borrow = p.source.catalog.borrow_mut();
        let catalog = &mut *borrow;
        catalog
            .swash
            .get_image_uncached(&mut catalog.fonts, g.physical((0.0, 0.0), 1.25).cache_key)
            .is_some_and(|image| image.content == SwashContent::Color)
    });
    assert!(color, "fixture must exercise an actual Swash color bitmap");
    for y in [-0.625, -119.375, -p.height + 70.25] {
        let mut view = View::at(y, 1.25);
        view.transform = view.transform.pre_rotate(-11.0);
        compare(&mut engine, &p, &palette(), view, None);
    }
}

#[test]
fn unsafe_transform_fails_open_and_catalog_replacement_rebuilds() {
    let mut engine = engine();
    let p = engine.layout(&spec(&"catalog\n".repeat(100)), Some(200.0));
    let all = p.paint_glyphs(&palette()).count();
    let mut view = View::at(-400.0, 1.0);
    view.transform = Transform::from_scale(0.0, 1.0);
    assert_eq!(compare(&mut engine, &p, &palette(), view, None), all);
    view.transform = Transform::from_row(f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0);
    assert_eq!(compare(&mut engine, &p, &palette(), view, None), all);
    let builds = engine.ink_builds;
    engine.catalog.borrow_mut().ink_catalog = Rc::new(());
    compare(&mut engine, &p, &palette(), View::at(-400.0, 1.0), None);
    assert_eq!(engine.ink_builds, builds + 1);
}

#[test]
fn raster_nested_scroll_clip_bounds_work_and_pop_restores_parent() {
    use crate::paint::{Backend, Shape};
    use crate::raster::{rounded_rect, Raster};
    let mut engine = engine();
    let p = engine.layout(&spec(&"nested scrollport line\n".repeat(160)), Some(230.0));
    let parent = Shape {
        rect: (10.125, 7.25, 275.5, 110.5),
        radii: [5.5; 4],
    };
    let child = Shape {
        rect: (18.375, 49.25, 210.5, 9.25),
        radii: [2.5; 4],
    };
    let ts = Transform::from_rotate(2.0);
    let view = View::at(-377.625, 1.0);
    let mut raster = Raster::new();
    raster.begin(320.0, 128.0, 1.0);
    raster.push_clip(&parent, ts);
    raster.push_clip(&child, ts);
    let mut mask = Mask::new(320, 128).unwrap();
    mask.fill_path(&rounded_rect(&parent).unwrap(), FillRule::Winding, true, ts);
    mask.intersect_path(&rounded_rect(&child).unwrap(), FillRule::Winding, true, ts);
    let expected = full(&mut engine, &p, &palette(), view, Some(&mask));
    engine.ink_visits = 0;
    raster.text(
        &mut engine,
        &p,
        &palette(),
        view.origin,
        Transform::identity(),
    );
    let child_visits = engine.ink_visits;
    assert_eq!(raster.finish().unwrap().data(), expected.data());
    // A new frame exercises clear, while pop below exercises restoration.
    raster.begin(320.0, 128.0, 1.0);
    raster.push_clip(&parent, ts);
    raster.push_clip(&child, ts);
    raster.pop_clip();
    let mut mask = Mask::new(320, 128).unwrap();
    mask.fill_path(&rounded_rect(&parent).unwrap(), FillRule::Winding, true, ts);
    let expected = full(&mut engine, &p, &palette(), view, Some(&mask));
    engine.ink_visits = 0;
    raster.text(
        &mut engine,
        &p,
        &palette(),
        view.origin,
        Transform::identity(),
    );
    assert_eq!(raster.finish().unwrap().data(), expected.data());
    assert!(
        child_visits < engine.ink_visits / 2,
        "child visited {child_visits}, parent {}",
        engine.ink_visits
    );
}

#[test]
fn refused_index_keeps_full_paint_without_rebuilding_until_scale_changes() {
    let mut engine = engine();
    let p = engine.layout(&spec(&"bounded metadata\n".repeat(100)), Some(220.0));
    let all = p.paint_glyphs(&palette()).count();
    {
        let mut cache = p.ink.borrow_mut();
        cache.reset(&engine.catalog.borrow().ink_catalog, 1.0);
        cache.index =
            ink::Index::with_limit(&mut engine.catalog.borrow_mut(), &p, 1.0, 64).map(Into::into);
        assert!(cache.index.is_none());
    }
    let builds = engine.ink_builds;
    for y in [-0.875, -711.25, -p.height + 80.125] {
        assert_eq!(
            compare(&mut engine, &p, &palette(), View::at(y, 1.0), None),
            all
        );
    }
    assert_eq!(p.ink_capacity_bytes(), 0);
    assert_eq!(engine.ink_builds, builds);
    compare(&mut engine, &p, &palette(), View::at(-300.0, 1.25), None);
    assert!(p.ink_capacity_bytes() > 0);
    assert_eq!(engine.ink_builds, builds + 1);
}

#[test]
fn fractional_phase_edges_reflection_and_skew_match_full_pixels() {
    let mut engine = engine();
    let mut s = spec(&"fj Áe\u{301} italic\n".repeat(40));
    s.runs[0].italic = true;
    s.runs[0].size = 21.25;
    let p = engine.layout(&s, Some(230.0));
    for phase in -8..8 {
        for transform in [
            Transform::identity(),
            Transform::from_row(-1.0, 0.125, -0.25, 1.0, 260.25, -5.125),
            Transform::from_row(0.8, 0.375, 0.25, 1.25, 6.75, -3.375),
        ] {
            let view = View {
                origin: (phase as f32 / 8.0, -p.baselines[15] + phase as f32 / 8.0),
                scale: 1.25,
                transform,
            };
            compare(&mut engine, &p, &palette(), view, None);
        }
    }
}

mod clip_masks {
    use super::{engine, full, palette, spec, View};
    use crate::paint::{Backend, Shape};
    use crate::raster::{rounded_rect, Raster};
    use std::rc::Weak;
    use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Transform};

    const COLOR: [u8; 4] = [37, 91, 121, 210];

    fn root() -> Shape {
        Shape {
            rect: (10.125, 7.25, 275.5, 110.5),
            radii: [5.5, 4.25, 3.5, 2.75],
        }
    }

    // Independent, uncached full-device mask and full-RGBA output. No Raster
    // mask constructor, clip stack, bounds selection or cache is consulted.
    fn oracle(size: (f32, f32), scale: f32, shape: Option<&Shape>, ts: Transform) -> Pixmap {
        let width = ((size.0 * scale).round() as u32).max(1);
        let height = ((size.1 * scale).round() as u32).max(1);
        let mask = shape.map(|shape| {
            let mut mask = Mask::new(width, height).unwrap();
            if let Some(path) = rounded_rect(shape) {
                mask.fill_path(
                    &path,
                    FillRule::Winding,
                    true,
                    Transform::from_scale(scale, scale).pre_concat(ts),
                );
            }
            mask
        });
        let mut result = Pixmap::new(width, height).unwrap();
        result.fill(Color::WHITE);
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(COLOR[0], COLOR[1], COLOR[2], COLOR[3]));
        paint.anti_alias = true;
        result.fill_path(
            &PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, size.0, size.1).unwrap()),
            &paint,
            FillRule::Winding,
            Transform::from_scale(scale, scale),
            mask.as_ref(),
        );
        result
    }

    fn frame(
        raster: &mut Raster,
        size: (f32, f32),
        scale: f32,
        shape: &Shape,
        ts: Transform,
    ) -> (Pixmap, Weak<Mask>) {
        raster.begin(size.0, size.1, scale);
        raster.push_clip(shape, ts);
        let owner = raster.clip_weak();
        raster.fill(
            &Shape::rect((0.0, 0.0, size.0, size.1)),
            COLOR,
            Transform::identity(),
        );
        raster.pop_clip();
        (raster.finish().unwrap(), owner)
    }

    #[test]
    fn clip_mask_reuses_actual_allocation_across_scrolled_text_frames() {
        let mut engine = engine();
        let p = engine.layout(&spec(&"nested scrollport line\n".repeat(160)), Some(230.0));
        let mut raster = Raster::new();
        let shape = root();
        let ts = Transform::from_rotate(2.0);
        let mut owners = Vec::new();
        let mut pictures = Vec::new();
        for y in [-377.625, -417.625] {
            let view = View::at(y, 1.0);
            let mut mask = Mask::new(320, 128).unwrap();
            mask.fill_path(&rounded_rect(&shape).unwrap(), FillRule::Winding, true, ts);
            let expected = full(&mut engine, &p, &palette(), view, Some(&mask));
            raster.begin(320.0, 128.0, 1.0);
            raster.push_clip(&shape, ts);
            owners.push(raster.clip_weak());
            raster.text(
                &mut engine,
                &p,
                &palette(),
                view.origin,
                Transform::identity(),
            );
            raster.pop_clip();
            let actual = raster.finish().unwrap();
            assert_eq!(actual.data(), expected.data());
            pictures.push(actual);
        }
        assert_ne!(
            pictures[0].data(),
            pictures[1].data(),
            "real scroll changes ink"
        );
        assert_eq!(raster.clip_allocations.get(), 1, "old path allocates twice");
        assert!(owners[0].ptr_eq(&owners[1]));
        assert_eq!(
            owners[0].strong_count(),
            1,
            "only the cache retains the mask"
        );
        drop(raster);
        assert!(owners.iter().all(|owner| owner.upgrade().is_none()));
    }

    #[test]
    fn clip_mask_every_key_class_rebuilds_and_equal_key_reuses_exact_pixels() {
        let base = root();
        let mut moved = base;
        moved.rect.0 += 0.125;
        let mut rounded = base;
        rounded.radii[3] += 0.25;
        let cases = [
            ((320.0, 128.0), 1.0, base, Transform::identity()),
            ((320.0, 128.0), 1.0, moved, Transform::identity()),
            ((320.0, 128.0), 1.0, rounded, Transform::identity()),
            (
                (320.0, 128.0),
                1.0,
                base,
                Transform::from_translate(0.25, -0.5),
            ),
            ((320.0, 128.0), 1.0, base, Transform::from_rotate(2.0)),
            (
                (320.0, 128.0),
                1.0,
                base,
                Transform::from_row(1.0, 0.125, 0.25, 1.0, 0.0, 0.0),
            ),
            ((321.0, 128.0), 1.0, base, Transform::identity()),
            ((321.0, 129.0), 1.0, base, Transform::identity()),
            ((320.0, 128.0), 1.25, base, Transform::identity()),
            ((320.0, 128.0), 1.0, base, Transform::identity()),
            // Same device dimensions and effective transform as the first
            // case: the explicit scale still belongs to the exact key.
            ((160.0, 64.0), 2.0, base, Transform::from_scale(0.5, 0.5)),
        ];
        let mut raster = Raster::new();
        for (i, (size, scale, shape, ts)) in cases.into_iter().enumerate() {
            let expected = oracle(size, scale, Some(&shape), ts);
            let (first, owner) = frame(&mut raster, size, scale, &shape, ts);
            let (second, again) = frame(&mut raster, size, scale, &shape, ts);
            assert_eq!(first.data(), expected.data(), "key case {i}");
            assert_eq!(second.data(), expected.data(), "warm key case {i}");
            assert_eq!(raster.clip_allocations.get(), i + 1, "key case {i}");
            assert!(owner.ptr_eq(&again));
        }
    }

    #[test]
    fn clip_mask_nested_and_later_parentless_do_not_mutate_or_replace_first() {
        let mut engine = engine();
        let p = engine.layout(&spec(&"nested scrollport line\n".repeat(160)), Some(230.0));
        let parent = root();
        let child = Shape::new((18.375, 49.25, 210.5, 9.25), [2.5; 4]);
        let ts = Transform::from_rotate(2.0);
        let view = View::at(-377.625, 1.0);
        let mut raster = Raster::new();
        raster.begin(320.0, 128.0, 1.0);
        raster.push_clip(&parent, ts);
        let first = raster.clip_weak();
        let first_bytes = first.upgrade().unwrap().data().to_vec();
        raster.push_clip(&child, ts);
        let nested = raster.clip_weak();
        let mut mask = Mask::new(320, 128).unwrap();
        mask.fill_path(&rounded_rect(&parent).unwrap(), FillRule::Winding, true, ts);
        mask.intersect_path(&rounded_rect(&child).unwrap(), FillRule::Winding, true, ts);
        let expected = full(&mut engine, &p, &palette(), view, Some(&mask));
        raster.text(
            &mut engine,
            &p,
            &palette(),
            view.origin,
            Transform::identity(),
        );
        raster.pop_clip();
        assert!(nested.upgrade().is_none());
        assert_eq!(raster.clip_weak().upgrade().unwrap().data(), first_bytes);
        raster.pop_clip();
        assert_eq!(raster.finish().unwrap().data(), expected.data());

        raster.begin(320.0, 128.0, 1.0);
        raster.push_clip(&parent, ts);
        assert!(first.ptr_eq(&raster.clip_weak()));
        raster.pop_clip();
        raster.push_clip(&child, ts);
        let later = raster.clip_weak();
        raster.fill(
            &Shape::rect((0.0, 0.0, 320.0, 128.0)),
            COLOR,
            Transform::identity(),
        );
        raster.pop_clip();
        assert_eq!(
            raster.finish().unwrap().data(),
            oracle((320.0, 128.0), 1.0, Some(&child), ts).data()
        );
        assert!(
            later.upgrade().is_none(),
            "second parentless mask is not retained"
        );
        assert_eq!(first.upgrade().unwrap().data(), first_bytes);
        let (_, again) = frame(&mut raster, (320.0, 128.0), 1.0, &parent, ts);
        assert!(first.ptr_eq(&again));
        assert_eq!(
            raster.clip_allocations.get(),
            2,
            "one root plus one later root"
        );
    }

    #[test]
    fn clip_mask_replacement_drops_old_before_allocating_and_keeps_no_history() {
        let mut raster = Raster::new();
        let a = root();
        let b = Shape::rect((22.5, 17.25, 120.0, 80.0));
        let (_, old) = frame(&mut raster, (320.0, 128.0), 1.0, &a, Transform::identity());
        assert_eq!(old.strong_count(), 1);
        raster.clip_watch = Some(old.clone());
        let (picture, current) = frame(&mut raster, (320.0, 128.0), 1.0, &b, Transform::identity());
        assert_eq!(raster.watched_owners_at_allocation.get(), 0);
        assert!(old.upgrade().is_none());
        assert_eq!(
            picture.data(),
            oracle((320.0, 128.0), 1.0, Some(&b), Transform::identity()).data()
        );
        let (_, rebuilt) = frame(&mut raster, (320.0, 128.0), 1.0, &a, Transform::identity());
        assert_eq!(raster.clip_allocations.get(), 3);
        assert!(!old.ptr_eq(&rebuilt));
        assert!(current.upgrade().is_none());
        assert_eq!(rebuilt.strong_count(), 1);
        drop(raster);
        assert!(rebuilt.upgrade().is_none());
    }

    #[test]
    fn clip_mask_nonfinite_or_empty_first_consumes_slot_without_retaining_refusal() {
        let bad = [
            (Shape::rect((0.0, 0.0, 0.0, 10.0)), Transform::identity()),
            (
                Shape::rect((f32::NAN, 0.0, 10.0, 10.0)),
                Transform::identity(),
            ),
            (
                root(),
                Transform::from_row(1.0, 0.0, 0.0, 1.0, f32::NAN, 0.0),
            ),
        ];
        for (shape, ts) in bad {
            let mut raster = Raster::new();
            let (_, old) = frame(
                &mut raster,
                (320.0, 128.0),
                1.0,
                &root(),
                Transform::identity(),
            );
            raster.clip_watch = Some(old.clone());
            let (actual, refused) = frame(&mut raster, (320.0, 128.0), 1.0, &shape, ts);
            assert_eq!(
                actual.data(),
                oracle((320.0, 128.0), 1.0, Some(&shape), ts).data()
            );
            assert_eq!(raster.watched_owners_at_allocation.get(), 0);
            assert!(old.upgrade().is_none());
            assert!(
                refused.upgrade().is_none(),
                "fallback mask has no cache owner"
            );
            raster.begin(320.0, 128.0, 1.0);
            raster.push_clip(&shape, ts);
            let refused_again = raster.clip_weak();
            raster.pop_clip();
            raster.push_clip(&root(), Transform::identity());
            let later = raster.clip_weak();
            raster.fill(
                &Shape::rect((0.0, 0.0, 320.0, 128.0)),
                COLOR,
                Transform::identity(),
            );
            raster.pop_clip();
            assert_eq!(
                raster.finish().unwrap().data(),
                oracle((320.0, 128.0), 1.0, Some(&root()), Transform::identity()).data()
            );
            assert!(refused_again.upgrade().is_none());
            assert!(
                later.upgrade().is_none(),
                "no promotion of a later valid root"
            );
            let (_, next) = frame(
                &mut raster,
                (320.0, 128.0),
                1.0,
                &root(),
                Transform::identity(),
            );
            assert_eq!(next.strong_count(), 1);
        }
    }

    #[test]
    fn clip_mask_payload_boundary_and_oversize_drop_all_cache_owners() {
        let mut raster = Raster::new();
        let shape = Shape::new((10.25, 8.5, 990.0, 990.0), [4.5; 4]);
        let ts = Transform::identity();
        let (_, exact) = frame(&mut raster, (1024.0, 1024.0), 1.0, &shape, ts);
        assert_eq!(exact.strong_count(), 1, "boundary mask has one cache owner");
        assert_eq!(exact.upgrade().unwrap().data().len(), 1_048_576);
        raster.clip_watch = Some(exact.clone());
        let (actual, oversize) = frame(&mut raster, (1025.0, 1024.0), 1.0, &shape, ts);
        assert_eq!(
            actual.data(),
            oracle((1025.0, 1024.0), 1.0, Some(&shape), ts).data()
        );
        assert_eq!(raster.watched_owners_at_allocation.get(), 0);
        assert!(exact.upgrade().is_none());
        assert!(
            oversize.upgrade().is_none(),
            "oversize is drawn but never retained"
        );
        let (_, again) = frame(&mut raster, (1025.0, 1024.0), 1.0, &shape, ts);
        assert!(again.upgrade().is_none());
        assert_eq!(raster.clip_allocations.get(), 3);
    }

    #[test]
    fn clip_mask_no_clip_frame_clears_active_mask_and_drop_releases_cache() {
        let mut raster = Raster::new();
        let (_, owner) = frame(
            &mut raster,
            (320.0, 128.0),
            1.0,
            &root(),
            Transform::identity(),
        );
        raster.begin(320.0, 128.0, 1.0);
        assert!(raster.clip_weak().upgrade().is_none());
        raster.fill(
            &Shape::rect((0.0, 0.0, 320.0, 128.0)),
            COLOR,
            Transform::identity(),
        );
        assert_eq!(
            raster.finish().unwrap().data(),
            oracle((320.0, 128.0), 1.0, None, Transform::identity()).data()
        );
        assert_eq!(raster.clip_allocations.get(), 1);
        assert_eq!(owner.strong_count(), 1, "inactive immutable cache only");
        drop(raster);
        assert!(owner.upgrade().is_none());
    }
}

mod cached_placements {
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
        let oracle = ink::Index::uncached_oracle(
            &mut p.source.catalog.borrow_mut(),
            p,
            scale,
            ink::MAX_BYTES,
        )
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
        let paint_batch =
            |engine: &mut TextEngine, paragraphs: &[Rc<Paragraph>], views: &[View]| {
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
        for (paragraphs, views, captured) in [(&a, &views_a, &pixels_a), (&b, &views_b, &pixels_b)]
        {
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
}
