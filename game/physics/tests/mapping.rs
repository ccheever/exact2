use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Shape};

#[test]
fn heightfield_mesh_and_cylinder_use_world_entities() {
    for shape in [
        Shape::Heightfield {
            rows: 2,
            cols: 2,
            heights: vec![0.0; 4],
            scale: Vec3::new(10.0, 1.0, 10.0),
        },
        Shape::Mesh {
            vertices: vec![
                Vec3::new(-5.0, 0.0, -5.0),
                Vec3::new(-5.0, 0.0, 5.0),
                Vec3::new(5.0, 0.0, -5.0),
                Vec3::new(5.0, 0.0, 5.0),
            ],
            indices: vec![[0, 1, 2], [2, 1, 3]],
        },
        Shape::Cylinder {
            radius: 2.0,
            height: 1.0,
        },
    ] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        let floor = w.spawn((
            Transform::default(),
            Collider {
                shape: shape.clone(),
                ..Collider::default()
            },
        ));
        let hit = physics::raycast(&w, Vec3::new(0.2, 3.0, 0.3), -Vec3::Y, 5.0, u32::MAX).unwrap();
        assert_eq!(hit.entity, floor);
        let expected = if matches!(shape, Shape::Cylinder { .. }) {
            2.5
        } else {
            3.0
        };
        assert!((hit.distance - expected).abs() < 0.001);
        let box_id = w.spawn((
            Transform::at(0.2, 2.0, 0.3),
            Body::default(),
            Collider::default(),
        ));
        for _ in 0..360 {
            physics::step(&mut w);
        }
        assert!(w.get::<Body>(box_id).unwrap().asleep);
        assert!((w.get::<Transform>(box_id).unwrap().position.y - (3.5 - expected)).abs() < 0.01);
        let saved = w.save();
        let mut restored = World::new(60, 0);
        physics::register(&mut restored);
        restored.load(&saved).unwrap();
        assert_eq!(w.hash(), restored.hash());
        physics::step(&mut w);
        physics::step(&mut restored);
        assert_eq!(w.hash(), restored.hash());
    }
}
#[test]
fn fast_rigid_body_does_not_tunnel_and_queries_are_read_only() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w.spawn((
        Transform::at(1.0, 0.0, 0.0),
        Collider {
            shape: Shape::Box {
                half: Vec3::new(0.01, 10.0, 10.0),
            },
            ..Collider::default()
        },
    ));
    let e = w.spawn((
        Transform::default(),
        Body {
            velocity: Vec3::X * 200.0,
            gravity: 0.0,
            ..Body::default()
        },
        Collider {
            shape: Shape::Sphere { radius: 0.1 },
            ..Collider::default()
        },
    ));
    for _ in 0..10 {
        physics::step(&mut w);
        assert!(w.get::<Transform>(e).unwrap().position.x < 1.0);
    }
    let hash = w.hash();
    physics::raycast(&w, Vec3::ZERO, Vec3::X, 10.0, u32::MAX);
    physics::overlap(&w, &Shape::default(), Transform::default(), u32::MAX);
    assert_eq!(hash, w.hash());
    w.remove::<Collider>(e);
    physics::step(&mut w);
    w.remove::<Body>(e);
    w.insert(e, Collider::default());
    physics::step(&mut w);
    assert_eq!(
        physics::raycast(&w, Vec3::new(-1.0, 0.0, 0.0), Vec3::X, 10.0, u32::MAX)
            .unwrap()
            .entity,
        e
    );
}

#[test]
fn colliders_match_primitive_dimensions_and_plane_top() {
    use exact_game::Mesh;
    for (mesh, half_height) in [
        (Mesh::cube(2.0), 1.0),
        (Mesh::cuboid(Vec3::new(2.0, 3.0, 4.0)), 1.5),
        (Mesh::sphere(0.6), 0.6),
        (Mesh::capsule(0.4, 1.8), 0.9),
        (Mesh::cylinder(0.5, 1.2), 0.6),
        (Mesh::plane(40.0, 20.0), 0.0),
    ] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        w.spawn((Transform::default(), Collider::of(&mesh)));
        let hit = physics::raycast(&w, Vec3::new(0.0, 3.0, 0.0), -Vec3::Y, 5.0, u32::MAX).unwrap();
        assert!(
            (hit.point.y - half_height).abs() < 1e-5,
            "{mesh:?}: {hit:?}"
        );
        if matches!(mesh, Mesh::Plane { .. }) {
            let bottom =
                physics::raycast(&w, Vec3::new(0.0, -1.0, 0.0), Vec3::Y, 2.0, u32::MAX).unwrap();
            assert!((bottom.point.y + 0.01).abs() < 1e-5);
        }
    }
}
