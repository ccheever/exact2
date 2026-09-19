use super::*;
use crate::{animation, asset, Pose, SocketFollow, Sprite};

fn fixture() -> (World, Entity, Entity) {
    let mut w = World::new(60, 7);
    let subject = w.spawn_named("subject", (Transform::default(), Mesh::cube(2.)));
    let camera = w.spawn((
        Transform::at(0., 0., 10.),
        Camera::orthographic(20.).integer_scale(),
    ));
    (w, subject, camera)
}
fn fraction(w: &World, subject: Entity, camera: Entity) -> f32 {
    let mut sight = Sight::new(w, subject, None, Some(camera)).unwrap();
    let points = corners(sight.pose(subject).unwrap(), Vec3::ONE, Vec3::ZERO);
    occlusion(&mut sight, Vec3::new(0., 0., 10.), &points)
        .unwrap()
        .0
}
#[test]
fn sprite_size_camera_and_signed_scale_invalidate_displayed_geometry() {
    let (mut w, subject, camera) = fixture();
    let sprite = w.spawn((
        Transform::at(0., 0., 5.).with_scale(-1.),
        Sprite::new("late.tex", [4., 4.]),
    ));
    let before = (w.save(), w.hash(), w.mutation_epoch());
    assert_eq!(fraction(&w, subject, camera), 1.);
    assert_eq!(fraction(&w, subject, camera), 1.);
    let view = View::new(&w, Vec2::new(800., 600.)).unwrap();
    assert_eq!(pick(&w, &view, Vec2::new(400., 300.)).unwrap().0, sprite);
    assert_eq!((w.save(), w.hash(), w.mutation_epoch()), before);
    w.get_mut::<Sprite>(sprite).unwrap().size = Vec2::splat(0.01);
    assert!(fraction(&w, subject, camera) < 1.);
    w.get_mut::<Sprite>(sprite).unwrap().size = Vec2::splat(4.);
    assert_eq!(fraction(&w, subject, camera), 1.);
    let rotation = crate::Quat::from_rotation_y(0.7);
    w.get_mut::<Transform>(camera).unwrap().rotation = rotation;
    let sight = Sight::new(&w, subject, None, Some(camera)).unwrap();
    let pose = sight.pose(sprite).unwrap();
    assert!((pose.transform_vector3(Vec3::Z) - rotation * Vec3::Z).length() < 1e-6);
    drop(sight);
    w.get_mut::<Camera>(camera).unwrap().active = false;
    assert!(View::new(&w, Vec2::splat(600.)).is_none());
    let sight = Sight::new(&w, subject, None, None).unwrap();
    assert_eq!(
        sight.pose(sprite).unwrap().transform_vector3(Vec3::Z),
        Vec3::Z
    );
    drop(sight);
    w.despawn(sprite);
    assert_eq!(fraction(&w, subject, camera), 0.);
}
fn rig(w: &mut World, nodes: usize) -> Entity {
    w.assets.declared.insert("rig.model".into());
    w.assets.models.insert(
        "rig.model".into(),
        asset::ModelAsset::from(asset::Model {
            nodes: (0..nodes).map(|_| asset::Node::default()).collect(),
            ..Default::default()
        }),
    );
    w.spawn_named(
        "rig",
        (
            Transform::at(0., 0., 5.),
            Mesh::asset("rig.model"),
            Visible(false),
        ),
    )
}
#[test]
fn socket_pose_offset_parent_and_cache_hit_leases_follow_current_affine_geometry() {
    let (mut w, subject, camera) = fixture();
    let rig = rig(&mut w, 1);
    let follower = w.spawn((
        Transform::at(50., 0., 0.),
        SocketFollow::new(rig, ""),
        Mesh::cube(4.),
    ));
    assert_eq!(fraction(&w, subject, camera), 1.);
    let local = animation::bind_pose(w.model("rig.model").unwrap());
    let pose = Pose {
        local: local.clone(),
        previous: local,
        ..Default::default()
    };
    w.insert(rig, pose);
    w.get_mut::<Pose>(rig).unwrap().local[0] = 20.;
    assert_eq!(fraction(&w, subject, camera), 0.);
    w.get_mut::<Pose>(rig).unwrap().local[0] = 0.;
    assert_eq!(fraction(&w, subject, camera), 1.);
    let saved = w.save();
    let epoch = w.mutation_epoch();
    let expected = w.current_global(follower).unwrap();
    let sight = Sight::new(&w, subject, None, Some(camera)).unwrap();
    assert_eq!(sight.pose(follower), Some(expected));
    drop(sight);
    assert_eq!((w.save(), w.mutation_epoch()), (saved, epoch));
    // Mutable leases must refuse on a warm cache, not return stale geometry.
    let held = w.get_mut::<Pose>(rig).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Sight::new(
            &w,
            subject,
            None,
            Some(camera)
        )))
        .is_err()
    );
    drop(held);
    w.get_mut::<SocketFollow>(follower)
        .unwrap()
        .offset
        .position
        .x = 20.;
    assert_eq!(fraction(&w, subject, camera), 0.);
    w.get_mut::<SocketFollow>(follower).unwrap().joint = "missing".into();
    w.get_mut::<Transform>(follower).unwrap().position = Vec3::new(0., 0., 5.);
    assert_eq!(
        fraction(&w, subject, camera),
        1.,
        "invalid socket keeps the engine Transform fallback"
    );
    let parent = w.spawn(Transform::at(20., 0., 0.));
    w.insert(follower, Parent(parent));
    assert_eq!(
        fraction(&w, subject, camera),
        0.,
        "fresh parent reads do not require propagate"
    );
    w.despawn(parent);
    assert_eq!(fraction(&w, subject, camera), 1.);
    w.get_mut::<SocketFollow>(follower).unwrap().joint.clear();
    let parent = w.spawn(Transform {
        rotation: crate::Quat::from_rotation_y(0.73),
        scale: Vec3::new(2., 3., 0.5),
        ..Transform::at(4., 2., 1.)
    });
    w.insert(rig, Parent(parent));
    w.get_mut::<Transform>(rig).unwrap().rotation = crate::Quat::from_rotation_x(0.31);
    let expected = w.current_global(follower).unwrap().to_cols_array();
    let sight = Sight::new(&w, subject, None, Some(camera)).unwrap();
    let actual = sight.pose(follower).unwrap().to_cols_array();
    for (a, b) in actual.into_iter().zip(expected) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }
}
#[test]
fn socket_model_work_and_runtime_cycles_refuse_without_changing_state() {
    let (mut w, subject, camera) = fixture();
    let rig = rig(&mut w, 1001);
    let mut followers = Vec::new();
    for _ in 0..334 {
        followers.push(w.spawn((Transform::default(), SocketFollow::new(rig, ""))));
    }
    let before = (w.hash(), w.save(), w.mutation_epoch());
    for _ in 0..2 {
        let error = Sight::new(&w, subject, None, Some(camera)).err().unwrap();
        assert!(error.contains("pose work budget exceeded"), "{error}");
    }
    assert_eq!((w.hash(), w.save(), w.mutation_epoch()), before);
    for e in followers {
        w.despawn(e);
    }
    assert_eq!(fraction(&w, subject, camera), 0.);
    let a = w.spawn(Transform::default());
    let b = w.spawn((Transform::default(), Parent(a)));
    w.insert(a, Parent(b));
    let before = (w.hash(), w.save(), w.mutation_epoch());
    assert!(Sight::new(&w, subject, None, Some(camera))
        .err()
        .unwrap()
        .contains("cycle"));
    assert_eq!((w.hash(), w.save(), w.mutation_epoch()), before);
    w.remove::<Parent>(a);
    assert_eq!(fraction(&w, subject, camera), 0.);
}
