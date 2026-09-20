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
