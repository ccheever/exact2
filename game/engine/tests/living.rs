use exact_game::*;
struct Living;
impl Game for Living {
    const ID: &'static str = "living-rest";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("sun", (Transform::default(), Ambient));
        w.insert_resource(Environment::default());
        w.ambient_resource::<Environment>();
        w.derived_publication("countdown");
        w.publish("countdown", 60.);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.require_mut::<Transform>("sun").rotation = Quat::from_rotation_x(w.tick_end().seconds());
        w.resource_mut::<Environment>().ambient = 1. - w.tick_end().seconds() / 60.;
        w.publish("countdown", 60. - w.tick_end().seconds() as f64);
    }
}
#[test]
fn derived_outputs_allow_rest_but_keep_save_and_hash_rules() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut s = Sim::<Living>::new(()).unwrap().paranoid(mode);
        assert!(s.settle());
        let hash = s.world().hash();
        let save = s.save().unwrap();
        let published = s.world().published("countdown");
        s.restore(&save).unwrap();
        assert_eq!(s.world().published("countdown"), published);
        assert_eq!(s.world().hash(), hash);
        assert!(s.settle());
        s.run(60_000.);
        assert!(s.world().resource::<Environment>().ambient <= 0.);
        assert_ne!(s.world().hash(), hash);
        // Exclusion from observation never excludes authored data from the hash.
        let hash = s.world().hash();
        s.world().resource_mut::<Environment>().ambient = 0.25;
        assert_ne!(s.world().hash(), hash);
        let hash = s.world().hash();
        s.world().require_mut::<Transform>("sun").position.x += 1.;
        assert_ne!(s.world().hash(), hash);
        // Publications remain outside the world hash, even when derived.
        let hash = s.world().hash();
        s.world().publish("countdown", 123.);
        assert_eq!(s.world().hash(), hash);
    }
}
struct Lantern;
impl Game for Lantern {
    const ID: &'static str = "saved-light-spring";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "light",
            (
                Transform::default(),
                PointLight::default(),
                Lit(Spring::new(0.)),
            ),
        );
        w.require_mut::<Lit>("light").to(w.now(), 1.);
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn light_spring_rest_retarget_and_restore_are_continuous() {
    let mut s = Sim::<Lantern>::new(()).unwrap();
    s.run(100.);
    assert!(!s.quiescent());
    let now = s.world().now();
    let old = s.world().require::<Lit>("light").0.value(now);
    let velocity = s.world().require::<Lit>("light").0.velocity(now);
    assert!(old > 0. && old < 1.);
    s.world().require_mut::<Lit>("light").to(now, 0.4);
    assert_eq!(s.world().require::<Lit>("light").0.value(now), old);
    assert_eq!(s.world().require::<Lit>("light").0.velocity(now), velocity);
    let save = s.save().unwrap();
    let mut restored = Sim::<Lantern>::new(()).unwrap();
    restored.restore(&save).unwrap();
    for tick in [7, 9, 15, 120] {
        let seconds = (tick as f64 - 0.5) / 60.;
        assert_eq!(
            s.world().require::<Lit>("light").0.value_at(seconds, 60),
            restored
                .world()
                .require::<Lit>("light")
                .0
                .value_at(seconds, 60)
        );
    }
    assert_eq!(
        s.save().unwrap(),
        save,
        "frame sampling changed world bytes"
    );
    assert!(s.settle() && restored.settle());
    assert_eq!(s.save().unwrap(), restored.save().unwrap());
    assert!(s.world().require::<Lit>("light").0.at_rest(s.world().now()));
}

struct Never;
impl Game for Never {
    const ID: &'static str = "settle-exhaustion";
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.busy("crate remains awake");
    }
}
#[test]
fn bounded_settle_names_unfinished_work() {
    let mut s = Sim::<Never>::new(()).unwrap();
    assert!(!s.settle());
    assert!(s.world().tick() <= 16 * 120);
    assert!(s
        .world()
        .journal()
        .iter()
        .any(|e| e.line.contains("settle exhausted: crate remains awake")));
}
// Save mode once round-tripped every tick: 205-310 s against 15 s Off for the
// forest's proof. It now rebuilds at each advance's last tick (what a proof
// observes) and every 16th tick inside one, and still agrees with Off.
#[test]
fn paranoid_save_rebuilds_at_observed_and_sampled_ticks_only() {
    let mut off = Sim::<Living>::new(()).unwrap();
    let mut save = Sim::<Living>::new(()).unwrap().paranoid(Paranoid::Save);
    for s in [&mut off, &mut save] {
        s.run(0.);
    }
    let rebuilds = |s: &Sim<Living>| s.world().presentation_generation();
    let start = rebuilds(&save);
    save.run(1000.);
    off.run(1000.);
    // Ticks 16, 32 and 48 of 60, then the last.
    assert_eq!(rebuilds(&save) - start, 4);
    save.run(1000. / 60.);
    off.run(1000. / 60.);
    assert_eq!(rebuilds(&save) - start, 5);
    assert_eq!(rebuilds(&off), 0);
    assert_eq!(save.world().hash(), off.world().hash());
    assert_eq!(save.save().unwrap(), off.save().unwrap());
}
