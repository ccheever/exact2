use super::*;
use exact_game::{Arg, Args, Camera, Input, Mesh, Transform, Vec3};
use exact_gpu::{fixture, Gpu};

struct Move;
impl Game for Move {
    const ID: &'static str = "surface-regressions";
    const ARGS: &'static [Arg] = &[Arg::live("move"), Arg::setup("run")];
    fn setup(w: &mut World, _: &Args) -> Result<(), String> {
        w.spawn((Transform::default(), Mesh::Cube));
        w.spawn((Transform::at(0., 0., 8.), Camera::default()));
        Ok(())
    }
    fn tick(w: &mut World, _: &Input) {
        if w.args().flag("move").unwrap() {
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
        let mut after = observer(&mut render, &mut perf, &mut error, measure, 3600);
        for left in (0..3600).rev() {
            after(&world, left);
        }
        assert_eq!(CLOCK_READS.with(|n| n.get()), if measure { 480 } else { 0 });
    }
    let mut sim = Sim::<Move>::new(&[Value::Bool(true), Value::Number(0.)]).unwrap();
    sim.advance(0., Clock::Seekable);
    assert_eq!(sim.ticks_due(60_000., Clock::Seekable), 3600);
    assert_eq!(sim.ticks_due(60_000., Clock::Live), 15);
}
#[test]
fn capacity_error_during_timed_bind_does_not_refuse_committed_values() {
    struct Grow;
    impl Game for Grow {
        const ID: &'static str = "capacity-bind";
        const ARGS: &'static [Arg] = &[Arg::live("paused")];
        fn setup(w: &mut World, _: &Args) -> Result<(), String> {
            w.spawn(Transform::default());
            Ok(())
        }
        fn tick(w: &mut World, _: &Input) {
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
    assert!(s.sim().unwrap().world().args().flag("paused").unwrap());
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
        const ID: &'static str = "teleport-pixels";
        fn setup(w: &mut World, _: &Args) -> Result<(), String> {
            let root = w.spawn_named("vehicle", Transform::at(-2., 0., 0.));
            w.spawn((
                Transform::default(),
                Parent(root),
                Mesh::Cube,
                Material::rgb(1., 0.2, 0.05),
            ));
            w.spawn((
                Transform::at(0., 0., 8.).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ));
            Ok(())
        }
        fn tick(w: &mut World, _: &Input) {
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
    assert!(!restored.sim().unwrap().world().args().flag("move").unwrap());
    assert_eq!(
        restored
            .sim()
            .unwrap()
            .world()
            .args()
            .integer("run")
            .unwrap(),
        9
    );
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
