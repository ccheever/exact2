use exact_game::*;
use skinned_fixture_logic::{Options, SmallGame};
use std::{collections::BTreeMap, sync::OnceLock};
fn assets() -> &'static BTreeMap<String, Vec<u8>> {
    static ASSETS: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../art/fox.glb");
        let (model, textures) = exact_game_bake::assets(&path).unwrap();
        let mut data: BTreeMap<_, _> = textures
            .into_iter()
            .map(|(n, t)| (n, bin::to_vec(&t)))
            .collect();
        data.insert("fox.model".into(), bin::to_vec(&model));
        data
    })
}
fn sim() -> Sim<SmallGame> {
    let mut s = Sim::new(Options::default()).unwrap();
    s.load_assets(|name| assets().get(name).cloned().ok_or(name.to_owned()))
        .unwrap();
    s.viewport(1280., 720.);
    s
}
#[test]
fn pinned_pose_and_hash() {
    let mut s = sim();
    s.run(1000.);
    let pose = s.agent(r#"{"op":"state","entity":"fox","pose":true}"#);
    let hash = s.world().hash();
    println!("tick60 0x{hash:016x}\n{pose}");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tick60.json");
    if std::env::var_os("EXACT_PIN_POSE").is_some() {
        std::fs::write(&path, &pose).unwrap();
    }
    assert_eq!(pose, std::fs::read_to_string(path).unwrap());
    s.run(1000.);
    println!("tick120 0x{:016x}", s.world().hash());
    assert_eq!(s.world().tick(), 120);
    assert_eq!(s.world().hash(), 0xb05ce95a6c799acf);
}
#[test]
fn paranoid_roundtrip_every_tick_and_mid_fade_fresh_process() {
    let mut reference = sim();
    let mut paranoid = sim();
    for tick in 1..=120 {
        let at = tick as f64 * 1000. / 60. + 0.001;
        reference.advance(0., Clock::Seekable);
        reference.advance(at, Clock::Seekable);
        paranoid.run(1000. / 60. + 0.0001);
        assert_eq!(
            reference.world().hash(),
            paranoid.world().hash(),
            "tick {tick}"
        );
        let saved = paranoid.save().unwrap();
        let mut fresh = sim();
        fresh.restore(&saved).unwrap();
        assert_eq!(
            reference.world().hash(),
            fresh.world().hash(),
            "restore {tick}"
        );
        assert_eq!(
            *reference.get::<Pose>("fox").unwrap().local,
            *fresh.get::<Pose>("fox").unwrap().local
        );
        paranoid = fresh;
    }
    let mut a = sim();
    a.run(750.);
    let saved = a.save().unwrap();
    a.run(1250.);
    let mut b = sim();
    b.restore(&saved).unwrap();
    b.run(1250.);
    assert_eq!(a.world().hash(), b.world().hash());
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
    let target = a + (c - a) * 0.8 + Vec3::new(2., 0., 0.);
    let pole = b + Vec3::new(0., 20., 0.);
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
    let charm = s.position("charm").unwrap();
    assert!(charm.is_finite() && charm.y > 0.5);
}
#[test]
#[ignore = "release diagnostic: 100 skinned foxes"]
fn hundred_fox_tick_cost() {
    let mut s = sim();
    let w = s.world_mut();
    let fox = w.named("fox").unwrap();
    let mut animator = w.get::<Animator>(fox).unwrap().clone();
    animator.current = 1;
    if let Play::Blend(b) = &mut animator.states[1].play {
        b.axis = 1.6;
    }
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
            if let Play::Blend(b) = &mut w.get_mut::<Animator>("fox").unwrap().states[1].play {
                b.clips[2].0 = 4.;
            }
        }
        fn tick(w: &mut World, i: &Input, a: &Options) {
            SmallGame::tick(w, i, a);
        }
    }
    fn surface<G: Game>() -> WorldSurface<G, (), true> {
        let mut s = WorldSurface::default();
        s.bind(&[Value::Number(0.)], None).unwrap();
        for _ in 0..16 {
            let names = s.assets();
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
    if let Play::Blend(b) = &animator.states[1].play {
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
    let Ok(gpu) = fixture::device() else { return };
    let mut s = WorldSurface::<SmallGame, (), true>::default();
    s.device_ready();
    s.bind(&[Value::Number(0.)], None).unwrap();
    for _ in 0..16 {
        for n in s.assets() {
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
    let saved = s.carry().unwrap();
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
