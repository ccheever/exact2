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
