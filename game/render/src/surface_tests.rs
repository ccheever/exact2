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
    s.bind(&[Value::Bool(true), Value::Number(0.)]).unwrap();
    s
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
    let sim_save = s.sim.as_ref().unwrap().save();
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
    s.bind(&[Value::Bool(true), Value::Number(1.)]).unwrap();
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
        let mut after = observer(
            &mut render,
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
    s.bind(&[Value::Bool(false)]).unwrap();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    assert_eq!(s.bind_at(&[Value::Bool(true)], Some(17.)), Ok(()));
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
    s.bind(&[]).unwrap();
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
    let saved = original.carry().unwrap();
    let mut restored = WorldSurface::<Move>::default();
    restored
        .bind(&[Value::Bool(false), Value::Number(9.)])
        .unwrap();
    let generation = restored.sim().unwrap().generation();
    restored.restore(&saved).unwrap();
    assert_eq!(restored.sim().unwrap().world().tick(), 6);
    assert!(restored.sim().unwrap().generation() > generation);
    assert!(!restored.sim().unwrap().args().r#move);
    assert_eq!(restored.sim().unwrap().args().run, 9);
    assert_eq!(restored.published().as_deref(), Some(r#"{"score":7}"#));
    assert!(restored
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains(r#""restored":true"#));
    let before = restored.carry().unwrap();
    assert!(restored.restore(b"invalid").unwrap_err().contains("save"));
    assert_eq!(restored.carry().unwrap(), before);
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
fn asset_refusal_reaches_the_surface_error_with_its_name() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = surface();
    let w = s.sim.as_mut().unwrap().world_mut();
    *w.query::<&mut Mesh>().one().unwrap() = Mesh::asset("castle");
    fixture::render(&gpu, &mut s, &frame(0.0)).unwrap();
    assert_eq!(s.assets(), ["castle"]);
    s.asset("castle", None);
    let error = s.take_error().expect("a missing asset is refused").0;
    assert!(
        error.contains("castle") && error.contains("missing file"),
        "{error}"
    );
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
    let setup = module
        .agent(id, r#"{"op":"state","now":0,"width":1280,"height":720}"#)
        .unwrap();
    assert!(setup.contains("0x7df5e5a89b4d0207"), "{setup}");
    assert!(setup.contains("\"device\":false"));
    assert!(module.input_json(
        id,
        r#"{"t":"key","code":"KeyW","key":"w","down":true,"repeat":false,"at":0}"#
    ));
    let tick = module.agent(id, r#"{"op":"clock","now":1500}"#).unwrap();
    assert!(tick.contains("\"tick\":90"), "{tick}");
    assert!(tick.contains("0x0f14b8b231091d12"), "{tick}");
    assert_eq!(module.render(id, &frame(1500.)), None);
    assert_eq!(module.take_error(), "");
    let save = module.carry(id).unwrap();
    module.lose_device();
    assert!(module.restore(id, &save));
    let state = module.agent(id, r#"{"op":"state","now":1500}"#).unwrap();
    assert!(state.contains("0x0f14b8b231091d12"), "{state}");
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
    s.bind(&[Value::Number(7.), Value::Bool(false)]).unwrap();
    fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
    fixture::render(&gpu, &mut s, &frame(17.)).unwrap();
    assert_eq!(s.presentation.frames.len(), 2);
    assert_eq!(s.presentation.frames[1].0, 1);
    assert!(s.presentation.frames.iter().all(|f| f.2 && f.3));
    s.bind(&[Value::Number(7.), Value::Bool(true)]).unwrap();
    let mut hidden = frame(34.);
    hidden.width = 0.;
    fixture::render(&gpu, &mut s, &hidden).unwrap();
    assert!(!s.presentation.frames[2].2);
    let saved = s.carry().unwrap();
    s.restore(&saved).unwrap();
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
        s.bind_at(&[], Some(0.)).unwrap();
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
    headless
        .agent(r#"{"op":"state","now":17,"width":64,"height":64}"#)
        .unwrap();
    assert_eq!(rendered.sim().unwrap().position("player").unwrap().x, 1.);
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
