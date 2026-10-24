use super::*;
use crate::paint::border;
use exact_kernel::corner::CornerShape;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn transforms() -> [Transform; 4] {
    [
        Transform::identity(),
        Transform::from_row(-1.0, 0.0, 0.0, 1.0, 96.0, 0.0),
        Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, 80.0),
        Transform::from_row(0.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    ]
}

fn shape(radius: f32, corners: Option<CornerShape>) -> Shape {
    Shape {
        rect: (4.0, 4.0, 80.0, 60.0),
        radii: [(4.0, 4.0), (radius, radius), (4.0, 4.0), (4.0, 4.0)],
        corners,
    }
}

#[derive(Debug, Clone, Copy)]
enum Draw {
    Fill,
    Gradient,
    Clip,
}

fn draw(shape: &Shape, ts: Transform, alpha: u8, mode: Draw) -> Pixmap {
    let mut raster = Raster::new();
    raster.begin(96.0, 80.0, 0.75);
    let color = [10, 20, 30, alpha];
    match mode {
        Draw::Fill => raster.fill(shape, color, ts),
        Draw::Gradient => raster.fill_gradient(
            shape,
            &GradientPaint {
                geometry: Geometry::Linear {
                    start: (4.0, 4.0),
                    end: (84.0, 64.0),
                },
                repeat: false,
                stops: vec![
                    (0.0, exact_kernel::Color::rgba(10, 20, 30, alpha)),
                    (1.0, exact_kernel::Color::rgba(80, 60, 40, alpha)),
                ],
            },
            ts,
        ),
        Draw::Clip => {
            raster.push_clip(shape, ts);
            raster.fill(
                &Shape::rect((0.0, 0.0, 96.0, 80.0)),
                color,
                Transform::identity(),
            );
            raster.pop_clip();
        }
    }
    raster.finish().unwrap()
}

#[test]
fn malformed_radii_do_not_panic_in_shape_paint() {
    let mut panics = Vec::new();
    for corners in [None, Some(CornerShape::check("squircle").unwrap())] {
        for radius in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e30,
            1e30,
            41.0,
        ] {
            for ts in transforms() {
                for alpha in [255, 128] {
                    for mode in [Draw::Fill, Draw::Gradient, Draw::Clip] {
                        if catch_unwind(AssertUnwindSafe(|| {
                            draw(&shape(radius, corners), ts, alpha, mode)
                        }))
                        .is_err()
                        {
                            panics
                                .push(format!("{corners:?}, {radius}, {ts:?}, {alpha}, {mode:?}"));
                        }
                    }
                }
            }
        }
    }
    assert!(panics.is_empty(), "panicked: {panics:#?}");
}

#[test]
fn malformed_radii_paint_like_sanitized_css_radii() {
    for corners in [None, Some(CornerShape::check("squircle").unwrap())] {
        for radius in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e30,
            1e30,
            41.0,
        ] {
            let input = shape(radius, corners);
            let sanitized = shape(
                if radius.is_finite() {
                    radius.max(0.0)
                } else {
                    0.0
                },
                corners,
            );
            let expected = Shape {
                radii: border::reduced(sanitized.radii, 80.0, 60.0),
                ..sanitized
            };
            for ts in transforms() {
                for alpha in [255, 128] {
                    for mode in [Draw::Fill, Draw::Gradient, Draw::Clip] {
                        let actual = draw(&input, ts, alpha, mode);
                        let reference = draw(&expected, ts, alpha, mode);
                        assert!(reference.data().chunks_exact(4).any(|p| p != [255; 4]));
                        assert!(
                            actual.data() == reference.data(),
                            "{corners:?}, {radius}, {ts:?}, {alpha}, {mode:?}"
                        );
                    }
                }
            }
        }
        // Half-box limits belong to the interior-fill optimizations, not CSS.
        let valid = shape(41.0, corners);
        assert_eq!(border::reduced(valid.radii, 80.0, 60.0), valid.radii);
        let mut original = Vec::new();
        border::shape_path(&mut original, &valid);
        assert_eq!(rounded_rect(&valid), tiny_path(&original));
    }
}

#[test]
fn unsafe_border_regions_and_clips_are_not_rasterized() {
    let mut panics = Vec::new();
    for corners in [None, Some(CornerShape::check("squircle").unwrap())] {
        for radius in [-1e30, 1e30] {
            let mut unsafe_path = Vec::new();
            border::shape_path(&mut unsafe_path, &shape(radius, corners));
            let mut safe_path = Vec::new();
            border::shape_path(&mut safe_path, &shape(4.0, corners));
            for ts in transforms() {
                for alpha in [255, 128] {
                    for field in 0..3 {
                        let part = BorderFill {
                            region: if field == 0 {
                                unsafe_path.clone()
                            } else {
                                safe_path.clone()
                            },
                            clip: (field == 1).then(|| unsafe_path.clone()),
                            ring_clip: (field == 2).then(|| unsafe_path.clone()),
                            color: [10, 20, 30, alpha],
                            ring: None,
                        };
                        let result = catch_unwind(AssertUnwindSafe(|| {
                            let mut raster = Raster::new();
                            raster.begin(96.0, 80.0, 0.75);
                            raster.fill_border(&part, ts);
                            raster.finish().unwrap()
                        }));
                        match result {
                            Ok(pixels) => {
                                // The squircle outline already makes a negative radius square.
                                let safe = corners.is_some() && radius < 0.0;
                                assert_eq!(
                                    pixels.data().chunks_exact(4).any(|p| p != [255; 4]),
                                    safe,
                                    "{corners:?}, {radius}, {ts:?}, {alpha}, field {field}"
                                );
                            }
                            Err(_) => panics.push(format!(
                                "{corners:?}, {radius}, {ts:?}, {alpha}, field {field}"
                            )),
                        }
                    }
                }
            }
        }
    }
    assert!(panics.is_empty(), "panicked: {panics:#?}");
}

#[test]
fn constructor_radii_preserve_original_paths_and_pixels() {
    let mut changed = Vec::new();
    for corners in [None, Some(CornerShape::check("squircle").unwrap())] {
        for shape in [
            Shape::new((0.0, 0.0, 80.0, 60.0), [51.0; 4]),
            Shape::elliptical((0.0, 0.0, 60.0, 80.0), [(51.0, 51.0); 4]),
            Shape::new((0.0, 0.0, 82.0, 62.0), [52.0; 4]).inset(1.0),
        ] {
            let shape = shape.with_corners(corners);
            let mut ops = Vec::new();
            border::shape_path(&mut ops, &shape);
            let original = tiny_path(&ops).unwrap();
            if rounded_rect(&shape).as_ref() != Some(&original) {
                changed.push(format!("path: {shape:?}"));
            }
            for ts in transforms() {
                for alpha in [255, 128] {
                    let mut reference = Pixmap::new(72, 60).unwrap();
                    reference.fill(Color::WHITE);
                    reference.fill_path(
                        &original,
                        &solid([10, 20, 30, alpha]),
                        FillRule::Winding,
                        Transform::from_scale(0.75, 0.75).pre_concat(ts),
                        None,
                    );
                    if draw(&shape, ts, alpha, Draw::Fill).data() != reference.data() {
                        changed.push(format!("pixels: {shape:?}, {ts:?}, {alpha}"));
                    }
                }
            }
        }
    }
    for edge in [f32::from_bits(3), f32::MIN_POSITIVE, 0.001, 60.0, 8192.0] {
        for radius in [edge, edge * 1.7, 51.0, 100.0, 1e30] {
            let shape = Shape::new((0.0, 0.0, edge, edge), [radius; 4]);
            if shape.rounded() {
                let mut ops = Vec::new();
                border::shape_path(&mut ops, &shape);
                assert_eq!(rounded_rect(&shape), tiny_path(&ops), "{shape:?}");
            }
        }
    }
    assert!(
        changed.is_empty(),
        "changed original geometry: {changed:#?}"
    );
}

#[test]
fn large_covering_boxes_preserve_original_pixels() {
    let mut changed = Vec::new();
    for rect in [
        (0.0, 0.0, 1_500_000_000.0, 40.0),
        (-1_500_000_000.0, 0.0, 1_500_000_128.0, 40.0),
        (0.0, 0.0, 40.0, 1_500_000_000.0),
        (0.0, -1_500_000_000.0, 40.0, 1_500_000_128.0),
    ] {
        let shape = Shape::rect(rect);
        let mut ops = Vec::new();
        border::shape_path(&mut ops, &shape);
        let path = tiny_path(&ops).unwrap();
        for ts in transforms() {
            for alpha in [255, 128] {
                let color = [10, 20, 30, alpha];
                let mut reference = Pixmap::new(200, 100).unwrap();
                reference.fill(Color::WHITE);
                reference.fill_path(&path, &solid(color), FillRule::Winding, ts, None);
                assert!(reference.data().chunks_exact(4).any(|p| p != [255; 4]));
                for mode in 0..5 {
                    let mut raster = Raster::new();
                    raster.begin(200.0, 100.0, 1.0);
                    match mode {
                        0 => raster.fill(&shape, color, ts),
                        1 => raster.fill_gradient(
                            &shape,
                            &GradientPaint {
                                geometry: Geometry::Linear {
                                    start: (0.0, 0.0),
                                    end: (200.0, 100.0),
                                },
                                repeat: false,
                                stops: vec![
                                    (0.0, exact_kernel::Color::rgba(10, 20, 30, alpha)),
                                    (1.0, exact_kernel::Color::rgba(10, 20, 30, alpha)),
                                ],
                            },
                            ts,
                        ),
                        _ => raster.fill_border(
                            &BorderFill {
                                region: ops.clone(),
                                clip: (mode == 3).then(|| ops.clone()),
                                ring_clip: (mode == 4).then(|| ops.clone()),
                                color,
                                ring: None,
                            },
                            ts,
                        ),
                    }
                    if raster.finish().unwrap().data() != reference.data() {
                        changed.push(format!("{rect:?}, {ts:?}, {alpha}, mode {mode}"));
                    }
                }
            }
        }
    }
    assert!(
        changed.is_empty(),
        "changed original coverage: {changed:#?}"
    );
}

#[test]
fn border_scanline_overflow_is_not_rasterized() {
    let mut unsafe_path = Vec::new();
    border::shape_path(
        &mut unsafe_path,
        &Shape::rect((0.0, -1_700_000_000.0, 40.0, 1_700_000_128.0)),
    );
    let mut safe_path = Vec::new();
    border::shape_path(&mut safe_path, &Shape::rect((0.0, 0.0, 40.0, 80.0)));
    for field in 0..3 {
        let mut raster = Raster::new();
        raster.begin(200.0, 100.0, 1.0);
        raster.fill_border(
            &BorderFill {
                region: if field == 0 {
                    unsafe_path.clone()
                } else {
                    safe_path.clone()
                },
                clip: (field == 1).then(|| unsafe_path.clone()),
                ring_clip: (field == 2).then(|| unsafe_path.clone()),
                color: [10, 20, 30, 255],
                ring: None,
            },
            Transform::identity(),
        );
        assert!(raster.finish().unwrap().data().iter().all(|v| *v == 255));
    }
}
