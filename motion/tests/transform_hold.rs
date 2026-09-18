//! One fixed Translate + Scale takeover, with independent token cleanup.
use exact_motion::{
    Change, Engine, EngineError, HoldEnd, HoldStart, Property, SpringConfig, TimingFunction,
    TransformHold, Transition, TransitionProperty, Transitions, Value,
};

const NODE: u64 = (7 << 32) | 31;
const PAIR: [Property; 2] = [Property::Translate, Property::Scale];

fn commit(e: &mut Engine, property: Property, value: Value) {
    e.observe(Change {
        node: NODE,
        property,
        value,
        velocity: None,
    })
    .unwrap();
}

fn spring(config: SpringConfig) -> Transitions {
    Transitions(vec![Transition::new(
        TransitionProperty::All,
        0.0,
        TimingFunction::Spring(config),
    )])
}

fn engine() -> Engine {
    let mut e = Engine::new();
    e.set_transitions(NODE, spring(SpringConfig::default()))
        .unwrap();
    commit(&mut e, PAIR[0], Value::new(-0.0, 0.0));
    commit(&mut e, PAIR[1], Value::scalar(1.0));
    e.frame();
    e
}

fn moving() -> Engine {
    let mut e = engine();
    commit(&mut e, PAIR[0], Value::new(140.0, -50.0));
    commit(&mut e, PAIR[1], Value::scalar(2.5));
    e.advance(0.04).unwrap();
    e.frame();
    e
}

// Engine's derived Debug includes every slot, curve, transition, dirty entry,
// hold serial and clock. Also compare all public value bits (including -0).
fn snapshot(e: &Engine) -> (String, Vec<u64>) {
    let mut bits = vec![e.now().to_bits()];
    for property in Property::ALL {
        for value in [e.value(NODE, property), e.target(NODE, property)]
            .into_iter()
            .flatten()
        {
            bits.extend([value.x.to_bits(), value.y.to_bits()]);
        }
    }
    (format!("{e:?}"), bits)
}

fn starts(held: TransformHold) -> [HoldStart; 2] {
    [held.translate(), held.scale()]
}

fn values(held: TransformHold) -> [Value; 2] {
    starts(held).map(|start| start.value)
}

#[test]
fn malformed_second_sample_preserves_both_curves_clock_and_pending_frame() {
    for bad in [Value::scalar(f64::NAN), Value::new(2.0, 1.0)] {
        let mut e = moving();
        let before = snapshot(&e);
        assert!(e
            .begin_transform_hold(NODE, 0.2, Some([Value::new(44.0, 12.0), bad]))
            .is_err());
        assert_eq!(snapshot(&e), before);
        assert!(e.frame().is_empty());
    }
}

#[test]
fn missing_second_slot_never_takes_over_first_curve() {
    let mut e = moving();
    e.remove_property(NODE, PAIR[1]);
    let before = snapshot(&e);
    assert!(e.begin_transform_hold(NODE, 0.2, None).unwrap().is_none());
    assert_eq!(snapshot(&e), before);
}

#[test]
fn either_missing_slot_is_checked_before_time_or_samples() {
    for missing in PAIR {
        let mut e = moving();
        e.remove_property(NODE, missing);
        let before = snapshot(&e);
        assert!(e
            .begin_transform_hold(
                NODE,
                f64::NAN,
                Some([Value::new(f64::NAN, 1.0), Value::new(2.0, 1.0)]),
            )
            .unwrap()
            .is_none());
        assert_eq!(snapshot(&e), before);
    }
}

#[test]
fn failed_rebegin_preserves_both_existing_tokens_and_presentations() {
    let mut e = moving();
    let old = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
    e.frame();
    let before = snapshot(&e);
    assert_eq!(
        e.begin_transform_hold(NODE, 0.3, Some([Value::ZERO, Value::new(1.0, 1.0)])),
        Err(EngineError::InvalidValueShape)
    );
    assert_eq!(snapshot(&e), before);
    assert!(starts(old).iter().all(|start| e.has_hold(start.token)));
}

#[test]
fn invalid_time_leaves_both_old_curves_unchanged() {
    for (time, error) in [
        (0.01, EngineError::ClockWentBackwards),
        (f64::NAN, EngineError::NonFinite),
        (f64::INFINITY, EngineError::NonFinite),
    ] {
        let mut e = moving();
        let before = snapshot(&e);
        assert_eq!(e.begin_transform_hold(NODE, time, None), Err(error));
        assert_eq!(snapshot(&e), before);
    }
}

#[test]
fn catches_both_midcurve_at_one_time_without_zero_displacement_jump() {
    let mut reference = moving();
    reference.advance(0.125).unwrap();
    let expected = PAIR.map(|property| reference.value(NODE, property).unwrap());
    let mut e = moving();
    let held = e.begin_transform_hold(NODE, 0.125, None).unwrap().unwrap();
    assert_eq!(values(held), expected);
    assert_eq!(starts(held).map(|s| s.token.property()), PAIR);
    assert_eq!(
        held.scale().token.serial(),
        held.translate().token.serial() + 1
    );
    assert!(e.update_transform_hold(held, 0.15, expected).unwrap());
    assert_eq!(PAIR.map(|p| e.value(NODE, p).unwrap()), expected);
    assert_eq!(e.target(NODE, PAIR[0]), Some(Value::new(140.0, -50.0)));
    assert_eq!(e.target(NODE, PAIR[1]), Some(Value::scalar(2.5)));
    assert!(e.quiescent());
    assert!(e.settle_time().is_none());
    e.frame();
    e.advance(2.0).unwrap();
    assert!(
        e.frame().is_empty(),
        "holding alone creates no idle frame work"
    );
}

#[test]
fn capture_matches_single_valid_controls_during_delay_completion_and_existing_hold() {
    for now in [0.1, 0.2, 0.3, 8.0] {
        for held_property in [None, Some(PAIR[0]), Some(PAIR[1])] {
            let prepare = || {
                let mut e = engine();
                let mut declarations = spring(SpringConfig::default());
                declarations.0[0].delay = 0.2;
                e.set_transitions(NODE, declarations).unwrap();
                commit(&mut e, PAIR[0], Value::new(120.0, 50.0));
                commit(&mut e, PAIR[1], Value::scalar(2.0));
                if let Some(property) = held_property {
                    e.begin_hold(NODE, property, 0.05, Some(Value::scalar(0.7)))
                        .unwrap();
                }
                e
            };
            let mut control = prepare();
            let expected = PAIR.map(|p| {
                control
                    .begin_hold(NODE, p, now, None)
                    .unwrap()
                    .unwrap()
                    .value
            });
            let mut e = prepare();
            let pair = e.begin_transform_hold(NODE, now, None).unwrap().unwrap();
            for (actual, expected) in values(pair).into_iter().zip(expected) {
                assert_eq!(actual.x.to_bits(), expected.x.to_bits());
                assert_eq!(actual.y.to_bits(), expected.y.to_bits());
            }
            assert_eq!(e.now().to_bits(), control.now().to_bits());
            assert_eq!(e.frame(), control.frame());
        }
    }
}

#[test]
fn supplied_presentations_preserve_bits_and_scale_remains_generic() {
    for scale in [-2.0, 0.0, 1.75] {
        let mut e = moving();
        let shown = [Value::new(-0.0, 12.0), Value::scalar(scale)];
        let held = e
            .begin_transform_hold(NODE, 0.1, Some(shown))
            .unwrap()
            .unwrap();
        assert_eq!(held.translate().value.x.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(values(held), shown);
        assert_eq!(PAIR.map(|p| e.value(NODE, p).unwrap()), shown);
        assert_eq!(e.target(NODE, PAIR[1]), Some(Value::scalar(2.5)));
    }
}

#[test]
fn paired_update_validates_both_values_and_time_before_mutation() {
    for (time, value, error) in [
        (0.3, Value::new(2.0, 1.0), EngineError::InvalidValueShape),
        (0.3, Value::scalar(f64::INFINITY), EngineError::NonFinite),
        (f64::NAN, Value::scalar(2.0), EngineError::NonFinite),
        (0.0, Value::scalar(2.0), EngineError::ClockWentBackwards),
    ] {
        let mut e = moving();
        let held = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
        e.frame();
        let before = snapshot(&e);
        assert_eq!(
            e.update_transform_hold(held, time, [Value::new(200.0, -30.0), value]),
            Err(error)
        );
        assert_eq!(snapshot(&e), before);
    }
}

#[test]
fn one_property_rebegin_rejects_old_pair_before_values_or_clock() {
    for replaced in PAIR {
        let mut e = moving();
        let old = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
        let successor = e.begin_hold(NODE, replaced, 0.2, None).unwrap().unwrap();
        e.frame();
        let before = snapshot(&e);
        assert!(!e
            .update_transform_hold(old, f64::NAN, [Value::scalar(f64::NAN); 2])
            .unwrap());
        assert!(!e
            .update_transform_hold(old, 10.0, [Value::ZERO, Value::scalar(1.0)])
            .unwrap());
        assert_eq!(snapshot(&e), before);

        // No paired cancel: independent old-token cleanup must release the
        // survivor without revoking the new owner of the replaced property.
        for start in starts(old) {
            let live = e.has_hold(start.token);
            assert_eq!(live, start.token.property() != replaced);
            assert_eq!(e.end_hold(start.token, 0.2, HoldEnd::Cancel).unwrap(), live);
        }
        assert!(e.has_hold(successor.token));
        assert_eq!(e.value(NODE, replaced), Some(successor.value));
        assert!(starts(old).iter().all(|start| !e.has_hold(start.token)));
    }
}

#[test]
fn pair_rebegin_invalidates_old_callbacks_without_changing_successor() {
    let mut e = moving();
    let old = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
    let next = e
        .begin_transform_hold(
            NODE,
            0.2,
            Some([Value::new(30.0, 10.0), Value::scalar(1.2)]),
        )
        .unwrap()
        .unwrap();
    e.frame();
    let before = snapshot(&e);
    assert!(!e.update_transform_hold(old, 90.0, values(old)).unwrap());
    for start in starts(old) {
        assert!(!e.end_hold(start.token, f64::NAN, HoldEnd::Cancel).unwrap());
    }
    assert_eq!(snapshot(&e), before);
    assert!(starts(next).iter().all(|start| e.has_hold(start.token)));
}

#[test]
fn removed_and_readopted_node_cannot_accept_old_pair() {
    let mut e = moving();
    let old = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
    e.remove(NODE);
    commit(&mut e, PAIR[0], Value::new(90.0, 7.0));
    commit(&mut e, PAIR[1], Value::scalar(3.0));
    let new = e.begin_transform_hold(NODE, 0.2, None).unwrap().unwrap();
    let before = snapshot(&e);
    assert!(!e.update_transform_hold(old, 50.0, values(old)).unwrap());
    for start in starts(old) {
        assert!(!e.end_hold(start.token, f64::NAN, HoldEnd::Cancel).unwrap());
    }
    assert_eq!(snapshot(&e), before);
    assert!(starts(new).iter().all(|start| e.has_hold(start.token)));
}

#[test]
fn pair_from_another_engine_is_stale_even_with_equal_node_number() {
    let mut first = engine();
    let other = first
        .begin_transform_hold(NODE, 0.0, None)
        .unwrap()
        .unwrap();
    let mut e = engine();
    let own = e.begin_transform_hold(NODE, 0.0, None).unwrap().unwrap();
    let before = snapshot(&e);
    assert!(!e
        .update_transform_hold(other, f64::NAN, [Value::scalar(f64::NAN); 2])
        .unwrap());
    assert_eq!(snapshot(&e), before);
    assert!(starts(own).iter().all(|start| e.has_hold(start.token)));
}

#[test]
fn commits_while_both_held_release_to_latest_targets_and_declaration() {
    let mut e = moving();
    let held = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
    let shown = [Value::new(40.0, -12.0), Value::scalar(1.6)];
    e.update_transform_hold(held, 0.2, shown).unwrap();
    let targets = [Value::new(100.0, 5.0), Value::scalar(2.0)];
    e.frame();
    for (p, v) in PAIR.into_iter().zip(targets) {
        commit(&mut e, p, v);
    }
    let config = SpringConfig {
        stiffness: 420.0,
        ..SpringConfig::default()
    };
    e.set_transitions(NODE, spring(config)).unwrap();
    assert!(starts(held).iter().all(|start| e.has_hold(start.token)));
    assert_eq!(PAIR.map(|p| e.value(NODE, p).unwrap()), shown);
    assert!(e.frame().is_empty());
    for (start, velocity) in starts(held)
        .into_iter()
        .zip([Value::new(45.0, -2.0), Value::scalar(0.3)])
    {
        assert!(e
            .end_hold(start.token, 0.2, HoldEnd::Release { velocity })
            .unwrap());
        let running = e.spring_descriptor(NODE, start.token.property()).unwrap();
        assert_eq!(running.config, config);
        assert_eq!(running.velocity, velocity);
    }
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(PAIR.map(|p| e.value(NODE, p).unwrap()), targets);
    assert!(starts(held).iter().all(|start| !e.has_hold(start.token)));
}

#[test]
fn terminal_action_can_remove_one_property_then_cleanup_the_survivor() {
    let mut e = engine();
    let held = e.begin_transform_hold(NODE, 0.0, None).unwrap().unwrap();
    assert!(starts(held).iter().all(|start| e.has_hold(start.token)));
    // Simulate the synchronous authored action/receipt while both tokens live.
    commit(&mut e, PAIR[0], Value::new(18.0, 22.0));
    e.remove_property(NODE, PAIR[1]);
    for start in starts(held) {
        if e.has_hold(start.token) {
            assert!(e.end_hold(start.token, 0.1, HoldEnd::Cancel).unwrap());
        }
    }
    assert!(starts(held).iter().all(|start| !e.has_hold(start.token)));
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(e.value(NODE, PAIR[0]), Some(Value::new(18.0, 22.0)));
    assert_eq!(e.value(NODE, PAIR[1]), None);
}

#[test]
fn taking_the_pair_does_not_stop_unrelated_opacity_motion() {
    let mut e = moving();
    commit(&mut e, Property::Opacity, Value::scalar(1.0));
    commit(&mut e, Property::Opacity, Value::scalar(0.3));
    let curve = e.spring_descriptor(NODE, Property::Opacity).unwrap();
    let held = e.begin_transform_hold(NODE, 0.1, None).unwrap().unwrap();
    let opacity = e.value(NODE, Property::Opacity).unwrap();
    assert!(!e.quiescent());
    e.update_transform_hold(held, 0.2, values(held)).unwrap();
    assert_ne!(e.value(NODE, Property::Opacity), Some(opacity));
    assert_eq!(e.spring_descriptor(NODE, Property::Opacity), Some(curve));
}
