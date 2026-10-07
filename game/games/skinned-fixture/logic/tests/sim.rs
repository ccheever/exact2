#[path = "../../../../bake/tests/support/mod.rs"]
mod baked;
#[path = "../../../../render/tests/fixture/device.rs"]
mod gpu_test;
use exact_game::*;
use skinned_fixture_logic::{Options, SmallGame};
fn assets() -> std::collections::BTreeMap<String, Vec<u8>> {
    baked::assets(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../art/fox.glb")).unwrap()
}
fn sim() -> Sim<SmallGame> {
    let assets = assets();
    let mut s = Sim::with_assets(Options::default(), |name| {
        assets.get(name).cloned().ok_or(name.to_owned())
    })
    .unwrap();
    s.viewport(1280., 720.);
    s
}
#[test]
fn pinned_pose_and_hash() {
    let mut s = sim();
    s.run(1000.);
    let pose = s.agent_with_inspector(
        r#"{"op":"state","entity":"fox","pose":true}"#,
        |_, _| {},
        animation::inspect,
    );
    let hash = s.world().hash();
    println!("tick60 0x{hash:016x}\n{pose}");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tick60.json");
    if std::env::var_os("EXACT_PIN_POSE").is_some() {
        std::fs::write(&path, &pose).unwrap();
    }
    s.assert_pin(include_str!("../../pins.json"));
    assert_eq!(pose, std::fs::read_to_string(path).unwrap());
    s.run(1000.);
    println!("tick120 0x{:016x}", s.world().hash());
    assert_eq!(s.world().tick(), 120);
    s.assert_pin(include_str!("../../pins.json"));
}
#[test]
fn paranoid_roundtrip_every_tick_and_mid_fade_fresh_process() {
    for mode in [Paranoid::Save, Paranoid::FreshGame] {
        let mut reference = sim().paranoid(Paranoid::Off);
        let mut paranoid = sim().paranoid(mode);
        let start = std::time::Instant::now();
        for tick in 1..=120 {
            reference.run(1000. / 60. + 0.0001);
            paranoid.run(1000. / 60. + 0.0001);
            assert_eq!(
                reference.world().hash(),
                paranoid.world().hash(),
                "{mode:?} tick {tick}"
            );
            assert_eq!(
                *reference.get::<Pose>("fox").unwrap().local,
                *paranoid.get::<Pose>("fox").unwrap().local
            );
            assert!(
                reference.save().unwrap() == paranoid.save().unwrap(),
                "{mode:?} bytes at {tick}"
            );
        }
        println!(
            "PARANOID skinned-fixture {mode:?} 120 tick pairs: {:?}",
            start.elapsed()
        );
    }
    let mut a = sim();
    a.run(750.);
    let saved = a.save().unwrap();
    a.run(1250.);
    let mut b = sim();
    b.restore(&saved).unwrap();
    b.run(1250.);
    assert_eq!(a.world().hash(), b.world().hash());
    assert_eq!(a.save().unwrap(), b.save().unwrap());
}
#[test]
fn fox_leg_ik_and_socket() {
    let mut s = sim();
    s.run(500.);
    let model = s.world().model("fox.model").unwrap();
    let chain = ["b_LeftLeg01_015", "b_LeftLeg02_016", "b_LeftFoot01_017"].map(String::from);
    let ids = chain
        .each_ref()
        .map(|n| model.nodes.iter().position(|v| &v.name == n).unwrap() as u32);
    let original = s.get::<Pose>("fox").unwrap().local.clone();
    let [a, b, c] = ids.map(|i| {
        animation::joint_matrix(model, &original, i)
            .w_axis
            .truncate()
    });
    let target = a + (c - a) * 0.8 + Vec3::new(0.05, 0., 0.);
    let pole = b + Vec3::new(0., 0.5, 0.);
    let mut local = original.clone();
    let mut ik = Ik {
        chain,
        target,
        pole,
        weight: 0.,
    };
    animation::solve_ik(model, &mut local, &ik).unwrap();
    assert_eq!(local, original);
    ik.weight = 1.;
    animation::solve_ik(model, &mut local, &ik).unwrap();
    let reached = animation::joint_matrix(model, &local, ids[2])
        .w_axis
        .truncate();
    assert!(
        reached.distance(target) < 1e-4,
        "{reached:?} target {target:?}"
    );
    local.clone_from(&original);
    ik.target = a + Vec3::X * 1000.;
    animation::solve_ik(model, &mut local, &ik).unwrap();
    let tip = animation::joint_matrix(model, &local, ids[2])
        .w_axis
        .truncate();
    assert!((tip - a).normalize().distance(Vec3::X) < 1e-4);
    assert!(((tip - a).length() - (a.distance(b) + b.distance(c))).abs() < 1e-4);
    let socket = animation::socket(s.world(), "fox", "b_Head_05").unwrap();
    let head = animation::socket_node(s.world(), "fox", "b_Head_05").unwrap();
    let fox = *s.get::<Transform>("fox").unwrap();
    let local = s.get::<Pose>("fox").unwrap();
    let expected =
        exact_game::Mat4::from_scale_rotation_translation(fox.scale, fox.rotation, fox.position)
            * animation::joint_matrix(model, &local.local, head);
    assert!(socket.position.distance(expected.w_axis.truncate()) < 1e-4);
    // Displayed charm uses the local chain, not its simulation fallback Transform.
    let Some(gpu) = gpu_test::device_or_skip(exact_game_render::exact_gpu::fixture::device())
    else {
        return;
    };
    let mut renderer = exact_game_render::Renderer::new(
        &gpu.device,
        &gpu.queue,
        exact_game_render::exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    renderer.prepare_model("fox.model", model).unwrap();
    let mut feed = exact_game_render::Feed::default();
    feed.feed(s.world(), &mut renderer).unwrap();
    let frame = feed.frame(s.world(), 1., 16. / 9.);
    let charm = frame
        .attachments
        .iter()
        .find(|a| a.entity == s.world().named("charm").unwrap())
        .unwrap()
        .pose;
    let offset = s.get::<SocketFollow>("charm").unwrap().offset;
    let expected = (expected
        * exact_game::Mat4::from_scale_rotation_translation(
            offset.scale,
            offset.rotation,
            offset.position,
        ))
    .w_axis
    .truncate();
    assert!(
        s.world()
            .global_position("charm")
            .unwrap()
            .distance(expected)
            < 1e-4
    );
    drop(local);
    #[derive(Default, exact_game::Data)]
    struct Layout {
        entity: EntityLayout,
    }
    #[derive(Default, exact_game::Data)]
    struct EntityLayout {
        world: WorldPose,
    }
    #[derive(Default, exact_game::Data)]
    struct WorldPose {
        position: Vec3,
    }
    let layout: Layout =
        exact_game::json::from_str(&s.agent(r#"{"op":"layout","entity":"charm"}"#)).unwrap();
    assert!(
        layout.entity.world.position.distance(expected) < 1e-4,
        "layout world:charm sits on the head plus offset"
    );
    assert!(
        charm.position.distance(expected) < 1e-4,
        "{charm:?}, expected {expected:?}"
    );
}
#[test]
#[ignore = "release diagnostic: 100 skinned foxes"]
fn hundred_fox_tick_cost() {
    let mut s = sim();
    let w = s.world_mut();
    let fox = w.named("fox").unwrap();
    let mut animator = w.get::<Animator>(fox).unwrap().clone();
    animator.current = 1;
    animator.set("speed", 1.6);
    for _ in 1..100 {
        w.spawn((
            Transform::default(),
            Mesh::asset("fox.model"),
            animator.clone(),
        ));
    }
    s.advance(0., Clock::Live);
    s.advance(1000., Clock::Seekable);
    let start = std::time::Instant::now();
    for i in 1..=600 {
        s.advance(1000. + i as f64 * 1000. / 60., Clock::Live);
    }
    let elapsed = start.elapsed().as_secs_f64() * 1000. / 600.;
    println!("100 foxes live tick: {elapsed:.6} ms");
}

#[test]
fn dev_carry_changed_blend_keeps_pose_and_open_keeps_saved_definitions() {
    use exact_game_render::{
        exact_gpu::{Restore, Surface},
        WorldSurface,
    };
    struct Changed;
    impl Game for Changed {
        const ID: &'static str = SmallGame::ID;
        const ASSETS: &'static [&'static str] = SmallGame::ASSETS;
        type Args = Options;
        fn setup(w: &mut World, a: &Options) {
            SmallGame::setup(w, a);
            w.require_mut::<Animator>("fox")
                .blend_mut("travel")
                .unwrap()
                .clips[2]
                .0 = 4.;
        }
        fn tick(w: &mut World, i: &Input, a: &Options) {
            SmallGame::tick(w, i, a);
        }
    }
    fn surface<G: Game>() -> WorldSurface<G, exact_game_render::ModelExecutor, true> {
        let mut s = WorldSurface::default();
        s.bind(&[Value::Number(0.)], None).unwrap();
        for _ in 0..16 {
            let names = s.assets().requests;
            if names.is_empty() {
                break;
            }
            for n in names {
                s.asset(&n, Ok(&assets()[&n]));
            }
        }
        s
    }
    let mut old = sim();
    old.run(750.);
    let saved = old.save().unwrap();
    let locals = old.get::<Pose>("fox").unwrap().local.clone();
    let mut carry = surface::<Changed>();
    carry.restore(&saved, Restore::Carry).unwrap();
    assert_eq!(
        carry.sim().unwrap().get::<Pose>("fox").unwrap().local,
        locals
    );
    let animator = carry.sim().unwrap().get::<Animator>("fox").unwrap();
    assert_eq!(animator.since, old.get::<Animator>("fox").unwrap().since);
    if let Play::Blend(b) = &animator.state_named("travel").unwrap().play {
        assert_eq!(b.clips[2].0, 4.);
    } else {
        panic!("blend")
    }
    let mut open = surface::<Changed>();
    open.restore(&saved, Restore::Open).unwrap();
    assert_eq!(open.sim().unwrap().world().hash(), old.world().hash());
}

#[test]
fn moving_skin_and_shadow_pixels() {
    use exact_game_render::{
        exact_gpu::{fixture, wgpu, Frame, Surface},
        WorldSurface,
    };
    let Some(gpu) = gpu_test::device_or_skip(exact_game_render::exact_gpu::fixture::device())
    else {
        return;
    };
    let mut s = WorldSurface::<SmallGame, exact_game_render::ModelExecutor, true>::default();
    s.device_ready(wgpu::Features::empty());
    s.bind(&[Value::Number(0.)], None).unwrap();
    for _ in 0..16 {
        for n in s.assets().requests {
            s.asset(&n, Ok(&assets()[&n]));
        }
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    }
    let mut frame = Frame {
        width: 1280.,
        height: 720.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    let (bind, _) = fixture::render(&gpu, &mut s, &frame).unwrap();
    frame.now_ms = 1000.;
    let (stride, _) = fixture::render(&gpu, &mut s, &frame).unwrap();
    assert_ne!(bind, stride);
    stride.save("fox-tick60");
    let saved = s.carry().unwrap().unwrap();
    s.restore(&saved, exact_game_render::exact_gpu::Restore::Open)
        .unwrap();
    // Establish the fresh host epoch. Local pose histories survive restore.
    frame.now_ms = 0.;
    let (restored, _) = fixture::render(&gpu, &mut s, &frame).unwrap();
    let changed = stride
        .data
        .chunks_exact(4)
        .zip(restored.data.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 4))
        .count();
    println!(
        "restore pixel changes (>4 levels): {changed} / {}",
        stride.width * stride.height
    );
    assert!(
        changed < (stride.width * stride.height / 50) as usize,
        "restore must preserve the stride; only the existing entity-history snap may differ"
    );
    assert!(s.take_error().is_none());
}

#[test]
fn root_motion_walks_the_fox_forward_and_emits_steps() {
    let mut s = sim();
    for tick in 1..=120 {
        let before = s.global_position("fox").unwrap();
        s.run(1000. / 60. + 0.0001);
        let transform = s.get::<Transform>("fox").unwrap();
        let delta = transform.position - before;
        if tick > 30 {
            let forward = transform.rotation * Vec3::Z;
            assert!(
                delta.dot(forward) > 0.,
                "tick {tick} must advance along the Fox's facing"
            );
            assert!((delta - forward * delta.length()).length() < 1e-5);
        }
        let model = s.world().model("fox.model").unwrap();
        let root = model
            .nodes
            .iter()
            .position(|n| n.name == "b_Root_00")
            .unwrap();
        assert_eq!(
            &s.get::<Pose>("fox").unwrap().local[root * 10..root * 10 + 3],
            &[0., 0., 0.]
        );
    }
    assert!(s
        .world()
        .journal()
        .iter()
        .any(|e| e.line.ends_with("fox footstep")));
}

#[test]
fn paused_setup_and_pre_step_restore_keep_bind_pose() {
    struct Paused;
    impl Game for Paused {
        const ID: &'static str = "paused-fox";
        const ASSETS: &'static [&'static str] = SmallGame::ASSETS;
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            SmallGame::setup(w, args);
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {
            panic!("paused setup stepped");
        }
        fn paused(_: &Options) -> bool {
            true
        }
    }
    let mut s = Sim::<Paused>::new(Options::default()).unwrap();
    s.load_assets(|name| assets().get(name).cloned().ok_or(name.to_owned()))
        .unwrap();
    let before = s.world().global_position("charm").unwrap();
    assert!(before.length() > 0.1);
    let save = s.save().unwrap();
    for restored in [false, true] {
        if restored {
            s.restore(&save).unwrap();
        }
        s.run(1000.);
        assert_eq!(s.world().tick(), 0);
        assert_eq!(s.world().global_position("charm"), Some(before));
        assert!(animation::socket(s.world(), "fox", "b_Head_05").is_ok());
    }
}
