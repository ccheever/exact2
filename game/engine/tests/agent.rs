use exact_game::*;
struct Scene;
impl Game for Scene {
    type Args = ();
    const ID: &'static str = "Scene";
    fn setup(w: &mut World, _: &Self::Args) {
        let child = w.spawn_named("child", Transform::at(0.0, 1.0, 0.0));
        let root = w.spawn_named("root", Transform::at(2.0, 0.0, 0.0));
        w.insert(child, Parent(root));
        w.spawn_named("camera", (Transform::at(0.0, 0.0, 10.0), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
}
#[test]
fn hierarchy_preorder_under_and_cap() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    assert_eq!(
        s.agent(r#"{"op":"tree","under":"root"}"#),
        r#"{"tick":0,"entities":[{"id":1,"name":"root","parent":null,"depth":0,"components":["Transform"],"tags":[]},{"id":0,"name":"child","parent":1,"depth":1,"components":["Parent","Transform"],"tags":[]}],"truncated":false}"#
    );
    assert!(s
        .agent(r#"{"op":"layout","entity":"child"}"#)
        .contains("\"position\":[2,1,0]"));
    for _ in 0..520 {
        s.world_mut().spawn(());
    }
    let tree = s.agent(r#"{"op":"tree"}"#);
    assert_eq!(tree.matches("\"id\":").count(), 512);
    assert!(tree.ends_with("\"truncated\":true}"));
    assert_eq!(
        s.agent(r#"{"op":"tree","under":"bad"}"#),
        r#"{"tick":0,"error":"no entity named `bad`"}"#
    );
}
#[test]
fn pick_uses_oriented_boxes_and_exact_spheres_and_capsules() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    s.viewport(800.0, 600.0);
    let e = s
        .world_mut()
        .spawn_named("shape", (Transform::default(), Mesh::sphere(1.0)));
    s.world_mut().propagate();
    let pick = |s: &mut Sim<Scene>, x: f32, y: f32| {
        s.agent(&format!("{{\"op\":\"layout\",\"x\":{x},\"y\":{y}}}"))
    };
    assert!(pick(&mut s, 400.0, 300.0).contains("\"distance\":9"));
    // This ray intersects the sphere's box near its corner, but misses the sphere.
    assert!(pick(&mut s, 447.0, 253.0).contains("\"hit\":null"));
    s.world_mut().insert(
        e,
        Mesh::Capsule {
            radius: 0.4,
            height: 1.0,
        },
    );
    assert!(pick(&mut s, 400.0, 300.0).contains("\"distance\":9.6"));
    s.world_mut().insert(e, Mesh::cube(1.0));
    s.world_mut().teleport(
        e,
        Transform {
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
            ..Transform::default().with_scale(Vec3::new(4.0, 0.2, 1.0))
        },
    );
    assert!(pick(&mut s, 400.0, 300.0).contains("\"distance\":9.5"));
    // World AABB includes (1,-1,0); the narrow rotated box does not.
    assert!(pick(&mut s, 450.0, 350.0).contains("\"hit\":null"));
    s.world_mut().insert(e, Visible(false));
    assert!(pick(&mut s, 400.0, 300.0).contains("\"hit\":null"));
}
#[test]
fn projection_refusals_and_lossless_state_are_read_only() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    let e = s.world().named("child").unwrap();
    s.world_mut()
        .teleport(e, Transform::at(0.1234567, 0.0, 0.0));
    let hash = s.world().hash();
    assert!(s
        .agent(r#"{"op":"state","entity":"child"}"#)
        .contains("0.1234567"));
    assert!(s
        .agent(r#"{"op":"layout","entity":"child"}"#)
        .contains("\"position\":[2.1235,0,0]"));
    assert!(s
        .agent(r#"{"op":"layout","x":0,"y":0}"#)
        .contains("needs an active camera and viewport"));
    for request in [
        r#"{"op":"tree","width":0,"height":600}"#,
        r#"{"op":"state","now":"late"}"#,
        r#"{"op":"logs","since":-1}"#,
        r#"{"op":"tree","op":"state"}"#,
        r#"{"op":"tree"} trailing"#,
    ] {
        assert!(s.agent(request).contains("\"error\":"), "{request}");
    }
    assert_eq!(s.world().hash(), hash);
    let saved = s.save();
    assert!(s
        .agent(r#"{"op":"layout","entity":"child","width":800,"height":600}"#)
        .contains("\"inFrustum\":true"));
    assert_eq!(s.save(), saved);
    s.world_mut().teleport(e, Transform::at(0.0, 0.0, 20.0));
    let behind = s.agent(r#"{"op":"layout","entity":"child","width":800,"height":600}"#);
    assert!(behind.contains("\"behindCamera\":true"));
    assert!(behind.contains("\"inFrustum\":false"));
}

#[test]
fn capsule_pick_from_inside_ignores_internal_cap_surfaces() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    s.viewport(800.0, 600.0);
    let camera = s.world().named("camera").unwrap();
    s.world_mut()
        .teleport(camera, Transform::default().looking_at(Vec3::Y, Vec3::Z));
    s.world_mut().spawn_named(
        "capsule",
        (
            Transform::default(),
            Mesh::Capsule {
                radius: 1.0,
                height: 3.0,
            },
        ),
    );
    s.world_mut().propagate();
    let reply = s.agent(r#"{"op":"layout","x":400,"y":300}"#);
    assert!(reply.contains("\"distance\":1.5"), "{reply}");
}

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
        r#"{"tick":0,"entity":{"id":0,"name":"subject","world":{"position":[0,0,0],"rotation":[0,0,0,1],"scale":[1,1,1]},"bounds":{"min":[-1,-1,-1],"max":[1,1,1]},"screen":{"unavailable":true},"depth":null,"visible":{"unavailable":true},"facing":{"forward":[0,0,-1],"towardCamera":null,"bearingTo":0,"distanceTo":10,"lineOfSight":true}}}"#
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
    assert!(reply.contains(r#""forward":[-1,0,-0]"#), "{reply}");
    assert!(
        reply.contains(r#""bearingTo":-90,"distanceTo":10,"lineOfSight":false"#),
        "{reply}"
    );
}

#[test]
fn model_bounds_replace_unit_fallback_for_layout_pick_and_occlusion() {
    let mut s = Sim::<Eyes>::new(()).unwrap();
    s.viewport(800.0, 600.0);
    let subject = s.world().named("subject").unwrap();
    s.world_mut().insert(subject, Mesh::asset("subject.model"));
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
