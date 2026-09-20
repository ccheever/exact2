//! The spring: closed form in every regime, settle-derived duration, and a
//! keyframe lowering the web plays that matches the native evaluator.

use exact_motion::spring::{keyframes, MAX_DURATION, REST_THRESHOLD, SAMPLE_RATE};
use exact_motion::{SpringConfig, SpringError};

fn under() -> SpringConfig {
    SpringConfig {
        stiffness: 100.0,
        damping: 10.0,
        mass: 1.0,
    }
}

fn critical() -> SpringConfig {
    SpringConfig {
        stiffness: 100.0,
        damping: 20.0,
        mass: 1.0,
    }
}

fn over() -> SpringConfig {
    SpringConfig {
        stiffness: 100.0,
        damping: 40.0,
        mass: 1.0,
    }
}

#[test]
fn every_regime_starts_where_released_and_settles_at_the_target() {
    for config in [under(), critical(), over()] {
        let start = config.sample(100.0, 0.0, 0.0);
        assert_eq!(start.displacement, 100.0, "{config:?}");
        assert!(
            start.velocity.abs() < 1e-9,
            "{config:?}: {}",
            start.velocity
        );
        let t = config.settle_time(100.0, 0.0);
        assert!(t > 0.0 && t < MAX_DURATION, "{config:?} settles in {t}s");
        assert!(config.sample(100.0, 0.0, t).at_rest(), "{config:?}");
        assert!(
            !config.sample(100.0, 0.0, t / 2.0).at_rest(),
            "{config:?} is not at rest halfway"
        );
    }
}

#[test]
fn an_underdamped_spring_overshoots_and_a_critical_one_does_not() {
    let overshoot = (0..2000)
        .map(|n| {
            under()
                .sample(100.0, 0.0, n as f64 / SAMPLE_RATE)
                .displacement
        })
        .fold(f64::INFINITY, f64::min);
    assert!(
        overshoot < -1.0,
        "underdamped crosses the target: {overshoot}"
    );
    let min = (0..2000)
        .map(|n| {
            critical()
                .sample(100.0, 0.0, n as f64 / SAMPLE_RATE)
                .displacement
        })
        .fold(f64::INFINITY, f64::min);
    assert!(
        min >= -REST_THRESHOLD,
        "critically damped never crosses: {min}"
    );
}

#[test]
fn a_released_velocity_carries_through() {
    let kicked = under().sample(0.0, 500.0, 0.05).displacement;
    assert!(
        kicked > 5.0,
        "a kick away from rest moves the value: {kicked}"
    );
    let t = under().settle_time(0.0, 500.0);
    assert!(under().sample(0.0, 500.0, t).at_rest());
}

#[test]
fn a_spring_already_at_rest_settles_immediately() {
    assert_eq!(under().settle_time(0.0, 0.0), 0.0);
    let (duration, frames) = keyframes(&under(), 3.0, 0.0, 3.0);
    assert_eq!(duration, 0.0);
    assert_eq!(frames.len(), 2);
}

#[test]
fn the_keyframe_lowering_is_the_closed_form_sampled_on_the_grid() {
    let config = under();
    let (duration, frames) = keyframes(&config, 0.0, 0.0, 200.0);
    assert!(duration > 0.0);
    assert_eq!(
        frames.first().map(|f| (f.offset, f.value)),
        Some((0.0, 0.0))
    );
    assert_eq!(
        frames.last().map(|f| (f.offset, f.value)),
        Some((1.0, 200.0))
    );
    for pair in frames.windows(2) {
        assert!(pair[0].offset < pair[1].offset, "offsets strictly increase");
    }
    // Every interior frame is bit-identical to the native evaluator's sample
    // at that time: the web plays what native would have computed.
    for (n, frame) in frames.iter().enumerate().take(frames.len() - 1) {
        let t = n as f64 / SAMPLE_RATE;
        let native = 200.0 + config.sample(-200.0, 0.0, t).displacement;
        assert_eq!(frame.value.to_bits(), native.to_bits(), "frame {n}");
    }
    // Between frames the browser interpolates linearly. The error is bounded
    // by ω²·A·dt²/8 ≈ 100·200/(8·240²) ≈ 0.04 pt here — under a pixel.
    for pair in frames.windows(2) {
        let mid_t = (pair[0].offset + pair[1].offset) * 0.5 * duration;
        let mid_linear = (pair[0].value + pair[1].value) * 0.5;
        let native = 200.0 + config.sample(-200.0, 0.0, mid_t).displacement;
        assert!(
            (mid_linear - native).abs() < 0.05,
            "{mid_linear} vs {native}"
        );
    }
}

#[test]
fn invalid_parameters_are_refused_by_name() {
    let bad = |f: fn(&mut SpringConfig)| {
        let mut c = under();
        f(&mut c);
        c.validate()
    };
    assert_eq!(
        bad(|c| c.stiffness = 0.0),
        Err(SpringError::NonPositiveStiffness)
    );
    assert_eq!(bad(|c| c.damping = -1.0), Err(SpringError::NegativeDamping));
    assert_eq!(bad(|c| c.mass = 0.0), Err(SpringError::NonPositiveMass));
    assert_eq!(bad(|c| c.mass = f64::INFINITY), Err(SpringError::NonFinite));
    assert_eq!(under().validate(), Ok(()));
}

#[test]
fn the_engine_lowers_a_running_spring_to_the_frames_it_would_sample() {
    use exact_motion::{
        Change, Engine, Property, TimingFunction, Transition, TransitionProperty, Transitions,
        Value,
    };
    let mut engine = Engine::new();
    let spring = Transition::new(
        TransitionProperty::All,
        0.0,
        TimingFunction::Spring(under()),
    );
    engine
        .set_transitions(1, Transitions(vec![spring]))
        .unwrap();
    let observe = |engine: &mut Engine, property, value| {
        engine
            .observe(Change {
                node: 1,
                property,
                value,
                velocity: None,
            })
            .unwrap();
    };
    observe(&mut engine, Property::Translate, Value::ZERO);
    observe(&mut engine, Property::Opacity, Value::scalar(1.0));
    assert!(
        engine.spring_frames(1, Property::Translate).is_none(),
        "nothing runs yet"
    );
    observe(&mut engine, Property::Translate, Value::new(100.0, 10.0));
    observe(&mut engine, Property::Opacity, Value::scalar(0.0));

    // A scalar's frames are exactly `keyframes()`.
    let frames = engine.spring_frames(1, Property::Opacity).unwrap();
    let (duration, expected) = keyframes(&under(), 1.0, 0.0, 0.0);
    assert_eq!(frames.duration, duration);
    assert_eq!(frames.start, 0.0);
    assert_eq!(
        frames
            .values
            .iter()
            .map(|v| v.x.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|k| k.value.to_bits())
            .collect::<Vec<_>>()
    );

    // A pair runs until both components rest; its frames are the engine's
    // own samples on the grid, bit for bit.
    let frames = engine.spring_frames(1, Property::Translate).unwrap();
    let x = under().settle_time(-100.0, 0.0);
    let y = under().settle_time(-10.0, 0.0);
    assert_eq!(frames.duration, x.max(y));
    assert_eq!(frames.values[0], Value::ZERO);
    assert_eq!(*frames.values.last().unwrap(), Value::new(100.0, 10.0));
    for (n, value) in frames
        .values
        .iter()
        .enumerate()
        .take(frames.values.len() - 1)
    {
        engine.advance(n as f64 / SAMPLE_RATE).unwrap();
        let sampled = engine.value(1, Property::Translate).unwrap();
        assert_eq!(
            (sampled.x.to_bits(), sampled.y.to_bits()),
            (value.x.to_bits(), value.y.to_bits()),
            "frame {n}"
        );
    }
}

#[test]
fn slow_oscillators_do_not_settle_at_a_temporary_rest_sample() {
    let c = SpringConfig {
        stiffness: 0.0324,
        damping: 0.,
        mass: 1.,
    };
    let t = c.settle_time(0.005, 0.);
    assert_eq!(
        t, MAX_DURATION,
        "undamped motion must run to the presentation snap cap"
    );
    assert!(c.sample(0.005, 0., 17.5).displacement.abs() > 0.004);
}

#[test]
fn finite_but_overflowing_oscillator_parameters_are_refused() {
    for c in [
        SpringConfig {
            stiffness: 1.,
            damping: f64::MAX,
            mass: f64::MIN_POSITIVE,
        },
        SpringConfig {
            stiffness: f64::MAX,
            damping: 1.,
            mass: 1.,
        },
        SpringConfig {
            stiffness: f64::MIN_POSITIVE,
            damping: 0.,
            mass: f64::MAX,
        },
    ] {
        assert!(c.validate().is_err(), "{c:?}");
    }
}

#[test]
fn admitted_extreme_configs_keep_long_horizon_samples_finite() {
    for stiffness in [1e-12, 0.0324, 1., 100., 1e12] {
        for damping in [0., 1e-6, 1., 2e6] {
            let c = SpringConfig {
                stiffness,
                damping,
                mass: 1.,
            };
            c.validate().unwrap();
            for t in [0., 1e-9, 0.1, 10., 1e6, u64::MAX as f64] {
                let s = c.sample(100., -50., t);
                assert!(
                    s.displacement.is_finite() && s.velocity.is_finite(),
                    "{c:?} {t}: {s:?}"
                );
                if c.rest_after(100., -50., t) {
                    for later in [0., 0.1, 10., 1000.] {
                        assert!(c.sample(100., -50., t + later).at_rest());
                    }
                }
            }
        }
    }
}
