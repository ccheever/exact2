//! @ref LLP 0099#animation-drivers

use super::*;

fn spring_after(refresh_hz: f64, duration_seconds: f64) -> MotionSample {
    let mut spring =
        SpringDriver::new(120.0, -340.0, 0.0, SpringConfig::default()).expect("valid spring");
    let frame = 1.0 / refresh_hz;
    for _ in 0..(refresh_hz * duration_seconds).round() as usize {
        spring.advance(frame).expect("monotonic finite frame");
    }
    spring.sample()
}

#[test]
fn analytic_spring_is_refresh_invariant_and_true_elapsed_gap_settles() {
    let expected = spring_after(240.0, 0.5);
    for refresh_hz in [60.0_f64, 80.0, 90.0, 120.0] {
        let sample = spring_after(refresh_hz, 0.5);
        assert!((sample.position - expected.position).abs() < 1.0e-9);
        assert!((sample.velocity - expected.velocity).abs() < 1.0e-9);
    }

    let mut spring =
        SpringDriver::new(120.0, -340.0, 0.0, SpringConfig::default()).expect("valid spring");
    let sample = spring.advance(10.0).expect("suspension gap is valid");
    assert_eq!(sample.position, 0.0);
    assert_eq!(sample.velocity, 0.0);
    assert!(sample.settled);
}

#[test]
fn spring_solver_covers_critical_and_overdamped_regimes() {
    for damping in [20.0, 40.0] {
        let config = SpringConfig {
            damping,
            stiffness: 100.0,
            mass: 1.0,
            ..SpringConfig::default()
        };
        let mut spring = SpringDriver::new(10.0, 2.0, 0.0, config).expect("valid config");
        let sample = spring.advance(0.25).expect("valid sample");
        assert!(sample.position.is_finite());
        assert!(sample.velocity.is_finite());
        assert!(sample.position.abs() < 10.5);
    }
}

#[test]
fn finite_driver_inputs_cannot_publish_non_finite_samples_or_stay_scheduled() {
    let unsafe_spring = SpringConfig {
        damping: f64::MAX,
        stiffness: f64::MAX,
        mass: f64::MIN_POSITIVE,
        ..SpringConfig::default()
    };
    assert!(matches!(
        SpringDriver::new(1.0, 0.0, 0.0, unsafe_spring),
        Err(MotionDriverError::NonFinite(_))
    ));
    assert!(matches!(
        TimingDriver::new(
            -f64::MAX,
            f64::MAX,
            TimingConfig {
                duration_seconds: 1.0,
                easing: MotionEasing::Linear,
            },
        ),
        Err(MotionDriverError::NonFinite("timing.distance"))
    ));
    assert!(matches!(
        DecayDriver::new(
            0.0,
            f64::MAX,
            DecayConfig {
                deceleration: 1.0 - f64::EPSILON,
                ..DecayConfig::default()
            },
        ),
        Err(MotionDriverError::NonFinite("decay.terminal_displacement"))
    ));

    let driver = AnimationDriverSpec::Spring {
        target: 0.0,
        config: SpringConfig {
            damping: 0.0,
            stiffness: f64::MAX,
            mass: 1.0,
            ..SpringConfig::default()
        },
    }
    .instantiate(1.0, 0.0)
    .expect("finite initial spring state");
    let mut table = MotionDriverTable::default();
    table.start(7, driver).expect("start driver");
    let publications = table
        .advance(f64::MAX)
        .expect("numerical failure becomes a terminal publication");
    assert_eq!(publications.len(), 1);
    assert!(publications[0].sample.position.is_finite());
    assert!(publications[0].sample.velocity.is_finite());
    assert!(publications[0].sample.settled);
    assert_eq!(
        publications[0]
            .terminal
            .as_ref()
            .map(|terminal| terminal.reason),
        Some(DriverTerminalReason::NumericalFailure)
    );
    assert_eq!(table.active_count(), 0);
}

#[test]
fn analytic_timing_and_decay_have_exact_terminals_and_clamps() {
    let mut timing = TimingDriver::new(
        0.0,
        10.0,
        TimingConfig {
            duration_seconds: 1.0,
            easing: MotionEasing::CubicBezier {
                x1: 0.25,
                y1: 0.1,
                x2: 0.25,
                y2: 1.0,
            },
        },
    )
    .expect("valid timing");
    assert!(timing.advance(0.5).expect("valid sample").position > 5.0);
    assert_eq!(
        timing.advance(0.5).expect("valid sample"),
        MotionSample {
            position: 10.0,
            velocity: 0.0,
            settled: true,
        }
    );

    let mut decay = DecayDriver::new(
        0.0,
        1_000.0,
        DecayConfig {
            clamp: Some((0.0, 100.0)),
            ..DecayConfig::default()
        },
    )
    .expect("valid decay");
    let sample = decay.advance(2.0).expect("valid gap");
    assert_eq!(sample.position, 100.0);
    assert_eq!(sample.velocity, 0.0);
    assert!(sample.settled);
}

#[test]
fn sequence_and_repeat_use_a_refresh_invariant_fixed_quantum() {
    let timing = |target| AnimationDriverSpec::Timing {
        target,
        config: TimingConfig {
            duration_seconds: 0.1,
            easing: MotionEasing::Linear,
        },
    };
    let sequence = AnimationDriverSpec::Sequence(vec![timing(10.0), timing(20.0)]);
    let mut results = Vec::new();
    for refresh_hz in [60.0_f64, 80.0, 90.0, 120.0] {
        let mut driver = sequence.instantiate(0.0, 0.0).expect("valid sequence");
        let frames = (refresh_hz * 0.25).round() as usize;
        for _ in 0..frames {
            driver.advance(1.0 / refresh_hz).expect("valid frame");
        }
        results.push(driver.sample());
    }
    for sample in &results {
        assert_eq!(sample.position, 20.0);
        assert!(sample.settled);
    }

    let repeat = AnimationDriverSpec::Repeat {
        inner: Box::new(timing(10.0)),
        count: 2,
        reverse: true,
    };
    let mut driver = repeat.instantiate(0.0, 0.0).expect("valid repeat");
    let advance = driver.advance(0.25).expect("valid frame");
    assert_eq!(advance.sample.position, 0.0);
    assert!(advance.sample.settled);
    assert_eq!(advance.terminal, Some(DriverTerminalReason::Settled));
    assert_eq!(
        driver.advance(1.0).expect("post-terminal sample").terminal,
        None
    );

    let infinite = AnimationDriverSpec::Repeat {
        inner: Box::new(timing(10.0)),
        count: -1,
        reverse: true,
    };
    let mut driver = infinite.instantiate(0.0, 0.0).expect("valid repeat");
    assert!(!driver.advance(1.0).expect("valid frame").sample.settled);
}

#[test]
fn driver_table_enforces_exclusivity_and_exactly_once_terminals() {
    let spec = AnimationDriverSpec::Timing {
        target: 1.0,
        config: TimingConfig {
            duration_seconds: 0.1,
            easing: MotionEasing::Linear,
        },
    };
    let mut table = MotionDriverTable::default();
    assert_eq!(
        table
            .start(7, spec.instantiate(0.0, 0.0).expect("valid driver"))
            .expect("sequence available"),
        None
    );
    let replaced = table
        .start(7, spec.instantiate(0.25, 0.0).expect("valid driver"))
        .expect("sequence available")
        .expect("replacement terminal");
    assert_eq!(replaced.reason, DriverTerminalReason::Replaced);
    assert_eq!(replaced.driver_sequence, 1);

    let publications = table.advance(0.2).expect("valid frame");
    assert_eq!(publications.len(), 1);
    assert_eq!(
        publications[0].terminal.expect("settled terminal").reason,
        DriverTerminalReason::Settled
    );
    assert_eq!(table.active_count(), 0);
    assert!(table.advance(1.0).expect("valid frame").is_empty());

    table
        .start(8, spec.instantiate(0.0, 0.0).expect("valid driver"))
        .expect("sequence available");
    assert_eq!(
        table
            .cancel_for_imperative_write(8)
            .expect("cancellation")
            .reason,
        DriverTerminalReason::ImperativeWrite
    );
    assert_eq!(table.cancel_for_imperative_write(8), None);

    assert_eq!(table.reserve_sequence().expect("synthetic terminal"), 4);
    table
        .start(9, spec.instantiate(0.0, 0.0).expect("valid driver"))
        .expect("sequence available");
    let after_synthetic = table
        .start(9, spec.instantiate(0.0, 0.0).expect("valid driver"))
        .expect("sequence available")
        .expect("replacement terminal");
    assert_eq!(after_synthetic.driver_sequence, 5);
}
