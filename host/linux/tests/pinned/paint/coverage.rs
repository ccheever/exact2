//! Backend-method fixtures for LLP 1149 D1.2, held to the whole-app band on each crop.
use super::*;
use exact_kernel::{gradient::Geometry, Color};
use exact_linux::paint::{Backend, GradientPaint, Ink as Paint, Shape, SvgPaint};
use std::sync::Arc;
use tiny_skia::Transform;

fn compare(name: &str, draw: impl Fn(&mut dyn Backend)) {
    if !ink_available() {
        return;
    }
    let mut cpu = exact_linux::raster::Raster::new();
    let mut ink = exact_linux::ink::Ink::new().unwrap();
    for scale in [1., 1.5] {
        for painter in [&mut cpu as &mut dyn Backend, &mut ink] {
            painter.page([37, 57, 79, 255]);
            painter.begin(128., 112., scale);
            draw(painter);
        }
        let a = cpu.finish().unwrap();
        let b = ink.finish().unwrap();
        let (mean, over) = band(&a, &b);
        eprintln!("{name} scale={scale}: mean {mean:.3}/255, {over:.3}% beyond 32");
        assert!(
            mean < 5. && over < 6.,
            "{name} scale={scale}: {mean}, {over}%"
        );
        assert!(
            a.data().chunks_exact(4).any(|p| p != [37, 57, 79, 255]),
            "fixture must draw"
        );
    }
}

fn gradient() -> GradientPaint {
    GradientPaint {
        geometry: Geometry::Linear {
            start: (8., 8.),
            end: (118., 100.),
        },
        repeat: false,
        stops: vec![
            (0., Color::rgba(230, 50, 20, 255)),
            (1., Color::rgba(10, 120, 240, 35)),
        ],
    }
}

#[test]
fn per_side_border_with_nonround_corners_matches_cpu() {
    compare("nonround border", |b| {
        let mut shape = Shape::new((7., 8., 110., 94.), [24., 18., 30., 12.]);
        shape.corners = exact_kernel::corner::CornerShape::parse("bevel scoop squircle notch");
        assert!(shape.corners.is_some());
        b.fill(&shape, [215, 210, 180, 255], Transform::identity());
        for part in exact_linux::paint::border::border_fills(
            &shape,
            [4., 9., 7., 6.],
            [
                [230, 40, 20, 255],
                [20, 220, 90, 170],
                [90, 50, 200, 220],
                [20, 170, 220, 255],
            ],
        ) {
            b.fill_border(&part, Transform::identity());
        }
    });
}

#[test]
fn coverage_clips_intersect_svg_unions_and_restore_the_parent() {
    use exact_kernel::svg::scene::{Clip, ClipShape};
    compare("coverage clips", |b| {
        let css = exact_kernel::clip::ClipPath::parse("path('M 8 7 L 122 21 Q 114 105 30 106 Z')")
            .unwrap();
        assert!(b.push_css_clip(&css, Transform::identity()));
        let clip = Clip {
            shapes: vec![
                ClipShape {
                    path: exact_kernel::svg::parse_d("M 10 15 H 80 V 98 H 10 Z"),
                    even_odd: false,
                },
                ClipShape {
                    path: exact_kernel::svg::parse_d("M 55 9 H 115 V 75 H 55 Z"),
                    even_odd: false,
                },
            ],
            then: Some(Box::new(Clip {
                shapes: vec![ClipShape {
                    path: exact_kernel::svg::parse_d(
                        "M 6 6 H 120 V 104 H 6 Z M 40 40 H 65 V 65 H 40 Z",
                    ),
                    even_odd: true,
                }],
                then: None,
            })),
        };
        let count = b.push_svg_clip(&clip, Transform::identity());
        assert_eq!(count, 1);
        let path = exact_kernel::svg::parse_d("M 0 0 H 128 V 112 H 0 Z");
        b.svg_path(
            &SvgPaint {
                path: &path,
                fill: Some(Paint::Solid([240, 180, 20, 255])),
                stroke: None,
                even_odd: false,
                order: [0, 1, 2],
                width: 0.,
                cap: 0,
                join: 0,
                miter: 4.,
                dash: vec![],
                phase: 0.,
            },
            Transform::identity(),
        );
        b.pop_clip();
        b.fill(
            &Shape::rect((75., 80., 50., 24.)),
            [50, 220, 140, 255],
            Transform::identity(),
        );
        b.pop_clip();
        b.fill(
            &Shape::rect((2., 2., 7., 7.)),
            [255; 4],
            Transform::identity(),
        );
    });
}

#[test]
fn gradient_mask_applies_once_to_overlapping_children() {
    compare("gradient mask", |b| {
        let shape = Shape::new((8., 8., 112., 96.), [19.; 4]);
        b.push_mask(&shape, Transform::identity());
        b.fill(
            &Shape::rect((0., 0., 128., 112.)),
            [220, 50, 25, 220],
            Transform::identity(),
        );
        b.fill(
            &Shape::rect((30., 20., 80., 80.)),
            [30, 230, 170, 240],
            Transform::identity(),
        );
        b.pop_mask(&shape, &Ok(gradient()), Transform::identity());
    });
}

#[test]
fn ordered_backdrop_blur_and_saturation_match_cpu() {
    use exact_kernel::style::{BackdropFilter, BackdropOp};
    compare("backdrop blur saturate", |b| {
        for n in 0..8 {
            b.fill(
                &Shape::rect((n as f32 * 16., 0., 16., 112.)),
                if n % 2 == 0 {
                    [230, 45, 120, 255]
                } else {
                    [30, 150, 210, 255]
                },
                Transform::identity(),
            );
        }
        b.backdrop_filter(
            &Shape::new((9., 8., 108., 96.), [17.; 4]),
            &BackdropFilter(vec![BackdropOp::Blur(4.), BackdropOp::Saturate(1.8)]),
            Transform::identity(),
        );
    });
}

#[test]
fn every_island_blend_matches_cpu_over_translucent_destination() {
    let mut source = Pixmap::new(64, 56).unwrap();
    for (n, p) in source.pixels_mut().iter_mut().enumerate() {
        *p = tiny_skia::Color::from_rgba8((n * 13) as u8, (n / 64 * 5) as u8, 170, 180)
            .premultiply()
            .to_color_u8();
    }
    let source = Arc::new(source);
    for mode in 1..=15 {
        compare(&format!("blend {mode}"), |b| {
            // An opacity root exercises the blend with a nonopaque destination.
            b.push_opacity(0.8);
            b.fill(
                &Shape::rect((7., 9., 113., 94.)),
                [70, 140, 230, 160],
                Transform::identity(),
            );
            b.island_image(
                source.clone(),
                (11., 13., 104., 88.),
                Transform::identity(),
                mode,
            );
            b.pop_opacity();
        });
    }
}

#[test]
fn svg_gradient_and_pattern_brushes_follow_stroked_and_filled_paths() {
    use exact_kernel::svg::server::{Server, ServerKind, Spread};
    let path = exact_kernel::svg::parse_d("M 8 10 H 112 V 96 H 15 Z M 35 38 H 75 V 70 H 35 Z");
    let mut tile = Pixmap::new(12, 10).unwrap();
    for (n, p) in tile.pixels_mut().iter_mut().enumerate() {
        *p = tiny_skia::Color::from_rgba8(if n % 12 < 6 { 220 } else { 30 }, 160, 80, 220)
            .premultiply()
            .to_color_u8();
    }
    let tile = Arc::new(tile);
    for (name, kind) in [
        (
            "linear",
            ServerKind::Linear {
                x1: 0.,
                y1: 0.,
                x2: 40.,
                y2: 20.,
            },
        ),
        (
            "radial",
            ServerKind::Radial {
                cx: 60.,
                cy: 55.,
                r: 55.,
                fx: 35.,
                fy: 40.,
                fr: 5.,
            },
        ),
    ] {
        let server = Server {
            kind,
            stops: vec![],
            spread: Spread::Reflect,
            transform: [1., 0., 0., 1., 0., 0.],
        };
        compare(&format!("SVG {name} and pattern"), |b| {
            b.svg_path(
                &SvgPaint {
                    path: &path,
                    fill: Some(Paint::Gradient {
                        server: &server,
                        stops: vec![(0., [240, 50, 20, 255]), (1., [10, 100, 230, 180])],
                        transform: [1., 0.1, 0.2, 1., -8., -5.],
                    }),
                    stroke: Some(Paint::Pattern {
                        tile: tile.clone(),
                        transform: [1., 0.15, -0.2, 1., 0., 0.],
                        opacity: 0.8,
                    }),
                    even_odd: true,
                    order: [1, 0, 2],
                    width: 7.,
                    cap: 1,
                    join: 1,
                    miter: 4.,
                    dash: vec![9., 4.],
                    phase: 2.5,
                },
                Transform::identity(),
            );
        });
    }
}

#[test]
fn nested_coverage_clips_keep_the_backdrop_and_blend_destination() {
    use exact_kernel::style::{BackdropFilter, BackdropOp};
    let mut pixels = Pixmap::new(1, 1).unwrap();
    pixels.fill(tiny_skia::Color::from_rgba8(200, 80, 120, 170));
    let pixels = Arc::new(pixels);
    compare("clipped backdrop and blend", |b| {
        b.push_opacity(0.75);
        for n in 0..8 {
            b.fill(
                &Shape::rect((n as f32 * 16., 0., 16., 112.)),
                if n % 2 == 0 {
                    [230, 45, 120, 160]
                } else {
                    [30, 150, 210, 200]
                },
                Transform::identity(),
            );
        }
        let a = exact_kernel::clip::ClipPath::parse("path('M 4 3 H 120 V 105 H 4 Z')").unwrap();
        let c = exact_kernel::clip::ClipPath::parse("path('M 12 7 L 114 16 L 100 102 L 6 94 Z')")
            .unwrap();
        assert!(b.push_css_clip(&a, Transform::identity()));
        assert!(b.push_css_clip(&c, Transform::identity()));
        b.backdrop_filter(
            &Shape::new((8., 8., 110., 94.), [17.; 4]),
            &BackdropFilter(vec![BackdropOp::Blur(5.), BackdropOp::Saturate(0.4)]),
            Transform::identity(),
        );
        b.island_image(
            pixels.clone(),
            (25., 30., 70., 50.),
            Transform::identity(),
            3,
        );
        b.pop_clip();
        b.pop_clip();
        b.pop_opacity();
    });
}

#[test]
fn an_unchanged_app_has_no_atlas_misses() {
    if !ink_available() {
        return;
    }
    let mut app = boot(PainterChoice::Cpu);
    let ink = exact_linux::ink::Ink::new().unwrap();
    let counters = ink.statistics();
    app.reader_replaced(Box::new(ink));
    app.frame();
    let first = counters.read();
    assert!(first.glyphs.entries > 0);
    assert_eq!(first.glyphs.overflow_bytes + first.paths.overflow_bytes, 0);
    app.repaint();
    app.frame();
    let warm = counters.read();
    assert_eq!(warm.glyphs.entries, first.glyphs.entries);
    assert_eq!(warm.paths.entries, first.paths.entries);
    assert_eq!(warm.glyphs.misses + warm.paths.misses, 0);
    eprintln!("Caltrain first 390x844: {first:?}");
    eprintln!("Caltrain warm 390x844: {warm:?}");
}
