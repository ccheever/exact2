use super::*;
use exact_game::{Camera, Input, Mesh, Transform, Vec3};
use exact_gpu::{fixture, Gpu};

struct Move;
#[derive(Default, exact_game::Args)]
struct MoveArgs {
    /// Canvas live argument.
    #[live]
    pub r#move: bool,
    /// Canvas setup argument.
    pub run: u32,
}
impl Game for Move {
    const ID: &'static str = "surface-regressions";
    type Args = MoveArgs;
    fn setup(w: &mut World, _: &Self::Args) {
        w.spawn((Transform::default(), Mesh::cube(1.0)));
        w.spawn((Transform::at(0., 0., 8.), Camera::default()));
    }
    fn tick(w: &mut World, _: &Input, args: &Self::Args) {
        if args.r#move {
            for (_, (_, t)) in w.query::<(&Mesh, &mut Transform)>().iter() {
                t.position.x += 1.;
            }
        }
    }
}
fn frame(now_ms: f64) -> Frame {
    Frame {
        width: 64.,
        height: 64.,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.0,
        children_generation: 0,
        shader_generation: 0,
    }
}
fn gpu() -> Option<Gpu> {
    crate::test_device::device_or_skip(exact_gpu::fixture::device())
}
fn surface() -> WorldSurface<Move> {
    let mut s = WorldSurface::default();
    s.bind(&[Value::Bool(true), Value::Number(0.)], None)
        .unwrap();
    s
}

#[test]
fn primitive_asset_boundary_refuses_late_names_and_rechecks_after_restore() {
    #[derive(Default, exact_game::Args)]
    struct Args {
        sprite: bool,
    }
    struct Late;
    impl Game for Late {
        const ID: &'static str = "late-primitive-asset";
        type Args = Args;
        fn setup(w: &mut World, _: &Args) {
            w.register::<exact_game::Sprite>();
            w.spawn_named("shape", (Transform::default(), Mesh::cube(1.)));
        }
        fn tick(w: &mut World, _: &Input, args: &Args) {
            if w.tick() == 1 {
                if args.sprite {
                    w.spawn(exact_game::Sprite::new("late.tex", [1., 1.]));
                } else {
                    *w.get_mut::<Mesh>(w.resolve("shape").unwrap()).unwrap() =
                        Mesh::asset("late.model");
                }
            }
        }
    }
    for sprite in [false, true] {
        let name = if sprite { "late.tex" } else { "late.model" };
        let mut s = WorldSurface::<Late>::default();
        s.bind(&[Value::Bool(sprite)], None).unwrap();
        assert!(s.assets().requests.is_empty());
        assert!(s.error().is_none());
        let fresh = s.carry().unwrap().unwrap();
        for _ in 0..2 {
            s.sim.as_mut().unwrap().run(100.);
            let hash = s.sim().unwrap().world().hash();
            assert!(s.assets().requests.is_empty());
            let error = &s
                .error()
                .expect("late asset must name its missing executor")
                .0;
            assert!(
                error.contains(name) && error.contains("game.assets"),
                "{error}"
            );
            let state = s.agent(r#"{"op":"state"}"#).unwrap();
            assert!(state.contains("\"assets\":[]"), "{state}");
            assert!(
                state.contains("renderError") && state.contains(name),
                "{state}"
            );
            assert_eq!(s.sim().unwrap().world().hash(), hash);
            assert!(s.take_error().is_some());
            assert!(s.take_error().is_none());
            s.restore(&fresh, Restore::Open).unwrap();
            assert!(s.assets().requests.is_empty());
            assert!(s.error().is_none());
        }
    }
    let mut s = WorldSurface::<Late>::default();
    s.bind(&[Value::Bool(false)], None).unwrap();
    let mut replacement = World::new(Late::HZ, 0);
    replacement.register::<exact_game::Sprite>();
    replacement.spawn_named(
        "shape",
        (Transform::default(), Mesh::asset("replacement.model")),
    );
    assert_eq!(
        replacement.revision::<Mesh>(),
        s.sim().unwrap().world().revision::<Mesh>()
    );
    *s.sim.as_mut().unwrap().world_mut() = replacement;
    assert!(s.assets().requests.is_empty());
    assert!(s
        .error()
        .expect("replacement world must be rechecked")
        .0
        .contains("replacement.model"));
}

#[test]
fn live_surface_applies_a_period_and_slews_a_changed_horizon() {
    let Some(gpu) = gpu() else { return };
    let mut s = surface();
    let mut f = frame(0.);
    f.seekable = false;
    f.period_ms = 1000. / 60.;
    fixture::render(&gpu, &mut s, &f).unwrap();
    for n in 1..=3 {
        f.now_ms = n as f64 * 1000. / 60.;
        fixture::render(&gpu, &mut s, &f).unwrap();
        assert_eq!(s.sim().unwrap().world().tick(), n);
        assert_eq!(s.sim().unwrap().alpha(), 1.);
    }
    f.period_ms = 1000. / 144.;
    f.now_ms += f.period_ms;
    fixture::render(&gpu, &mut s, &f).unwrap();
    let sim = s.sim().unwrap();
    let drawn = (sim.world().tick() - 1) as f64 + sim.alpha() as f64;
    // ΔR = (1000/144) * .9975 ms, rather than a 2.778 ms reversal.
    assert!((drawn - 3. - (60. / 144.) * 0.9975).abs() < 0.000002);
}
#[test]
fn transient_empty_and_nonfinite_frames_advance_without_poisoning_viewport() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = surface();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    let mut hidden = frame(17.);
    hidden.width = 0.;
    hidden.height = 0.;
    fixture::render(&gpu, &mut s, &hidden).unwrap();
    assert_eq!(s.sim().unwrap().world().tick(), 1);
    assert!(s.take_error().is_none());
    hidden.width = f32::NAN;
    hidden.now_ms = 34.;
    fixture::render(&gpu, &mut s, &hidden).unwrap();
    assert_eq!(s.sim().unwrap().world().tick(), 2);
    hidden.now_ms = f64::NAN;
    fixture::render(&gpu, &mut s, &hidden).unwrap();
    let (pixels, _) = fixture::render(&gpu, &mut s, &frame(34.)).unwrap();
    assert!(pixels.count(|p| p[0] > 0) > 0);
    assert!(s.error().is_none());
    // Input's last valid viewport remains a finite 64x64 throughout the skip.
    let reply = s.agent(r##"{"op":"layout","entity":"#0"}"##).unwrap();
    assert!(!reply.contains("NaN"));
}
#[test]
fn restore_and_load_are_seen_by_render_without_a_bind_or_tick() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = surface();
    let (initial, _) = fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    let sim_save = s.sim.as_ref().unwrap().save().unwrap();
    let (moved, _) = fixture::render(&gpu, &mut s, &frame(42.)).unwrap();
    assert_ne!(initial, moved);
    let generation = s.sim.as_ref().unwrap().generation();
    s.sim.as_mut().unwrap().restore(&sim_save).unwrap();
    assert!(s.sim.as_ref().unwrap().generation() > generation);
    assert_eq!(
        initial,
        fixture::render(&gpu, &mut s, &frame(42.)).unwrap().0
    );
    let generation = s.sim.as_ref().unwrap().generation();
    s.bind(&[Value::Bool(true), Value::Number(1.)], None)
        .unwrap();
    assert!(s.sim.as_ref().unwrap().generation() > generation);
    fixture::render(&gpu, &mut s, &frame(42.)).unwrap();
    fixture::render(&gpu, &mut s, &frame(84.)).unwrap();
    let w = s.sim.as_mut().unwrap().world_mut();
    for (_, (_, t)) in w.query::<(&Mesh, &mut Transform)>().iter() {
        t.position.x = 0.;
    }
    let world_save = w.save();
    w.load(&world_save).unwrap();
    assert_eq!(
        initial,
        fixture::render(&gpu, &mut s, &frame(84.)).unwrap().0
    );
}
#[test]
fn seekable_observers_make_no_clock_calls_and_long_advances_time_only_retained_ticks() {
    use crate::perf::CLOCK_READS;
    let world = World::new(60, 0);
    let mut render = None;
    let mut perf = Perf::default();
    let mut error = None;
    for measure in [false, true] {
        CLOCK_READS.with(|n| n.set(0));
        let mut trace = None;
        let mut placed = crate::placed::Placements::default();
        let mut hook_poses = crate::hooks::Poses::default();
        let mut after = observer::<false>(
            &mut render,
            &mut placed,
            &mut perf,
            &mut trace,
            &mut error,
            &mut hook_poses,
            measure,
            3600,
        );
        for left in (0..3600).rev() {
            after(&world, left);
        }
        assert_eq!(CLOCK_READS.with(|n| n.get()), if measure { 480 } else { 0 });
    }
    let mut sim = Sim::<Move>::from_values(&[Value::Bool(true), Value::Number(0.)]).unwrap();
    sim.advance(0., Clock::Seekable);
    assert_eq!(sim.ticks_due(60_000., Clock::Seekable), 3600);
    assert_eq!(sim.ticks_due(60_000., Clock::Live), 15);
}
#[test]
fn capacity_error_during_timed_bind_does_not_refuse_committed_values() {
    struct Grow;
    #[derive(Default, exact_game::Args)]
    struct GrowArgs {
        /// Canvas live argument.
        #[live]
        pub paused: bool,
    }
    impl Game for Grow {
        const ID: &'static str = "capacity-bind";
        type Args = GrowArgs;
        fn setup(w: &mut World, _: &Self::Args) {
            w.spawn(Transform::default());
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            for _ in 0..32 {
                w.spawn(Transform::default());
            }
        }
    }
    let Some(mut gpu) = gpu() else {
        return;
    };
    let limits = wgpu::Limits {
        max_storage_buffer_binding_size: 64 * 16,
        ..Default::default()
    };
    let (device, queue) =
        exact_gpu::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: limits,
            ..Default::default()
        }))
        .unwrap();
    gpu.device = device;
    gpu.queue = queue;
    let mut s = WorldSurface::<Grow>::default();
    s.bind(&[Value::Bool(false)], None).unwrap();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    assert_eq!(s.bind(&[Value::Bool(true)], Some(17.)), Ok(()));
    assert!(s.sim().unwrap().args().paused);
    assert_eq!(s.sim().unwrap().world().tick(), 1);
    assert!(s.take_error().unwrap().0.contains("limit 16"));
    assert!(s.take_error().is_none());
    fixture::render(&gpu, &mut s, &frame(17.)).unwrap();
    assert!(
        s.take_error().is_some(),
        "a later failed draw reports its sticky refusal again"
    );
}

#[test]
fn replacement_device_rechecks_storage_capacity_without_changing_world() {
    let Some(gpu) = gpu() else { return };
    let (limited, queue) =
        exact_gpu::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits {
                max_storage_buffers_per_shader_stage: 4,
                ..Default::default()
            },
            ..Default::default()
        }))
        .unwrap();
    fn check<const ASSETS: bool>(gpu: &Gpu, limited: &wgpu::Device, queue: &wgpu::Queue) {
        let mut s = WorldSurface::<Move, (), ASSETS>::default();
        s.bind(&[Value::Bool(false), Value::Number(0.)], None)
            .unwrap();
        let before = fixture::render(gpu, &mut s, &frame(0.)).unwrap().0;
        let saved = s.carry().unwrap().unwrap();
        assert_eq!(
            s.storage_limit,
            gpu.device.limits().max_storage_buffers_per_shader_stage
        );
        s.device_lost();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.prepare_assets(limited, queue, wgpu::TextureFormat::Rgba8Unorm);
        let needed = if ASSETS {
            crate::STORAGE_BINDINGS
        } else {
            crate::SCENE_STORAGE_BINDINGS
        };
        assert_eq!(
            s.take_error().unwrap().0,
            format!("renderer needs {needed} vertex storage buffers; device grants 4")
        );
        assert!(s.take_error().is_none());
        assert_eq!(s.carry().unwrap().unwrap(), saved);
        s.device_lost();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.restore(&saved, Restore::Open).unwrap();
        let after = fixture::render(gpu, &mut s, &frame(0.)).unwrap().0;
        assert!(s.take_error().is_none());
        assert_eq!(
            s.storage_limit,
            gpu.device.limits().max_storage_buffers_per_shader_stage
        );
        assert_eq!(before, after);
        assert_eq!(s.carry().unwrap().unwrap(), saved);
    }
    check::<false>(&gpu, &limited, &queue);
    check::<true>(&gpu, &limited, &queue);
}

#[test]
fn teleported_parent_child_pixels_at_half_alpha_equal_only_the_new_pose() {
    use exact_game::{Material, Parent};
    struct Vehicle;
    impl Game for Vehicle {
        type Args = ();
        const ID: &'static str = "teleport-pixels";
        fn setup(w: &mut World, _: &Self::Args) {
            let root = w.spawn_named("vehicle", Transform::at(-2., 0., 0.));
            w.spawn((
                Transform::default(),
                Parent(root),
                Mesh::cube(1.0),
                Material::rgb(1., 0.2, 0.05),
            ));
            w.spawn((
                Transform::at(0., 0., 8.).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ));
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            if w.tick() == 1 {
                w.teleport(w.named("vehicle").unwrap(), Transform::at(2., 0., 0.));
            }
        }
    }
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = WorldSurface::<Vehicle>::default();
    s.bind(&[], None).unwrap();
    let (old, _) = fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    fixture::render(&gpu, &mut s, &frame(17.)).unwrap();
    let (half, _) = fixture::render(&gpu, &mut s, &frame(41.667)).unwrap();
    let (new, _) = fixture::render(&gpu, &mut s, &frame(50.)).unwrap();
    assert_ne!(old, new);
    assert_eq!(half, new, "no pixels at the old or interpolated location");
    half.save("world-teleported-parent-half");
}

#[test]
fn surface_carry_retains_current_bindings_and_refusal_is_atomic() {
    let mut original = surface();
    original.sim.as_mut().unwrap().advance(0., Clock::Seekable);
    original
        .sim
        .as_mut()
        .unwrap()
        .advance(100., Clock::Seekable);
    original.sim.as_ref().unwrap().world().publish("score", 7);
    let saved = original.carry().unwrap().unwrap();
    let mut restored = WorldSurface::<Move>::default();
    restored
        .bind(&[Value::Bool(false), Value::Number(9.)], None)
        .unwrap();
    let generation = restored.sim().unwrap().generation();
    restored.restore(&saved, exact_gpu::Restore::Open).unwrap();
    assert_eq!(restored.sim().unwrap().world().tick(), 6);
    assert!(restored.sim().unwrap().generation() > generation);
    assert!(!restored.sim().unwrap().args().r#move);
    assert_eq!(restored.sim().unwrap().args().run, 9);
    assert_eq!(restored.published().as_deref(), Some(r#"{"score":7}"#));
    assert!(restored
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains(r#""restored":true"#));
    let before = restored.carry().unwrap().unwrap();
    assert!(restored
        .restore(b"invalid", exact_gpu::Restore::Open)
        .unwrap_err()
        .contains("save"));
    assert_eq!(restored.carry().unwrap().unwrap(), before);
    restored
        .sim
        .as_mut()
        .unwrap()
        .advance(1000., Clock::Seekable);
    restored
        .sim
        .as_mut()
        .unwrap()
        .advance(1017., Clock::Seekable);
    assert!(restored
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains(r#""restored":false"#));
}

#[test]
fn asset_refusal_is_named_without_poisoning_the_surface() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = WorldSurface::<Move, crate::ModelExecutor, true>::default();
    s.bind(&[Value::Bool(true), Value::Number(0.)], None)
        .unwrap();
    let w = s.sim.as_mut().unwrap().world_mut();
    *w.query::<&mut Mesh>().one().unwrap() = Mesh::asset("castle.model");
    fixture::render(&gpu, &mut s, &frame(0.0)).unwrap();
    assert_eq!(s.assets().requests, ["castle.model"]);
    s.asset("castle.model", Err(exact_gpu::AssetError::Missing));
    assert!(s.take_error().is_none());
    let state = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(
        state.contains("castle.model") && state.contains("missing file"),
        "{state}"
    );
    fixture::render(&gpu, &mut s, &frame(17.)).unwrap();
}

#[test]
fn presentation_trace_is_opt_in_read_once_and_never_advances_the_clock() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = surface();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    assert!(s.trace.is_none());
    let tick = s.sim().unwrap().world().tick();
    assert_eq!(
        s.agent(r##"{"op":"state","trace":{"entity":"#0","frames":4}}"##)
            .unwrap(),
        r#"{"trace":"armed"}"#
    );
    assert_eq!(s.sim().unwrap().world().tick(), tick);
    for time in [17., 25., 34.] {
        fixture::render(&gpu, &mut s, &frame(time)).unwrap();
    }
    let tick = s.sim().unwrap().world().tick();
    #[derive(Default, exact_game::Data)]
    struct Capture {
        frames: Vec<f64>,
        stride: u32,
        overflow: bool,
    }
    #[derive(Default, exact_game::Data)]
    struct Reply {
        trace: Capture,
    }
    let reply: Reply =
        exact_game::json::from_str(&s.agent(r#"{"op":"state","trace":"read"}"#).unwrap()).unwrap();
    // 14 timing/position slots, then the drawn world-to-clip matrix and the
    // canvas's pixel size (18), so the feel probe can project positions to pixels.
    assert_eq!(reply.trace.stride, 32);
    assert!(!reply.trace.overflow);
    assert_eq!(reply.trace.frames.len(), 96);
    let rows: Vec<_> = reply.trace.frames.chunks_exact(32).collect();
    // Seekable T = [17, 25, 34] ms, L = 0, step = 1000/60 ms.
    // R = T - step = [1/3, 25/3, 52/3] ms; x = R/step = [.02, .5, 1.04].
    for (row, x) in rows.iter().zip([0.02, 0.5, 1.04]) {
        assert!((row[2] - x).abs() < 1e-6);
        assert_eq!(&row[5..8], &[0., 0., 8.]);
        assert_eq!(&row[30..32], &[64., 64.]);
        // Column-major projection, exactly the analyzer's homogeneous divide.
        let m = &row[14..30];
        let clip_w = m[3] + m[15]; // known world point (1,0,0)
        let px = (1. + (m[0] + m[12]) / clip_w) * row[30] / 2.;
        let py = (1. - (m[1] + m[13]) / clip_w) * row[31] / 2.;
        assert!((px - (32. + 4. * 3_f64.sqrt())).abs() < 1e-5, "{px}");
        assert!((py - 32.).abs() < 1e-5);
    }
    assert_eq!(s.sim().unwrap().world().tick(), tick);
    assert!(s.trace.is_none());
    assert!(s
        .agent(r#"{"op":"state","trace":"read"}"#)
        .unwrap()
        .contains("not armed"));
    assert_eq!(
        s.agent(r#"{"op":"state","trace":"stop"}"#).unwrap(),
        r#"{"trace":"stopped"}"#
    );
}

#[test]
fn full_seekable_render_has_no_performance_samples() {
    let Some(gpu) = gpu() else { return };
    let mut s = surface();
    for now in [0.0, 17.0, 1000.0] {
        crate::perf::CLOCK_READS.with(|n| n.set(0));
        fixture::render(&gpu, &mut s, &frame(now)).unwrap();
        assert_eq!(crate::perf::CLOCK_READS.with(|n| n.get()), 0);
    }
}

#[test]
fn headless_fixture_ticks_under_the_agent_clock_to_the_native_hash() {
    let mut reference = exact_game::Sim::<crate::test_game::Fixture>::from_values(&[
        Value::Number(7.),
        Value::Bool(false),
        Value::Bool(false),
    ])
    .unwrap();
    let initial = format!("0x{:016x}", reference.world().hash());
    reference.input(exact_game::InputEvent::Key {
        code: "KeyW".into(),
        down: true,
        at_ms: 0.,
    });
    reference.run(1500.);
    let moved = format!("0x{:016x}", reference.world().hash());
    use exact_gpu::{Module, Registry};
    static REGISTRY: Registry = Registry {
        surfaces: &[("world", 3, || {
            Box::<WorldSurface<crate::test_game::Fixture>>::default()
        })],
        shaders: &[],
    };
    let mut module = Module::new(&REGISTRY);
    let id = module.create_headless("world").unwrap();
    assert!(module.bind(
        id,
        &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
        Some(0.)
    ));
    let setup = module
        .agent(id, r#"{"op":"state","now":0,"width":1280,"height":720}"#)
        .unwrap();
    assert!(setup.contains(&initial), "{setup}");
    assert!(setup.contains("\"device\":false"));
    assert!(module.input_json(
        id,
        r#"{"t":"key","code":"KeyW","key":"w","down":true,"repeat":false,"at":0}"#
    ));
    let tick = module.agent(id, r#"{"op":"clock","now":1500}"#).unwrap();
    assert!(tick.contains("\"tick\":90"), "{tick}");
    assert!(tick.contains(&moved), "{tick}");
    assert_eq!(module.render(id, &frame(1500.)), None);
    assert_eq!(module.take_error(), "");
    let save = module.carry(id).unwrap().unwrap();
    module.lose_device();
    assert!(module.restore(id, &save, exact_gpu::Restore::Open));
    let state = module.agent(id, r#"{"op":"state","now":1500}"#).unwrap();
    assert!(state.contains(&moved), "{state}");
}

#[test]
fn presentation_hook_follows_frames_transport_and_gestures() {
    #[derive(Default)]
    struct Probe {
        frames: Vec<(u64, u64, bool, bool)>,
        gestures: usize,
    }
    impl Executor for Probe {
        fn sync(&mut self, w: &World, generation: u64, playing: bool, seekable: bool) {
            self.frames.push((w.tick(), generation, playing, seekable));
        }
        fn unlock(&mut self) {
            self.gestures += 1;
        }
    }
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = WorldSurface::<crate::test_game::Fixture, Probe>::default();
    s.bind(
        &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
        None,
    )
    .unwrap();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    fixture::render(&gpu, &mut s, &frame(17.)).unwrap();
    assert_eq!(s.presentation.frames.len(), 2);
    assert_eq!(s.presentation.frames[1].0, 1);
    assert!(s.presentation.frames.iter().all(|f| f.2 && f.3));
    s.bind(
        &[Value::Number(7.), Value::Bool(true), Value::Bool(false)],
        None,
    )
    .unwrap();
    let mut hidden = frame(34.);
    hidden.width = 0.;
    fixture::render(&gpu, &mut s, &hidden).unwrap();
    assert!(!s.presentation.frames[2].2);
    let saved = s.carry().unwrap().unwrap();
    s.restore(&saved, exact_gpu::Restore::Open).unwrap();
    fixture::render(&gpu, &mut s, &frame(34.)).unwrap();
    assert!(s.presentation.frames[3].1 > s.presentation.frames[2].1);
    s.input(&InputEvent::Blur { at_ms: 34. });
    assert_eq!(s.presentation.gestures, 0);
    s.input(&InputEvent::Key {
        code: "KeyW".into(),
        key: "w".into(),
        down: true,
        repeat: false,
        at_ms: 34.,
    });
    assert_eq!(s.presentation.gestures, 1);
}

#[test]
fn fresh_touch_region_matches_rendered_and_headless_worlds() {
    struct Touch;
    impl Game for Touch {
        const ID: &'static str = "touch-parity";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("player", Transform::default());
        }
        fn actions() -> exact_game::Actions {
            exact_game::Actions::new().button_touch("act", exact_game::Region::Right)
        }
        fn tick(w: &mut World, i: &Input, _: &()) {
            if i.held("act") {
                w.get_mut::<Transform>("player").unwrap().position.x += 1.;
            }
        }
    }
    let Some(gpu) = gpu() else { return };
    let mut rendered = WorldSurface::<Touch>::default();
    let mut headless = WorldSurface::<Touch>::default();
    for s in [&mut rendered, &mut headless] {
        s.bind(&[], Some(0.)).unwrap();
        s.sim.as_mut().unwrap().advance(0., Clock::Seekable);
        s.input(&InputEvent::Pointer {
            id: 1,
            phase: exact_gpu::PointerPhase::Down,
            x: 48.,
            y: 32.,
            dx: 0.,
            dy: 0.,
            kind: exact_gpu::PointerKind::Touch,
            buttons: 1,
            at_ms: 0.,
        });
    }
    fixture::render(&gpu, &mut rendered, &frame(17.)).unwrap();
    headless
        .agent(r#"{"op":"state","now":17,"width":64,"height":64}"#)
        .unwrap();
    assert_eq!(
        rendered.sim().unwrap().global_position("player").unwrap().x,
        1.
    );
    assert_eq!(
        rendered.sim().unwrap().world().hash(),
        headless.sim().unwrap().world().hash()
    );
}

#[test]
fn device_state_reports_target_before_first_draw_and_after_loss() {
    let mut s = surface();
    assert!(s
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains("\"device\":false"));
    s.device_ready(exact_gpu::wgpu::Features::empty());
    assert!(s.render.is_none());
    assert!(s
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains("\"device\":true"));
    s.device_lost();
    assert!(s
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains("\"device\":false"));
}

struct Art;
impl Game for Art {
    const ID: &'static str = "asset-window";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn((Transform::default(), Mesh::asset("crate.model")));
        w.spawn((Transform::at(0., 0., 8.), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn peer_assets_finish_gpu_work_before_loaded_and_restore_keeps_the_loading_window_honest() {
    let Some(gpu) = gpu() else { return };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../bake/tests/fixtures/crate.gltf");
    let (mut model, mut textures) = exact_game_bake::assets(&path).unwrap();
    // The authored RGBA8 names: what a device without block families fetches.
    textures.retain(|name, _| model.textures.contains(name));
    for material in &mut model.materials {
        material.double_sided = false;
    }
    let mut mirrored = model.nodes[0].clone();
    mirrored.transform = (glam::Mat4::from_scale(Vec3::new(-1., 1., 1.))
        * glam::Mat4::from_cols_array(&mirrored.transform))
    .to_cols_array();
    model.nodes.push(mirrored);
    let bytes = exact_game::bin::to_vec(&model);
    let fresh = || {
        let mut s = WorldSurface::<Art, crate::ModelExecutor, true>::default();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.bind(&[], None).unwrap();
        s
    };
    let deliver = |s: &mut WorldSurface<Art, crate::ModelExecutor, true>| {
        assert_eq!(s.assets().requests, ["crate.model"]);
        s.asset("crate.model", Ok(&bytes));
        assert!(s.sim().unwrap().is_loading());
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert!(
            s.sim().unwrap().is_loading(),
            "texture upload still gates setup"
        );
        assert_eq!(
            s.assets().requests,
            textures.keys().cloned().collect::<Vec<_>>()
        );
        for (name, data) in &textures {
            s.asset(name, Ok(&exact_game::bin::to_vec(data)));
        }
        assert!(
            !s.sim().unwrap().is_loading(),
            "Loaded describes content, independent of the device"
        );
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert!(!s.sim().unwrap().is_loading());
        assert!(
            s.sim.as_mut().unwrap().take_textures().is_empty(),
            "CPU mips released"
        );
    };
    let mut original = fresh();
    assert!(original.carry().is_err());
    deliver(&mut original);
    let work = original.render.as_ref().unwrap().0.asset_work();
    assert_eq!(
        work,
        (10, 4),
        "both winding variants and one shared texture plus three defaults"
    );
    let renderer = &original.render.as_ref().unwrap().0 as *const _;
    for _ in 0..2 {
        original.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(&original.render.as_ref().unwrap().0 as *const _, renderer);
        assert_eq!(
            original.render.as_ref().unwrap().0.asset_work(),
            work,
            "matching presentation/readback format must not rebuild or upload"
        );
    }
    for tick in 0..=30 {
        fixture::render(&gpu, &mut original, &frame(tick as f64 * 1000. / 60.)).unwrap();
        assert_eq!(
            original.render.as_ref().unwrap().0.asset_work(),
            work,
            "no asset compilation/upload during play"
        );
    }
    original.input(&InputEvent::Key {
        code: "KeyW".into(),
        key: "w".into(),
        down: true,
        repeat: false,
        at_ms: 500.,
    });
    let saved = original.carry().unwrap().unwrap();
    let hash = original.sim().unwrap().world().hash();
    let mut restored = fresh();
    restored.restore(&saved, exact_gpu::Restore::Open).unwrap();
    let state = restored.agent(r#"{"op":"state"}"#).unwrap();
    assert!(
        state.contains("\"tick\":0")
            && state.contains("\"loading\":[\"crate.model\"]")
            && state.contains("\"restored\":false"),
        "{state}"
    );
    assert!(restored.carry().is_err());
    deliver(&mut restored);
    assert_eq!(restored.sim().unwrap().world().tick(), 30);
    assert_eq!(restored.sim().unwrap().world().hash(), hash);
    let state = restored.agent(r#"{"op":"state"}"#).unwrap();
    assert!(
        state.contains("\"restored\":true") && state.contains("\"forwarded\":[\"KeyW\"]"),
        "{state}"
    );
    // Recovery retains content hashes; preparation reconstructs a retired digest
    // when a same-device restore has revived its CPU model.
    for retain_digest in [true, false] {
        if !retain_digest {
            restored.placed.attachments.model_digests.clear();
        }
        let hashes = crate::models::model_hash_count();
        restored.device_lost();
        assert_eq!(
            crate::models::model_hash_count(),
            hashes,
            "device loss does not rehash retained CPU content"
        );
        restored.device_ready(exact_gpu::wgpu::Features::empty());
        restored.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(
            crate::models::model_hash_count(),
            hashes + usize::from(!retain_digest),
            "preparation hashes only a model whose digest was retired"
        );
        for (name, data) in &textures {
            restored.asset(name, Ok(&exact_game::bin::to_vec(data)));
        }
        restored.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert!(restored
            .render
            .as_ref()
            .unwrap()
            .0
            .models
            .loaded
            .contains_key("crate.model"));
        assert!(restored.sim().unwrap().model_prepared("crate.model"));
        fixture::render(&gpu, &mut restored, &frame(500.)).unwrap();
        assert!(
            !restored
                .render
                .as_ref()
                .unwrap()
                .0
                .models
                .records
                .is_empty(),
            "recovered model must be drawn"
        );
    }
    let mut primitive = surface();
    fixture::render(&gpu, &mut primitive, &frame(0.)).unwrap();
    assert_eq!(primitive.render.as_ref().unwrap().0.asset_work(), (0, 0));
    assert!(primitive
        .render
        .as_ref()
        .unwrap()
        .0
        .models
        .instances
        .is_none());
}

#[test]
fn performance_recording_arms_only_through_diagnostic_state() {
    let mut s = surface();
    s.perf.tick.push(2.0);
    s.perf.frame(0., false);
    s.perf.frame(10., false);
    assert!(!s.perf.armed());
    let state = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(state.contains("\"armed\":false"));
    assert!(state.contains("\"count\":1,\"mean\":2"));
    let state = s.agent(r#"{"op":"state","perf":true}"#).unwrap();
    assert!(s.perf.armed() && state.contains("\"armed\":true"));
    assert!(state.contains("\"count\":1,\"mean\":2"));
    s.perf.frame(20., false);
    let cadence = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(cadence.contains("\"count\":2,\"mean\":10"));
    s.perf.tick.push(4.0);
    let state = s.agent(r#"{"op":"state","perf":true}"#).unwrap();
    assert!(state.contains("\"p50\":4"));
    let state = s.agent(r#"{"op":"state","perf_reset":true}"#).unwrap();
    assert!(state.contains("\"p50\":0"));
}

#[test]
fn world_surface_retires_and_readds_a_real_placed_child() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut s = surface();
    let entity = s
        .sim
        .as_mut()
        .unwrap()
        .world_mut()
        .spawn_named("sign", (Transform::default(), exact_game::Placed::child(0)));
    for _ in 0..2 {
        assert_eq!(s.children_mode(), exact_gpu::ChildrenMode::Each);
        s.child(0, "", None, [0., 0., 100., 50.]);
        fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
        assert!(s.placement(0).is_some());
        s.sim
            .as_mut()
            .unwrap()
            .world_mut()
            .remove::<exact_game::Placed>(entity);
        assert_eq!(s.children_mode(), exact_gpu::ChildrenMode::Overlay);
        s.child(0, "", None, [0.; 4]);
        s.child(1, "", None, [0.; 4]); // later removals must not recreate entries
        assert!(s.placed.children.is_empty());
        fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
        assert!(s.placement(0).is_none());
        assert!(s.placed.children.is_empty());
        s.sim
            .as_mut()
            .unwrap()
            .world_mut()
            .insert(entity, exact_game::Placed::child(0));
    }
}

#[test]
fn control_json_reaches_world_and_unknown_names_refuse_without_poisoning_it() {
    struct Controls;
    impl Game for Controls {
        const ID: &'static str = "control-abi";
        type Args = ();
        fn actions() -> exact_game::Actions {
            exact_game::Actions::new().button("jump", &["Space"])
        }
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("player", (Transform::default(),));
        }
        fn tick(w: &mut World, input: &Input, _: &()) {
            if input.pressed("jump") {
                w.get_mut::<Transform>("player").unwrap().position.y += 1.;
            }
        }
    }
    let mut surface = WorldSurface::<Controls>::default();
    surface.bind(&[], Some(1000.)).unwrap();
    let event = exact_gpu::json::parse_input(
        r#"{"t":"control","name":"typo","phase":"down","id":7,"x":0,"y":0,"at":1000}"#,
    )
    .unwrap();
    surface.input(&event);
    assert!(surface.take_error().unwrap().0.contains("typo"));
    let event = exact_gpu::json::parse_input(
        r#"{"t":"control","name":"jump","phase":"down","id":7,"x":0,"y":0,"at":1000}"#,
    )
    .unwrap();
    surface.input(&event);
    assert!(surface.take_error().is_none());
    surface
        .sim
        .as_mut()
        .unwrap()
        .advance(1100., Clock::Seekable);
    assert_eq!(
        surface
            .sim()
            .unwrap()
            .get::<Transform>("player")
            .unwrap()
            .position
            .y,
        1.
    );
}

#[test]
fn placement_and_renderer_share_warnings_across_restore_and_prune_dead_followers() {
    struct Missing;
    impl Game for Missing {
        const ID: &'static str = "shared-attachment-warning";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named(
                "charm",
                (
                    Transform::default(),
                    exact_game::SocketFollow::new("missing", "head"),
                    exact_game::Placed::child(0),
                ),
            );
            w.spawn((Transform::at(0., 0., 8.), Camera::default()));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn paused(_: &()) -> bool {
            true
        }
    }
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<Missing, crate::ModelExecutor, true>::default();
    s.bind(&[], None).unwrap();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    let registry = s.placed.attachments.diagnostics.clone();
    assert!(std::rc::Rc::ptr_eq(
        &registry,
        s.render.as_ref().unwrap().1.attachment_diagnostics()
    ));
    assert_eq!(registry.borrow().len(), 1);
    let save = s.carry().unwrap().unwrap();
    for _ in 0..3 {
        s.restore(&save, Restore::Open).unwrap();
        fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
        assert!(std::rc::Rc::ptr_eq(
            &registry,
            s.render.as_ref().unwrap().1.attachment_diagnostics()
        ));
        assert_eq!(registry.borrow().len(), 1);
    }
    let w = s.sim.as_mut().unwrap().world_mut();
    let e = w.named("charm").unwrap();
    w.despawn(e);
    s.dirty = true;
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    assert!(registry.borrow().is_empty());
}

#[test]
fn headless_named_placed_refusal_reaches_the_agent() {
    let mut s = surface();
    s.sim.as_mut().unwrap().world_mut().spawn_named(
        "label",
        (Transform::default(), exact_game::Placed::child("sign")),
    );
    s.child(0, "sign", None, [0., 0., 100., 50.]);
    s.child(1, "sign", None, [0., 50., 100., 50.]);
    let reply = s
        .agent(r#"{"op":"state","width":200,"height":200}"#)
        .unwrap();
    assert!(reply.contains("renderError"), "{reply}");
    assert!(reply.contains("duplicate testId"), "{reply}");
    assert!(s.take_error().unwrap().0.contains("duplicate testId"));
}

#[test]
fn a_level_declares_its_asset_requirement_without_a_model_list() {
    struct LevelOnly;
    impl Game for LevelOnly {
        const ID: &'static str = "level-only-surface";
        const LEVEL: Option<exact_game::asset::Level> =
            Some(exact_game::asset::Level::of::<u32>("seed.level.json"));
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let seed = w.level::<u32>("seed.level.json").unwrap();
            w.publish("seed", seed);
            w.spawn((Transform::default(), Mesh::cube(seed as f32 / 7.)));
            w.spawn((Transform::at(0., 0., 8.), Camera::default()));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    fn loaded<P: Executor, const A: bool>() -> WorldSurface<LevelOnly, P, A> {
        let mut surface = WorldSurface::<LevelOnly, P, A>::default();
        surface.bind(&[], None).unwrap();
        assert_eq!(surface.assets().requests, ["seed.level.json"]);
        assert!(surface.assets().requests.is_empty());
        surface.asset("seed.level.json", Ok(b"7"));
        assert!(!surface.sim().unwrap().is_loading());
        assert_eq!(
            surface.sim().unwrap().world().published("seed"),
            Some(Value::Number(7.))
        );
        let save = surface.carry().unwrap().unwrap();
        let mut fresh = WorldSurface::<LevelOnly, P, A>::default();
        fresh.bind(&[], None).unwrap();
        fresh.restore(&save, Restore::Open).unwrap();
        assert!(fresh.pending_restore.is_some());
        fresh.asset("seed.level.json", Ok(b"7"));
        assert!(fresh.pending_restore.is_none());
        assert_eq!(fresh.carry().unwrap().unwrap(), save);
        for bytes in [
            Err(AssetError::Missing),
            Ok(b"\xff".as_slice()),
            Ok(b"\"bad\"".as_slice()),
        ] {
            let mut failed = WorldSurface::<LevelOnly, P, A>::default();
            failed.bind(&[], None).unwrap();
            failed.asset("seed.level.json", bytes);
            assert!(failed.sim().unwrap().is_loading());
            let state = failed.agent(r#"{"op":"state"}"#).unwrap();
            let reasons = state.split("\"readyReasons\":").nth(1).unwrap();
            assert!(reasons.contains("seed.level.json"), "{state}");
        }
        surface
    }
    let mut primitive = loaded::<(), false>();
    let mut models = loaded::<crate::ModelExecutor, true>();
    assert_eq!(primitive.carry().unwrap(), models.carry().unwrap());
    let Some(gpu) = gpu() else { return };
    let before = fixture::render(&gpu, &mut models, &frame(0.)).unwrap();
    let after = fixture::render(&gpu, &mut primitive, &frame(0.)).unwrap();
    assert_ne!(
        after.0.at(32, 32),
        after.0.at(0, 0),
        "the level's cube is visible"
    );
    assert_eq!(before, after);
    assert_eq!(primitive.render.as_ref().unwrap().0.asset_work(), (0, 0));
}

// exact_game::Offset on a rigged owner moves its socketed prop with it; one on
// the prop moves the prop alone. Sockets draw from the attachment chain, not
// the follower's own pose, so both are applied there.
struct Socketed;
#[derive(Default, exact_game::Args)]
struct SocketLook {
    #[live]
    owner_x: f64,
    #[live]
    prop_y: f64,
}
impl Game for Socketed {
    const ID: &'static str = "offset-socket";
    const ASSETS: &'static [&'static str] = &["fox.model"];
    type Args = SocketLook;
    fn setup(w: &mut World, _: &SocketLook) {
        w.insert_resource(exact_game::Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            ..Default::default()
        });
        w.spawn_named(
            "rig",
            (Transform::at(-3., 0., 0.), Mesh::asset("fox.model")),
        );
        w.spawn_named(
            "prop",
            (
                Transform::default(),
                exact_game::SocketFollow::new("rig", "joint").offset(Transform::at(0., 1.5, 0.)),
                Mesh::cube(0.6),
                exact_game::Material::rgb(0., 0., 1.),
            ),
        );
        w.spawn((Transform::at(0., 0., 10.), Camera::orthographic(8.)));
        w.spawn((
            Transform::at(0., 0., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            exact_game::DirectionalLight::default(),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &SocketLook) {}
    fn paused(_: &SocketLook) -> bool {
        true
    }
    fn present(w: &mut World, look: &SocketLook) {
        let rig = w.named("rig").unwrap();
        w.insert(
            rig,
            exact_game::Offset(Transform::at(look.owner_x as f32, 0., 0.)),
        );
        let prop = w.named("prop").unwrap();
        w.insert(
            prop,
            exact_game::Offset(Transform::at(0., look.prop_y as f32, 0.)),
        );
    }
}
#[test]
fn offsets_move_socketed_props_with_their_owner_and_alone() {
    let Some(gpu) = gpu() else { return };
    // The blue prop's pixel centre, (x, y).
    let drawn = |owner_x: f64, prop_y: f64| {
        let mut surface = WorldSurface::<Socketed, crate::ModelExecutor, true>::default();
        surface.device_ready(exact_gpu::wgpu::Features::empty());
        surface
            .bind(&[Value::Number(owner_x), Value::Number(prop_y)], None)
            .unwrap();
        surface.asset(
            "fox.model",
            Ok(&exact_game::bin::to_vec(&crate::test_model::skinned_model())),
        );
        surface.asset(
            "fox/0-srgb-straight.tex",
            Ok(include_bytes!(
                "../../bake/tests/fixtures/crate/0-srgb-straight.tex"
            )),
        );
        let frame = Frame {
            width: 160.,
            height: 160.,
            ..frame(0.)
        };
        fixture::render(&gpu, &mut surface, &frame).unwrap();
        let image = fixture::render(&gpu, &mut surface, &frame).unwrap().0;
        assert!(surface.take_error().is_none());
        let blue: Vec<(u32, u32)> = (0..160)
            .flat_map(|y| (0..160).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = image.at(x, y).map(u16::from);
                p[2] > 60 && p[2] > p[0] + 40 && p[2] > p[1] + 40
            })
            .collect();
        assert!(!blue.is_empty(), "the prop draws");
        let n = blue.len() as f64;
        (
            blue.iter().map(|p| f64::from(p.0)).sum::<f64>() / n,
            blue.iter().map(|p| f64::from(p.1)).sum::<f64>() / n,
        )
    };
    let still = drawn(0., 0.);
    let owner = drawn(2., 0.);
    let prop = drawn(0., 1.);
    // 8 world units span 160 pixels: 20 pixels a unit.
    assert!(
        (owner.0 - still.0 - 40.).abs() < 3. && (owner.1 - still.1).abs() < 3.,
        "{still:?} -> {owner:?}"
    );
    assert!(
        (prop.1 - still.1 + 20.).abs() < 3. && (prop.0 - still.0).abs() < 3.,
        "{still:?} -> {prop:?}"
    );
}
