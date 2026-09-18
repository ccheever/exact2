use exact_game::*;

#[derive(Default, Component)]
struct Counter {
    value: u32,
    #[data(skip)]
    hidden: u32,
}
struct BadGame;
impl Game for BadGame {
    const ID: &'static str = "deliberately-unsaved-counter";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("counter", Counter::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let mut c = w.get_mut::<Counter>("counter").unwrap();
        c.hidden += 1;
        c.value += c.hidden;
    }
}
#[test]
fn skipped_game_field_is_detected_by_normal_comparison() {
    let run = |mode| {
        let mut s = Sim::<BadGame>::new(()).unwrap().paranoid(mode);
        s.run(50.0);
        s.world().hash()
    };
    let normal = run(Paranoid::Off);
    assert_ne!(
        normal,
        run(Paranoid::Save),
        "Counter.hidden must be detected"
    );
    assert_ne!(
        normal,
        run(Paranoid::FreshGame),
        "Counter.hidden must be detected"
    );
}

struct ClockGame;
impl Game for ClockGame {
    const ID: &'static str = "paranoid-live-clock";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("counter", Counter::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.get_mut::<Counter>("counter").unwrap().value += 1;
    }
}
#[test]
fn reconstruction_preserves_live_phase_period_and_batch_horizon() {
    for hz in [59.94, 60.0, 144.0] {
        let mut sims = [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame]
            .map(|mode| Sim::<ClockGame>::new(()).unwrap().paranoid(mode));
        for sim in &mut sims {
            sim.frame_period(1000.0 / hz);
        }
        for frame in 0..120 {
            let now = 1234.567 + frame as f64 * 1000.0 / hz + if frame >= 50 { 40.0 } else { 0.0 };
            let steps = sims.each_mut().map(|s| s.advance(now, Clock::Live));
            assert_eq!(steps, [steps[0]; 3]);
            for other in &sims[1..] {
                assert_eq!(sims[0].alpha(), other.alpha(), "{hz} frame {frame}");
                assert_eq!(sims[0].save(), other.save(), "{hz} frame {frame}");
            }
        }
        assert!(sims[0].world().tick() > 40);
    }
}
