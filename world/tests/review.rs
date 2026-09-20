use exact_world::*;
#[derive(Default, Component)]
struct Counter {
    n: u32,
}
struct Board;
impl Game for Board {
    const ID: &'static str = "k1d";
    type Args = ();
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
        w.register::<Counter>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("counter", Counter { n: 7 }).unwrap();
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.get_mut::<Counter>("counter").unwrap().n += 1;
    }
}
#[test]
fn untouched_and_edited_candidates_preserve_complete_continuation() {
    for drained in [false, true] {
        let mut s = Sim::<Board>::new(()).unwrap();
        s.run(17.).unwrap();
        s.world().publish("score", 7u32);
        s.world().log("saved game history").unwrap();
        s.world().emit("pending delivery");
        s.world().session_log("session history").unwrap();
        if drained {
            s.world().take_published();
        }
        let before = s.save().unwrap();
        s.world()
            .candidate()
            .unwrap()
            .commit(s.world_mut())
            .unwrap();
        assert_eq!(s.save().unwrap(), before);
        assert_eq!(s.world().take_published().is_some(), !drained);
        assert!(s
            .world()
            .logs(LogCursor::default())
            .unwrap()
            .entries
            .contains("session history"));
        let e = s.world().named("counter").unwrap();
        let mut c = s.world().candidate().unwrap();
        c.edit(
            Some(e),
            "Counter",
            &bin::to_vec(&Counter { n: 99 }).unwrap(),
        )
        .unwrap();
        c.commit(s.world_mut()).unwrap();
        assert_eq!(s.world().get::<Counter>(e).unwrap().n, 99);
        assert_eq!(s.world().publications().len(), 1);
        assert!(s
            .world()
            .logs(LogCursor::default())
            .unwrap()
            .entries
            .contains("saved game history"));
        assert_eq!(s.world().take_messages(), ["pending delivery"]);
    }
}

#[test]
fn five_empty_columns_do_not_change_observation_of_200k_entities() {
    #[derive(Default, Component)]
    struct A;
    #[derive(Default, Component)]
    struct B;
    #[derive(Default, Component)]
    struct C;
    #[derive(Default, Component)]
    struct D;
    #[derive(Default, Component)]
    struct E;
    let mut w = World::new(60, 0);
    w.register::<A>()
        .unwrap()
        .register::<B>()
        .unwrap()
        .register::<C>()
        .unwrap()
        .register::<D>()
        .unwrap()
        .register::<E>()
        .unwrap();
    for _ in 0..MAX_ENTITIES {
        w.spawn(()).unwrap();
    }
    let high = w.entity_at(MAX_ENTITIES - 1).unwrap();
    let before = w.sample().unwrap();
    w.insert(high, A).unwrap();
    w.remove::<A>(high);
    w.insert(high, B).unwrap();
    w.remove::<B>(high);
    w.insert(high, C).unwrap();
    w.remove::<C>(high);
    w.insert(high, D).unwrap();
    w.remove::<D>(high);
    w.insert(high, E).unwrap();
    w.remove::<E>(high);
    let after = w.sample().unwrap();
    assert_eq!((after.hash, after.components), (before.hash, 0));
    let bytes = w.save().unwrap();
    w.load(&bytes).unwrap();
    assert_eq!(w.sample().unwrap().hash, before.hash);
    w.insert(high, A).unwrap();
    assert_eq!(w.sample().unwrap().components, 1);
    assert_ne!(w.sample().unwrap().hash, before.hash);
}

#[test]
fn extreme_rebased_clocks_refuse_without_mutation() {
    let mut s = Sim::<Board>::new(()).unwrap();
    s.run(17.).unwrap();
    s.reconcile_input(0., &[]).unwrap();
    let before = s.save().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.advance_to((i64::MAX / 1000) as f64)
    }));
    assert!(result.is_ok(), "clock overflow panicked");
    assert!(result.unwrap().is_err());
    assert_eq!(s.save().unwrap(), before);
    s.reconcile_input((i64::MAX / 1000) as f64, &[]).unwrap();
    let before = s.save().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.settle(2)));
    assert!(result.is_ok(), "settle overflow panicked");
    assert!(result.unwrap().is_err());
    assert_eq!(s.save().unwrap(), before);
    assert_eq!(s.world().observation(), None);
}

#[test]
fn built_in_motion_values_cannot_save_bytes_they_refuse() {
    for (start_value, target, duration) in [
        (0., 1., -1.),
        (f32::NAN, 1., 1.),
        (0., f32::INFINITY, 1.),
        (0., 1., f32::NAN),
    ] {
        let value = Tween {
            start_value,
            target,
            duration,
            start_tick: 0,
        };
        assert!(bin::to_vec(&value).is_err(), "invalid Tween was encoded");
    }
    for value in [
        SpringConfig {
            mass: 0.,
            ..Default::default()
        },
        SpringConfig {
            damping: f64::NAN,
            ..Default::default()
        },
    ] {
        assert!(
            bin::to_vec(&value).is_err(),
            "invalid SpringConfig was encoded"
        );
    }
    let value = Tween::new(5.);
    let bytes = bin::to_vec(&value).unwrap();
    assert_eq!(
        bin::to_vec(&bin::from_slice::<Tween>(&bytes).unwrap()).unwrap(),
        bytes
    );
    let value = SpringConfig::default();
    let bytes = bin::to_vec(&value).unwrap();
    assert_eq!(
        bin::to_vec(&bin::from_slice::<SpringConfig>(&bytes).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn smoothstep_preserves_nan_input() {
    assert!(math::smoothstep(0., 1., f32::NAN).is_nan());
    assert_eq!(math::smoothstep(0., 1., 0.5), 0.5);
}

#[test]
fn panicking_tick_cannot_be_saved_through_the_underlying_world() {
    struct Panics;
    impl Game for Panics {
        const ID: &'static str = "panics";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.spawn(()).unwrap();
            panic!("tick interrupted");
        }
    }
    let mut s = Sim::<Panics>::new(()).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.run(17.))).is_err());
    assert!(s.save().is_err());
    assert!(
        s.world().save().is_err(),
        "partial tick escaped as valid EXGAME"
    );
}

#[test]
fn owned_world_load_is_refused_before_changing_simulation() {
    let mut s = Sim::<Board>::new(()).unwrap();
    let bytes = s.world().save().unwrap();
    s.run(17.).unwrap();
    let before = s.save().unwrap();
    assert!(s.world_mut().load(&bytes).is_err());
    assert_eq!(s.save().unwrap(), before);
}

#[derive(Default, Args)]
struct Options {
    #[live]
    paused: bool,
    label: String,
}
struct Configured;
impl Game for Configured {
    const ID: &'static str = "configured";
    type Args = Options;
    fn setup(_: &mut World, _: &Options) {}
    fn tick(_: &mut World, _: &Input, _: &Options) {}
}
#[test]
fn args_admission_and_live_carry_identity() {
    assert!(Sim::<Configured>::new(Options {
        label: "x".repeat(1_048_577),
        ..Default::default()
    })
    .is_err());
    let bytes = Sim::<Configured>::new(Options::default())
        .unwrap()
        .save()
        .unwrap();
    let mut s = Sim::<Configured>::new(Options {
        paused: true,
        ..Default::default()
    })
    .unwrap();
    assert!(
        s.carry(&bytes).unwrap(),
        "retained live arguments change canonical bytes"
    );
    assert!(s.args().paused);
    assert_ne!(s.save().unwrap(), bytes);
}

#[test]
fn natural_ownership_chains_refuse_at_a_bounded_walk() {
    let mut w = World::new(60, 0);
    let mut parent = w.spawn(()).unwrap();
    for _ in 0..256 {
        let child = w.spawn(()).unwrap();
        w.set_parent(child, Some(parent)).unwrap();
        parent = child;
    }
    let child = w.spawn(()).unwrap();
    let before = w.save().unwrap();
    assert!(w.set_parent(child, Some(parent)).is_err());
    assert_eq!(w.save().unwrap(), before);
    assert_eq!(w.query::<&Parent>().iter().count(), 256);
}

#[test]
fn manual_data_shared_mutation_cannot_return_a_stale_hash() {
    #[derive(Default)]
    struct Manual(std::cell::Cell<u32>);
    impl Data for Manual {
        fn write(&self, w: &mut dyn Writer) {
            self.0.get().write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            let mut n = 0;
            n.read(r)?;
            self.0.set(n);
            Ok(())
        }
    }
    impl Component for Manual {
        const NAME: &'static str = "Manual";
    }
    let mut w = World::new(60, 0);
    w.register::<Manual>().unwrap();
    let e = w.spawn(Manual::default()).unwrap();
    let before = w.hash();
    w.get::<Manual>(e).unwrap().0.set(5);
    assert_ne!(w.hash(), before);
}

#[test]
fn registration_collision_names_both_rust_types_and_stale_insert_refuses() {
    mod left {
        #[derive(Default, exact_world::Component)]
        pub struct Same;
    }
    mod right {
        #[derive(Default, exact_world::Component)]
        pub struct Same;
    }
    let mut w = World::new(60, 0);
    w.register::<left::Same>().unwrap();
    let error = match w.register::<right::Same>() {
        Err(e) => e.to_string(),
        Ok(_) => panic!("collision admitted"),
    };
    assert!(
        error.contains("left::Same") && error.contains("right::Same"),
        "{error}"
    );
    let stale = w.spawn(left::Same).unwrap();
    w.despawn(stale);
    assert!(w.insert(stale, left::Same).is_err());
}
