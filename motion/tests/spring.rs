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
