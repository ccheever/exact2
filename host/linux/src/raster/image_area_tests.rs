use super::*;

fn opaque_source(width: u32, height: u32) -> Arc<Bitmap> {
    let gate = exact_raster::Gate::new();
    let session = gate.session();
    let bytes = u64::from(width) * u64::from(height) * 4;
    let reservation = session.reserve_allocation(bytes).unwrap();
    let mut pixels = Pixmap::new(width, height).unwrap();
    pixels.fill(Color::WHITE);
    let bitmap = Arc::new(Bitmap::new(pixels, (width, height), reservation.charge()));
    reservation.commit(bytes).unwrap();
    bitmap
}

#[test]
fn fractional_image_edges_match_bilinear_coverage() {
    let bitmap = opaque_source(8, 8);
    for x in [0.5, 0.25, 0.75] {
        let dst = (x, 0., 2., 2.);
        let mut alpha = Vec::new();
        for clipped in [false, true] {
            let mut actual = Raster::transparent();
            actual.begin(4., 2., 1.);
            let clips = if clipped {
                vec![Shape::rect(dst)]
            } else {
                vec![]
            };
            let mask = actual.mask_with(&clips, Transform::identity());
            let mut expected = Pixmap::new(4, 2).unwrap();
            expected.draw_pixmap(
                0,
                0,
                bitmap.pixels(),
                &PixmapPaint {
                    quality: FilterQuality::Bilinear,
                    ..Default::default()
                },
                Transform::from_translate(x, 0.).pre_scale(0.25, 0.25),
                mask.as_ref(),
            );
            actual.image(&bitmap, dst, &clips, Transform::identity(), None);
            let actual = actual.finish().unwrap();
            alpha.push((
                actual.pixel(0, 0).unwrap().alpha(),
                expected.pixel(0, 0).unwrap().alpha(),
            ));
            for (a, b) in actual.pixels().iter().zip(expected.pixels()) {
                assert!(
                    a.alpha().abs_diff(b.alpha()) <= 1,
                    "x={x} clipped={clipped}"
                );
            }
        }
        assert!(alpha.iter().all(|(a, b)| a.abs_diff(*b) <= 1));
        if x == 0.5 {
            assert_eq!(alpha, [(0, 0), (0, 0)]);
        }
    }
}

#[test]
fn nearly_zero_image_bounds_match_bilinear_rejection() {
    let bitmap = opaque_source(1, 1);
    let threshold = 1. / 4096_f32;
    let mut broad_clip = Mask::new(2, 2).unwrap();
    broad_clip.fill_path(
        &PathBuilder::from_rect(Rect::from_xywh(-4., -4., 8., 8.).unwrap()),
        FillRule::Winding,
        true,
        Transform::identity(),
    );
    for (size, rejected) in [
        (0.00002, true),
        (threshold, true),
        // The transformed endpoints still differ by exactly the threshold.
        (f32::from_bits(threshold.to_bits() + 1), true),
        (0.0003, false),
    ] {
        for sign in [1., -1.] {
            let scale = size * sign;
            let origin = 0.5 - scale / 2.;
            for dev in [
                Transform::from_row(scale, 0., 0., 1., origin, 0.),
                Transform::from_row(1., 0., 0., scale, 0., origin),
            ] {
                for mask in [None, Some(&broad_clip)] {
                    let mut actual = Pixmap::new(2, 2).unwrap();
                    let mut expected = actual.clone();
                    expected.draw_pixmap(
                        0,
                        0,
                        bitmap.pixels(),
                        &PixmapPaint {
                            quality: FilterQuality::Bilinear,
                            ..Default::default()
                        },
                        dev,
                        mask,
                    );
                    assert!(image_area::draw(
                        &mut actual,
                        bitmap.pixels(),
                        dev,
                        mask,
                        &mut image_area::Scratch::default(),
                    ));
                    assert_eq!(
                        expected.pixel(0, 0).unwrap().alpha(),
                        if rejected { 0 } else { 255 },
                        "{dev:?} clipped={}",
                        mask.is_some()
                    );
                    assert_eq!(
                        actual.data(),
                        expected.data(),
                        "{dev:?} clipped={}",
                        mask.is_some()
                    );
                }
            }
        }
    }
}

#[test]
fn minified_edges_match_bilinear_under_square_and_rounded_clips_at_device_scales() {
    for (width, height) in [(96, 96), (96, 32), (32, 96)] {
        let bitmap = opaque_source(width, height);
        for scale in [1.25, 1.5] {
            for offset in [-4.75, -0.5, 0., 0.25, 0.499, 0.5, 0.501, 0.75] {
                // Three source pixels per device pixel on each shrinking axis.
                let dst = (
                    (4. + offset) / scale,
                    (3. + offset) / scale,
                    32. / scale,
                    32. / scale,
                );
                for radius in [0., 8.] {
                    let clips = [Shape::new(dst, [radius; 4])];
                    for ts in [
                        Transform::identity(),
                        Transform::from_row(-1., 0., 0., -1., 40. / scale, 40. / scale),
                    ] {
                        let mut actual = Raster::transparent();
                        actual.begin(40. / scale, 40. / scale, scale);
                        let mask = actual.mask_with(&clips, ts);
                        let dev = actual.device(ts).pre_concat(
                            Transform::from_translate(dst.0, dst.1)
                                .pre_scale(dst.2 / width as f32, dst.3 / height as f32),
                        );
                        assert!(dev.sx.abs() < 0.5 || dev.sy.abs() < 0.5);
                        let mut expected = actual.target.pixels().unwrap().clone();
                        expected.draw_pixmap(
                            0,
                            0,
                            bitmap.pixels(),
                            &PixmapPaint {
                                quality: FilterQuality::Bilinear,
                                ..Default::default()
                            },
                            dev,
                            mask.as_ref(),
                        );
                        actual.image(&bitmap, dst, &clips, ts, None);
                        let actual = actual.finish().unwrap();
                        for (i, (a, b)) in actual.pixels().iter().zip(expected.pixels()).enumerate()
                        {
                            assert!(
                                a.alpha().abs_diff(b.alpha()) <= 1,
                                "{width}x{height} scale={scale} offset={offset} radius={radius} ts={ts:?} pixel={i}: {a:?} != {b:?}"
                            );
                        }
                        assert!(actual.pixels().iter().any(|p| p.alpha() == 255));
                    }
                }
            }
        }
    }
}

#[test]
fn an_identical_rect_clip_preserves_integer_image_edges() {
    let bitmap = source();
    for scale in [1., 1.25, 1.5] {
        let dst = (2. / scale, 4. / scale, 12. / scale, 10. / scale);
        let mut actual = Raster::transparent();
        let mut expected = Raster::transparent();
        for r in [&mut actual, &mut expected] {
            r.begin(24., 24., scale);
        }
        actual.push_clip(&Shape::rect(dst), Transform::identity());
        for r in [&mut actual, &mut expected] {
            r.image(&bitmap, dst, &[], Transform::identity(), None);
        }
        assert_eq!(
            actual.finish().unwrap().data(),
            expected.finish().unwrap().data(),
            "scale={scale}"
        );
    }
}

fn source() -> Arc<Bitmap> {
    let gate = exact_raster::Gate::new();
    let session = gate.session();
    let reservation = session.reserve_allocation(30 * 30 * 4).unwrap();
    let mut pixels = Pixmap::new(30, 30).unwrap();
    for (i, p) in pixels.pixels_mut().iter_mut().enumerate() {
        let [r, g, b, a] = match (i % 30 + i / 30 * 3) % 5 {
            0 => [255, 30, 10, 255],
            1 => [0, 255, 255, 0],
            2 => [230, 40, 170, 83],
            3 => [20, 100, 255, 171],
            _ => [0, 0, 0, 255],
        };
        *p = Color::from_rgba8(r, g, b, a).premultiply().to_color_u8();
    }
    let bitmap = Arc::new(Bitmap::new(pixels, (30, 30), reservation.charge()));
    reservation.commit(30 * 30 * 4).unwrap();
    bitmap
}

fn reference(
    target: &mut Pixmap,
    source: tiny_skia::PixmapRef<'_>,
    dev: Transform,
    mask: Option<&Mask>,
) {
    let inverse = dev.invert().unwrap();
    let mut edges = Mask::new(target.width(), target.height()).unwrap();
    edges.fill_path(
        &PathBuilder::from_rect(
            Rect::from_xywh(0., 0., source.width() as f32, source.height() as f32).unwrap(),
        ),
        FillRule::Winding,
        false,
        dev,
    );
    let taps = |at: usize, scale: f32, offset: f32, size: u32, center: f32| {
        let (scale, offset) = (f64::from(scale), f64::from(offset));
        let overlap =
            |a: f64, b: f64| ((at as f64 + 1.).min(a.max(b)) - (at as f64).max(a.min(b))).max(0.);
        if scale.abs() < 0.5 {
            let coverage = overlap(offset, offset + f64::from(size) * scale);
            if coverage == 0. {
                return vec![];
            }
            (0..size)
                .map(|i| {
                    (
                        i,
                        overlap(
                            offset + f64::from(i) * scale,
                            offset + f64::from(i + 1) * scale,
                        ) / coverage,
                    )
                })
                .filter(|(_, weight)| *weight != 0.)
                .collect::<Vec<_>>()
        } else {
            let first = (center - 0.5).floor() as i64;
            let fraction = f64::from((center + 0.5).fract());
            [(first, 1. - fraction), (first + 1, fraction)]
                .map(|(i, w)| (i.clamp(0, i64::from(size) - 1) as u32, w))
                .to_vec()
        }
    };
    let horizontal: Vec<_> = (0..target.width())
        .map(|x| {
            taps(
                x as usize,
                dev.sx,
                dev.tx,
                source.width(),
                (x as f32 + 0.5) * inverse.sx + inverse.tx,
            )
        })
        .collect();
    let vertical: Vec<_> = (0..target.height())
        .map(|y| {
            taps(
                y as usize,
                dev.sy,
                dev.ty,
                source.height(),
                (y as f32 + 0.5) * inverse.sy + inverse.ty,
            )
        })
        .collect();
    let stride = target.width() as usize;
    for (i, out) in target.data_mut().chunks_exact_mut(4).enumerate() {
        if edges.data()[i] == 0 {
            continue;
        }
        let (x, y) = (i % stride, i / stride);
        let mut color = [0.; 4];
        for (iy, wy) in &vertical[y] {
            for (ix, wx) in &horizontal[x] {
                let at = ((iy * source.width() + ix) * 4) as usize;
                for (c, value) in color.iter_mut().enumerate() {
                    *value += f64::from(source.data()[at + c]) * wx * wy;
                }
            }
        }
        let cover = mask.map_or(1., |m| f64::from(m.data()[i]) / 255.);
        for value in &mut color {
            *value *= cover;
        }
        for c in 0..4 {
            out[c] = (color[c] + f64::from(out[c]) * (1. - color[3] / 255.)).round() as u8;
        }
    }
}

#[test]
fn minified_images_average_area_with_fractional_edges_masks_tint_and_parent_clips() {
    let bitmap = source();
    for (w, h) in [
        (12., 10.),
        (10., 12.),
        (30., 12.),
        (12., 30.),
        (45., 10.),
        (10., 45.),
        (12., 15.),
        (15., 12.),
        (1., 1.),
    ] {
        for (x, y) in [
            (3., 4.),
            (3.25, 4.5),
            (-2.25, -1.375),
            (39.75, 39.5),
            (42., 45.),
        ] {
            for clip in 0..3 {
                for parent in [false, true] {
                    for tint in [None, Some([173, 65, 20, 180])] {
                        let dst = (x, y, w, h);
                        let clips = match clip {
                            0 => vec![],
                            1 => vec![Shape::rect((0., 0., 40., 40.))],
                            _ => vec![Shape::new((2.25, 3.5, 10., 9.), [2.; 4])],
                        };
                        let mut actual = Raster::new();
                        let mut expected = Raster::new();
                        for r in [&mut actual, &mut expected] {
                            r.begin(40., 40., 1.);
                            r.target
                                .pixels()
                                .unwrap()
                                .fill(Color::from_rgba8(21, 70, 120, 91));
                            if parent {
                                r.push_clip(
                                    &Shape::new((1.5, 2.25, 13., 12.), [3.; 4]),
                                    Transform::identity(),
                                );
                            }
                        }
                        actual.image(&bitmap, dst, &clips, Transform::identity(), tint);
                        let mask = expected.mask_with(&clips, Transform::identity());
                        if tint.is_some() {
                            expected.push_opacity(1.);
                        }
                        let target = expected.target.pixels().unwrap();
                        reference(
                            target,
                            bitmap.pixels(),
                            Transform::from_translate(x, y).pre_scale(w / 30., h / 30.),
                            mask.as_ref(),
                        );
                        if let Some(tint) = tint {
                            let mut paint = solid(tint);
                            paint.blend_mode = tiny_skia::BlendMode::SourceIn;
                            target.fill_rect(
                                Rect::from_xywh(0., 0., 40., 40.).unwrap(),
                                &paint,
                                Transform::identity(),
                                None,
                            );
                            expected.pop_opacity();
                        }
                        let actual = actual.finish().unwrap();
                        let expected = expected.finish().unwrap();
                        assert_eq!(actual.data().len(), expected.data().len());
                        for (i, (a, b)) in actual.data().iter().zip(expected.data()).enumerate() {
                            assert_eq!(
                                a, b,
                                "dst={dst:?} clip={clip} parent={parent} tint={tint:?} byte={i}: {a} != {b}"
                            );
                        }
                        assert!(actual
                            .data()
                            .chunks_exact(4)
                            .all(|p| p[..3].iter().all(|c| *c <= p[3])));
                    }
                }
            }
        }
    }
}

#[test]
fn other_image_scales_and_transforms_keep_the_bilinear_path_byte_exact() {
    let bitmap = source();
    for scale in [1., 1.5, 2.] {
        for (w, h) in [
            (15., 15.),
            (15., 30.),
            (30., 15.),
            (30., 30.),
            (45., 60.),
            (9., 9.),
        ] {
            for ts in [
                Transform::identity(),
                Transform::from_rotate(15.),
                Transform::from_row(-1., 0., 0., 1., 35., 0.),
                Transform::from_row(1., 0., 0., -1., 0., 35.),
                Transform::from_row(-1., 0., 0., -1., 35., 35.),
                Transform::from_row(1., 0.2, 0.1, 1., 0., 0.),
            ] {
                let dev = Transform::from_scale(scale, scale)
                    .pre_concat(ts)
                    .pre_concat(Transform::from_translate(3.25, 4.5).pre_scale(w / 30., h / 30.));
                if dev.kx == 0. && dev.ky == 0. && (dev.sx.abs() < 0.5 || dev.sy.abs() < 0.5) {
                    continue;
                }
                let mut actual = Raster::new();
                actual.begin(40., 40., scale);
                actual.push_clip(
                    &Shape::new((1.5, 2.25, 30., 32.), [3.; 4]),
                    Transform::identity(),
                );
                let mask = actual.active_clip().cloned();
                let mut expected = actual.target.pixels().unwrap().clone();
                actual.image(&bitmap, (3.25, 4.5, w, h), &[], ts, None);
                expected.draw_pixmap(
                    0,
                    0,
                    bitmap.pixels(),
                    &PixmapPaint {
                        quality: FilterQuality::Bilinear,
                        ..Default::default()
                    },
                    dev,
                    mask.as_deref(),
                );
                assert_eq!(
                    actual.finish().unwrap().data(),
                    expected.data(),
                    "scale={scale} size={w},{h} ts={ts:?}"
                );
            }
        }
    }
}

#[test]
fn mixed_axes_keep_bilinear_ramps_and_hard_edges() {
    for transpose in [false, true] {
        let mut source =
            Pixmap::new(if transpose { 2 } else { 8 }, if transpose { 8 } else { 2 }).unwrap();
        for y in 0..source.height() {
            for x in 0..source.width() {
                let v = if (if transpose { x } else { y }) == 0 {
                    0
                } else {
                    255
                };
                let at = ((y * source.width() + x) * 4) as usize;
                source.data_mut()[at..at + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        for offset in [0., 0.25] {
            let mut target =
                Pixmap::new(if transpose { 9 } else { 2 }, if transpose { 2 } else { 9 }).unwrap();
            let transform = if transpose {
                Transform::from_translate(offset, 0.).pre_scale(4., 0.25)
            } else {
                Transform::from_translate(0., offset).pre_scale(0.25, 4.)
            };
            assert!(image_area::draw(
                &mut target,
                source.as_ref(),
                transform,
                None,
                &mut image_area::Scratch::default()
            ));
            let (values, alpha) = if offset == 0. {
                (
                    [0, 0, 32, 96, 159, 223, 255, 255, 0],
                    [255, 255, 255, 255, 255, 255, 255, 255, 0],
                )
            } else {
                (
                    [0, 0, 16, 80, 143, 207, 255, 255, 0],
                    [255, 255, 255, 255, 255, 255, 255, 255, 0],
                )
            };
            for i in 0..9 {
                for j in 0..2 {
                    let (x, y) = if transpose { (i, j) } else { (j, i) };
                    let p = target.pixel(x as u32, y as u32).unwrap();
                    assert_eq!(
                        [p.red(), p.green(), p.blue(), p.alpha()],
                        [values[i], values[i], values[i], alpha[i]]
                    );
                }
            }
        }
    }
}

#[test]
fn mirrored_minification_averages_area_on_each_axis() {
    let bitmap = source();
    let mut scratch = image_area::Scratch::default();
    for (sx, sy) in [(0.25, 0.375), (0.25, 1.5), (1.5, 0.25)] {
        let rows = match sy {
            0.375 => [30, 26, 27],
            1.5 => [26, 28, 8],
            _ => [30, 24, 28],
        };
        for (flip_x, flip_y) in [(-1., 1.), (1., -1.), (-1., -1.)] {
            for ((x, y), rows) in [(1., 2.), (-3.25, -1.5), (30.75, 29.5)]
                .into_iter()
                .zip(rows)
            {
                let transform = Transform::from_row(
                    sx * flip_x,
                    0.,
                    0.,
                    sy * flip_y,
                    x + if flip_x < 0. { sx * 30. } else { 0. },
                    y + if flip_y < 0. { sy * 30. } else { 0. },
                );
                let mut actual = Pixmap::new(40, 40).unwrap();
                actual.fill(Color::from_rgba8(21, 70, 120, 91));
                let mut expected = actual.clone();
                assert!(image_area::draw(
                    &mut actual,
                    bitmap.pixels(),
                    transform,
                    None,
                    &mut scratch
                ));
                reference(&mut expected, bitmap.pixels(), transform, None);
                assert_eq!(actual.data(), expected.data(), "{transform:?}");
                assert_eq!(scratch.rows_filtered, rows, "{transform:?}");
            }
        }
    }
}

#[test]
fn long_constant_sources_preserve_colour_and_filter_each_source_row_once() {
    let mut scratch = image_area::Scratch::default();
    for (w, h, dw, dh) in [
        (1, 4_000_000, 1, 1),
        (4_000_000, 1, 1, 1),
        (2048, 1, 1, 4096),
        (1, 2048, 4096, 1),
        (1, 4_000_000, 4096, 1),
    ] {
        let mut source = Pixmap::new(w, h).unwrap();
        for rgba in [[255, 0, 0, 255], [71, 29, 13, 127]] {
            for pixel in source.data_mut().chunks_exact_mut(4) {
                pixel.copy_from_slice(&rgba);
            }
            let transform = Transform::from_scale(dw as f32 / w as f32, dh as f32 / h as f32);
            for background in [Color::TRANSPARENT, Color::WHITE] {
                let mut target = Pixmap::new(dw, dh).unwrap();
                target.fill(background);
                assert!(image_area::draw(
                    &mut target,
                    source.as_ref(),
                    transform,
                    None,
                    &mut scratch
                ));
                let expected = if background == Color::WHITE {
                    [
                        rgba[0] + (255 - rgba[3]),
                        rgba[1] + (255 - rgba[3]),
                        rgba[2] + (255 - rgba[3]),
                        255,
                    ]
                } else {
                    rgba
                };
                assert!(
                    target.data().chunks_exact(4).all(|pixel| pixel == expected),
                    "{w}x{h} {rgba:?}"
                );
                assert_eq!(scratch.rows_filtered, h as usize);
                assert_eq!(scratch.pixels_filtered, w as usize * h as usize);
            }
        }
    }
}

#[test]
fn masks_bound_filtering_and_skip_empty_rows() {
    let mut source = Pixmap::new(2048, 2048).unwrap();
    source.fill(Color::from_rgba8(71, 29, 13, 255));
    let mut target = Pixmap::new(512, 512).unwrap();
    let mut mask = Mask::new(512, 512).unwrap();
    let mut scratch = image_area::Scratch::default();
    for rows in [&[510, 511][..], &[], &[509, 511]] {
        target.fill(Color::TRANSPARENT);
        mask.data_mut().fill(0);
        for y in rows {
            mask.data_mut()[y * 512 + 510..y * 512 + 512].fill(255);
        }
        assert!(image_area::draw(
            &mut target,
            source.as_ref(),
            Transform::from_scale(0.25, 0.25),
            Some(&mask),
            &mut scratch,
        ));
        assert_eq!(scratch.rows_filtered, rows.len() * 4);
        assert_eq!(scratch.pixels_filtered, rows.len() * 32);
        for (pixel, cover) in target.data().chunks_exact(4).zip(mask.data()) {
            assert_eq!(
                pixel,
                if *cover == 0 {
                    [0; 4]
                } else {
                    [71, 29, 13, 255]
                }
            );
        }
    }
}

#[test]
fn vertical_reduction_reads_only_columns_needed_by_horizontal_taps() {
    let mut source = Pixmap::new(32, 64).unwrap();
    source.fill(Color::WHITE);
    let mut target = Pixmap::new(128, 16).unwrap();
    let mut mask = Mask::new(128, 16).unwrap();
    mask.data_mut()[15 * 128 + 61] = 128;
    let mut scratch = image_area::Scratch::default();
    assert!(image_area::draw(
        &mut target,
        source.as_ref(),
        Transform::from_scale(4., 0.25),
        Some(&mask),
        &mut scratch,
    ));
    assert_eq!(scratch.rows_filtered, 4);
    assert_eq!(scratch.pixels_filtered, 8);
    for (pixel, cover) in target.data().chunks_exact(4).zip(mask.data()) {
        assert_eq!(pixel, [*cover; 4]);
    }
}

#[test]
fn mirrored_fractional_origin_has_known_bytes() {
    let mut source = Pixmap::new(3, 2).unwrap();
    source.data_mut().copy_from_slice(&[
        240, 0, 0, 255, 0, 0, 0, 0, 0, 0, 240, 255, 120, 60, 30, 255, 80, 40, 20, 85, 40, 80, 160,
        170,
    ]);
    let mut target = Pixmap::new(2, 1).unwrap();
    assert!(image_area::draw(
        &mut target,
        source.as_ref(),
        Transform::from_row(-0.25, 0., 0., 0.25, 1.125, 0.25),
        None,
        &mut image_area::Scratch::default(),
    ));
    assert_eq!(target.data(), [60, 30, 87, 153, 0, 0, 0, 0]);
}
