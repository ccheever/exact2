use exact_game::*;
use particles_fixture_logic::SmallGame;

#[test]
fn tick300_and_restore() {
    let mut a = Sim::<SmallGame>::new(()).unwrap();
    a.run(2500.);
    let saved = a.save().unwrap();
    a.run(2500.);
    let hash = a.world().hash();
    println!("particles tick300 0x{hash:016x}");
    assert_eq!(hash, 0x7b8d9188b32e7d5c);
    assert_eq!(a.world().len(), 21);
    let count: u32 = a
        .world()
        .query::<&Emitter>()
        .iter()
        .map(|(_, e)| e.state.alive)
        .sum();
    assert_eq!(count, 20_000);
    for (_, e) in a.world().query::<&Emitter>().iter() {
        let mut count = 0;
        e.particles(60, 1., |_| count += 1);
        assert_eq!(count, e.state.alive);
    }
    let mut b = Sim::<SmallGame>::new(()).unwrap();
    b.restore(&saved).unwrap();
    b.run(2500.);
    assert_eq!(a.save().unwrap(), b.save().unwrap());
    assert_eq!(hash, b.world().hash());
}
#[test]
fn admission_budget_and_quiescence() {
    struct Budget;
    impl Game for Budget {
        const ID: &'static str = "budget";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            for _ in 0..2 {
                w.spawn((
                    Transform::default(),
                    Emitter::sparks().rate(0.).lifetime(0.5).burst(40_000),
                ));
            }
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            emitter::step(w);
        }
    }
    let mut s = Sim::<Budget>::new(()).unwrap();
    s.run(100.);
    let sum: u32 = s
        .world()
        .query::<&Emitter>()
        .iter()
        .map(|(_, e)| e.state.alive)
        .sum();
    let drops: u64 = s
        .world()
        .query::<&Emitter>()
        .iter()
        .map(|(_, e)| e.state.dropped)
        .sum();
    assert_eq!((sum, drops), (65_536, 14_464));
    assert!(s.agent(r#"{"op":"clock"}"#).contains("\"quiescent\":false"));
    assert!(s.settle(), "finite bursts must settle after the last death");
    assert_eq!(
        s.world()
            .query::<&Emitter>()
            .iter()
            .map(|(_, e)| e.state.alive)
            .sum::<u32>(),
        0
    );
    struct Dust;
    impl Game for Dust {
        const ID: &'static str = "dust";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn((Transform::default(), Emitter::sparks(), Ambient));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            emitter::step(w);
        }
    }
    assert!(Sim::<Dust>::new(()).unwrap().settle());
}
#[test]
fn primitive_particles_render_and_do_not_pick() {
    use exact_game_render::{
        exact_gpu::{fixture, Frame, Surface},
        WorldSurface,
    };
    let gpu = fixture::device().unwrap();
    let mut s = WorldSurface::<SmallGame>::default();
    s.bind(&[], None).unwrap();
    let mut frame = Frame {
        width: 640.,
        height: 360.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let first = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    frame.now_ms = 5000.;
    let (shown, stats) = fixture::render(&gpu, &mut s, &frame).unwrap();
    assert_ne!(first, shown);
    shown.save("particles-300");
    println!("particle render stats {stats:?}");
    assert!(s.take_error().is_none());
    let mut sim = Sim::<SmallGame>::new(()).unwrap();
    sim.viewport(640., 360.);
    sim.run(5000.);
    assert!(sim.layout("sparks-0").unwrap().screen.w > 0.);
    assert!(sim.pick(Vec2::new(320., 180.)).is_none());
}
