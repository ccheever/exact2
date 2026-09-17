use super::{geometry::*, *};
use exact_game::{Quat, Transform, Vec3};
use rapier3d::{
    math::{Pose, Rotation, Vector},
    parry::{query, shape::SharedShape},
};
fn oracle(shape: &Shape) -> SharedShape {
    match *shape {
        Shape::Sphere { radius } => SharedShape::ball(radius),
        Shape::Capsule { radius, height } => SharedShape::capsule_y(height * 0.5 - radius, radius),
        Shape::Box { half } => SharedShape::cuboid(half.x, half.y, half.z),
    }
}
fn pose(p: Transform) -> Pose {
    Pose::from_parts(
        Vector::from_array(p.position.to_array()),
        Rotation::from_array(p.rotation.to_array()),
    )
}
fn rotation(a: f32) -> Quat {
    Quat::from_xyzw(
        0.0,
        exact_game::math::sin(a * 0.5),
        0.0,
        exact_game::math::cos(a * 0.5),
    )
}
fn compare(a: &Shape, pa: Transform, b: &Shape, pb: Transform) {
    let got = collide(Geometry::new(a, pa), Geometry::new(b, pb));
    let sa = oracle(a);
    let sb = oracle(b);
    let expected = query::contact(&pose(pa), &*sa, &pose(pb), &*sb, 20.0)
        .unwrap()
        .unwrap();
    // GJK/EPA is approximate for curved pairs. Sample the center segment
    // independently; refine around the best sample to verify the normal too.
    if let (
        Geometry::Round {
            a: p,
            b: q,
            radius: r,
        },
        Geometry::Round {
            a: x,
            b: y,
            radius: s,
        },
    ) = (Geometry::new(a, pa), Geometry::new(b, pb))
    {
        let (p, q, x, y) = (p.as_dvec3(), q.as_dvec3(), x.as_dvec3(), y.as_dvec3());
        let mut best = (f64::INFINITY, glam::DVec3::ZERO);
        for t in 0..=100000 {
            let u = p + (q - p) * (t as f64 / 100000.0);
            let d = y - x;
            let s = if d.length_squared() > 0.0 {
                ((u - x).dot(d) / d.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let v = x + d * s - u;
            if v.length_squared() < best.0 {
                best = (v.length_squared(), v);
            }
        }
        let distance = best.0.sqrt() as f32;
        assert!((got.separation() - (distance - r - s)).abs() < 1e-4);
        if distance > 0.01 {
            assert!(
                got.normal.distance(best.1.as_vec3() / distance) < 1e-4,
                "round normal {:?} sampled {:?}",
                got.normal,
                best.1 / distance as f64
            );
        }
        return;
    }
    let ga = Geometry::new(a, pa);
    let gb = Geometry::new(b, pb);
    let round_box = match (ga, gb) {
        (r @ Geometry::Round { .. }, b @ Geometry::Box { .. }) => Some((r, b, 1.0)),
        (b @ Geometry::Box { .. }, r @ Geometry::Round { .. }) => Some((r, b, -1.0)),
        _ => None,
    };
    if let Some((
        Geometry::Round { a: p, b: q, radius },
        Geometry::Box { center, axes, half },
        sign,
    )) = round_box
    {
        if got.separation() > 0.0 {
            let (p, q, center, half) = (
                p.as_dvec3(),
                q.as_dvec3(),
                center.as_dvec3(),
                half.as_dvec3(),
            );
            let axes = axes.map(|a| a.as_dvec3());
            let mut best = (f64::INFINITY, glam::DVec3::ZERO);
            for t in 0..=100000 {
                let u = p + (q - p) * (t as f64 / 100000.0);
                let d = u - center;
                let v = center
                    + axes[0] * d.dot(axes[0]).clamp(-half.x, half.x)
                    + axes[1] * d.dot(axes[1]).clamp(-half.y, half.y)
                    + axes[2] * d.dot(axes[2]).clamp(-half.z, half.z)
                    - u;
                if v.length_squared() < best.0 {
                    best = (v.length_squared(), v);
                }
            }
            let distance = best.0.sqrt() as f32;
            assert!((got.separation() - (distance - radius)).abs() < 1e-4);
            assert!(
                got.normal.distance(best.1.as_vec3() * (sign / distance)) < 1e-4,
                "round-box normal {:?} sampled {:?}",
                got.normal,
                best.1 / distance as f64
            );
            return;
        }
    }
    assert!(
        (got.separation() - expected.dist).abs() < 1e-4,
        "{a:?} {b:?} pa={pa:?} pb={pb:?}: got {} expected {}",
        got.separation(),
        expected.dist
    );
    let normal = Vec3::from_array(expected.normal1.to_array());
    // At coincident centers the separating direction is not unique.
    if (pb.position - pa.position).length_squared() > 1e-6 && expected.dist > 0.0001 {
        assert!(
            got.normal.distance(normal) < 1e-4,
            "normal {a:?} {b:?}: {:?} != {:?}",
            got.normal,
            normal
        );
    }
    assert!(got.points.len() <= 4);
    for p in &got.points {
        assert!(((p.b - p.a).dot(got.normal) - p.separation).abs() < 1e-5);
    }
}
#[test]
fn all_six_pairs_against_independent_geometry() {
    let shapes = [
        Shape::Sphere { radius: 0.4 },
        Shape::Capsule {
            radius: 0.3,
            height: 1.8,
        },
        Shape::Box {
            half: Vec3::new(0.5, 0.7, 0.6),
        },
    ];
    for a in 0..3 {
        for b in a..3 {
            for i in 0..30 {
                let pa = Transform {
                    rotation: rotation(i as f32 * 0.13),
                    ..Transform::default()
                };
                let pb = Transform {
                    position: Vec3::new(i as f32 * 0.077, 0.17 + (i % 5) as f32 * 0.13, 0.23),
                    rotation: rotation(i as f32 * -0.17),
                    ..Transform::default()
                };
                compare(&shapes[a], pa, &shapes[b], pb);
                compare(&shapes[b], pb, &shapes[a], pa);
            }
        }
    }
}
#[test]
fn degenerate_alignments_and_inside_sphere() {
    let capsule = Shape::Capsule {
        radius: 0.3,
        height: 2.0,
    };
    let cube = Shape::Box { half: Vec3::ONE };
    for p in [
        Vec3::ZERO,
        Vec3::new(0.5, 0.0, 0.0),
        Vec3::new(0.6, 1.0, 0.0),
        Vec3::new(0.0, 2.0, 0.0),
    ] {
        let t = Transform {
            position: p,
            ..Transform::default()
        };
        compare(&capsule, Transform::default(), &capsule, t);
        compare(&cube, Transform::default(), &cube, t);
    }
    let patch = collide(
        Geometry::new(&Shape::Sphere { radius: 0.2 }, Transform::at(0.7, 0.0, 0.0)),
        Geometry::new(&cube, Transform::default()),
    );
    assert!((patch.separation() + 0.5).abs() < 1e-6);
    assert_eq!(patch.normal, -Vec3::X);
    compare(
        &Shape::Sphere { radius: 0.2 },
        Transform::default(),
        &cube,
        Transform::default(),
    );
}
#[test]
fn segment_closest_points_against_dense_samples() {
    for i in 0..12 {
        let a = Vec3::new(-0.8, 0.1, 0.0);
        let b = Vec3::new(1.2, 0.9, 0.1);
        let c = Vec3::new(0.2, i as f32 * 0.2, -0.6);
        let d = c + Vec3::new(0.0, 0.2, 1.3);
        let (x, y, _, _) = segments(a, b, c, d);
        let distance = x.distance(y);
        let mut sampled = f32::INFINITY;
        for t in 0..=10000 {
            let p = a + (b - a) * (t as f32 / 10000.0);
            sampled = sampled.min(p.distance(segment(c, d, p).0));
        }
        assert!((distance - sampled).abs() < 1e-4);
    }
}
#[test]
fn clipped_face_has_four_stable_features() {
    let s = Shape::Box {
        half: Vec3::splat(0.5),
    };
    let a = Geometry::new(&s, Transform::default());
    let b = Geometry::new(&s, Transform::at(0.0, 0.99, 0.0));
    let patch = collide(a, b);
    assert_eq!(patch.points.len(), 4);
    let next = collide(a, Geometry::new(&s, Transform::at(0.0, 0.98, 0.0)));
    assert_eq!(
        patch.points.iter().map(|p| p.feature).collect::<Vec<_>>(),
        next.points.iter().map(|p| p.feature).collect::<Vec<_>>()
    );
}
