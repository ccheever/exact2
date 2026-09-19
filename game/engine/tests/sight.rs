use exact_game::*;
struct Eyes;
impl Game for Eyes {
    type Args = ();
    const ID: &'static str = "eyes";
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("subject", (Transform::default(), Mesh::cube(2.0)));
        w.spawn_named("camera", (Transform::at(0.0, 0.0, 10.0), Camera::default()));
        w.spawn_named("target", (Transform::at(0.0, 0.0, -10.0), Mesh::cube(1.0)));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn eyes_snapshot_is_a_pure_read_and_optional_target_is_validated() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    let before = (s.world().mutation_epoch(), s.world().hash(), s.save());
    let reply = s.agent(r#"{"op":"layout","entity":"subject","to":"target"}"#);
    assert_eq!(
        reply,
        r#"{"tick":0,"entity":{"id":0,"name":"subject","world":{"position":[0.0,0.0,0.0],"rotation":[0,0,0,1],"scale":[1,1,1]},"bounds":{"min":[-1,-1,-1],"max":[1,1,1]},"screen":{"unavailable":true},"depth":null,"visible":{"unavailable":true},"facing":{"forward":[0,0,-1],"towardCamera":null,"bearingTo":0,"distanceTo":10,"lineOfSight":true}},"route":{"blocker":null,"nearestClearSide":null}}"#
    );
    assert_eq!(
        (s.world().mutation_epoch(), s.world().hash(), s.save()),
        before
    );
    s.viewport(800.0, 600.0);
    let epoch = s.world().mutation_epoch();
    let reply = s.agent(r#"{"op":"layout","entity":"subject","to":"target"}"#);
    assert!(reply.contains(r#""visible":{"inFrustum":true,"behindCamera":false,"distance":10,"occluded":0,"occluders":[]}"#), "{reply}");
    assert!(reply.contains(r#""towardCamera":-1"#));
    assert_eq!(s.world().mutation_epoch(), epoch);
    assert!(s
        .agent(r#"{"op":"layout","entity":"subject","to":"missing"}"#)
        .contains("no entity named"));
}
#[test]
fn occlusion_counts_samples_orders_caps_and_ignores_hidden_self_and_distant_bounds() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800.0, 600.0);
    let subject = s.world().named("subject").unwrap();
    // A half-width wall hides exactly 10 of 15 samples.
    let wall = s.world_mut().spawn_named(
        "wall",
        (
            Transform::at(2.0, 0.0, 5.0),
            Mesh::cuboid(Vec3::new(4.0, 8.0, 1.0)),
        ),
    );
    s.world_mut().propagate();
    let request = r#"{"op":"layout","entity":"subject","to":"target"}"#;
    let reply = s.agent(request);
    assert!(
        reply.contains(r#""occluded":0.6667,"occluders":["wall"]"#),
        "{reply}"
    );
    s.world_mut().insert(wall, Visible(false));
    assert!(s.agent(request).contains(r#""occluded":0,"occluders":[]"#));
    s.world_mut().insert(wall, Visible(true));
    for i in 0..5 {
        s.world_mut().spawn_named(
            format!("cover-{i}"),
            (
                Transform::at(0.0, 0.0, 6.0 + i as f32 * 0.5),
                Mesh::cuboid(Vec3::new(4.0, 4.0, 0.2)),
            ),
        );
    }
    s.world_mut().propagate();
    assert!(s
        .agent(request)
        .contains(r#""occluded":1,"occluders":["cover-4","cover-3","cover-2","cover-1"]"#));
    s.world_mut()
        .teleport(subject, Transform::at(0.0, 0.0, -12.0));
    assert!(s.agent(request).contains(r#""lineOfSight":true"#));
}
#[test]
fn facing_uses_parent_pose_and_segment_blocks_only_between_endpoints() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    let subject = s.world().named("subject").unwrap();
    let parent = s.world_mut().spawn(Transform {
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        ..Transform::default()
    });
    s.world_mut().insert(subject, Parent(parent));
    s.world_mut()
        .spawn_named("block", (Transform::at(0.0, 0.0, -5.0), Mesh::cube(1.0)));
    s.world_mut().propagate();
    let reply = s.agent(r#"{"op":"layout","entity":"subject","to":"target"}"#);
    assert!(reply.contains(r#""forward":[-1,0,0]"#), "{reply}");
    assert!(
        reply.contains(r#""bearingTo":-90,"distanceTo":10,"lineOfSight":false"#),
        "{reply}"
    );
}

#[test]
fn declared_model_bounds_replace_authored_bounds_for_layout_pick_and_occlusion() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800.0, 600.0);
    let subject = s.world().named("subject").unwrap();
    s.world_mut().insert(subject, Mesh::asset("subject.model"));
    s.world_mut().insert(
        subject,
        asset::ModelBounds([-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]),
    );
    let layout = r#"{"op":"layout","entity":"subject"}"#;
    let pick = r#"{"op":"layout","x":400,"y":300}"#;
    assert!(s
        .agent(layout)
        .contains(r#""bounds":{"min":[-0.5,-0.5,-0.5],"max":[0.5,0.5,0.5]}"#));
    assert!(s.agent(pick).contains(r#""distance":9.5"#));
    let model = asset::Model {
        bounds: [-2.0, -2.0, -2.0, 2.0, 2.0, 2.0],
        ..Default::default()
    };
    struct DeclaredEyes;
    impl Game for DeclaredEyes {
        const ID: &'static str = "declared-eyes";
        const ASSETS: &'static [&'static str] = &["subject.model"];
        type Args = ();
        fn setup(w: &mut World, args: &()) {
            Eyes::setup(w, args);
            let subject = w.named("subject").unwrap();
            w.insert(subject, Mesh::asset("subject.model"));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut s = Sim::<DeclaredEyes>::new(()).unwrap();
    assert!(s.is_loading());
    s.viewport(800.0, 600.0);
    s.asset("subject.model", Some(&bin::to_vec(&model)))
        .unwrap();
    assert!(s
        .agent(layout)
        .contains(r#""bounds":{"min":[-2,-2,-2],"max":[2,2,2]}"#));
    assert!(s.agent(pick).contains(r#""distance":8"#));
    let reply = s.agent(r#"{"op":"layout","entity":"target"}"#);
    assert!(
        reply.contains(r#""occluded":1,"occluders":["subject"]"#),
        "{reply}"
    );
}

#[test]
fn unbounded_cosmetic_never_blocks_pick_occlusion_or_facing_even_after_arrival() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800.0, 600.0);
    let subject = s.world().named("subject").unwrap();
    s.world_mut().insert(subject, Mesh::asset("subject.model"));
    let layout = r#"{"op":"layout","entity":"subject"}"#;
    let target = r#"{"op":"layout","entity":"target","to":"camera"}"#;
    let pick = r#"{"op":"layout","x":400,"y":300}"#;
    let before = [s.agent(layout), s.agent(target), s.agent(pick)];
    assert!(before[0].contains(r#""bounds":null,"screen":{"unavailable":true},"depth":null,"visible":{"unavailable":true}"#), "{}", before[0]);
    assert!(
        before[1].contains(r#""occluded":0,"occluders":[]"#),
        "{}",
        before[1]
    );
    assert!(before[1].contains(r#""lineOfSight":true"#), "{}", before[1]);
    assert!(!before[2].contains(r#""name":"subject""#), "{}", before[2]);
    s.asset(
        "subject.model",
        Some(&bin::to_vec(&asset::Model {
            bounds: [-20., -20., -20., 20., 20., 20.],
            ..Default::default()
        })),
    )
    .unwrap();
    assert_eq!([s.agent(layout), s.agent(target), s.agent(pick)], before);
    // Negative control: authored bounds must actually make the same entity obstruct.
    s.world_mut()
        .insert(subject, asset::ModelBounds([-2., -2., -2., 2., 2., 2.]));
    assert!(s
        .agent(target)
        .contains(r#""occluded":1,"occluders":["subject"]"#));
    assert!(s.agent(target).contains(r#""lineOfSight":false"#));
    assert!(s.agent(pick).contains(r#""name":"subject""#));
}

#[test]
fn sight_excludes_camera_and_both_endpoint_ancestors_and_descendants_but_not_siblings() {
    for endpoint in ["subject", "target", "camera"] {
        for ancestor in [false, true] {
            let mut s = Sim::<Eyes>::new(()).unwrap();
            s.viewport(800., 600.);
            let e = s.world().named(endpoint).unwrap();
            let z = if endpoint == "target" { -5. } else { 5. };
            let geometry = s.world_mut().spawn_named(
                "endpoint-geometry",
                (Transform::at(0., 0., z), Mesh::cube(6.)),
            );
            let request = r#"{"op":"layout","entity":"subject","to":"target"}"#;
            s.world_mut().propagate();
            let before = s.agent(request);
            let obstructed = if endpoint == "target" {
                r#""lineOfSight":false"#
            } else {
                r#""occluded":1"#
            };
            assert!(
                before.contains(obstructed),
                "{endpoint} {ancestor}: {before}"
            );
            if ancestor {
                // Keep the endpoint's world pose constant when attaching its parent.
                s.world_mut().insert(e, Parent(geometry));
                s.world_mut().get_mut::<Transform>(e).unwrap().position.z -= z;
            } else {
                // Exercise transitive descendant exclusion through a meshless link.
                let link = s.world_mut().spawn((Transform::default(), Parent(e)));
                s.world_mut().insert(geometry, Parent(link));
                let endpoint_z = s.world().get::<Transform>(e).unwrap().position.z;
                s.world_mut()
                    .get_mut::<Transform>(geometry)
                    .unwrap()
                    .position
                    .z -= endpoint_z;
            }
            s.world_mut().propagate();
            let after = s.agent(request);
            assert!(
                after.contains(r#""occluded":0,"occluders":[]"#),
                "{endpoint} {ancestor}: {after}"
            );
            assert!(after.contains(r#""lineOfSight":true"#), "{after}");
            if ancestor {
                let sibling = s.world_mut().spawn_named(
                    "sibling",
                    (Transform::default(), Parent(geometry), Mesh::cube(6.)),
                );
                s.world_mut().propagate();
                assert!(s.agent(request).contains(obstructed), "sibling {sibling:?}");
            }
        }
    }
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800., 600.);
    let camera = s.world().named("camera").unwrap();
    s.world_mut().insert(camera, Mesh::cube(2.));
    assert!(s
        .agent(r#"{"op":"layout","entity":"subject"}"#)
        .contains(r#""occluded":0,"occluders":[]"#));
}

#[test]
fn unavailable_poses_collapsed_forward_and_outside_frustum_are_explicit() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800., 600.);
    let subject = s.world().named("subject").unwrap();
    let target = s.world().named("target").unwrap();
    s.world_mut().remove::<Transform>(subject);
    let request = r#"{"op":"layout","entity":"subject","to":"target"}"#;
    let reply = s.agent(request);
    assert!(
        reply.contains(r#""world":null"#) && reply.contains("missing global pose"),
        "{reply}"
    );
    s.world_mut().insert(subject, Transform::default());
    s.world_mut().remove::<Transform>(target);
    let reply = s.agent(request);
    assert!(
        reply.contains(
            r#""distanceTo":null,"lineOfSight":null,"reason":"target global pose unavailable"#
        ),
        "{reply}"
    );
    s.world_mut().teleport(
        subject,
        Transform::default().with_scale(Vec3::new(1., 1., 0.)),
    );
    let reply = s.agent(request);
    assert!(
        reply.contains(r#""forward":null,"towardCamera":null"#),
        "{reply}"
    );
    for (position, reason) in [
        (Vec3::new(0., 0., 20.), "behind camera"),
        (Vec3::new(100., 0., 0.), "outside frustum"),
    ] {
        s.world_mut().teleport(
            subject,
            Transform {
                position,
                ..Default::default()
            },
        );
        let reply = s.agent(request);
        assert!(
            reply.contains(r#""occluded":null,"occluders":[]"#) && reply.contains(reason),
            "{reply}"
        );
    }
    // Bounds straddling the eye are not wholly behind, regardless of their origin.
    s.world_mut().teleport(subject, Transform::at(0., 0., 10.));
    assert!(s.agent(request).contains(r#""behindCamera":false"#));
    // G reads current parent components before propagation as well.
    let unpropagated = s
        .world_mut()
        .spawn_named("unpropagated", (Transform::default(), Parent(subject)));
    let reply = s.agent(&format!(
        r##"{{"op":"layout","entity":"#{}"}}"##,
        unpropagated.index()
    ));
    assert!(reply.contains(r#""position":[0.0,0.0,10.0]"#), "{reply}");
}

#[test]
fn room_shell_entry_only_and_fully_degenerate_bounds_never_block() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800., 600.);
    let room = s
        .world_mut()
        .spawn_named("room", (Transform::at(0., 0., 5.), Mesh::cube(40.)));
    let request = r#"{"op":"layout","entity":"subject","to":"target"}"#;
    let reply = s.agent(request);
    assert!(reply.contains(r#""occluded":0,"occluders":[]"#), "{reply}");
    assert!(reply.contains(r#""lineOfSight":true"#), "{reply}");
    // Even the room's exit surface is ignored; pick deliberately still sees exits.
    let target = s.world().named("target").unwrap();
    s.world_mut().teleport(target, Transform::at(0., 0., -50.));
    assert!(s.agent(request).contains(r#""lineOfSight":true"#));
    s.world_mut().despawn(room);
    let point = s.world_mut().spawn_named(
        "point",
        (
            Transform::at(0., 0., 5.),
            Mesh::asset("empty.model"),
            asset::ModelBounds([0.; 6]),
        ),
    );
    assert!(s.agent(request).contains(r#""occluded":0,"occluders":[]"#));
    s.world_mut()
        .insert(point, asset::ModelBounds([-2., -2., -2., 2., 2., 2.]));
    assert!(s
        .agent(request)
        .contains(r#""occluded":1,"occluders":["point"]"#));
    s.world_mut()
        .insert(point, asset::ModelBounds([1., 1., 1., -1., -1., -1.]));
    assert!(s.agent(request).contains(r#""occluded":0,"occluders":[]"#));
}

#[test]
fn rotated_scaled_parented_blocker_uses_its_oriented_geometry_and_cache_invalidation() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800., 600.);
    let parent = s.world_mut().spawn(Transform {
        position: Vec3::new(0., 0., 5.),
        rotation: Quat::from_rotation_y(0.2),
        scale: Vec3::new(2., 3., 0.5),
    });
    let wall = s.world_mut().spawn_named(
        "affine-wall",
        (
            Parent(parent),
            Transform {
                rotation: Quat::from_rotation_z(0.7),
                scale: Vec3::new(2., 1., 1.),
                ..Default::default()
            },
            Mesh::cuboid(Vec3::new(4., 4., 0.1)),
        ),
    );
    s.world_mut().propagate();
    let request = r#"{"op":"layout","entity":"subject"}"#;
    assert!(s
        .agent(request)
        .contains(r#""occluded":1,"occluders":["affine-wall"]"#));
    // Parent motion, same-tick component writes, visibility, and slot reuse all invalidate.
    s.world_mut()
        .get_mut::<Transform>(parent)
        .unwrap()
        .position
        .x = 100.;
    s.world_mut().propagate();
    assert!(s.agent(request).contains(r#""occluded":0,"occluders":[]"#));
    s.world_mut().teleport(parent, Transform::at(0., 0., 5.));
    assert!(s.agent(request).contains(r#""occluded":1"#));
    s.world_mut().insert(wall, Visible(false));
    assert!(s.agent(request).contains(r#""occluded":0"#));
    s.world_mut().insert(wall, Visible(true));
    s.world_mut().insert(wall, Mesh::cube(0.01));
    assert!(!s.agent(request).contains(r#""occluded":1,"#));
    s.world_mut().despawn(wall);
    let replacement = s
        .world_mut()
        .spawn_named("replacement", (Transform::at(0., 0., 5.), Mesh::cube(4.)));
    assert_eq!(replacement.index(), wall.index());
    assert!(s
        .agent(request)
        .contains(r#""occluded":1,"occluders":["replacement"]"#));
}

#[test]
fn quantized_occluder_ties_use_entity_index_and_json_canonicalizes_angles_and_zero() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800., 600.);
    // The lower entity index is slightly farther, but both lie in the same 0.1mm bin.
    s.world_mut()
        .spawn_named("first", (Transform::at(0., 0., 5.), Mesh::cube(4.)));
    s.world_mut()
        .spawn_named("second", (Transform::at(0., 0., 5.000001), Mesh::cube(4.)));
    let request = r#"{"op":"layout","entity":"subject","to":"camera"}"#;
    let reply = s.agent(request);
    assert!(
        reply.contains(r#""occluded":1,"occluders":["first","second"]"#),
        "{reply}"
    );
    assert!(reply.contains(r#""bearingTo":180"#), "{reply}");
    let before = (s.save(), s.world().hash(), s.world().mutation_epoch());
    assert_eq!(s.agent(request), reply);
    assert_eq!(
        (s.save(), s.world().hash(), s.world().mutation_epoch()),
        before
    );
    let mut json = json::Encoder::rounded();
    [-0.0f32, -0.00000001].write(&mut json);
    assert_eq!(json.finish().unwrap(), "[0,0]");
}

#[test]
fn inverted_model_bounds_are_rejected_at_asset_load() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    for bounds in [
        [1., 0., 0., -1., 1., 1.],
        [0., 1., 0., 1., -1., 1.],
        [0., 0., 1., 1., 1., -1.],
    ] {
        let bytes = bin::to_vec(&asset::Model {
            bounds,
            ..Default::default()
        });
        assert!(s
            .asset("bad.model", Some(&bytes))
            .unwrap_err()
            .contains("invalid bounds"));
    }
}
