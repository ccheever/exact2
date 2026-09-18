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
