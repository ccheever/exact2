use super::*;
use crate::paint::border;
use exact_kernel::corner::CornerShape;
use std::panic::{catch_unwind, AssertUnwindSafe};

// The original shape path, without passing through radius normalization.
fn original_shape_path(shape: &Shape) -> Vec<PathOp> {
    let mut ops = Vec::new();
    if let Some(corners) = shape
        .corners
        .as_ref()
        .filter(|c| !c.is_round() && shape.rounded())
    {
        for seg in exact_kernel::corner::outline(shape.rect, shape.radii, corners).0 {
            ops.push(match seg {
                exact_kernel::svg::Seg::Move(x, y) => PathOp::Move(x, y),
                exact_kernel::svg::Seg::Line(x, y) => PathOp::Line(x, y),
                exact_kernel::svg::Seg::Cubic(a, b, c, d, e, f) => PathOp::Cubic(a, b, c, d, e, f),
                exact_kernel::svg::Seg::Close => PathOp::Close,
            });
        }
    } else {
        border::rounded_rect(&mut ops, shape.rect, shape.radii);
    }
    ops
}

fn draw_border(part: &BorderFill, ts: Transform) -> Pixmap {
    let mut raster = Raster::new();
    raster.begin(96.0, 80.0, 0.75);
    raster.fill_border(part, ts);
    raster.finish().unwrap()
}

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
        let original = original_shape_path(&valid);
        assert_eq!(rounded_rect(&valid), tiny_path(&original));
    }
}

#[test]
fn malformed_border_radii_paint_like_sanitized_css_radii() {
    let mut changed = Vec::new();
    for corners in [None, Some(CornerShape::check("squircle").unwrap())] {
        for radius in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e30,
            1e30,
            41.0,
        ] {
            let mut actual_ops = Vec::new();
            border::shape_path(&mut actual_ops, &shape(radius, corners));
            let sanitized = shape(
                if radius.is_finite() {
                    radius.max(0.0)
                } else {
                    0.0
                },
                corners,
            );
            let reference_ops = original_shape_path(&Shape {
                radii: border::reduced(sanitized.radii, 80.0, 60.0),
                ..sanitized
            });
            let safe_ops = original_shape_path(&shape(4.0, corners));
            for ts in transforms() {
                for alpha in [255, 128] {
                    for field in 0..3 {
                        let part = |ops: &Vec<PathOp>| BorderFill {
                            region: if field == 0 {
                                ops.clone()
                            } else {
                                safe_ops.clone()
                            },
                            clip: (field == 1).then(|| ops.clone()),
                            ring_clip: (field == 2).then(|| ops.clone()),
                            color: [10, 20, 30, alpha],
                            ring: None,
                        };
                        let reference = draw_border(&part(&reference_ops), ts);
                        assert!(reference.data().chunks_exact(4).any(|p| p != [255; 4]));
                        if draw_border(&part(&actual_ops), ts).data() != reference.data() {
                            changed.push(format!(
                                "{corners:?}, {radius}, {ts:?}, {alpha}, field {field}"
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        changed.is_empty(),
        "incorrect border geometry: {changed:#?}"
    );
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
            let ops = original_shape_path(&shape);
            let mut actual_ops = Vec::new();
            border::shape_path(&mut actual_ops, &shape);
            assert_eq!(actual_ops, ops, "{shape:?}");
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
                let ops = original_shape_path(&shape);
                let mut actual_ops = Vec::new();
                border::shape_path(&mut actual_ops, &shape);
                assert_eq!(actual_ops, ops, "{shape:?}");
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
fn sheared_border_regions_and_clips_preserve_original_pixels() {
    let ops = vec![
        PathOp::Move(0.0, 0.0),
        PathOp::Line(600_000_000.0, 600_000_000.0),
        PathOp::Line(600_000_000.0, 600_000_064.0),
        PathOp::Line(0.0, 64.0),
        PathOp::Close,
    ];
    let ts = Transform::from_row(1.0, -1.0, 0.0, 1.0, 0.0, 0.0);
    let path = tiny_path(&ops).unwrap();
    let mut changed = Vec::new();
    for alpha in [255, 128] {
        let color = [10, 20, 30, alpha];
        let mut reference = Pixmap::new(200, 100).unwrap();
        reference.fill(Color::WHITE);
        reference.fill_path(&path, &solid(color), FillRule::EvenOdd, ts, None);
        assert!(reference.data()[..200 * 64 * 4]
            .chunks_exact(4)
            .all(|p| p != [255; 4]));
        assert!(reference.data()[200 * 64 * 4..].iter().all(|v| *v == 255));
        for field in 0..3 {
            let mut raster = Raster::new();
            raster.begin(200.0, 100.0, 1.0);
            raster.fill_border(
                &BorderFill {
                    region: ops.clone(),
                    clip: (field == 1).then(|| ops.clone()),
                    ring_clip: (field == 2).then(|| ops.clone()),
                    color,
                    ring: None,
                },
                ts,
            );
            if raster.finish().unwrap().data() != reference.data() {
                changed.push((alpha, field));
            }
        }
    }
    assert!(changed.is_empty(), "lost shear coverage: {changed:?}");
}
