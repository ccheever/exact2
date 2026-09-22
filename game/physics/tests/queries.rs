mod common;
use exact_game::{Parent, Transform, Vec3, World};
use exact_game_physics::{self as physics, Collider, Shape};

#[test]
fn rays_have_closed_form_distances_and_normals() {
    for shape in [
        Shape::Sphere { radius: 0.5 },
        Shape::Capsule {
            radius: 0.4,
            height: 2.0,
        },
        Shape::Box {
            half: Vec3::new(0.5, 0.7, 0.6),
        },
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
            let expected = match shape {
                Shape::Sphere { radius } => {
                    let d = (radius as f64).powi(2)
                        - (origin.y as f64).powi(2)
                        - (origin.z as f64).powi(2);
                    (d >= 0.0).then(|| (3.0 - d.sqrt()) as f32)
                }
                Shape::Capsule { radius, height } => {
                    let y =
                        (origin.y.abs() as f64 - (height as f64 * 0.5 - radius as f64)).max(0.0);
                    let d = (radius as f64).powi(2) - (origin.z as f64).powi(2) - y * y;
                    (d >= 0.0).then(|| (3.0 - d.sqrt()) as f32)
                }
                Shape::Box { half } => (origin.y.abs() <= half.y).then_some(3.0 - half.x),
                _ => unreachable!(),
            };
            let got = physics::raycast(&w, origin, Vec3::X, 6.0, u32::MAX);
            assert_eq!(got.is_some(), expected.is_some());
            if let (Some(g), Some(distance)) = (got, expected) {
                assert_eq!(g.entity, e);
                // 3 mm bounds the iterative capsule ray hit for gameplay picking.
                let tolerance = if matches!(shape, Shape::Capsule { .. }) {
                    0.003
                } else {
                    1e-4
                };
                assert!(
                    (g.distance - distance).abs() <= tolerance,
                    "ray {shape:?} origin={origin:?}: got={} closed form={distance}",
                    g.distance
                );
                let normal = match shape {
                    Shape::Sphere { .. } => g.point.normalize(),
                    Shape::Capsule { radius, height } => {
                        let h = height * 0.5 - radius;
                        (g.point - Vec3::Y * g.point.y.clamp(-h, h)).normalize()
                    }
                    _ => -Vec3::X,
                };
                // Rapier's measured capsule-ray normal error is 1.8448492°; allow 3°.
                let error = (g.normal - normal).length().min(2.0) * 0.5;
                let degrees = error.asin().to_degrees() * 2.0;
                if matches!(shape, Shape::Capsule { .. }) {
                    assert!(
                        degrees <= 3.0,
                        "ray {origin:?}: normal error={degrees} degrees"
                    );
                } else {
                    assert!(g.normal.distance(normal) < 1e-4);
                }
            }
        }
    }
}
#[test]
fn translational_sweeps_match_independent_distance_bisection() {
    let (mut worst_round, mut worst_exact) = (0.0f32, 0.0f32);
    for shape in [
        Shape::Sphere { radius: 0.5 },
        Shape::Capsule {
            radius: 0.4,
            height: 2.0,
        },
        Shape::Box {
            half: Vec3::new(0.5, 0.7, 0.6),
        },
    ] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        w.spawn((
            Transform::default(),
            Collider {
                shape: shape.clone(),
                ..Collider::default()
            },
        ));
        for i in -15..=15 {
            for (radius, segment_half) in [(0.2, 0.0), (0.2, 0.2)] {
                let moving = if segment_half == 0.0 {
                    Shape::Sphere { radius }
                } else {
                    Shape::Capsule {
                        radius,
                        height: 0.8,
                    }
                };
                let origin = Vec3::new(-3.0, i as f32 * 0.07, 0.13);
                let distance = |t: f32| {
                    let p = origin + Vec3::X * t;
                    match shape {
                        Shape::Sphere { radius: r } => {
                            Vec3::new(p.x, (p.y.abs() - segment_half).max(0.0), p.z).length()
                                - r
                                - radius
                        }
                        Shape::Capsule { radius: r, height } => {
                            Vec3::new(
                                p.x,
                                (p.y.abs() - segment_half - (height * 0.5 - r)).max(0.0),
                                p.z,
                            )
                            .length()
                                - r
                                - radius
                        }
                        Shape::Box { half } => {
                            Vec3::new(
                                (p.x.abs() - half.x).max(0.0),
                                (p.y.abs() - half.y - segment_half).max(0.0),
                                (p.z.abs() - half.z).max(0.0),
                            )
                            .length()
                                - radius
                        }
                        _ => unreachable!(),
                    }
                };
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
                assert_eq!(got.is_some(), distance(3.0) <= 0.0);
                if let Some(g) = got {
                    let (mut lo, mut hi) = (0.0, 3.0);
                    for _ in 0..30 {
                        let mid = (lo + hi) * 0.5;
                        if distance(mid) > 0.0 {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    let error = (g.distance - hi).abs();
                    if matches!(shape, Shape::Capsule { .. })
                        || matches!(moving, Shape::Capsule { .. })
                    {
                        worst_round = worst_round.max(error);
                    } else {
                        worst_exact = worst_exact.max(error);
                    }
                }
                let p = Transform::at(i as f32 * 0.07, 0.0, 0.0);
                let expected = match shape {
                    Shape::Sphere { radius: r } | Shape::Capsule { radius: r, .. } => {
                        p.position.x.abs() <= r + radius
                    }
                    Shape::Box { half } => p.position.x.abs() <= half.x + radius,
                    _ => unreachable!(),
                };
                assert_eq!(
                    !physics::overlap(&w, &moving, p, u32::MAX).is_empty(),
                    expected
                );
            }
        }
    }
    eprintln!(
        "sweep worst: capsule {} mm, sphere/box {} mm",
        worst_round * 1000.0,
        worst_exact * 1000.0
    );
    // parry's shape cast stops at a relative tolerance near 1e-3: millimetres over a 3 m cast.
    assert!(worst_round <= 0.005, "capsule sweep error {worst_round} m");
    assert!(
        worst_exact <= 1e-4,
        "sphere/box sweep error {worst_exact} m"
    );
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

#[test]
fn recycled_collider_reports_its_current_entity_generation() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let retired = w.spawn((Transform::at(3.0, 0.0, 0.0), Collider::default()));
    assert_eq!(
        physics::raycast(&w, Vec3::ZERO, Vec3::X, 10.0, 1)
            .unwrap()
            .entity,
        retired
    );
    w.despawn(retired);
    let current = w.spawn((Transform::at(3.0, 0.0, 0.0), Collider::default()));
    assert_eq!(current.index(), retired.index());
    assert_ne!(current.generation(), retired.generation());
    assert_eq!(
        physics::raycast(&w, Vec3::ZERO, Vec3::X, 10.0, 1)
            .unwrap()
            .entity,
        current
    );
}
