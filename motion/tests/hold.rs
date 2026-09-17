//! A gesture temporarily owns presentation, while committed style owns its target.
use exact_motion::{
    Change, Easing, Engine, EngineError, HoldEnd, Property, SpringConfig, TimingFunction,
    Transition, TransitionProperty, Transitions, Value,
};

const NODE: u64 = (3 << 32) | 7;
const P: Property = Property::Translate;
fn spring() -> Transitions {
    Transitions(vec![Transition::new(
        TransitionProperty::All,
        0.0,
        TimingFunction::Spring(SpringConfig::default()),
    )])
}
fn target(e: &mut Engine, value: Value) {
    e.observe(Change {
        node: NODE,
        property: P,
        value,
        velocity: None,
    })
    .unwrap();
}
fn engine() -> Engine {
    let mut e = Engine::new();
    e.set_transitions(NODE, spring()).unwrap();
    target(&mut e, Value::ZERO);
    e.frame();
    e
}
fn x(e: &Engine) -> f64 {
    e.value(NODE, P).unwrap().x
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn takeover_samples_the_running_curve_without_jumping_to_its_target() {
    let mut reference = engine();
    target(&mut reference, Value::new(100.0, 20.0));
    reference.advance(0.125).unwrap();
    let expected = reference.value(NODE, P).unwrap();
    let mut e = engine();
    target(&mut e, Value::new(100.0, 20.0));
    let held = e.begin_hold(NODE, P, 0.125, None).unwrap().unwrap();
    assert_eq!(held.value, expected);
    assert_eq!(e.value(NODE, P), Some(expected));
    assert_eq!(e.target(NODE, P), Some(Value::new(100.0, 20.0)));
    assert!(e.quiescent());
    assert!(e.settle_time().is_none());
    assert!(e.spring_frames(NODE, P).is_none());
    assert_eq!(held.token.node(), NODE);
    assert_eq!(held.token.property(), P);
    e.update_hold(held.token, 0.25, held.value + Value::new(12.0, -3.0))
        .unwrap();
    assert_eq!(e.value(NODE, P), Some(expected + Value::new(12.0, -3.0)));
}

#[test]
fn commits_during_hold_only_change_the_latest_authored_target() {
    let mut e = engine();
    let held = e.begin_hold(NODE, P, 0.1, None).unwrap().unwrap();
    e.update_hold(held.token, 0.2, Value::new(30.0, 0.0))
        .unwrap();
    e.frame();
    target(&mut e, Value::ZERO); // MotionSync repeats targets for any touched node.
    target(&mut e, Value::new(20.0, 0.0));
    target(&mut e, Value::new(80.0, 0.0));
    assert_eq!(x(&e), 30.0);
    assert_eq!(e.target(NODE, P), Some(Value::new(80.0, 0.0)));
    assert!(
        e.frame().is_empty(),
        "commits cannot repaint the held property"
    );
    e.advance(0.6).unwrap();
    assert_eq!(x(&e), 30.0);
    assert!(e.quiescent());
    assert!(e
        .end_hold(
            held.token,
            0.6,
            HoldEnd::Release {
                velocity: Value::new(100.0, 0.0)
            }
        )
        .unwrap());
    assert_eq!(x(&e), 30.0);
    e.advance(0.6 + 1.0 / 240.0).unwrap();
    assert!(x(&e) > 30.0);
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(x(&e), 80.0);
}

#[test]
fn browser_takeover_accepts_the_actual_computed_presentation_sample() {
    let mut e = engine();
    target(&mut e, Value::new(100.0, 0.0));
    let actual = Value::new(77.25, -3.0);
    let held = e.begin_hold(NODE, P, 0.1, Some(actual)).unwrap().unwrap();
    assert_eq!(held.value, actual);
    assert_eq!(e.value(NODE, P), Some(actual));
    assert_eq!(e.target(NODE, P), Some(Value::new(100.0, 0.0)));
    assert!(e.quiescent());
}

#[test]
fn rebegin_invalidates_old_moves_and_releases_before_they_can_move_time() {
    let mut e = engine();
    let old = e.begin_hold(NODE, P, 0.1, None).unwrap().unwrap();
    e.update_hold(old.token, 0.2, Value::new(20.0, 0.0))
        .unwrap();
    let new = e.begin_hold(NODE, P, 0.3, None).unwrap().unwrap();
    assert_eq!(new.value, Value::new(20.0, 0.0));
    assert_ne!(old.token.serial(), new.token.serial());
    e.frame();
    assert!(!e
        .update_hold(old.token, 999.0, Value::new(f64::NAN, 0.0))
        .unwrap());
    assert!(!e
        .end_hold(
            old.token,
            999.0,
            HoldEnd::Release {
                velocity: Value::new(f64::NAN, 0.0)
            }
        )
        .unwrap());
    assert_eq!(e.now(), 0.3);
    assert_eq!(x(&e), 20.0);
    assert!(e.frame().is_empty());
    assert!(e.end_hold(new.token, 0.4, HoldEnd::Cancel).unwrap());
    assert!(!e.end_hold(new.token, 1000.0, HoldEnd::Cancel).unwrap());
    assert_eq!(e.now(), 0.4);
}

#[test]
fn removal_and_recreation_never_resurrect_a_retired_hold() {
    let mut e = engine();
    let old = e.begin_hold(NODE, P, 0.1, None).unwrap().unwrap();
    e.remove(NODE);
    assert!(!e
        .update_hold(old.token, 500.0, Value::new(80.0, 0.0))
        .unwrap());
    assert!(e.begin_hold(NODE, P, 600.0, None).unwrap().is_none());
    assert_eq!(e.now(), 0.1);
    assert_eq!(e.value(NODE, P), None);
    assert!(e.frame().is_empty());
    // Even accidental reuse of the exact node number cannot revive this token.
    target(&mut e, Value::new(10.0, 0.0));
    let new = e.begin_hold(NODE, P, 0.2, None).unwrap().unwrap();
    assert!(!e.end_hold(old.token, 700.0, HoldEnd::Cancel).unwrap());
    assert!(e
        .update_hold(new.token, 0.3, Value::new(15.0, 0.0))
        .unwrap());
    assert_eq!(x(&e), 15.0);
}

#[test]
fn a_token_from_another_engine_cannot_control_the_same_node_number() {
    let mut old = engine();
    let token = old.begin_hold(NODE, P, 0.0, None).unwrap().unwrap().token;
    let mut e = engine();
    let held = e.begin_hold(NODE, P, 0.0, None).unwrap().unwrap();
    assert_ne!(token.serial(), held.token.serial());
    assert!(!e.update_hold(token, 999.0, Value::new(90.0, 0.0)).unwrap());
    assert!(!e.end_hold(token, 999.0, HoldEnd::Cancel).unwrap());
    assert_eq!(e.now(), 0.0);
    assert!(e
        .update_hold(held.token, 0.1, Value::new(12.0, 0.0))
        .unwrap());
}

#[test]
fn invalid_live_requests_leave_the_hold_and_clock_untouched() {
    let mut e = engine();
    assert_eq!(
        e.begin_hold(NODE, P, 0.9, Some(Value::new(f64::NAN, 0.0))),
        Err(EngineError::NonFinite)
    );
    assert_eq!(e.now(), 0.0);
    let held = e.begin_hold(NODE, P, 0.2, None).unwrap().unwrap();
    e.frame();
    assert_eq!(
        e.update_hold(held.token, 0.9, Value::new(f64::INFINITY, 0.0)),
        Err(EngineError::NonFinite)
    );
    assert_eq!(
        e.end_hold(
            held.token,
            0.9,
            HoldEnd::Release {
                velocity: Value::new(0.0, f64::NAN)
            }
        ),
        Err(EngineError::NonFinite)
    );
    assert_eq!(
        e.update_hold(held.token, 0.1, Value::ZERO),
        Err(EngineError::ClockWentBackwards)
    );
    assert_eq!(
        e.begin_hold(NODE, P, f64::INFINITY, None),
        Err(EngineError::NonFinite)
    );
    assert_eq!(e.now(), 0.2);
    assert!(e.frame().is_empty());
    assert!(e
        .update_hold(held.token, 0.3, Value::new(10.0, 0.0))
        .unwrap());
    assert!(e.end_hold(held.token, 0.4, HoldEnd::Cancel).unwrap());
}

#[test]
fn zero_distance_nonzero_release_velocity_still_starts_a_spring() {
    let mut e = engine();
    let held = e.begin_hold(NODE, P, 0.0, None).unwrap().unwrap();
    e.end_hold(
        held.token,
        0.1,
        HoldEnd::Release {
            velocity: Value::new(100.0, 0.0),
        },
    )
    .unwrap();
    assert_eq!(x(&e), 0.0);
    assert!(!e.quiescent());
    let frames = e.spring_frames(NODE, P).unwrap();
    assert_eq!(frames.values[0], Value::ZERO);
    assert!(frames.values.iter().any(|v| v.x > 0.0));
    assert_eq!(*frames.values.last().unwrap(), Value::ZERO);
    e.advance(0.1 + 1.0 / 240.0).unwrap();
    assert!(x(&e) > 0.0);
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(x(&e), 0.0);
}

#[test]
fn release_uses_the_latest_declaration_and_easings_ignore_velocity() {
    let mut e = engine();
    let held = e.begin_hold(NODE, P, 0.0, None).unwrap().unwrap();
    e.update_hold(held.token, 0.1, Value::new(40.0, 0.0))
        .unwrap();
    target(&mut e, Value::new(80.0, 0.0));
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::Property(P),
            0.5,
            TimingFunction::Easing(Easing::Linear),
        )]),
    )
    .unwrap();
    e.end_hold(
        held.token,
        0.2,
        HoldEnd::Release {
            velocity: Value::new(999.0, 0.0),
        },
    )
    .unwrap();
    assert!(e.spring_frames(NODE, P).is_none());
    e.advance(0.45).unwrap();
    close(x(&e), 60.0);
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(x(&e), 80.0);
}

#[test]
fn removed_transition_snaps_to_latest_target_on_release_for_every_property() {
    for property in Property::ALL {
        let mut e = engine();
        e.observe(Change {
            node: NODE,
            property,
            value: property.identity().unwrap_or(Value::scalar(180.0)),
            velocity: None,
        })
        .unwrap();
        let held = e.begin_hold(NODE, property, 0.0, None).unwrap().unwrap();
        e.update_hold(held.token, 0.1, Value::new(0.25, 0.0))
            .unwrap();
        e.observe(Change {
            node: NODE,
            property,
            value: Value::new(0.75, 0.0),
            velocity: None,
        })
        .unwrap();
        e.set_transitions(NODE, Transitions::NONE).unwrap();
        e.end_hold(
            held.token,
            0.2,
            HoldEnd::Release {
                velocity: Value::new(99.0, 0.0),
            },
        )
        .unwrap();
        assert_eq!(e.value(NODE, property), Some(Value::new(0.75, 0.0)));
        assert!(e.quiescent());
    }
}

#[test]
fn cancellation_returns_to_current_target_with_zero_velocity() {
    let run = |end| {
        let mut e = engine();
        let held = e.begin_hold(NODE, P, 0.0, None).unwrap().unwrap();
        e.update_hold(held.token, 0.1, Value::new(40.0, 0.0))
            .unwrap();
        target(&mut e, Value::new(10.0, 0.0));
        e.end_hold(held.token, 0.2, end).unwrap();
        e.advance(0.25).unwrap();
        e.value(NODE, P)
    };
    assert_eq!(
        run(HoldEnd::Cancel),
        run(HoldEnd::Release {
            velocity: Value::ZERO
        })
    );
}

#[test]
fn holding_one_property_leaves_other_animations_running() {
    let mut e = engine();
    e.observe(Change {
        node: NODE,
        property: Property::Opacity,
        value: Value::scalar(1.0),
        velocity: None,
    })
    .unwrap();
    e.observe(Change {
        node: NODE,
        property: Property::Opacity,
        value: Value::ZERO,
        velocity: None,
    })
    .unwrap();
    let held = e.begin_hold(NODE, P, 0.1, None).unwrap().unwrap();
    e.update_hold(held.token, 0.2, Value::new(40.0, 0.0))
        .unwrap();
    assert!(!e.quiescent());
    let opacity = e.value(NODE, Property::Opacity);
    e.frame();
    e.advance(0.3).unwrap();
    assert_ne!(e.value(NODE, Property::Opacity), opacity);
    assert_eq!(x(&e), 40.0);
    assert!(e.frame().iter().all(|p| p.property != P));
}

#[test]
fn released_curve_keeps_seek_determinism() {
    let run = |steps: &[f64]| {
        let mut e = engine();
        let held = e.begin_hold(NODE, P, 0.1, None).unwrap().unwrap();
        e.update_hold(held.token, 0.2, Value::new(40.0, 0.0))
            .unwrap();
        target(&mut e, Value::new(90.0, 0.0));
        e.end_hold(
            held.token,
            0.3,
            HoldEnd::Release {
                velocity: Value::new(200.0, 0.0),
            },
        )
        .unwrap();
        for t in steps {
            e.advance(*t).unwrap();
        }
        x(&e).to_bits()
    };
    assert_eq!(run(&[0.7]), run(&[0.4, 0.5, 0.6, 0.7]));
}

#[test]
fn scalar_shape_errors_cannot_replace_a_hold_or_move_the_clock() {
    for property in [Property::Scale, Property::Rotate, Property::Opacity] {
        let mut e = engine();
        e.observe(Change {
            node: NODE,
            property,
            value: property.identity().unwrap_or(Value::scalar(180.0)),
            velocity: None,
        })
        .unwrap();
        let held = e.begin_hold(NODE, property, 0.1, None).unwrap().unwrap();
        e.frame();
        let malformed = Value::new(0.5, 1.0);
        assert_eq!(
            e.begin_hold(NODE, property, 9.0, Some(malformed)),
            Err(EngineError::InvalidValueShape)
        );
        assert_eq!(
            e.update_hold(held.token, 9.0, malformed),
            Err(EngineError::InvalidValueShape)
        );
        assert_eq!(
            e.end_hold(
                held.token,
                9.0,
                HoldEnd::Release {
                    velocity: malformed
                }
            ),
            Err(EngineError::InvalidValueShape)
        );
        assert_eq!(
            e.observe(Change {
                node: NODE,
                property,
                value: malformed,
                velocity: None
            }),
            Err(EngineError::InvalidValueShape)
        );
        assert_eq!(
            e.observe(Change {
                node: NODE,
                property,
                value: Value::ZERO,
                velocity: Some(malformed)
            }),
            Err(EngineError::InvalidValueShape)
        );
        assert!(e.has_hold(held.token));
        assert!(e.is_held(NODE, property));
        assert_eq!(e.now(), 0.1);
        assert_eq!(
            e.value(NODE, property),
            Some(property.identity().unwrap_or(Value::scalar(180.0)))
        );
        assert_eq!(
            e.target(NODE, property),
            Some(property.identity().unwrap_or(Value::scalar(180.0)))
        );
        assert!(e.frame().is_empty());
    }
}

#[test]
fn rebegin_uses_held_presentation_as_the_new_displacement_origin() {
    let mut e = engine();
    target(&mut e, Value::new(100.0, 20.0));
    let first = e.begin_hold(NODE, P, 0.1, None).unwrap().unwrap();
    let finger = first.value + Value::new(12.0, -3.0);
    e.update_hold(first.token, 0.2, finger).unwrap();
    target(&mut e, Value::new(200.0, 40.0));
    let second = e.begin_hold(NODE, P, 0.3, None).unwrap().unwrap();
    assert_eq!(second.value, finger);
    assert_eq!(e.target(NODE, P), Some(Value::new(200.0, 40.0)));
    assert!(!e.has_hold(first.token));
    assert!(e.has_hold(second.token));
    e.update_hold(second.token, 0.4, second.value + Value::new(1.0, 2.0))
        .unwrap();
    assert_eq!(e.value(NODE, P), Some(finger + Value::new(1.0, 2.0)));
    e.end_hold(second.token, 0.5, HoldEnd::Cancel).unwrap();
    assert!(!e.has_hold(second.token));
    assert!(!e.is_held(NODE, P));
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(e.value(NODE, P), Some(Value::new(200.0, 40.0)));
}
