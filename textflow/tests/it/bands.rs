use exact_textflow::{intervals, FlowShape};
use std::sync::Arc;

fn rect(x: f32, y: f32, width: f32, height: f32) -> FlowShape {
    FlowShape::RoundRect {
        x,
        y,
        width,
        height,
        radius: 0.0,
    }
}
fn band(shapes: &[FlowShape], top: f32, bottom: f32, width: f32, min: f32) -> Vec<(f32, f32)> {
    let mut out = vec![(f32::NAN, f32::NAN)];
    intervals(shapes, top, bottom, width, min, &mut out);
    out
}
#[test]
fn circle_widest_extent_22_point_bands_and_sliver() {
    let circle = [FlowShape::Circle {
        cx: 320.0,
        cy: 110.0,
        r: 44.0,
    }];
    assert_eq!(band(&circle, 0.0, 22.0, 640.0, 0.0), [(0.0, 640.0)]);
    assert_eq!(band(&circle, 154.0, 176.0, 640.0, 0.0), [(0.0, 640.0)]);
    assert_eq!(
        band(&circle, 110.0, 132.0, 640.0, 0.0),
        [(0.0, 276.0), (364.0, 640.0)]
    );
    let near = band(&circle, 66.0, 88.0, 640.0, 0.0);
    assert!((near[0].1 - (320.0 - (44.0_f32.powi(2) - 22.0_f32.powi(2)).sqrt())).abs() < 0.0001);
    assert!(near[0].1 > 276.0);
    assert_eq!(
        band(&[rect(5.0, 0.0, 20.0, 100.0)], 0.0, 22.0, 100.0, 6.0),
        [(25.0, 100.0)]
    );
}
#[test]
fn overlapping_offscreen_and_full_width() {
    assert!(band(&[rect(0.0, 0.0, 640.0, 100.0)], 0.0, 22.0, 640.0, 0.0).is_empty());
    assert_eq!(
        band(
            &[rect(40.0, 0.0, 30.0, 30.0), rect(20.0, 0.0, 30.0, 30.0)],
            0.0,
            22.0,
            100.0,
            0.0
        ),
        [(0.0, 20.0), (70.0, 100.0)]
    );
    assert_eq!(
        band(&[rect(-30.0, 0.0, 40.0, 30.0)], 0.0, 22.0, 100.0, 0.0),
        [(10.0, 100.0)]
    );
    assert_eq!(
        band(
            &[rect(-50.0, 0.0, 40.0, 30.0), rect(120.0, 0.0, 40.0, 30.0)],
            0.0,
            22.0,
            100.0,
            0.0
        ),
        [(0.0, 100.0)]
    );
    for w in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(band(&[], 0.0, 22.0, w, 0.0).is_empty());
    }
    assert!(band(&[], f32::NAN, 22.0, 100.0, 0.0).is_empty());
    assert!(band(&[], 22.0, 22.0, 100.0, 0.0).is_empty());
}
#[test]
fn polygon_triangle_and_concave_c() {
    let triangle = FlowShape::Polygon(Arc::from([(50.0, 0.0), (100.0, 100.0), (0.0, 100.0)]));
    assert_eq!(
        band(std::slice::from_ref(&triangle), 0.0, 20.0, 120.0, 0.0),
        [(0.0, 40.0), (60.0, 120.0)]
    );
    assert_eq!(band(&[triangle], 80.0, 100.0, 120.0, 0.0), [(100.0, 120.0)]);
    let c = FlowShape::Polygon(Arc::from([
        (20.0, 0.0),
        (100.0, 0.0),
        (100.0, 20.0),
        (40.0, 20.0),
        (40.0, 80.0),
        (100.0, 80.0),
        (100.0, 100.0),
        (20.0, 100.0),
    ]));
    assert_eq!(
        band(std::slice::from_ref(&c), 30.0, 50.0, 120.0, 0.0),
        [(0.0, 20.0), (40.0, 120.0)]
    );
    assert_eq!(
        band(std::slice::from_ref(&c), 10.0, 30.0, 120.0, 0.0),
        [(0.0, 20.0), (100.0, 120.0)]
    );
    assert_eq!(
        band(&[c], 20.0, 80.0, 120.0, 0.0),
        [(0.0, 20.0), (40.0, 120.0)]
    );
}
#[test]
fn silhouette_rows_are_half_open_and_empty_rows_stay_empty() {
    let shape = [FlowShape::Spans {
        x: 10.0,
        y: 20.0,
        row_height: 10.0,
        rows: Arc::from([(0.0, 0.0), (2.0, 20.0), (8.0, 40.0), (0.0, 0.0)]),
    }];
    assert_eq!(band(&shape, 20.0, 30.0, 100.0, 0.0), [(0.0, 100.0)]);
    assert_eq!(
        band(&shape, 30.0, 40.0, 100.0, 0.0),
        [(0.0, 12.0), (30.0, 100.0)]
    );
    assert_eq!(
        band(&shape, 39.0, 41.0, 100.0, 0.0),
        [(0.0, 12.0), (50.0, 100.0)]
    );
    assert_eq!(band(&shape, 50.0, 60.0, 100.0, 0.0), [(0.0, 100.0)]);
}

// Independent point-in-shape oracle: it never calls the production band geometry.
fn contains(s: &FlowShape, x: f32, y: f32) -> bool {
    match s {
        FlowShape::Circle { cx, cy, r } => (x - cx).powi(2) + (y - cy).powi(2) < r * r,
        FlowShape::Ellipse { cx, cy, rx, ry } => {
            ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) < 1.0
        }
        FlowShape::RoundRect {
            x: a,
            y: b,
            width,
            height,
            radius,
        } => {
            if x < *a || x >= a + width || y < *b || y >= b + height {
                return false;
            }
            let r = radius.min(width.min(*height) / 2.0);
            let cx = x.clamp(a + r, a + width - r);
            let cy = y.clamp(b + r, b + height - r);
            r == 0.0 || (x - cx).powi(2) + (y - cy).powi(2) < r * r
        }
        FlowShape::Polygon(p) | FlowShape::EvenOddPolygon(p) => {
            let mut winding = 0i32;
            for i in 0..p.len() {
                let (ax, ay) = p[i];
                let (bx, by) = p[(i + 1) % p.len()];
                if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
                    winding += if by > ay { 1 } else { -1 };
                }
            }
            if matches!(s, FlowShape::EvenOddPolygon(_)) {
                winding % 2 != 0
            } else {
                winding != 0
            }
        }
        FlowShape::Spans {
            x: a,
            y: b,
            row_height,
            rows,
        } => {
            let i = ((y - b) / row_height).floor();
            if i < 0.0 || i as usize >= rows.len() {
                return false;
            }
            let (l, r) = rows[i as usize];
            x >= a + l && x < a + r
        }
    }
}
#[test]
fn sampled_free_intervals_exclude_every_shape() {
    let shapes = [
        FlowShape::Circle {
            cx: 80.0,
            cy: 60.0,
            r: 30.0,
        },
        FlowShape::Ellipse {
            cx: 160.0,
            cy: 120.0,
            rx: 35.0,
            ry: 50.0,
        },
        FlowShape::RoundRect {
            x: 210.0,
            y: 30.0,
            width: 60.0,
            height: 120.0,
            radius: 20.0,
        },
        FlowShape::Polygon(Arc::from([(20.0, 140.0), (140.0, 160.0), (60.0, 200.0)])),
        FlowShape::Spans {
            x: -20.0,
            y: 0.0,
            row_height: 10.0,
            rows: Arc::from([(0.0, 40.0), (0.0, 0.0), (20.0, 55.0), (0.0, 30.0)]),
        },
    ];
    for count in 0..=shapes.len() {
        for top in (-30..240).step_by(7) {
            for height in [1.0, 22.0, 75.0] {
                for min in [0.0, 5.0, 30.0, 400.0] {
                    let intervals = band(
                        &shapes[..count],
                        top as f32,
                        top as f32 + height,
                        320.0,
                        min,
                    );
                    let mut previous = 0.0;
                    for (a, b) in intervals {
                        assert!(a >= previous && a >= 0.0 && a < b && b <= 320.0 && b - a >= min);
                        for xi in 0..9 {
                            for yi in 0..9 {
                                let x = a + (b - a) * (xi as f32 + 0.5) / 9.0;
                                let y = top as f32 + height * (yi as f32 + 0.5) / 9.0;
                                assert!(
                                    shapes[..count].iter().all(|s| !contains(s, x, y)),
                                    "point {x},{y}"
                                );
                            }
                        }
                        previous = b;
                    }
                }
            }
        }
    }
}
#[test]
fn growth_conservatively_contains_original_and_offsets() {
    let shapes = [
        FlowShape::Polygon(Arc::from([(20.0, 10.0), (80.0, 10.0), (40.0, 60.0)])),
        FlowShape::Spans {
            x: 10.0,
            y: 20.0,
            row_height: 5.0,
            rows: Arc::from([(0.0, 20.0), (0.0, 0.0), (15.0, 40.0)]),
        },
    ];
    for s in shapes {
        let grown = s.grow(3.0);
        for x in 0..100 {
            for y in 0..80 {
                if contains(&s, x as f32 + 0.2, y as f32 + 0.2) {
                    for dx in [-2.9, 0.0, 2.9] {
                        for dy in [-2.9, 0.0, 2.9] {
                            assert!(contains(&grown, x as f32 + 0.2 + dx, y as f32 + 0.2 + dy));
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn extreme_and_invalid_geometry_never_emits_nan() {
    for s in [
        FlowShape::Circle {
            cx: f32::MAX,
            cy: 0.0,
            r: f32::MAX,
        },
        FlowShape::Circle {
            cx: f32::NAN,
            cy: 0.0,
            r: 10.0,
        },
        rect(0.0, 0.0, -1.0, 10.0),
        FlowShape::Polygon(Arc::from([(0.0, 0.0)])),
        FlowShape::Spans {
            x: 0.0,
            y: 0.0,
            row_height: -1.0,
            rows: Arc::from([(0.0, 20.0)]),
        },
    ] {
        for v in [s.bounds().0, s.bounds().1, s.bounds().2, s.bounds().3] {
            assert!(v.is_finite());
        }
        for (a, b) in band(&[s], 0.0, 22.0, 640.0, 0.0) {
            assert!(a.is_finite() && b.is_finite());
        }
    }
}

#[test]
fn concave_polygon_keeps_its_center_gap_and_honors_fill_rule() {
    use exact_textflow::ShapeOutside;
    let shape = ShapeOutside::parse("polygon(0 0,20 0,20 80,80 80,80 0,100 0,100 100,0 100)")
        .unwrap()
        .resolve(100., 100.);
    let mut free = Vec::new();
    exact_textflow::intervals(&[shape], 40., 50., 100., 0., &mut free);
    assert_eq!(free, vec![(20., 80.)]);
    // Same square traced twice: nonzero fills it, evenodd cancels it, including
    // a band that crosses the whole shape rather than just its middle.
    for (rule, expected) in [("nonzero", vec![]), ("evenodd", vec![(0., 100.)])] {
        let shape = ShapeOutside::parse(&format!(
            "polygon({rule},0 0,100 0,100 100,0 100,0 0,100 0,100 100,0 100)"
        ))
        .unwrap();
        let roundtrip = ShapeOutside::parse(&shape.css()).unwrap();
        assert_eq!(shape, roundtrip);
        exact_textflow::intervals(&[shape.resolve(100., 100.)], -1., 101., 100., 0., &mut free);
        assert_eq!(free, expected, "{rule}");
    }
}

#[test]
fn sixty_four_crossing_vertices_match_independent_fill_oracle() {
    let points: Arc<[(f32, f32)]> = (0..64)
        .map(|i| {
            let angle = ((i * 13) % 64) as f32 * std::f32::consts::TAU / 64.;
            (100. + angle.cos() * 90., 100. + angle.sin() * 90.)
        })
        .collect();
    for shape in [
        FlowShape::Polygon(points.clone()),
        FlowShape::EvenOddPolygon(points),
    ] {
        for y in (0..200).step_by(10) {
            let free = band(
                std::slice::from_ref(&shape),
                y as f32,
                (y + 10) as f32,
                200.,
                0.,
            );
            for (a, b) in free {
                for x in a.ceil() as u32..b.floor() as u32 {
                    for dy in 0..10 {
                        assert!(!contains(&shape, x as f32 + 0.25, (y + dy) as f32 + 0.25));
                    }
                }
            }
        }
    }
}

#[test]
fn crossing_polygon_bands_preserve_vertices_and_open_edges_in_either_direction() {
    let points = [(20., 0.), (80., 100.), (20., 100.), (80., 0.)];
    for reverse in [false, true] {
        for rotation in 0..points.len() {
            let mut p = points.to_vec();
            p.rotate_left(rotation);
            if reverse {
                p.reverse();
            }
            for shape in [
                FlowShape::Polygon(p.clone().into()),
                FlowShape::EvenOddPolygon(p.clone().into()),
            ] {
                for (top, bottom, extent) in [
                    (-10., 0., None),
                    (0., 10., Some((20., 80.))),
                    (40., 50., Some((44., 56.))),
                    (50., 60., Some((44., 56.))),
                    (40., 60., Some((44., 56.))),
                    (90., 100., Some((20., 80.))),
                    (100., 110., None),
                ] {
                    let expected = extent.map_or_else(
                        || vec![(0., 100.)],
                        |(left, right)| vec![(0., left), (right, 100.)],
                    );
                    assert_eq!(
                        band(std::slice::from_ref(&shape), top, bottom, 100., 0.),
                        expected,
                        "reverse={reverse} rotation={rotation} band={top}..{bottom}"
                    );
                }
            }
        }
    }
}
