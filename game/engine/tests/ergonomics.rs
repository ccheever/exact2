use exact_game::*;

#[test]
fn project_world_points_uses_current_camera_clip_volume_and_css_pixels() {
    let mut world = World::new(60, 0);
    let size = Vec2::new(1280.0, 720.0);
    assert!(world.project(Vec3::NEG_Z, size).is_none());
    let rig = world.spawn(Transform::at(4.0, 1.0, 0.0));
    let camera = world.spawn((Parent(rig), Transform::default(), Camera::default()));
    let middle = Vec3::new(4.0, 1.0, -10.0);
    assert!((world.project(middle, size).unwrap() - size * 0.5).length() < 0.001);
    assert!(world.project(middle + Vec3::X, size).unwrap().x > 640.0);
    assert!(world.project(middle + Vec3::Y, size).unwrap().y < 360.0);
    for point in [
        Vec3::new(4.0, 1.0, 1.0),
        Vec3::new(4.0, 1.0, -0.001),
        Vec3::new(4.0, 1.0, -1e6),
        Vec3::new(1000.0, 1.0, -10.0),
        Vec3::splat(f32::NAN),
    ] {
        assert!(world.project(point, size).is_none(), "{point}");
    }
    assert!(world.project(middle, Vec2::ZERO).is_none());
    assert!(world.project(middle, Vec2::splat(f32::INFINITY)).is_none());
    // A game projects while ticking, before hierarchy propagation.
    world.require_mut::<Transform>(rig).position.x += 1.0;
    assert!(world.project(middle, size).unwrap().x < 640.0);
    *world.require_mut::<Camera>(camera) = Camera::orthographic(10.0);
    let a = world.project(Vec3::new(6.0, 2.0, -5.0), size).unwrap();
    let b = world.project(Vec3::new(6.0, 2.0, -50.0), size).unwrap();
    assert!(
        (a - b).length() < 0.001,
        "orthographic points do not shrink with depth"
    );
    assert!((a - Vec2::new(712.0, 288.0)).length() < 0.001);
}

#[derive(Args, Default, Debug)]
struct Settings {
    seed: u64,
    run: u32,
    signed: i64,
    small: i32,
    gain: f32,
    precise: f64,
    title: String,
    #[live]
    paused: bool,
}
#[test]
fn typed_fields_decode_in_order_and_refuse_by_name() {
    let values = [
        Value::Number(7.0),
        Value::Number(2.0),
        Value::Number(-5.0),
        Value::Number(-3.0),
        Value::Number(0.5),
        Value::Number(0.25),
        Value::str("hello"),
        Value::Bool(false),
    ];
    let a = Settings::decode(&values).unwrap();
    assert_eq!(
        (
            a.seed,
            a.run,
            a.signed,
            a.small,
            a.gain,
            a.precise,
            a.title.as_str()
        ),
        (7, 2, -5, -3, 0.5, 0.25, "hello")
    );
    let mut b = Settings::decode(&values).unwrap();
    b.paused = true;
    assert!(!a.setup_changed(&b));
    b.run += 1;
    assert!(a.setup_changed(&b));
    assert_eq!(Settings::FIELDS[7], ("paused", ArgumentKind::Live));
    for (i, bad, expected) in [
        (0, Value::str("x"), "whole number ≥ 0"),
        (1, Value::Number(4294967296.0), "whole number"),
        (2, Value::Number(1.5), "whole number"),
        (3, Value::Number(-2147483649.0), "whole number"),
        (4, Value::Number(f64::MAX), "finite f32"),
        (5, Value::Bool(true), "finite number"),
        (6, Value::Bool(false), "text"),
        (7, Value::Number(0.0), "boolean"),
    ] {
        let mut bad_values = values.clone();
        bad_values[i] = bad;
        let error = Settings::decode(&bad_values).unwrap_err();
        assert!(
            error.starts_with(Settings::FIELDS[i].0)
                && error.contains(expected)
                && error.contains("got"),
            "{error}"
        );
        assert!(Settings::decode(&values[..i])
            .unwrap_err()
            .contains(Settings::FIELDS[i].0));
    }
    assert!(<() as Args>::decode(&[Value::Bool(false)]).is_err());
}
#[derive(Default, Data)]
struct Bookkeeping(u64);
impl Resource for Bookkeeping {
    const NAME: &'static str = "Bookkeeping";
    const AMBIENT: bool = true;
}
struct Moving;
impl Game for Moving {
    type Args = ();
    const ID: &'static str = "observed";
    fn setup(w: &mut World, _: &()) {
        w.register::<Camera>().register::<Follow>();
        w.spawn_named("player", Transform::default());
        w.spawn_named("bird", (Transform::default(), Ambient));
        w.insert_resource(Bookkeeping::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        for (_, t) in w.query::<&mut Transform>().iter() {
            // Stops after tick 3; the ambient bird continues forever.
            if w.tick() < 3 || t.position.y == 1.0 {
                t.position.x += 1.0;
            }
        }
        w.resource_mut::<Bookkeeping>().0 += 1;
    }
}
#[test]
fn last_tick_observations_zero_ticks_ambient_resources_and_saves() {
    let mut s = Sim::<Moving>::new(()).unwrap();
    let bird = s.world().named("bird").unwrap();
    s.world().get_mut::<Transform>(bird).unwrap().position.y = 1.0;
    s.advance(0.0, Clock::Seekable);
    s.advance(17.0, Clock::Seekable);
    assert!(!s.quiescent());
    let reply = s.agent(r#"{"op":"clock"}"#);
    assert!(
        reply.contains(r#""changing":["player.Transform"]"#),
        "{reply}"
    );
    s.advance(17.0, Clock::Seekable);
    assert!(!s.quiescent());
    let mut restored = Sim::<Moving>::new(()).unwrap();
    restored.restore(&s.save().unwrap()).unwrap();
    assert!(!restored.quiescent());
    s.advance(1000.0, Clock::Seekable);
    assert!(s.quiescent(), "the last tick, not the whole seek, is still");
    assert!(s.agent(r#"{"op":"clock"}"#).contains(r#""changing":[]"#));
    // busy is legal while holding a component borrow.
    let pose = s.world().get_mut::<Transform>(bird).unwrap();
    s.world().busy("pending work");
    drop(pose);
    assert!(!s.quiescent());
    assert!(s.agent(r#"{"op":"clock"}"#).contains("1100"));
    assert!(s.agent(r#"{"op":"clock"}"#).contains("1100"));
}
#[test]
fn follow_arrives_snaps_teleports_and_survives_save() {
    let mut s = Sim::<Moving>::new(()).unwrap();
    let target = s.world().named("player").unwrap();
    let camera = s.world_mut().spawn_named(
        "camera",
        (
            Transform::default(),
            Camera::default(),
            Follow::new("player").offset(0.0, 9.0, 13.0).lag(0.15),
        ),
    );
    scene::follow(s.world());
    assert_eq!(
        s.world().get::<Transform>(camera).unwrap().position,
        Vec3::new(0.0, 9.0, 13.0)
    );
    s.world().get_mut::<Transform>(target).unwrap().position.x = 2.0;
    scene::follow(s.world());
    let x = s.world().get::<Transform>(camera).unwrap().position.x;
    assert!(x > 0.0 && x < 2.0);
    // Setup declares the types that can be spawned later, without spawning a camera.
    let saved = s.save().unwrap();
    let mut other = Sim::<Moving>::new(()).unwrap();
    other.restore(&saved).unwrap();
    for _ in 0..200 {
        scene::follow(s.world());
        scene::follow(other.world());
    }
    assert_eq!(s.world().hash(), other.world().hash());
    assert_eq!(s.world().get::<Transform>(camera).unwrap().position.x, 2.0);
    s.world_mut()
        .teleport(target, Transform::at(50.0, 0.0, 0.0));
    scene::follow(s.world());
    assert_eq!(
        s.world().get::<Transform>(camera).unwrap().position,
        Vec3::new(50.0, 9.0, 13.0)
    );
    let mut scalar = 0.0;
    for _ in 0..200 {
        scalar = math::ease(scalar, 1.0, 0.15, 1.0 / 60.0);
    }
    assert_eq!(scalar, 1.0);
}
#[test]
fn wildcard_state_is_complete_bounded_and_narrowable() {
    let mut s = Sim::<Moving>::new(()).unwrap();
    let parent = s.world().named("player").unwrap();
    s.world_mut()
        .spawn_named("child", (Parent(parent), Transform::at(1.0, 2.0, 3.0)));
    let reply = s.agent(r#"{"op":"state","entity":"*","under":"player"}"#);
    assert_eq!(reply.matches("\"components\":").count(), 2);
    assert!(reply.contains(&format!("\"hash\":\"0x{:016x}\"", s.world().hash())));
    assert!(reply.contains("child") && !reply.contains("bird"));
    for _ in 0..520 {
        s.world_mut().spawn(Transform::default());
    }
    let reply = s.agent(r#"{"op":"state","entity":"*"}"#);
    assert_eq!(reply.matches("\"components\":").count(), 512);
    assert!(reply.contains(r#""truncated":true"#) && reply.contains(r#""next":512"#));
    let reply = s.agent(r#"{"op":"state","entity":"*","from":512,"resources":true}"#);
    let total = s.world().len();
    assert_eq!(reply.matches("\"components\":").count(), total - 512);
    assert!(reply.contains(r#""truncated":false"#) && reply.contains(r#""resources":{"#));
}

#[test]
fn seeks_hash_only_rows_written_since_and_live_never_hashes() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static WRITES: AtomicUsize = AtomicUsize::new(0);
    #[derive(Default)]
    struct Counted;
    impl Data for Counted {
        fn write(&self, w: &mut dyn Writer) {
            WRITES.fetch_add(1, Ordering::Relaxed);
            0u32.write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            0u32.read(r)
        }
    }
    impl Component for Counted {
        const NAME: &'static str = "Counted";
    }
    let mut s = Sim::<Moving>::new(()).unwrap();
    s.world_mut().spawn(Counted);
    s.advance(0.0, Clock::Seekable);
    // A new row is hashed once; a seek's two samples share its unwritten page.
    s.advance(60_000.0, Clock::Seekable);
    assert_eq!(WRITES.swap(0, Ordering::Relaxed), 1);
    s.advance(60_000.0, Clock::Seekable);
    assert_eq!(WRITES.load(Ordering::Relaxed), 0);
    s.advance(60_017.0, Clock::Seekable);
    assert_eq!(WRITES.load(Ordering::Relaxed), 0);
    let _ = s.world().query::<&mut Counted>().iter().count();
    s.advance(60_034.0, Clock::Seekable);
    assert_eq!(WRITES.swap(0, Ordering::Relaxed), 1);
    s.advance(60_100.0, Clock::Live);
    assert_eq!(WRITES.load(Ordering::Relaxed), 0);
}

#[test]
fn engine_places_followers_after_setup_rebuild_and_restore_without_an_extra_tick() {
    #[derive(Default, Args)]
    struct Options {
        x: f32,
    }
    struct Following;
    impl Game for Following {
        const ID: &'static str = "placement";
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            w.spawn_named("player", Transform::at(args.x, 2.0, 3.0));
            w.spawn_named(
                "camera",
                (
                    Transform::default(),
                    Follow::new("player").offset(0.0, 9.0, 13.0).lag(0.15),
                ),
            );
        }
        fn tick(w: &mut World, _: &Input, _: &Options) {
            w.get_mut::<Transform>("player").unwrap().position.x += 1.0;
            scene::follow(w);
        }
    }
    let mut s = Sim::<Following>::new(Options { x: 4.0 }).unwrap();
    assert_eq!(
        s.world().get::<Transform>("camera").unwrap().position,
        Vec3::new(4.0, 11.0, 16.0)
    );
    s.bind(&[Value::Number(8.0)], None).unwrap();
    assert_eq!(
        s.world().get::<Transform>("camera").unwrap().position,
        Vec3::new(8.0, 11.0, 16.0)
    );
    s.run(100.0);
    let saved = s.save().unwrap();
    let snapshot = s.agent(r#"{"op":"state","entity":"*"}"#);
    let mut restored = Sim::<Following>::new(Options::default()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(snapshot, restored.agent(r#"{"op":"state","entity":"*"}"#));
    s.run(100.0);
    restored.run(100.0);
    assert_eq!(s.save().unwrap(), restored.save().unwrap());
    // A newly added or teleported follower can be unplaced in a carry.
    s.world_mut().spawn_named(
        "new-camera",
        (
            Transform::default(),
            Follow::new("player").offset(0.0, 5.0, 8.0),
        ),
    );
    restored.restore(&s.save().unwrap()).unwrap();
    let player = restored
        .world()
        .get::<Transform>("player")
        .unwrap()
        .position;
    assert_eq!(
        restored
            .world()
            .get::<Transform>("new-camera")
            .unwrap()
            .position,
        player + Vec3::new(0.0, 5.0, 8.0)
    );
}

#[test]
fn record_publication_keeps_names_nested_values_and_saved_state() {
    #[derive(Default, Data)]
    struct Detail {
        label: String,
        active: bool,
    }
    #[derive(Default, Data)]
    struct Hud {
        beacons: u32,
        detail: Detail,
        rows: Vec<Detail>,
        optional: Option<u32>,
        values: Vec<u32>,
    }
    let hud = Hud {
        beacons: 2,
        detail: Detail {
            label: "two".into(),
            active: true,
        },
        rows: vec![Detail::default()],
        optional: Some(4),
        values: vec![1, 3],
    };
    let mut sim = Sim::<Moving>::new(()).unwrap();
    sim.world().publish_record(&hud);
    let expected = r#"{"beacons":2,"detail":{"active":true,"label":"two"},"optional":4,"rows":[{"active":false,"label":""}],"values":[1,3]}"#;
    assert_eq!(sim.take_published().as_deref(), Some(expected));
    sim.world().publish_record(&hud);
    assert!(sim.take_published().is_none());
    sim.world().publish("beacons", 3);
    assert_eq!(
        sim.world().published("beacons").unwrap().as_number(),
        Some(3.0)
    );
    let saved = sim.save().unwrap();
    let mut restored = Sim::<Moving>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(sim.take_published(), restored.take_published());
    assert_eq!(saved, restored.save().unwrap());
}

#[test]
fn publication_preserves_owned_and_shared_contract_values() {
    let mut sim = Sim::<Moving>::new(()).unwrap();
    let saved = sim.world().save();
    let hash = sim.world().hash();
    let mut source = String::from("beacon ready");
    sim.world().publish("text", source.as_str());
    source.clear();
    source.push_str("changed after publication");
    drop(source);
    assert_eq!(
        sim.take_published().as_deref(),
        Some(r#"{"text":"beacon ready"}"#)
    );
    let events = sim.world().journal().len();
    let same = String::from("beacon ready");
    sim.world().publish("text", same.as_str());
    drop(same);
    assert!(sim.take_published().is_none());
    assert_eq!(sim.world().journal().len(), events);
    sim.world().publish("text", String::from("beacon ready"));
    assert!(sim.take_published().is_none());
    assert_eq!(sim.world().journal().len(), events);
    sim.world().publish("text", String::from("lit"));
    assert_eq!(sim.take_published().as_deref(), Some(r#"{"text":"lit"}"#));

    let value = || {
        Value::record(vec![
            Value::str("héllo"),
            Value::list(vec![Value::Number(7.0), Value::Bool(true)]),
            Value::Option(Some(std::rc::Rc::new(Value::str("present")))),
        ])
    };
    let shared = value();
    sim.world().publish("record", shared.clone());
    assert_eq!(
        shared,
        value(),
        "publication must leave shared inputs intact"
    );
    assert_eq!(sim.world().published("record"), Some(shared.clone()));
    assert_eq!(
        sim.take_published().as_deref(),
        Some(r#"{"record":["héllo",[7,true],"present"],"text":"lit"}"#)
    );
    let events = sim.world().journal().len();
    sim.world().publish("record", value());
    assert!(sim.take_published().is_none());
    assert_eq!(sim.world().journal().len(), events);
    assert_eq!(sim.world().published("record"), Some(shared));

    sim.world().publish("number", 0.0_f64);
    assert!(sim.take_published().is_some());
    let events = sim.world().journal().len();
    sim.world().publish("number", -0.0_f64);
    assert!(sim.take_published().is_none());
    assert_eq!(sim.world().journal().len(), events);
    sim.world().publish("number", f64::NAN);
    let with_nan = r#"{"number":null,"record":["héllo",[7,true],"present"],"text":"lit"}"#;
    assert_eq!(sim.take_published().as_deref(), Some(with_nan));
    let events = sim.world().journal().len();
    sim.world().publish("number", f64::NAN);
    assert_eq!(sim.take_published().as_deref(), Some(with_nan));
    assert_eq!(sim.world().journal().len(), events + 1);
    assert_eq!(sim.world().hash(), hash);
    assert_eq!(sim.world().save(), saved);
}

#[test]
fn proximity_uses_both_parent_chains_and_returns_mutable_global_rows() {
    #[derive(Default, Component)]
    struct Beacon {
        lit: bool,
    }
    let mut w = World::new(60, 0);
    let parent = w.spawn(Transform::at(10., 4., 0.));
    let player = w.spawn_named("player", (Parent(parent), Transform::at(2., 0., 0.)));
    let other_parent = w.spawn(Transform::at(11., 4., 0.));
    let close = w.spawn((
        Parent(other_parent),
        Transform::at(2., 0., 0.),
        Beacon::default(),
    ));
    let high = w.spawn((Transform::at(12., 100., 0.), Beacon::default()));
    w.spawn((Transform::at(2., 0., 0.), Beacon::default()));
    let rows: Vec<_> = w.near::<Beacon>(player, 1.).collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, close);
    assert_eq!(rows[0].1.position, Vec3::new(13., 4., 0.));
    let mut ids = Vec::new();
    for (e, _) in w.near_xz::<Beacon>("player", 1.) {
        w.get_mut::<Beacon>(e).unwrap().lit = true;
        ids.push(e);
    }
    assert_eq!(ids, [close, high]);
    assert_eq!(w.near::<Beacon>("missing", 1.).count(), 0);
    w.get_mut::<Transform>(parent).unwrap().position.x = 50.;
    assert_eq!(w.near_xz::<Beacon>(player, 1.).count(), 0);
}

#[test]
fn owned_entity_names_keep_resolution_diagnostics_and_custom_targets() {
    struct Routed {
        entity: Entity,
        label: &'static str,
    }
    impl Target for Routed {
        fn label(&self) -> String {
            self.label.into()
        }
        fn entity(&self, _: &World) -> Option<Entity> {
            Some(self.entity)
        }
    }

    let mut w = World::new(60, 0);
    let fox = w.spawn_named("fox", Transform::at(3., 0., 0.));
    assert_eq!(w.get::<Transform>("fox").unwrap().position.x, 3.);
    let name = String::from("fox");
    assert_eq!(w.get::<Transform>(&name).unwrap().position.x, 3.);
    let retained = w.require::<Transform>(name);
    assert_eq!(retained.position.x, 3.);
    drop(retained);

    let named_slot = format!("fox#{}", fox.index());
    assert_eq!(w.get::<Transform>(&named_slot).unwrap().position.x, 3.);
    let slot = format!("#{}", fox.index());
    assert_eq!(w.require::<Transform>(slot).position.x, 3.);

    let missing = String::from("missing");
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        w.require::<Mesh>(&missing);
    }))
    .unwrap_err();
    let message = failure.downcast_ref::<String>().unwrap();
    assert!(
        message.contains("missing") && message.contains("Mesh"),
        "{message}"
    );
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        w.require::<Mesh>(String::from("owned missing"));
    }))
    .unwrap_err();
    let message = failure.downcast_ref::<String>().unwrap();
    assert!(message.contains("owned missing"), "{message}");

    assert!(w.despawn(fox));
    let replacement = w.spawn(Transform::default());
    assert_eq!(replacement.index(), fox.index());
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        w.require::<Transform>(Routed {
            entity: fox,
            label: "remembered fox",
        });
    }))
    .unwrap_err();
    let message = failure.downcast_ref::<String>().unwrap();
    assert!(message.contains("remembered fox"), "{message}");
}

#[test]
fn numeric_record_lists_preserve_values_through_publication_and_restore() {
    #[derive(Default, Data)]
    struct Hud {
        bytes: Vec<u8>,
        shorts: Vec<u16>,
        words: Vec<u32>,
        floats: Vec<f32>,
    }
    let mut sim = Sim::<Moving>::new(()).unwrap();
    sim.world().publish_record(&Hud {
        bytes: vec![0, u8::MAX],
        shorts: vec![0, u16::MAX],
        words: vec![0, u32::MAX],
        floats: vec![-0.0, 1.25, f32::from_bits(0xffa12345), f32::INFINITY],
    });
    let Value::List(values) = sim.world().published("floats").unwrap() else {
        panic!("numeric vector must publish a list");
    };
    assert_eq!(
        values[0].as_number().unwrap().to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(
        values[2].as_number().unwrap().to_bits(),
        (f32::NAN as f64).to_bits()
    );
    assert_eq!(
        sim.take_published().unwrap(),
        r#"{"bytes":[0,255],"floats":[-0,1.25,null,null],"shorts":[0,65535],"words":[0,4294967295]}"#
    );
    let saved = sim.save().unwrap();
    let mut restored = Sim::<Moving>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(saved, restored.save().unwrap());
    assert_eq!(sim.world().hash(), restored.world().hash());
}

#[test]
fn record_units_and_safe_integer_boundaries() {
    #[derive(Default, Data)]
    struct Hud {
        unit: (),
        unsigned: u64,
        signed: i64,
    }
    let mut sim = Sim::<Moving>::new(()).unwrap();
    let limit = 9_007_199_254_740_991;
    sim.world().publish_record(&Hud {
        unit: (),
        unsigned: limit,
        signed: -(limit as i64),
    });
    assert_eq!(
        sim.take_published().unwrap(),
        r#"{"signed":-9007199254740991,"unit":null,"unsigned":9007199254740991}"#
    );
    for (unsigned, signed) in [
        (limit + 1, 0),
        (u64::MAX, 0),
        (0, limit as i64 + 1),
        (0, -(limit as i64) - 1),
        (0, i64::MIN),
    ] {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim
            .world()
            .publish_record(&Hud {
                unit: (),
                unsigned,
                signed
            })))
        .is_err());
        assert!(
            sim.take_published().is_none(),
            "refusal must not partially publish"
        );
    }
}
#[test]
fn record_optional_unit_is_explicitly_refused_when_present() {
    #[derive(Default, Data)]
    struct Hud {
        unit: Option<()>,
    }
    let mut sim = Sim::<Moving>::new(()).unwrap();
    sim.world().publish_record(&Hud { unit: None });
    assert_eq!(sim.take_published().unwrap(), r#"{"unit":null}"#);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim
        .world()
        .publish_record(&Hud { unit: Some(()) })))
    .is_err());
}
#[test]
fn typed_and_wire_layout_agree_without_transform() {
    let mut sim = Sim::<Moving>::new(()).unwrap();
    sim.world_mut()
        .spawn((Transform::at(0., 0., 10.), Camera::default()));
    sim.world_mut().spawn_named("marker", Mesh::cube(1.));
    sim.world_mut().propagate();
    sim.viewport(800., 600.);
    let typed = sim
        .layout("marker")
        .expect("wire's identity pose is shared");
    let wire = sim.agent(r#"{"op":"layout","entity":"marker"}"#);
    #[derive(Default, Data)]
    struct Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }
    #[derive(Default, Data)]
    struct Row {
        screen: Rect,
    }
    #[derive(Default, Data)]
    struct Reply {
        entity: Row,
    }
    let rect = json::from_str::<Reply>(&wire).unwrap().entity.screen;
    for (typed, wire) in [
        (typed.screen.x, rect.x),
        (typed.screen.y, rect.y),
        (typed.screen.w, rect.w),
        (typed.screen.h, rect.h),
    ] {
        assert!(
            (typed - wire).abs() <= 0.0001,
            "agent rounds to four decimal places: {typed} vs {wire}"
        );
    }
}
