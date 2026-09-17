//! Oracle-free geometry properties retained across the executor replacement.
use crate::{math, Body, Collider, Physics, Shape};
use exact_game::{Quat, Transform, Vec3, World};
use rapier3d::parry::query;
fn rotation(a: f32) -> Quat {
    Quat::from_xyzw(
        0.0,
        exact_game::math::sin(a * 0.5),
        0.0,
        exact_game::math::cos(a * 0.5),
    )
}
fn segment(shape: &Shape, pose: Transform) -> Option<(glam::DVec3, glam::DVec3, f32)> {
    let (r, h) = match shape {
        Shape::Sphere { radius } => (*radius, 0.0),
        Shape::Capsule { radius, height } => (*radius, height * 0.5 - radius),
        _ => return None,
    };
    Some((
        (pose.position - pose.rotation * Vec3::Y * h).as_dvec3(),
        (pose.position + pose.rotation * Vec3::Y * h).as_dvec3(),
        r,
    ))
}
fn compare(a: &Shape, pa: Transform, b: &Shape, pb: Transform) {
    let (sa, sb) = (math::shape(a, pa.scale), math::shape(b, pb.scale));
    let got = query::contact(&math::pose(pa), &*sa, &math::pose(pb), &*sb, 20.0)
        .unwrap()
        .unwrap();
    assert!(((got.point2 - got.point1).dot(got.normal1) - got.dist).abs() < 1e-5);
    if let (Some((p, q, r)), Some((x, y, s))) = (segment(a, pa), segment(b, pb)) {
        let mut best = (f64::INFINITY, glam::DVec3::ZERO);
        for t in 0..=100000 {
            let u = p + (q - p) * (t as f64 / 100000.0);
            let d = y - x;
            let t = if d.length_squared() > 0.0 {
                ((u - x).dot(d) / d.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let v = x + d * t - u;
            if v.length_squared() < best.0 {
                best = (v.length_squared(), v);
            }
        }
        let distance = best.0.sqrt() as f32;
        assert!(
            (got.dist - (distance - r - s)).abs() < 1e-4,
            "round distance {} != {}",
            got.dist,
            distance - r - s
        );
        if distance > 0.01 {
            assert!(
                got.normal1.distance(best.1.as_vec3() / distance) < 1e-4,
                "round normal {:?} != {:?}",
                got.normal1,
                best.1 / distance as f64
            );
        }
    } else if got.dist > 0.0 {
        let pair = match (segment(a, pa), segment(b, pb), a, b) {
            (Some(round), None, _, Shape::Box { half }) => Some((round, pb, *half, 1.0)),
            (None, Some(round), Shape::Box { half }, _) => Some((round, pa, *half, -1.0)),
            _ => None,
        };
        if let Some(((p, q, r), t, half, sign)) = pair {
            let center = t.position.as_dvec3();
            let half = half.as_dvec3();
            let axes = [
                t.rotation * Vec3::X,
                t.rotation * Vec3::Y,
                t.rotation * Vec3::Z,
            ]
            .map(|v| v.as_dvec3());
            let mut best = (f64::INFINITY, glam::DVec3::ZERO);
            for i in 0..=100000 {
                let u = p + (q - p) * (i as f64 / 100000.0);
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
            assert!((got.dist - (distance - r)).abs() < 1e-4);
            assert!(got.normal1.distance(best.1.as_vec3() * (sign / distance)) < 1e-4);
        }
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
    let sphere = math::shape(&Shape::Sphere { radius: 0.2 }, Vec3::ONE);
    let cube = math::shape(&cube, Vec3::ONE);
    let c = query::contact(
        &math::pose(Transform::at(0.7, 0.0, 0.0)),
        &*sphere,
        &math::pose(Transform::default()),
        &*cube,
        20.0,
    )
    .unwrap()
    .unwrap();
    assert!((c.dist + 0.5).abs() < 1e-6);
    assert_eq!(c.normal1, -Vec3::X);
}
#[test]
fn segment_closest_points_against_dense_samples() {
    use rapier3d::{
        math::Pose,
        parry::{query::ClosestPoints, shape::Segment},
    };
    for i in 0..12 {
        let a = Vec3::new(-0.8, 0.1, 0.0);
        let b = Vec3::new(1.2, 0.9, 0.1);
        let c = Vec3::new(0.2, i as f32 * 0.2, -0.6);
        let d = c + Vec3::new(0.0, 0.2, 1.3);
        let result = query::closest_points(
            &Pose::IDENTITY,
            &Segment::new(a, b),
            &Pose::IDENTITY,
            &Segment::new(c, d),
            10.0,
        )
        .unwrap();
        let distance = match result {
            ClosestPoints::WithinMargin(x, y) => x.distance(y),
            ClosestPoints::Intersecting => 0.0,
            _ => panic!("segments unexpectedly distant"),
        };
        let mut sampled = f32::INFINITY;
        for t in 0..=10000 {
            let p = a + (b - a) * (t as f32 / 10000.0);
            let s = ((p - c).dot(d - c) / (d - c).length_squared()).clamp(0.0, 1.0);
            sampled = sampled.min(p.distance(c + (d - c) * s));
        }
        assert!((distance - sampled).abs() < 1e-4);
    }
}
#[test]
fn clipped_face_has_four_stable_features() {
    let mut w = World::new(60, 0);
    crate::register(&mut w);
    w.spawn((Transform::default(), Collider::default()));
    let e = w.spawn((
        Transform::at(0.0, 0.99, 0.0),
        Collider::default(),
        Body {
            gravity: 0.0,
            ..Body::default()
        },
    ));
    let features = |w: &World| {
        let p = w.resource::<Physics>();
        let mut s = p.executor.0.borrow_mut();
        s.live()
            .rapier
            .narrow_phase
            .contact_pairs()
            .flat_map(|p| &p.manifolds)
            .flat_map(|m| &m.points)
            .map(|p| (p.fid1.0, p.fid2.0))
            .collect::<Vec<_>>()
    };
    crate::step(&mut w);
    let first = features(&w);
    assert_eq!(first.len(), 4);
    w.get_mut::<Transform>(e).unwrap().position.y = 0.98;
    crate::step(&mut w);
    assert_eq!(first, features(&w));
}
