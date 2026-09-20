use exact_world::{bin, hash, math, Now, Rng, Spring, SpringConfig};

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
fn rng_is_reproducible_bounded_and_round_trips() {
    let mut a = Rng::new(123);
    let mut b = Rng::new(123);
    for _ in 0..10000 {
        assert_eq!(a.next_u32(), b.next_u32());
        let n = a.next_f32();
        assert!((0.0..1.0).contains(&n));
        assert_eq!(n, b.next_f32());
    }
    let mut saved: Rng = bin::from_slice(&bin::to_vec(&a).unwrap()).unwrap();
    assert_eq!(a.next_u32(), saved.next_u32());
}
#[test]
fn math_uses_the_declared_functions() {
    assert_eq!(math::sin(1.0).to_bits(), libm::sinf(1.0).to_bits());
    assert_eq!(math::exp(2.0).to_bits(), libm::expf(2.0).to_bits());
    assert_eq!(math::smoothstep(0.0, 1.0, 0.5), 0.5);
    assert_eq!(
        math::wrap_angle(std::f32::consts::PI),
        -std::f32::consts::PI
    );
    assert_eq!(math::lerp(0.0, 10.0, 0.3), 3.0);
}

#[test]
fn tween_uses_exact_motion_and_saved_deadline() {
    use exact_world::{Data, Tween};
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
