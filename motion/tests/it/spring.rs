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

// The closed form from origin/main before the extreme-input stability path.
fn original_sample(
    config: SpringConfig,
    displacement: f64,
    velocity: f64,
    elapsed: f64,
) -> (f64, f64) {
    let alpha = config.damping / (2.0 * config.mass);
    let omega_squared = config.stiffness / config.mass;
    let discriminant = alpha * alpha - omega_squared;
    let epsilon = omega_squared * 1.0e-12;
    let decay = libm::exp(-alpha * elapsed);
    if discriminant < -epsilon {
        let omega_d = (-discriminant).sqrt();
        let a = displacement;
        let b = (velocity + alpha * displacement) / omega_d;
        let phase = omega_d * elapsed;
        let cosine = libm::cos(phase);
        let sine = libm::sin(phase);
        (
            decay * (a * cosine + b * sine),
            decay * ((-alpha * a + b * omega_d) * cosine + (-alpha * b - a * omega_d) * sine),
        )
    } else if discriminant.abs() <= epsilon {
        let a = displacement;
        let b = velocity + alpha * displacement;
        (
            decay * (a + b * elapsed),
            decay * (b - alpha * (a + b * elapsed)),
        )
    } else {
        let root = discriminant.sqrt();
        let r1 = -alpha + root;
        let r2 = -alpha - root;
        let c1 = (velocity - r2 * displacement) / (r1 - r2);
        let c2 = displacement - c1;
        let e1 = libm::exp(r1 * elapsed);
        let e2 = libm::exp(r2 * elapsed);
        (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
    }
}

#[test]
fn ordinary_spring_samples_and_keyframes_match_origin_main() {
    for config in [
        under(),
        critical(),
        over(),
        SpringConfig {
            stiffness: 3.0,
            damping: 4.0,
            mass: 1.0,
        },
    ] {
        for (displacement, velocity) in [(100.0, 0.0), (0.0, 30.0), (-7.0, 13.0), (1e100, -1e100)] {
            for elapsed in [
                0.0,
                1.0 / SAMPLE_RATE,
                0.1,
                0.5,
                std::f64::consts::LN_2,
                1.0,
                2.0,
            ] {
                let actual = config.sample(displacement, velocity, elapsed);
                let expected = original_sample(config, displacement, velocity, elapsed);
                for (got, want) in [
                    (actual.displacement, expected.0),
                    (actual.velocity, expected.1),
                ] {
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "{config:?}, x={displacement}, v={velocity}, t={elapsed}: {got} vs {want}"
                    );
                }
            }
        }

        let (duration, frames) = keyframes(&config, 100.0, 0.0, 0.0);
        let expected_duration = (0..=(MAX_DURATION * SAMPLE_RATE) as usize)
            .map(|n| n as f64 / SAMPLE_RATE)
            .find(|&t| {
                let (x, v) = original_sample(config, 100.0, 0.0, t);
                x.abs() < REST_THRESHOLD && v.abs() < REST_THRESHOLD
            })
            .unwrap_or(MAX_DURATION);
        assert_eq!(duration, expected_duration, "{config:?}");
        assert_eq!(
            frames.len(),
            (duration * SAMPLE_RATE) as usize + 1,
            "{config:?}"
        );
        for (n, frame) in frames.iter().enumerate().take(frames.len() - 1) {
            let t = n as f64 / SAMPLE_RATE;
            let expected = original_sample(config, 100.0, 0.0, t).0;
            assert_eq!(frame.offset, t / duration, "{config:?}, frame {n}");
            assert_eq!(
                frame.value.to_bits(),
                expected.to_bits(),
                "{config:?}, frame {n}: {} vs {expected}",
                frame.value
            );
        }
        assert_eq!(frames.last().unwrap().value, 0.0);
    }
    let half_life = SpringConfig {
        stiffness: 3.0,
        damping: 4.0,
        mass: 1.0,
    }
    .sample(1.0, 0.0, std::f64::consts::LN_2);
    assert!((half_life.displacement - 0.6875).abs() < 1e-12);
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
fn extreme_parameters_either_refuse_or_lower_to_finite_frames() {
    for config in [
        SpringConfig {
            stiffness: 100.0,
            damping: 1e200,
            mass: 1.0,
        },
        SpringConfig {
            stiffness: 1e200,
            damping: 10.0,
            mass: 1.0,
        },
        SpringConfig {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1e-200,
        },
        SpringConfig {
            stiffness: 1e-200,
            damping: 1e-200,
            mass: 1e200,
        },
    ] {
        if let Err(error) = config.validate() {
            assert_eq!(error, SpringError::OutOfRange, "{config:?}");
            continue;
        }
        for n in 0..=2400 {
            let sample = config.sample(1.0, 500.0, n as f64 / SAMPLE_RATE);
            assert!(
                sample.displacement.is_finite() && sample.velocity.is_finite(),
                "{config:?} at {n}"
            );
        }
        let (_, frames) = keyframes(&config, 1.0, 500.0, 0.0);
        assert!(
            frames
                .iter()
                .all(|frame| frame.value.is_finite() && frame.offset.is_finite()),
            "{config:?}"
        );
    }
}

#[test]
fn tiny_frequency_keeps_large_finite_release_and_keyframes_finite() {
    let config = SpringConfig {
        stiffness: 1e-308,
        damping: 0.0,
        mass: 1.0,
    };
    assert_eq!(config.validate(), Ok(()));
    let sample = config.sample(1.0, 1e156, 0.1);
    assert!(
        sample.displacement.is_finite() && sample.velocity.is_finite(),
        "{sample:?}"
    );
    assert!(
        (sample.displacement / 1e155 - 1.0).abs() < 1e-12,
        "{sample:?}"
    );
    assert!((sample.velocity / 1e156 - 1.0).abs() < 1e-12, "{sample:?}");
    let (_, frames) = keyframes(&config, 1.0, 1e156, 0.0);
    assert!(
        frames.iter().all(|frame| frame.value.is_finite()),
        "{frames:?}"
    );
}

#[test]
fn extreme_finite_releases_sample_in_every_regime() {
    let magnitudes = [
        0.0,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        1.0,
        f32::MAX as f64,
        1e100,
        1e308,
        f64::MAX,
    ];
    for config in [under(), critical(), over()] {
        assert_eq!(config.validate(), Ok(()));
        for displacement in magnitudes {
            for velocity in magnitudes {
                for displacement in [displacement, -displacement] {
                    for velocity in [velocity, -velocity] {
                        for elapsed in [0.0, 1.0 / SAMPLE_RATE, 0.1, 2.0, MAX_DURATION] {
                            let sample = config.sample(displacement, velocity, elapsed);
                            assert!(
                                sample.displacement.is_finite() && sample.velocity.is_finite(),
                                "{config:?}, x={displacement}, v={velocity}, t={elapsed}: {sample:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    let config = SpringConfig {
        stiffness: 1.0,
        damping: 2.0,
        mass: 1.0,
    };
    let sample = config.sample(1.0, 1e308, 2.0);
    assert!((sample.displacement / 1e307 - 2.7067056647).abs() < 1e-9);
    let (_, frames) = keyframes(&config, 1.0, 1e308, 0.0);
    assert!(frames.iter().all(|frame| frame.value.is_finite()));

    let slow_over = SpringConfig {
        stiffness: 1e-308,
        damping: 4e-154,
        mass: 1.0,
    };
    assert_eq!(slow_over.validate(), Ok(()));
    let sample = slow_over.sample(0.0, 1e308, 0.1);
    assert!(
        (sample.displacement / 1e307 - 1.0).abs() < 1e-12,
        "{sample:?}"
    );
    assert!((sample.velocity / 1e308 - 1.0).abs() < 1e-12, "{sample:?}");
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
