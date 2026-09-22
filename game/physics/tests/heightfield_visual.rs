use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, Collider, Shape};

#[test]
fn asymmetric_heightfield_visual_triangles_match_collision_samples() {
    let heights: Vec<_> = (0..20)
        .map(|i| ((i * 13 + 7) % 19) as f32 * 0.125)
        .collect();
    let (mesh, shape) = Shape::heightfield(4, 5, heights, Vec3::new(9., 2., 7.)).unwrap();
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w.spawn((
        Transform::default(),
        Collider {
            shape,
            ..Default::default()
        },
    ));
    for tri in mesh.indices.chunks_exact(3) {
        let point = |i: u32| Vec3::from_slice(&mesh.positions[i as usize * 3..][..3]);
        let p = point(tri[0]) * 0.2 + point(tri[1]) * 0.3 + point(tri[2]) * 0.5;
        let hit = physics::raycast(&w, Vec3::new(p.x, 20., p.z), -Vec3::Y, 30., u32::MAX).unwrap();
        assert!((20. - hit.distance - p.y).abs() < 0.00001, "{p:?}: {hit:?}");
        assert!(hit.normal.y > 0.);
    }
    assert!(Shape::heightfield(1, 2, vec![0.; 2], Vec3::ONE).is_err());
}

#[test]
fn sphere_contacts_pin_the_slope_and_upper_ledge() {
    let (_, shape) = Shape::heightfield(
        2,
        4,
        vec![0., 0.5, 2., 2., 0., 0.5, 2., 2.],
        Vec3::new(6., 1., 4.),
    )
    .unwrap();
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w.spawn((
        Transform::default(),
        Collider {
            shape,
            ..Default::default()
        },
    ));
    for (x, expected) in [(-2., 0.4561553), (2., 2.2)] {
        let hit = physics::sweep(
            &w,
            &Shape::Sphere { radius: 0.2 },
            Transform::at(x, 10., 0.),
            Vec3::new(0., -20., 0.),
            u32::MAX,
        )
        .unwrap();
        assert!(
            (10. - hit.distance - expected).abs() < 0.0001,
            "x={x}: {hit:?}"
        );
    }
}
