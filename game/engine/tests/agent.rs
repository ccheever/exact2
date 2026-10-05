use exact_game::*;
struct Scene;
impl Game for Scene {
    type Args = ();
    const ID: &'static str = "Scene";
    fn actions() -> Actions {
        Actions::new().button("act", &["KeyE"])
    }
    fn setup(w: &mut World, _: &Self::Args) {
        let child = w.spawn_named("child", Transform::at(0.0, 1.0, 0.0));
        let root = w.spawn_named("root", Transform::at(2.0, 0.0, 0.0));
        w.insert(child, Parent(root));
        w.spawn_named("camera", (Transform::at(0.0, 0.0, 10.0), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
}

fn assert_pending(s: &mut Sim<Scene>, counts: [usize; 6]) {
    let saved = s.save().unwrap();
    let [key, pointer, control, wheel, blur, message] = counts;
    let total: usize = counts.iter().sum();
    let expected = format!(
        r#""pending":{{"total":{total},"key":{key},"pointer":{pointer},"control":{control},"wheel":{wheel},"blur":{blur},"message":{message}}}"#
    );
    let reply = s.agent(r#"{"op":"state"}"#);
    assert!(reply.contains(&expected), "{reply}");
    assert!(reply.len() < 4096, "pending payloads must not expand state");
    assert_eq!(
        s.save().unwrap(),
        saved,
        "inspection preserves the full save"
    );
}

#[test]
fn pending_blurs_explain_different_saves_at_the_same_world_hash() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    s.advance(1000., Clock::Seekable);
    let before = s.save().unwrap();
    let hash = s.world().hash();
    assert_pending(&mut s, [0; 6]);
    for _ in 0..3 {
        s.input(InputEvent::Blur { at_ms: 1000. });
    }
    assert_eq!(s.world().hash(), hash);
    let saved = s.save().unwrap();
    assert_ne!(saved, before);
    assert_pending(&mut s, [0, 0, 0, 0, 3, 0]);
    let mut restored = Sim::<Scene>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert_pending(&mut restored, [0, 0, 0, 0, 3, 0]);
    restored.advance(7000., Clock::Seekable);
    assert_pending(&mut restored, [0, 0, 0, 0, 3, 0]);
    restored.advance(7100., Clock::Seekable);
    s.advance(1100., Clock::Seekable);
    assert_pending(&mut s, [0; 6]);
    assert_pending(&mut restored, [0; 6]);
    assert_eq!(restored.save().unwrap(), s.save().unwrap());
}

#[test]
fn pending_counts_describe_retained_events_without_copying_their_payloads() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    s.advance(0., Clock::Seekable);
    for _ in 0..2 {
        s.input(InputEvent::Key {
            code: "KeyE".into(),
            down: true,
            at_ms: 1.,
        });
    }
    for _ in 0..2 {
        s.input(InputEvent::Pointer {
            id: 1,
            phase: PointerPhase::Move,
            x: 1.,
            y: 2.,
            dx: 1.,
            dy: 2.,
            buttons: 0,
            at_ms: 1.,
        });
        s.input(InputEvent::Wheel {
            dx: 1.,
            dy: 2.,
            at_ms: 1.,
        });
    }
    s.input(InputEvent::Control {
        name: "act".into(),
        id: 2,
        phase: PointerPhase::Down,
        x: 0.,
        y: 0.,
        at_ms: 1.,
    });
    for _ in 0..2 {
        s.input(InputEvent::Blur { at_ms: 1. });
    }
    for _ in 0..3 {
        s.post("x".repeat(64 * 1024));
    }
    assert_pending(&mut s, [1, 1, 1, 1, 2, 3]);
    let saved = s.save().unwrap();
    s.restore(&saved).unwrap();
    assert_pending(&mut s, [1, 1, 1, 1, 2, 3]);
    s.advance(3000., Clock::Seekable);
    assert_pending(&mut s, [1, 1, 1, 1, 2, 3]);
    s.advance(3100., Clock::Seekable);
    assert_pending(&mut s, [0; 6]);
}
#[test]
fn hierarchy_preorder_under_and_cap() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    assert_eq!(
        s.agent(r#"{"op":"tree","under":"root"}"#),
        r#"{"tick":0,"entities":[{"id":1,"name":"root","parent":null,"depth":0,"components":["Transform"],"tags":[]},{"id":0,"name":"child","parent":1,"depth":1,"components":["Parent","Transform"],"tags":[]}],"truncated":false,"total":2}"#
    );
    assert!(s
        .agent(r#"{"op":"layout","entity":"child"}"#)
        .contains("\"position\":[2.0,1.0,0.0]"));
    for _ in 0..520 {
        s.world_mut().spawn(());
    }
    let tree = s.agent(r#"{"op":"tree"}"#);
    assert_eq!(tree.matches("\"id\":").count(), 512);
    assert!(tree.ends_with("\"truncated\":true,\"total\":523,\"next\":512}"));
    // The forest's 2k-tree world was refused as truncated: pages reach every entity.
    let rest = s.agent(r#"{"op":"tree","from":512,"limit":100}"#);
    assert_eq!(rest.matches("\"id\":").count(), 11);
    assert!(rest.ends_with("\"truncated\":false,\"total\":523}"));
    let all = s.agent(r#"{"op":"tree","limit":100000}"#);
    assert_eq!(all.matches("\"id\":").count(), 523);
    assert!(s
        .agent(r#"{"op":"tree","limit":0}"#)
        .contains("limit must be 1..=100000"));
    assert_eq!(
        s.agent(r#"{"op":"tree","under":"bad"}"#),
        r#"{"tick":0,"error":"no entity named `bad`; `tree world` lists names; add `w.spawn_named(\"bad\", (Transform::default(),));` in setup if intended"}"#
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
    let position = json::to_string(&s.global_position("child").unwrap()).unwrap();
    assert!(s
        .agent(r#"{"op":"layout","entity":"child"}"#)
        .contains(&format!("\"position\":{position}")));
    assert!(position.contains("2.1234567"), "{position}");
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
    assert!(s
        .agent(r#"{"op":"layout","entity":"child","width":800,"height":600}"#)
        .contains("\"inFrustum\":true"));
    s.world_mut().teleport(e, Transform::at(0.0, 0.0, 20.0));
    let behind = s.agent(r#"{"op":"layout","entity":"child"}"#);
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

#[test]
fn busy_is_returned_only_when_requested_and_clock_without_now_does_not_seek() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    s.advance(0., Clock::Seekable);
    s.advance(1000., Clock::Seekable);
    let before = s.world().tick();
    assert!(s
        .agent(r#"{"op":"state","entity":"*","busy":true}"#)
        .contains("\"busy\":"));
    assert!(!s
        .agent(r#"{"op":"state","entity":"*"}"#)
        .contains("\"busy\":"));
    s.agent(r#"{"op":"clock"}"#);
    assert_eq!(s.world().tick(), before);
}
#[test]
fn restore_format_refusal_is_not_double_wrapped() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    let error = s.restore(b"old-format").unwrap_err().to_string();
    assert_eq!(error.matches("restore refused").count(), 1, "{error}");
    assert!(
        error.contains("expected EXSIM v7") && error.contains("no cross-version migration"),
        "{error}"
    );
    assert!(!error.contains("named additions"), "{error}");
}

#[test]
fn cycle_refusal_contains_the_entities_and_parent_ids() {
    let mut s = Sim::<Scene>::new(()).unwrap();
    let w = s.world_mut();
    let a = w.resolve("root").unwrap();
    let b = w.resolve("child").unwrap();
    w.insert(a, Parent(b));
    for op in [r#"{"op":"tree"}"#, r#"{"op":"state","entity":"*"}"#] {
        let reply = s.agent(op);
        assert!(
            reply.contains("root")
                && reply.contains("child")
                && reply.contains("Parent #0")
                && reply.contains("Parent #1"),
            "{reply}"
        );
        assert!(!reply.contains("show Parent components"));
    }
}

#[test]
fn screen_bounds_clip_near_without_changing_saved_state() {
    struct Clipped;
    impl Game for Clipped {
        const ID: &'static str = "clipped-bounds";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named(
                "camera",
                (
                    Transform::default(),
                    Camera {
                        near: 1.,
                        far: 100.,
                        fov_y_degrees: 90.,
                        ..Camera::default()
                    },
                ),
            );
            w.spawn_named("box", (Transform::at(0., 0., -1.), Mesh::cube(2.)));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut s = Sim::<Clipped>::new(()).unwrap();
    s.viewport(200., 200.);
    let saved = s.save().unwrap();
    let rect = s.layout("box").unwrap().screen;
    for (a, b) in [rect.x, rect.y, rect.w, rect.h]
        .into_iter()
        .zip([0., 0., 200., 200.])
    {
        assert!((a - b).abs() < 0.0001, "{rect:?}");
    }
    assert_eq!(
        s.save().unwrap(),
        saved,
        "projection cannot change the complete save"
    );
    let entity = s.world().named("box").unwrap();
    s.world_mut().teleport(entity, Transform::at(0., 0., 2.));
    assert!(s.layout("box").is_none());
    // Exactly on the near plane, the rear face still supplies finite bounds.
    s.world_mut().teleport(entity, Transform::at(0., 0., 0.));
    let rect = s.layout("box").unwrap().screen;
    assert!((rect.w - 200.).abs() < 0.0001 && (rect.h - 200.).abs() < 0.0001);
}
