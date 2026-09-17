mod common;
use exact_game::{Parent, Transform, Vec3, World};
use exact_game_physics::{self as physics, Collider, Shape};
use rapier3d::{
    math::{Pose, Vector},
    parry::{
        query::{self, Ray, ShapeCastOptions},
        shape::SharedShape,
    },
};
#[test]
fn ray_overlap_and_sphere_sweep_against_oracle() {
    for (shape, oracle) in [
        (Shape::Sphere { radius: 0.5 }, SharedShape::ball(0.5)),
        (
            Shape::Capsule {
                radius: 0.4,
                height: 2.0,
            },
            SharedShape::capsule_y(0.6, 0.4),
        ),
        (
            Shape::Box {
                half: Vec3::new(0.5, 0.7, 0.6),
            },
            SharedShape::cuboid(0.5, 0.7, 0.6),
        ),
    ] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        let e = w.spawn((
            Transform::default(),
            Collider {
                shape: shape.clone(),
                ..Collider::default()
            },
        ));
        for i in -15..=15 {
            let origin = Vec3::new(-3.0, i as f32 * 0.07, 0.13);
            let dir = Vec3::X;
            let ray = Ray::new(Vector::from_array(origin.to_array()), Vector::X);
            let expected = oracle.cast_ray_and_get_normal(&Pose::IDENTITY, &ray, 6.0, true);
            let got = physics::raycast(&w, origin, dir, 6.0, u32::MAX);
            assert_eq!(got.is_some(), expected.is_some());
            if let (Some(g), Some(x)) = (got, expected) {
                assert_eq!(g.entity, e);
                assert!(
                    (g.distance - x.time_of_impact).abs() < 1e-4,
                    "ray {shape:?} origin={origin:?}: exact={} rapier={} point={:?}",
                    g.distance,
                    x.time_of_impact,
                    g.point
                );
                let normal = match shape {
                    Shape::Sphere { .. } => g.point.normalize(),
                    Shape::Capsule { radius, height } => {
                        let half = height * 0.5 - radius;
                        (g.point - Vec3::Y * g.point.y.clamp(-half, half)).normalize()
                    }
                    Shape::Box { .. } => -Vec3::X,
                };
                assert!(
                    g.normal.distance(normal) < 1e-4,
                    "ray normal {:?} != {:?}",
                    g.normal,
                    normal
                );
            }
            for moving in [
                Shape::Sphere { radius: 0.2 },
                Shape::Capsule {
                    radius: 0.2,
                    height: 0.8,
                },
            ] {
                let moving_oracle = if matches!(moving, Shape::Sphere { .. }) {
                    SharedShape::ball(0.2)
                } else {
                    SharedShape::capsule_y(0.2, 0.2)
                };
                let start = Pose::from_translation(Vector::from_array(origin.to_array()));
                let expected = query::cast_shapes(
                    &start,
                    Vector::X,
                    &*moving_oracle,
                    &Pose::IDENTITY,
                    Vector::ZERO,
                    &*oracle,
                    ShapeCastOptions {
                        max_time_of_impact: 6.0,
                        ..ShapeCastOptions::default()
                    },
                )
                .unwrap();
                let got = physics::sweep(
                    &w,
                    &moving,
                    Transform {
                        position: origin,
                        ..Transform::default()
                    },
                    Vec3::X * 6.0,
                    u32::MAX,
                );
                assert_eq!(got.is_some(), expected.is_some(), "sweep origin {origin:?}");
                if let (Some(g), Some(x)) = (got, expected) {
                    // Conservative advancement in Parry stops at a finite tolerance.
                    // Check the strict 0.1 mm bar with independent distance bisection.
                    let (radius, segment_half) = match moving {
                        Shape::Sphere { radius } => (radius, 0.0),
                        Shape::Capsule { radius, height } => (radius, height * 0.5 - radius),
                        _ => unreachable!(),
                    };
                    let distance = |t: f32| {
                        let p = origin + Vec3::X * t;
                        match shape {
                            Shape::Sphere { radius: r } => {
                                (p.x * p.x
                                    + p.z * p.z
                                    + (p.y.abs() - segment_half).max(0.0)
                                        * (p.y.abs() - segment_half).max(0.0))
                                .sqrt()
                                    - r
                                    - radius
                            }
                            Shape::Capsule { radius: r, height } => {
                                let y = (p.y.abs() - segment_half - (height * 0.5 - r)).max(0.0);
                                (p.x * p.x + p.z * p.z + y * y).sqrt() - r - radius
                            }
                            Shape::Box { half } => {
                                let d = Vec3::new(
                                    (p.x.abs() - half.x).max(0.0),
                                    (p.y.abs() - half.y - segment_half).max(0.0),
                                    (p.z.abs() - half.z).max(0.0),
                                );
                                d.length() - radius
                            }
                        }
                    };
                    let (mut lo, mut hi) = (0.0, 3.0);
                    assert!(distance(hi) <= 0.0);
                    for _ in 0..30 {
                        let mid = (lo + hi) * 0.5;
                        if distance(mid) > 0.0 {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    assert!(
                        (g.distance - hi).abs() < 1e-4,
                        "sweep {} != sampled {hi}, Parry {}",
                        g.distance,
                        x.time_of_impact
                    );
                }
                let p = Transform::at(i as f32 * 0.07, 0.1, 0.13);
                let expected = query::intersection_test(
                    &Pose::from_translation(Vector::from_array(p.position.to_array())),
                    &*moving_oracle,
                    &Pose::IDENTITY,
                    &*oracle,
                )
                .unwrap();
                assert_eq!(
                    !physics::overlap(&w, &moving, p, u32::MAX).is_empty(),
                    expected
                );
            }
        }
    }
}
#[test]
fn hierarchy_and_sorted_ties_are_live_reads() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let parent = w.spawn(Transform::at(3.0, 0.0, 0.0));
    let a = w.spawn((Transform::default(), Parent(parent), Collider::default()));
    let b = w.spawn((Transform::at(3.0, 0.0, 0.0), Collider::default()));
    assert_eq!(
        physics::raycast(&w, Vec3::ZERO, Vec3::X, 10.0, 1)
            .unwrap()
            .entity,
        a
    );
    assert_eq!(
        physics::overlap(&w, &Shape::default(), Transform::at(3.0, 0.0, 0.0), 1),
        vec![a, b]
    );
    w.get_mut::<Transform>(parent).unwrap().position.x = 6.0;
    assert_eq!(
        physics::raycast(&w, Vec3::ZERO, Vec3::X, 10.0, 1)
            .unwrap()
            .entity,
        b
    );
    assert!(physics::raycast(&w, Vec3::ZERO, Vec3::X, 10.0, 2).is_none());
}
