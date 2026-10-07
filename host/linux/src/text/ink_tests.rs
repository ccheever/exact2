//! Compare selected CPU paint with the pre-index full glyph loop, byte for byte.
use super::*;
use exact_kernel::StyleProps;
use tiny_skia::{Color, FillRule, PathBuilder, Rect};

#[allow(dead_code)]
#[path = "../../../../apps/messages-stress/data/src/model.rs"]
pub(super) mod messages_envelope_model;

fn engine() -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::installed_with(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets"
        )),
        "DejaVu Sans",
    ))
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
    let slots = ink::slots(&mut catalog, paragraph.lines());
    for (g, baseline, ink) in paragraph.paint_glyphs(palette) {
        if ink.color[3] == 0 {
            continue;
        }
        let (key, x, y) = ink::physical(
            g,
            slots[g.face as usize],
            (
                view.origin.0 * view.scale,
                (view.origin.1 + baseline) * view.scale,
            ),
            view.scale,
        );
        let Some(glyph) = catalog.glyph(key, ink.color) else {
            continue;
        };
        target.draw_pixmap(
            x + glyph.left,
            y - glyph.top,
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
    let mut engine = engine();
    let p = engine.layout(&spec("f"), Some(100.0));
    let mut g = p.layout_runs().next().unwrap().glyphs[0];
    g.x = 0.0;
    g.y = 0.375 - 0.013 * g.font_size;
    let mut x_bins = std::collections::HashSet::new();
    for scale in [0.75, 1.0, 1.25, 2.0] {
        for i in -32..32 {
            let offset = (i as f32 / 16.0, i as f32 / 13.0);
            let (key, _, y) = ink::physical(&g, 0, offset, scale);
            x_bins.insert(key.x_bin);
            assert_eq!(key.y_bin, 0);
            assert_eq!(y, g.y.mul_add(scale, offset.1).trunc() as i32);
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
        let mut catalog = p.source.catalog.borrow_mut();
        let slots = ink::slots(&mut catalog, p.lines());
        let (key, _, _) = ink::physical(g, slots[g.face as usize], (0.0, 0.0), 1.25);
        catalog.renders_color(key)
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
        radii: [(5.5, 5.5); 4],
        corners: None,
    };
    let child = Shape {
        rect: (18.375, 49.25, 210.5, 9.25),
        radii: [(2.5, 2.5); 4],
        corners: None,
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
            radii: [5.5, 4.25, 3.5, 2.75].map(|r| (r, r)),
            corners: None,
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
        rounded.radii[3].0 += 0.25;
        let mut vertical = base;
        vertical.radii[3].1 += 0.25;
        let cases = [
            ((320.0, 128.0), 1.0, base, Transform::identity()),
            ((320.0, 128.0), 1.0, moved, Transform::identity()),
            ((320.0, 128.0), 1.0, rounded, Transform::identity()),
            ((320.0, 128.0), 1.0, vertical, Transform::identity()),
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

#[path = "cached_placement_tests.rs"]
mod cached_placements;
