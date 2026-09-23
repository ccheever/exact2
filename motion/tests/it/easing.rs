//! CSS easing outputs, pinned to what a browser computes for the same input.
//! The keyword values at 0.5 are the ones `getComputedStyle` yields for a
//! 1s transition sampled at 500ms; a 1e-3 band covers solver precision.

use exact_motion::{Easing, EasingError, LinearStop, StepPosition};

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn keywords_match_the_browser_at_the_midpoint() {
    close(Easing::Linear.progress(0.5), 0.5);
    close(Easing::Ease.progress(0.5), 0.8024);
    close(Easing::EaseIn.progress(0.5), 0.3153);
    close(Easing::EaseOut.progress(0.5), 0.6847);
    close(Easing::EaseInOut.progress(0.5), 0.5);
}

#[test]
fn keywords_are_their_cubic_beziers() {
    let pairs = [
        (Easing::Ease, (0.25, 0.1, 0.25, 1.0)),
        (Easing::EaseIn, (0.42, 0.0, 1.0, 1.0)),
        (Easing::EaseOut, (0.0, 0.0, 0.58, 1.0)),
        (Easing::EaseInOut, (0.42, 0.0, 0.58, 1.0)),
    ];
    for (keyword, (x1, y1, x2, y2)) in pairs {
        let explicit = Easing::CubicBezier { x1, y1, x2, y2 };
        for i in 0..=20 {
            let x = i as f64 / 20.0;
            assert_eq!(
                keyword.progress(x).to_bits(),
                explicit.progress(x).to_bits()
            );
        }
    }
}

#[test]
fn every_curve_is_pinned_at_the_ends_and_clamped_outside() {
    let curves = [
        Easing::Linear,
        Easing::Ease,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
        Easing::CubicBezier {
            x1: 0.3,
            y1: -0.5,
            x2: 0.7,
            y2: 1.5,
        },
        Easing::Steps {
            count: 3,
            position: StepPosition::JumpEnd,
        },
        Easing::PiecewiseLinear(vec![
            LinearStop {
                input: 0.0,
                output: 0.0,
            },
            LinearStop {
                input: 0.5,
                output: 0.9,
            },
            LinearStop {
                input: 1.0,
                output: 1.0,
            },
        ]),
    ];
    for curve in curves {
        assert_eq!(curve.progress(0.0), 0.0, "{curve:?} at 0");
        assert_eq!(curve.progress(1.0), 1.0, "{curve:?} at 1");
        assert_eq!(curve.progress(-1.0), 0.0, "{curve:?} below 0");
        assert_eq!(curve.progress(2.0), 1.0, "{curve:?} above 1");
    }
}

#[test]
fn an_overshooting_bezier_leaves_the_unit_range() {
    let back = Easing::CubicBezier {
        x1: 0.68,
        y1: -0.55,
        x2: 0.27,
        y2: 1.55,
    };
    assert!(back.progress(0.15) < 0.0, "anticipation dips below 0");
    assert!(back.progress(0.85) > 1.0, "overshoot rises above 1");
}

#[test]
fn steps_follow_the_css_step_position_table() {
    let at = |count, position, x| Easing::Steps { count, position }.progress(x);
    // jump-end: floor(x·n)/n.
    close(at(4, StepPosition::JumpEnd, 0.3), 0.25);
    close(at(4, StepPosition::JumpEnd, 0.5), 0.5);
    close(at(4, StepPosition::JumpEnd, 0.999), 0.75);
    // jump-start: (floor(x·n)+1)/n.
    close(at(4, StepPosition::JumpStart, 0.0), 0.25);
    close(at(4, StepPosition::JumpStart, 0.3), 0.5);
    // jump-none: floor(x·n)/(n−1).
    close(at(4, StepPosition::JumpNone, 0.0), 0.0);
    close(at(4, StepPosition::JumpNone, 0.5), 2.0 / 3.0);
    close(at(4, StepPosition::JumpNone, 1.0), 1.0);
    // jump-both: (floor(x·n)+1)/(n+1).
    close(at(4, StepPosition::JumpBoth, 0.0), 0.2);
    close(at(4, StepPosition::JumpBoth, 0.5), 0.6);
    close(at(4, StepPosition::JumpBoth, 1.0), 1.0);
}

#[test]
fn piecewise_linear_interpolates_between_stops() {
    let curve = Easing::PiecewiseLinear(vec![
        LinearStop {
            input: 0.0,
            output: 0.0,
        },
        LinearStop {
            input: 0.25,
            output: 1.0,
        },
        LinearStop {
            input: 1.0,
            output: 1.0,
        },
    ]);
    close(curve.progress(0.125), 0.5);
    close(curve.progress(0.5), 1.0);
}

#[test]
fn invalid_easings_are_refused_by_name() {
    assert_eq!(
        Easing::CubicBezier {
            x1: 1.5,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0
        }
        .validate(),
        Err(EasingError::ControlPointOutOfRange)
    );
    assert_eq!(
        Easing::CubicBezier {
            x1: f64::NAN,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0
        }
        .validate(),
        Err(EasingError::NonFinite)
    );
    assert_eq!(
        Easing::Steps {
            count: 0,
            position: StepPosition::JumpEnd
        }
        .validate(),
        Err(EasingError::ZeroSteps)
    );
    assert_eq!(
        Easing::Steps {
            count: 1,
            position: StepPosition::JumpNone
        }
        .validate(),
        Err(EasingError::JumpNoneNeedsTwoSteps)
    );
    assert_eq!(
        Easing::PiecewiseLinear(vec![LinearStop {
            input: 0.0,
            output: 0.0
        }])
        .validate(),
        Err(EasingError::TooFewStops)
    );
    assert_eq!(
        Easing::PiecewiseLinear(vec![
            LinearStop {
                input: 0.5,
                output: 0.0
            },
            LinearStop {
                input: 0.2,
                output: 1.0
            }
        ])
        .validate(),
        Err(EasingError::StopsNotSorted)
    );
    assert_eq!(
        Easing::PiecewiseLinear(vec![
            LinearStop {
                input: 0.0,
                output: 0.0
            },
            LinearStop {
                input: 1.5,
                output: 1.0
            }
        ])
        .validate(),
        Err(EasingError::StopOutOfRange)
    );
}
