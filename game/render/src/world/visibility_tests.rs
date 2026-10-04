use super::*;
use exact_game::{SocketFollow, SpotLight};

#[test]
fn paused_reparenting_rebuilds_primitive_and_model_draw_and_shadow_batches() {
    let mut w = World::new(60, 0);
    let shown = w.spawn(Transform::default());
    let hidden = w.spawn((Transform::default(), Visible(false)));
    let root = w.spawn((Transform::default(), Parent(shown)));
    let cube = w.spawn((Transform::default(), Parent(root), Mesh::cube(1.)));
    let model = w.spawn((Transform::default(), Parent(root), Mesh::asset("rig.model")));
    let local_hidden = w.spawn((
        Transform::default(),
        Parent(root),
        Mesh::cube(1.),
        Visible(false),
    ));
    // Socket tracking alone is not scene ownership.
    let independent = w.spawn((
        Transform::default(),
        SocketFollow::new(root, "joint"),
        Mesh::cube(1.),
    ));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording {
        models: vec![(MeshId(99), crate::MaterialId(0), glam::Mat4::IDENTITY, None)],
        ..Default::default()
    };
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&cube.index()));
    assert!(!r.slots.contains(&local_hidden.index()));
    assert_eq!(r.instances.len(), 1);
    assert_eq!(r.instances[0].transform, model.index());
    // These same batches feed the color and shadow passes.
    assert!(r.batches.iter().any(|b| b.mesh == MeshId(99)));
    w.insert(root, Parent(hidden));
    w.propagate();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.slots, [independent.index()]);
    assert!(r.instances.is_empty());
    assert!(r.batches.iter().all(|b| b.mesh != MeshId(99)));
    w.remove::<Parent>(root);
    w.propagate();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&cube.index()));
    assert!(!r.slots.contains(&local_hidden.index()));
    assert_eq!(r.instances.len(), 1);
    w.insert(root, Visible(false));
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.slots, [independent.index()]);
    assert!(r.instances.is_empty());
    w.remove::<Visible>(root);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.instances.len(), 1);
    assert_eq!(w.tick(), 0);
}

#[test]
fn hidden_lights_do_not_starve_visible_slots_and_parent_edits_reselect_paused() {
    let mut w = World::new(60, 0);
    let hidden = w.spawn((Transform::default(), Visible(false)));
    let shown = w.spawn(Transform::default());
    // More nearer hidden lights than the entire GPU budget.
    for _ in 0..=crate::MAX_LIGHTS {
        w.spawn((Transform::default(), Parent(hidden), PointLight::default()));
    }
    let point = w.spawn((
        Transform::at(10., 0., 0.),
        Parent(shown),
        PointLight::default(),
    ));
    let spot = w.spawn((
        Transform::at(20., 0., 0.),
        Parent(shown),
        SpotLight::default(),
    ));
    w.propagate();
    let mut scene = Scene::default();
    scene.feed(&w, false, false, true, false);
    assert_eq!(scene.lights_for_test(), [point, spot]);
    assert_eq!(
        scene.frame(&w, 1., glam::Vec2::ONE, false).lights_dropped,
        0
    );
    w.insert(point, Parent(hidden));
    w.propagate();
    // Exercise parent-only invalidation independently of the moved hint.
    scene.feed(&w, false, false, false, true);
    assert_eq!(scene.lights_for_test(), [spot]);
    w.insert(shown, Visible(false));
    scene.feed(&w, false, false, false, false);
    assert!(scene.lights_for_test().is_empty());
    w.remove::<Visible>(shown);
    scene.feed(&w, false, false, false, false);
    assert_eq!(scene.lights_for_test(), [spot]);
}

#[test]
fn sun_fill_and_hook_frame_fallback_skip_hidden_ancestors_but_camera_does_not() {
    let mut w = World::new(60, 0);
    let hidden = w.spawn((Transform::default(), Visible(false)));
    w.spawn((Transform::at(2., 3., 4.), Parent(hidden), Camera::default()));
    for (intensity, parent) in [(100., Some(hidden)), (200., None), (300., None)] {
        let e = w.spawn((
            Transform::default(),
            DirectionalLight {
                illuminance: intensity,
                ..Default::default()
            },
        ));
        if let Some(parent) = parent {
            w.insert(e, Parent(parent));
        }
    }
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let frame = f.frame(&w, 1., 1.);
    assert_eq!(frame.camera_position, Vec3::new(2., 3., 4.));
    assert_eq!(
        frame.sun.unwrap().illuminance,
        200. * scene::PHOTOMETRIC_SCALE
    );
    assert_eq!(
        frame.fill.unwrap().illuminance,
        300. * scene::PHOTOMETRIC_SCALE
    );
    w.remove::<Visible>(hidden);
    f.feed_to(&w, &mut r).unwrap();
    let frame = f.frame(&w, 1., 1.);
    assert_eq!(
        frame.sun.unwrap().illuminance,
        100. * scene::PHOTOMETRIC_SCALE
    );
    assert_eq!(
        frame.fill.unwrap().illuminance,
        200. * scene::PHOTOMETRIC_SCALE
    );
}
