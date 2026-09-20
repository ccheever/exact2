use exact_world::{bin, hash, Now};
use exact_world_motion::{Spring, SpringConfig, Tween};

#[test]
fn spring_seeks_have_no_sampling_history() {
    for damping in [3.0, 20.0, 40.0] {
        let mut spring = Spring::new(3.0)
            .with_config(SpringConfig {
                stiffness: 100.0,
                damping,
                mass: 1.0,
            })
            .unwrap();
        spring.set_target(Now { tick: 0, hz: 60 }, 15.0);
        let direct = spring.value(Now { tick: 120, hz: 60 });
        let before = hash::of(&spring);
        let mut last = 0.0;
        for tick in 0..=120 {
            last = spring.value(Now { tick, hz: 60 });
        }
        assert_eq!(last.to_bits(), direct.to_bits());
        assert_eq!(hash::of(&spring), before);
        let old_value = spring.value(Now { tick: 60, hz: 60 });
        spring.set_target(Now { tick: 60, hz: 60 }, 20.0);
        assert!((spring.value(Now { tick: 60, hz: 60 }) - old_value).abs() < 1e-12);
        let loaded: Spring = bin::from_slice(&bin::to_vec(&spring).unwrap()).unwrap();
        assert_eq!(
            loaded.value(Now { tick: 120, hz: 60 }).to_bits(),
            spring.value(Now { tick: 120, hz: 60 }).to_bits()
        );
    }
    assert!(Spring::new(8.0).at_rest(Now { tick: 0, hz: 60 }));
}
#[test]
fn tween_uses_exact_motion_and_saved_deadline() {
    let mut t = Tween::new(-1.);
    t.to(Now { tick: 4, hz: 60 }, 3., 0.5);
    let bytes = bin::to_vec(&t).unwrap();
    let loaded: Tween = bin::from_slice(&bytes).unwrap();
    for tick in 4..35 {
        let progress = exact_motion::Easing::CubicBezier {
            x1: 1. / 3.,
            y1: 0.,
            x2: 2. / 3.,
            y2: 1.,
        }
        .progress((tick - 4) as f64 / 30.);
        assert_eq!(t.value(Now { tick, hz: 60 }), (-1. + 4. * progress) as f32);
        assert_eq!(
            loaded.value(Now { tick, hz: 60 }),
            t.value(Now { tick, hz: 60 })
        );
    }
    assert_eq!(t.settle_tick(Now { tick: 4, hz: 60 }), Some(34));
    assert_eq!(t.value(Now { tick: 34, hz: 60 }), 3.);
}

#[test]
fn spring_refuses_nonfinite_scalar_state() {
    use exact_world::{Data, DataError, Reader, Writer};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    #[derive(Default)]
    struct Forged;
    impl Data for Forged {
        fn write(&self, w: &mut dyn Writer) {
            w.begin_struct();
            w.field("start_velocity");
            f64::INFINITY.write(w);
            w.end_struct();
        }
        fn read(&mut self, _: &mut dyn Reader) -> Result<(), DataError> {
            unreachable!()
        }
    }
    assert!(bin::from_slice::<Spring>(&bin::to_vec(&Forged).unwrap()).is_err());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(catch_unwind(|| Spring::new(value)).is_err());
        let mut s = Spring::new(1.);
        let before = bin::to_vec(&s).unwrap();
        assert!(catch_unwind(AssertUnwindSafe(
            || s.set_target(Now { tick: 0, hz: 60 }, value)
        ))
        .is_err());
        assert_eq!(bin::to_vec(&s).unwrap(), before);
    }
}

#[test]
fn undamped_spring_has_no_settle_deadline_despite_temporary_rest() {
    let mut s = Spring::new(0.005)
        .with_config(SpringConfig {
            stiffness: 0.0324,
            damping: 0.,
            mass: 1.,
        })
        .unwrap();
    s.set_target(Now { tick: 0, hz: 60 }, 0.);
    for tick in [0, 450, 456, 500, 1050] {
        assert_eq!(s.settle_tick(Now { tick, hz: 60 }), None);
    }
    assert!(s.value(Now { tick: 1050, hz: 60 }).abs() > 0.004);
}

#[test]
fn module_deadline_survives_exact_restore_and_prevents_early_settle() {
    use exact_world::*;
    #[derive(Default, Component)]
    struct Motion(Tween);
    struct Animated;
    impl Game for Animated {
        const ID: &'static str = "module-motion";
        type Args = ();
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<Motion>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &()) -> Result<(), DataError> {
            let mut tween = Tween::new(0.);
            tween.to(w.now(), 1., 1.);
            w.work(
                "animation",
                Work::Deadline(tween.settle_tick(w.now()).unwrap()),
            )?;
            w.spawn(Motion(tween))?;
            Ok(())
        }
        fn tick(_: &mut World, _: &Input, _: &()) -> Result<(), DataError> {
            Ok(())
        }
    }
    let mut sim = Sim::<Animated>::new(()).unwrap();
    assert!(sim.settle(10).is_err());
    assert_eq!(sim.world().tick(), 10);
    assert!(!sim.world().quiescent());
    let bytes = sim.save().unwrap();
    let mut loaded = Sim::<Animated>::from_save(&bytes).unwrap();
    assert_eq!(loaded.save().unwrap(), bytes);
    assert_eq!(loaded.settle(60).unwrap(), 50);
    assert!(loaded.world().quiescent());
    let value = loaded
        .world()
        .query::<&Motion>()
        .iter()
        .next()
        .unwrap()
        .1
         .0
        .value(loaded.world().now());
    assert_eq!(value, 1.);
}

#[test]
fn module_refuses_overflowing_configs_without_changing_core_admission() {
    for (stiffness, damping, mass) in [
        (1., f64::MAX, f64::MIN_POSITIVE),
        (f64::MAX, 1., 1.),
        (f64::MIN_POSITIVE, 0., f64::MAX),
    ] {
        assert!(Spring::new(1.)
            .with_config(SpringConfig {
                stiffness,
                damping,
                mass
            })
            .is_err());
    }
    for stiffness in [1e-14, 1e-12, (1e-12_f64 as f32) as f64] {
        let core = exact_motion::SpringConfig {
            stiffness,
            damping: 0.,
            mass: 1.,
        };
        core.validate().unwrap();
        assert!(core.sample(1., 0., 10.).displacement.is_finite());
    }
}
