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
    fixture::device()
        .map_err(|e| eprintln!("SKIP surface GPU regression: {e}"))
        .ok()
}
fn surface() -> WorldSurface<Move> {
    let mut s = WorldSurface::default();
    s.bind(&[Value::Bool(true), Value::Number(0.)], None)
        .unwrap();
    s
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
    let mut recording = None;
    let mut perf = Perf::default();
    let mut error = None;
    for measure in [false, true] {
        CLOCK_READS.with(|n| n.set(0));
        let mut trace = None;
        let mut after = observer::<false>(
            &mut render,
            &mut recording,
            &mut perf,
            &mut trace,
            &mut error,
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
        max_storage_buffer_binding_size: 48 * 16,
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
    original
        .bind(&[Value::Bool(true), Value::Number(3.)], None)
        .unwrap();
    original.sim.as_mut().unwrap().advance(0., Clock::Seekable);
    original
        .sim
        .as_mut()
        .unwrap()
        .advance(100., Clock::Seekable);
    original.sim.as_ref().unwrap().world().publish("score", 7);
    let saved = original.carry().unwrap();
    let mut restored = WorldSurface::<Move>::default();
    restored
        .bind(&[Value::Bool(false), Value::Number(9.)], None)
        .unwrap();
    let generation = restored.sim().unwrap().generation();
    restored.restore(&saved, exact_gpu::Restore::Open).unwrap();
    assert_eq!(restored.sim().unwrap().world().tick(), 6);
    assert!(restored.sim().unwrap().generation() > generation);
    // Continue keeps the saved Setup identity; only current Live bindings win.
    assert!(!restored.sim().unwrap().args().r#move);
    assert_eq!(
        restored.sim().unwrap().args().run,
        original.sim().unwrap().args().run,
        "saved setup identity is retained while current live bindings win"
    );
    assert_eq!(restored.sim().unwrap().args().run, 3);
    assert_eq!(
        restored.sim().unwrap().world().hash(),
        original.sim().unwrap().world().hash()
    );
    assert_eq!(restored.published().as_deref(), Some(r#"{"score":7}"#));
    assert!(restored
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains(r#""restored":true"#));
    let before = restored.carry().unwrap();
    let generation = restored.sim().unwrap().generation();
    assert!(restored
        .restore(b"invalid", exact_gpu::Restore::Open)
        .unwrap_err()
        .contains("save"));
    assert_eq!(restored.carry().unwrap(), before);
    assert_eq!(restored.sim().unwrap().generation(), generation);
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
    assert_eq!(restored.sim().unwrap().world().tick(), 7);
    assert_eq!(
        restored
            .sim()
            .unwrap()
            .world()
            .query::<(&Mesh, &Transform)>()
            .one()
            .unwrap()
            .1
            .position
            .x,
        6.,
        "the current Live move=false binding prevents the next tick from moving"
    );
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
    let mut s = WorldSurface::<Move, (), true>::default();
    s.bind(&[Value::Bool(true), Value::Number(0.)], None)
        .unwrap();
    let w = s.sim.as_mut().unwrap().world_mut();
    *w.query::<&mut Mesh>().one().unwrap() = Mesh::asset("castle.model");
    fixture::render(&gpu, &mut s, &frame(0.0)).unwrap();
    assert_eq!(s.assets(), ["castle.model"]);
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
    assert_eq!(reply.trace.stride, 14);
    assert!(!reply.trace.overflow);
    assert_eq!(reply.trace.frames.len(), 42);
    let rows: Vec<_> = reply.trace.frames.chunks_exact(14).collect();
    // Seekable T = [17, 25, 34] ms, L = 0, step = 1000/60 ms.
    // R = T - step = [1/3, 25/3, 52/3] ms; x = R/step = [.02, .5, 1.04].
    for (row, x) in rows.iter().zip([0.02, 0.5, 1.04]) {
        assert!((row[2] - x).abs() < 1e-6);
        assert_eq!(&row[5..8], &[0., 0., 8.]);
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
fn headless_greybox_ticks_under_the_agent_clock_to_the_native_hash() {
    use exact_gpu::{Module, Registry};
    static REGISTRY: Registry = Registry {
        surfaces: &[("world", 2, || {
            Box::<WorldSurface<greybox_logic::Greybox>>::default()
        })],
        shaders: &[],
    };
    let mut module = Module::new(&REGISTRY);
    let id = module.create_headless("world").unwrap();
    assert!(module.bind(id, &[Value::Number(7.), Value::Bool(false)], Some(0.)));
    let before = module.carry(id).unwrap();
    let setup = module
        .agent(id, r#"{"op":"state","now":1500,"width":1280,"height":720}"#)
        .unwrap();
    assert!(setup.contains("0x7df5e5a89b4d0207"), "{setup}");
    assert!(setup.contains("\"device\":false"));
    assert_eq!(module.carry(id).unwrap(), before, "inspection is read-only");
    // A clock operation establishes the epoch and input viewport, not a read.
    let start = module
        .agent(id, r#"{"op":"clock","now":0,"width":1280,"height":720}"#)
        .unwrap();
    let start: serde_json::Value = serde_json::from_str(&start).unwrap();
    assert_eq!(start["tick"], 0);
    assert_eq!(start["hash"], "0x7df5e5a89b4d0207");
    assert!(module.input_json(
        id,
        r#"{"t":"key","code":"KeyW","key":"w","down":true,"repeat":false,"at":0}"#
    ));
    let tick = module.agent(id, r#"{"op":"clock","now":1500}"#).unwrap();
    let tick: serde_json::Value = serde_json::from_str(&tick).unwrap();
    assert_eq!(tick["tick"], 90);
    assert_eq!(tick["hash"], "0x0f14b8b231091d12");
    assert_eq!(module.render(id, &frame(1500.)), None);
    assert_eq!(module.take_error(), "");
    let save = module.carry(id).unwrap();
    module.lose_device();
    assert!(module.restore(id, &save, exact_gpu::Restore::Open));
    let restored = module.carry(id).unwrap();
    let state = module.agent(id, r#"{"op":"state","now":1500}"#).unwrap();
    assert!(state.contains("0x0f14b8b231091d12"), "{state}");
    assert_eq!(module.carry(id).unwrap(), restored);
}

#[test]
fn headless_module_restore_anchors_controlled_clock_even_when_assets_arrive_later() {
    use exact_gpu::{Module, Registry};
    struct Loading;
    impl Game for Loading {
        const ID: &'static str = "restore-clock-assets";
        const ASSETS: &'static [&'static str] = &["crate.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("crate", Transform::default());
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.get_mut::<Transform>("crate").unwrap().position.x += 1.;
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("world", 0, || {
            Box::<WorldSurface<Loading, (), true>>::default()
        })],
        shaders: &[],
    };
    let asset = exact_game::bin::to_vec(&exact_game::asset::Model::default());
    let mut original = Sim::<Loading>::new(()).unwrap();
    original.asset("crate.model", Some(&asset)).unwrap();
    original.run(500.);
    let saved = original.save().unwrap();
    original.run(500.);
    for deferred in [false, true] {
        let mut module = Module::new(&REGISTRY);
        let id = module.create_headless("world").unwrap();
        assert!(module.bind(id, &[], Some(9000.)));
        module.agent(id, r#"{"op":"clock","owner":"agent","now":9000}"#);
        assert_eq!(module.take_assets(id), ["crate.model"]);
        if !deferred {
            assert!(module.asset(id, "crate.model", Ok(&asset)));
        }
        assert!(module.restore(id, &saved, Restore::Open));
        if deferred {
            assert!(module.asset(id, "crate.model", Ok(&asset)));
        }
        let state = module.agent(id, r#"{"op":"state","now":999999}"#).unwrap();
        assert!(state.contains(r#""tick":30"#), "{state}");
        assert!(state.contains(r#""hostMicros":9000000"#), "{state}");
        assert_eq!(module.carry(id).unwrap(), saved);
        let clock = module.agent(id, r#"{"op":"clock","now":9500}"#).unwrap();
        assert!(clock.contains(r#""tick":60"#), "{clock}");
        assert_eq!(module.carry(id).unwrap(), original.save().unwrap());
    }
}

#[test]
fn presentation_hook_follows_frames_transport_and_gestures() {
    #[derive(Default)]
    struct Probe {
        frames: Vec<(u64, u64, bool, bool)>,
        gestures: usize,
    }
    impl Presentation for Probe {
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
    let mut s = WorldSurface::<greybox_logic::Greybox, Probe>::default();
    s.bind(&[Value::Number(7.), Value::Bool(false)], None)
        .unwrap();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    fixture::render(&gpu, &mut s, &frame(17.)).unwrap();
    assert_eq!(s.presentation.frames.len(), 2);
    assert_eq!(s.presentation.frames[1].0, 1);
    assert!(s.presentation.frames.iter().all(|f| f.2 && f.3));
    s.bind(&[Value::Number(7.), Value::Bool(true)], None)
        .unwrap();
    let mut hidden = frame(34.);
    hidden.width = 0.;
    fixture::render(&gpu, &mut s, &hidden).unwrap();
    assert!(!s.presentation.frames[2].2);
    let saved = s.carry().unwrap();
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
            kind: exact_gpu::PointerKind::Touch,
            buttons: 1,
            at_ms: 0.,
        });
    }
    fixture::render(&gpu, &mut rendered, &frame(17.)).unwrap();
    let before = headless.carry().unwrap();
    let layout = headless
        .agent(r#"{"op":"layout","entity":"player","now":17,"width":64,"height":64}"#)
        .unwrap();
    let layout: serde_json::Value = serde_json::from_str(&layout).unwrap();
    assert_eq!(layout["tick"], 0);
    assert_eq!(headless.carry().unwrap(), before, "layout is read-only");
    // Match the rendered frame's explicit viewport before consuming the touch.
    let tick = headless
        .agent(r#"{"op":"clock","now":17,"width":64,"height":64}"#)
        .unwrap();
    let tick: serde_json::Value = serde_json::from_str(&tick).unwrap();
    assert_eq!(tick["tick"], 1);
    for s in [&rendered, &headless] {
        assert_eq!(s.sim().unwrap().world().tick(), 1);
        assert_eq!(s.sim().unwrap().position("player").unwrap().x, 1.);
    }
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
    s.device_ready();
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

#[test]
fn layout_facing_snapshot_survives_surface_splicing_without_device() {
    let mut s = surface();
    let request = r##"{"op":"layout","entity":"#0","to":"#1"}"##;
    let epoch = s.sim().unwrap().world().mutation_epoch();
    let expected = r#"{"tick":0,"entity":{"id":0,"name":null,"world":{"position":[0,0,0],"rotation":[0,0,0,1],"scale":[1,1,1]},"bounds":{"min":[-0.5,-0.5,-0.5],"max":[0.5,0.5,0.5]},"screen":{"unavailable":true},"depth":null,"visible":{"unavailable":true},"facing":{"forward":[0,0,-1],"towardCamera":null,"bearingTo":180,"distanceTo":8,"lineOfSight":true}}}"#;
    assert_eq!(s.agent(request).unwrap(), expected);
    assert!(!world_state(expected));
    for device in [true, false] {
        s.device = device;
        assert_eq!(s.agent(request).unwrap(), expected);
        let state = s.agent(r#"{"op":"state"}"#).unwrap();
        assert!(world_state(&state));
        assert!(state.contains(&format!("\"device\":{device}")));
        use exact_game::Reader;
        let mut decoder = exact_game::json::Decoder::new(&state);
        decoder.skip().unwrap();
        decoder.finish().unwrap();
    }
    assert_eq!(s.sim().unwrap().world().mutation_epoch(), epoch);
}

struct Art;
impl Game for Art {
    const ID: &'static str = "asset-window";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = ();
    fn assets() -> &'static [exact_game::Asset] {
        &[exact_game::Asset {
            name: "fox.glb",
            bytes: include_bytes!("../../games/lanterns/assets/Fox.glb"),
        }]
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn((Transform::default(), Mesh::asset("crate.model")));
        w.spawn((Transform::at(4., 0., 0.), Mesh::asset("fox.glb")));
        w.spawn((Transform::at(0., 0., 8.), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn peer_assets_finish_gpu_work_before_loaded_and_restore_keeps_the_loading_window_honest() {
    let Some(gpu) = gpu() else { return };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (mut model, textures) = exact_game_bake::assets(&path).unwrap();
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
        let mut s = WorldSurface::<Art, (), true>::default();
        s.device_ready();
        s.bind(&[], None).unwrap();
        s
    };
    let deliver = |s: &mut WorldSurface<Art, (), true>| {
        assert_eq!(s.assets(), ["crate.model"]);
        s.asset("crate.model", Ok(&bytes));
        assert!(s.sim().unwrap().is_loading());
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        assert!(
            s.sim().unwrap().is_loading(),
            "texture upload still gates setup"
        );
        assert_eq!(s.assets(), textures.keys().cloned().collect::<Vec<_>>());
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
    assert!(original.carry().is_none());
    deliver(&mut original);
    let work = original.render.as_ref().unwrap().0.asset_work();
    assert_eq!(
        work,
        (10, 4),
        "both winding variants and one shared texture plus three defaults"
    );
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
    let saved = original.carry().unwrap();
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
    assert!(restored.carry().is_none());
    deliver(&mut restored);
    assert_eq!(restored.sim().unwrap().world().tick(), 30);
    assert_eq!(restored.sim().unwrap().world().hash(), hash);
    let state = restored.agent(r#"{"op":"state"}"#).unwrap();
    assert!(
        state.contains("\"restored\":true") && state.contains("\"forwarded\":[\"KeyW\"]"),
        "{state}"
    );
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
fn viewport_occlusion_crosses_world_surface_without_a_device() {
    struct Eyes;
    impl Game for Eyes {
        type Args = ();
        const ID: &'static str = "surface-eyes";
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("subject", (Transform::default(), Mesh::cube(2.)));
            w.spawn_named("camera", (Transform::at(0., 0., 10.), Camera::default()));
            w.spawn_named("first", (Transform::at(0., 0., 5.), Mesh::cube(4.)));
            w.spawn_named("second", (Transform::at(0., 0., 5.000001), Mesh::cube(4.)));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut s = WorldSurface::<Eyes>::default();
    s.bind(&[], None).unwrap();
    let request = r#"{"op":"layout","entity":"subject","to":"camera","width":800,"height":600}"#;
    let expected = r#"{"tick":0,"entity":{"id":0,"name":"subject","world":{"position":[0,0,0],"rotation":[0,0,0,1],"scale":[1,1,1]},"bounds":{"min":[-1,-1,-1],"max":[1,1,1]},"screen":{"x":342.265,"y":242.265,"w":115.47,"h":115.47},"depth":10,"visible":{"inFrustum":true,"behindCamera":false,"distance":10,"occluded":1,"occluders":["first","second"]},"facing":{"forward":[0,0,-1],"towardCamera":-1,"bearingTo":180,"distanceTo":10,"lineOfSight":false}}}"#;
    for device in [false, true] {
        s.device = device;
        assert_eq!(s.agent(request).unwrap(), expected);
        assert!(!world_state(expected));
    }
}

#[test]
fn presented_lifecycle_and_late_material_pipeline_are_accounted() {
    let Some(gpu) = gpu() else { return };
    let mut s = WorldSurface::<Art, (), true>::default();
    s.device_ready();
    s.bind(&[], None).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (mut model, textures) = exact_game_bake::assets(&path).unwrap();
    for material in &mut model.materials {
        material.double_sided = false;
    }
    s.asset("crate.model", Ok(&exact_game::bin::to_vec(&model)));
    for (name, texture) in textures {
        s.asset(&name, Ok(&exact_game::bin::to_vec(&texture)));
    }
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    assert!(
        !s.ready_reasons().is_empty(),
        "encoding is not presentation"
    );
    s.lifecycle(Lifecycle::Presented);
    assert!(s.ready_reasons().is_empty());
    let before = s.audit.json(true);
    assert_eq!(before["beforeReady"]["shaderModules"], 7);
    assert_eq!(before["beforeReady"]["renderPipelines"], 16);
    assert_eq!(before["beforeReady"]["bindGroupLayouts"], 9);
    assert_eq!(before["afterReady"]["violations"], 0);
    for material in &mut model.materials {
        material.double_sided = true;
    }
    // Direct renderer usage also counts; deliberately bypass declared delivery.
    s.render
        .as_mut()
        .unwrap()
        .0
        .prepare_model("late-variant.model", &model)
        .unwrap();
    let after = s.audit.json(true);
    assert_eq!(after["afterReady"]["renderPipelines"], 5, "{after}");
    assert!(after["afterReady"]["meshBytesUploaded"].as_u64().unwrap() > 0);
    assert!(after["afterReady"]["violations"].as_u64().unwrap() > 0);
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
