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
    fn surface<G: Game>() -> WorldSurface<G, exact_game_render::ModelPresentation, true> {
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
    let mut s = WorldSurface::<SmallGame, exact_game_render::ModelPresentation, true>::default();
    s.device_ready();
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

// D6 bakes centimetres into metres; the 100x instance compensation retains
// the original Fox rectangle and every pixel/negative-control assertion.
#[test]
fn first_presented_fox_matches_current_pose_in_fox_rectangle() {
    use exact_game_render::{
        exact_gpu::{fixture, wgpu, Frame, Surface},
        WorldSurface,
    };
    struct Birth<const HISTORY: u8>;
    impl<const HISTORY: u8> Game for Birth<HISTORY> {
        const ID: &'static str = "fox-birth";
        const ASSETS: &'static [&'static str] = &["fox.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let mut clip = Animation::play("Run").motion_root("b_Root_00").speed(0.);
            clip.time = 0.3;
            w.spawn_named(
                "fox",
                (
                    Transform::default().with_scale(2.5),
                    Mesh::asset("fox.model"),
                    clip,
                ),
            );
            w.spawn((
                Transform::at(6., 3.4, 7.).looking_at(Vec3::new(0., 0.9, 0.), Vec3::Y),
                Camera::default(),
            ));
            w.insert_resource(Environment {
                fog: None,
                ..Default::default()
            });
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            animation::step(w);
            if HISTORY != 0 {
                let bind = animation::bind_pose(w.model("fox.model").unwrap());
                let mut p = w.require_mut::<Pose>("fox");
                p.previous = if HISTORY == 1 { p.local.clone() } else { bind };
            }
        }
    }
    let gpu = fixture::device().unwrap();
    fn first<const H: u8>(gpu: &exact_game_render::exact_gpu::Gpu, event: &str) -> fixture::Pixels {
        let mut s = WorldSurface::<Birth<H>, exact_game_render::ModelPresentation, true>::default();
        s.device_ready();
        s.bind(&[], None).unwrap();
        for _ in 0..16 {
            for n in s.assets().requests {
                s.asset(&n, Ok(&assets()[&n]));
            }
            s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        }
        let mut f = Frame {
            width: 1280.,
            height: 720.,
            scale: 1.,
            now_ms: 0.,
            seekable: false,
            period_ms: 1000. / 60.,
            children_generation: 0,
            shader_generation: 0,
        };
        fixture::render(gpu, &mut s, &f).unwrap();
        f.now_ms = 1000. / 240.;
        let (mut image, _) = fixture::render(gpu, &mut s, &f).unwrap();
        if event != "birth" {
            match event {
                "restore" | "carry" => {
                    let saved = s.carry().unwrap().unwrap();
                    s.restore(
                        &saved,
                        if event == "restore" {
                            exact_game_render::exact_gpu::Restore::Open
                        } else {
                            exact_game_render::exact_gpu::Restore::Carry
                        },
                    )
                    .unwrap();
                }
                "model arrival" => {
                    s.device_lost();
                    s.device_ready();
                    for _ in 0..16 {
                        for n in s.assets().requests {
                            s.asset(&n, Ok(&assets()[&n]));
                        }
                        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
                    }
                }
                _ => unreachable!(),
            }
            // Restore rebases at the saved tick. A quarter-tick horizon keeps that
            // tick intact while exercising a nonzero interpolation alpha.
            f.period_ms = 1000. / 240.;
            image = fixture::render(gpu, &mut s, &f).unwrap().0;
        }
        assert_eq!(s.sim().unwrap().world().tick(), 1);
        assert!(s.take_error().is_none());
        image
    }
    for event in ["birth", "restore", "carry", "model arrival"] {
        let actual = if event == "birth" {
            first::<0>(&gpu, event)
        } else {
            first::<2>(&gpu, event)
        };
        let reference = first::<1>(&gpu, event);
        let bind_flash = first::<2>(&gpu, "birth");
        let differs = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).any(|(a, b)| a.abs_diff(b) > 2);
        // The deliberately corrupted history locates the Fox's affected rectangle;
        // background pixels cannot dilute the tolerance.
        let (mut x0, mut y0, mut x1, mut y1) = (reference.width, reference.height, 0, 0);
        let mut bind_changes = 0;
        for y in 0..reference.height {
            for x in 0..reference.width {
                if differs(reference.at(x, y), bind_flash.at(x, y)) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                    bind_changes += 1;
                }
            }
        }
        assert!(
            bind_changes > 50,
            "the oracle must detect a small Fox bind flash"
        );
        let area = (x1 - x0 + 1) * (y1 - y0 + 1);
        assert!(
            area < reference.width * reference.height / 10,
            "Fox rectangle is local: {area}"
        );
        let mut changed = 0;
        for y in y0..=y1 {
            for x in x0..=x1 {
                changed += u32::from(differs(actual.at(x, y), reference.at(x, y)));
            }
        }
        println!("{event}: Fox rectangle {x0},{y0}..{x1},{y1}: changes {changed}/{area}, bind flash {bind_changes}");
        assert!(
            changed == 0,
            "{event} must match current/current: zero changed Fox-rectangle pixels"
        );
    }
}
