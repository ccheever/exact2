//! The engine under CSS Transitions §3: what starts, what interrupts, what
//! reverses, and that the clock is a seek.

use exact_motion::{
    Change, Easing, Engine, EngineError, Property, SpringConfig, TimingFunction, Transition,
    TransitionError, TransitionProperty, Transitions, Value, VelocityTracker,
};

const NODE: u64 = 7;

fn ease(property: TransitionProperty, duration: f64, easing: Easing) -> Transition {
    Transition::new(property, duration, TimingFunction::Easing(easing))
}

fn opacity(value: f64) -> Change {
    Change {
        node: NODE,
        property: Property::Opacity,
        value: Value::scalar(value),
        velocity: None,
    }
}

fn engine_with(transitions: Vec<Transition>) -> Engine {
    let mut engine = Engine::new();
    engine
        .set_transitions(NODE, Transitions(transitions))
        .unwrap();
    engine.observe(opacity(1.0)).unwrap();
    engine.frame();
    engine
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-6,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn a_property_seen_for_the_first_time_takes_its_value_without_a_transition() {
    let mut engine = Engine::new();
    engine
        .set_transitions(
            NODE,
            Transitions(vec![ease(TransitionProperty::All, 1.0, Easing::Linear)]),
        )
        .unwrap();
    engine.observe(opacity(0.0)).unwrap();
    assert_eq!(
        engine.value(NODE, Property::Opacity),
        Some(Value::scalar(0.0))
    );
    assert!(engine.quiescent());
    assert_eq!(engine.frame().len(), 1);
}

#[test]
fn without_a_matching_declaration_a_change_is_immediate() {
    let mut engine = engine_with(vec![ease(
        TransitionProperty::Property(Property::Scale),
        1.0,
        Easing::Linear,
    )]);
    engine.observe(opacity(0.0)).unwrap();
    assert!(engine.quiescent());
    assert_eq!(
        engine.value(NODE, Property::Opacity),
        Some(Value::scalar(0.0))
    );
}

#[test]
fn a_matching_declaration_transitions_from_the_current_value() {
    let mut engine = engine_with(vec![ease(
        TransitionProperty::Property(Property::Opacity),
        1.0,
        Easing::Linear,
    )]);
    engine.observe(opacity(0.0)).unwrap();
    assert!(!engine.quiescent());
    assert_eq!(engine.settle_time(), Some(1.0));
    engine.advance(0.25).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.75);
    engine.advance(1.0).unwrap();
    assert_eq!(
        engine.value(NODE, Property::Opacity),
        Some(Value::scalar(0.0))
    );
    assert!(engine.quiescent());
}

#[test]
fn the_clock_is_a_seek() {
    let run = |steps: &[f64]| {
        let mut engine = engine_with(vec![ease(TransitionProperty::All, 1.0, Easing::EaseInOut)]);
        engine.observe(opacity(0.0)).unwrap();
        for t in steps {
            engine.advance(*t).unwrap();
        }
        engine.value(NODE, Property::Opacity).unwrap().x.to_bits()
    };
    let direct = run(&[0.375]);
    let stepped = run(&[0.125, 0.25, 0.3, 0.375]);
    assert_eq!(
        direct, stepped,
        "the value depends on t, not on the path to t"
    );
}

#[test]
fn the_same_inputs_give_the_same_bits() {
    let run = || {
        let mut engine = engine_with(vec![
            ease(TransitionProperty::All, 0.4, Easing::Ease),
            Transition::new(
                TransitionProperty::Property(Property::Translate),
                0.0,
                TimingFunction::Spring(SpringConfig::default()),
            ),
        ]);
        engine.observe(opacity(0.2)).unwrap();
        engine
            .observe(Change {
                node: NODE,
                property: Property::Translate,
                value: Value::new(0.0, 0.0),
                velocity: None,
            })
            .unwrap();
        engine.frame();
        engine
            .observe(Change {
                node: NODE,
                property: Property::Translate,
                value: Value::new(120.0, -40.0),
                velocity: Some(Value::new(300.0, 0.0)),
            })
            .unwrap();
        let mut trace = Vec::new();
        for n in 1..=90 {
            engine.advance(n as f64 / 60.0).unwrap();
            for p in engine.frame() {
                trace.push((p.node, p.property, p.value.x.to_bits(), p.value.y.to_bits()));
            }
        }
        trace
    };
    assert_eq!(run(), run());
}

#[test]
fn a_change_to_the_running_end_value_does_nothing() {
    let mut engine = engine_with(vec![ease(TransitionProperty::All, 1.0, Easing::Linear)]);
    engine.observe(opacity(0.0)).unwrap();
    engine.advance(0.5).unwrap();
    engine.frame();
    engine.observe(opacity(0.0)).unwrap();
    assert!(engine.frame().is_empty(), "nothing to repaint");
    assert_eq!(
        engine.settle_time(),
        Some(1.0),
        "the running transition keeps its end"
    );
}

#[test]
fn an_interruption_restarts_from_the_current_value() {
    let mut engine = engine_with(vec![ease(TransitionProperty::All, 1.0, Easing::Linear)]);
    engine.observe(opacity(0.0)).unwrap();
    engine.advance(0.5).unwrap();
    engine.observe(opacity(0.25)).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.5);
    assert_eq!(
        engine.settle_time(),
        Some(1.5),
        "a fresh full-length transition"
    );
    engine.advance(1.0).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.375);
}

#[test]
fn a_reversal_is_shortened_by_the_css_reversing_rule() {
    // CSS Transitions §3.2: reversing an ease-in-out at 25% elapsed, where the
    // curve's output is ≈0.129, yields a new transition of ≈0.129 × 1s — not a
    // full second back — so a hover-off that follows a brief hover-on is brief.
    let mut engine = engine_with(vec![ease(TransitionProperty::All, 1.0, Easing::EaseInOut)]);
    engine.observe(opacity(0.0)).unwrap();
    engine.advance(0.25).unwrap();
    let progress = Easing::EaseInOut.progress(0.25);
    engine.observe(opacity(1.0)).unwrap();
    let end = engine.settle_time().unwrap();
    close(end, 0.25 + progress);
    engine.advance(end).unwrap();
    assert_eq!(
        engine.value(NODE, Property::Opacity),
        Some(Value::scalar(1.0))
    );
    assert!(engine.quiescent());
}

#[test]
fn a_reversal_of_a_reversal_compounds_the_factor() {
    let mut engine = engine_with(vec![ease(TransitionProperty::All, 1.0, Easing::Linear)]);
    engine.observe(opacity(0.0)).unwrap();
    engine.advance(0.5).unwrap();
    engine.observe(opacity(1.0)).unwrap(); // factor 0.5 → 0.5 s back
    engine.advance(0.75).unwrap(); // halfway through the shortened one
    engine.observe(opacity(0.0)).unwrap(); // reverse again: |0.5·0.5 + 0.5| = 0.75
    close(engine.settle_time().unwrap(), 0.75 + 0.75);
}

#[test]
fn delay_holds_the_start_value_and_negative_delay_starts_partway() {
    let mut engine = engine_with(vec![Transition {
        property: TransitionProperty::All,
        duration: 1.0,
        delay: 0.5,
        timing: TimingFunction::Easing(Easing::Linear),
    }]);
    engine.observe(opacity(0.0)).unwrap();
    engine.advance(0.25).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 1.0);
    engine.advance(1.0).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.5);

    let mut engine = engine_with(vec![Transition {
        property: TransitionProperty::All,
        duration: 1.0,
        delay: -0.5,
        timing: TimingFunction::Easing(Easing::Linear),
    }]);
    engine.observe(opacity(0.0)).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.5);
    assert_eq!(engine.settle_time(), Some(0.5));
}

#[test]
fn a_zero_combined_duration_starts_nothing() {
    let mut engine = engine_with(vec![ease(TransitionProperty::All, 0.0, Easing::Ease)]);
    engine.observe(opacity(0.0)).unwrap();
    assert!(engine.quiescent());
    assert_eq!(
        engine.value(NODE, Property::Opacity),
        Some(Value::scalar(0.0))
    );
}

#[test]
fn the_last_covering_declaration_wins() {
    let mut engine = engine_with(vec![
        ease(TransitionProperty::All, 2.0, Easing::Linear),
        ease(
            TransitionProperty::Property(Property::Opacity),
            0.5,
            Easing::Linear,
        ),
    ]);
    engine.observe(opacity(0.0)).unwrap();
    assert_eq!(engine.settle_time(), Some(0.5));
}

#[test]
fn a_spring_inherits_velocity_across_an_interruption() {
    let spring = Transition::new(
        TransitionProperty::All,
        0.0,
        TimingFunction::Spring(SpringConfig::default()),
    );
    let mut engine = engine_with(vec![spring]);
    engine.observe(opacity(0.0)).unwrap();
    engine.advance(0.05).unwrap();
    let before = engine.value(NODE, Property::Opacity).unwrap().x;
    engine.observe(opacity(1.0)).unwrap();
    // Continuous: the retarget starts exactly where the value was …
    close(engine.value(NODE, Property::Opacity).unwrap().x, before);
    // … and keeps moving toward 0 for a moment before turning, because the
    // velocity carried over instead of resetting to rest.
    engine.advance(0.05 + 1.0 / 240.0).unwrap();
    assert!(engine.value(NODE, Property::Opacity).unwrap().x < before);
    let end = engine.settle_time().unwrap();
    engine.advance(end).unwrap();
    assert_eq!(
        engine.value(NODE, Property::Opacity),
        Some(Value::scalar(1.0))
    );
    assert!(engine.quiescent());
}

#[test]
fn a_gesture_holds_then_releases_into_a_spring_with_its_velocity() {
    let spring = Transition::new(
        TransitionProperty::Property(Property::Translate),
        0.0,
        TimingFunction::Spring(SpringConfig::default()),
    );
    let mut engine = engine_with(vec![spring]);
    let mut tracker = VelocityTracker::new();
    for n in 0..=6 {
        let t = n as f64 / 60.0;
        let x = 100.0 * t; // 100 pt/s drag
        engine.advance(t).unwrap();
        engine
            .hold(NODE, Property::Translate, Value::new(x, 0.0))
            .unwrap();
        tracker.push(t, Value::new(x, 0.0));
        assert!(engine.quiescent(), "a held value never transitions");
    }
    let release = engine.now();
    let velocity = tracker.estimate(release);
    close(velocity.x, 100.0);
    engine
        .observe(Change {
            node: NODE,
            property: Property::Translate,
            value: Value::ZERO,
            velocity: Some(velocity),
        })
        .unwrap();
    assert!(!engine.quiescent());
    // Released moving away from the target, the value keeps going out first.
    engine.advance(release + 1.0 / 240.0).unwrap();
    assert!(engine.value(NODE, Property::Translate).unwrap().x > 10.0);
    let end = engine.settle_time().unwrap();
    engine.advance(end).unwrap();
    assert_eq!(engine.value(NODE, Property::Translate), Some(Value::ZERO));
}

#[test]
fn frame_drains_in_node_order_and_only_what_changed() {
    let mut engine = Engine::new();
    engine
        .set_transitions(
            2,
            Transitions(vec![ease(TransitionProperty::All, 1.0, Easing::Linear)]),
        )
        .unwrap();
    for node in [9, 2, 5] {
        engine
            .observe(Change {
                node,
                property: Property::Opacity,
                value: Value::scalar(1.0),
                velocity: None,
            })
            .unwrap();
    }
    let first: Vec<u64> = engine.frame().into_iter().map(|p| p.node).collect();
    assert_eq!(first, [2, 5, 9]);
    engine
        .observe(Change {
            node: 2,
            property: Property::Opacity,
            value: Value::scalar(0.0),
            velocity: None,
        })
        .unwrap();
    engine.advance(0.5).unwrap();
    let second = engine.frame();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].node, 2);
    close(second[0].value.x, 0.5);
    assert!(engine.frame().is_empty());
}

#[test]
fn removing_a_node_forgets_it() {
    let mut engine = engine_with(vec![ease(TransitionProperty::All, 1.0, Easing::Linear)]);
    engine.observe(opacity(0.0)).unwrap();
    engine.remove(NODE);
    assert!(engine.quiescent());
    assert_eq!(engine.value(NODE, Property::Opacity), None);
    assert!(engine.frame().is_empty());

    // Forget every kind of slot and its queued frame, without disturbing a
    // neighbouring node in the same engine.
    for node in [NODE, NODE + 1] {
        for property in Property::ALL {
            engine
                .observe(Change {
                    node,
                    property,
                    value: property.identity(),
                    velocity: None,
                })
                .unwrap();
        }
    }
    engine.remove(NODE);
    for property in Property::ALL {
        assert_eq!(engine.value(NODE, property), None);
        assert_eq!(engine.value(NODE + 1, property), Some(property.identity()));
    }
    let frame = engine.frame();
    assert_eq!(frame.len(), Property::ALL.len());
    assert!(frame.iter().all(|value| value.node == NODE + 1));
}

#[test]
fn the_engine_refuses_bad_input_by_name() {
    let mut engine = Engine::new();
    engine.advance(1.0).unwrap();
    assert_eq!(engine.advance(0.5), Err(EngineError::ClockWentBackwards));
    assert_eq!(engine.advance(f64::NAN), Err(EngineError::NonFinite));
    assert_eq!(
        engine.observe(Change {
            node: 1,
            property: Property::Scale,
            value: Value::scalar(f64::INFINITY),
            velocity: None
        }),
        Err(EngineError::NonFinite)
    );
    assert_eq!(
        engine.set_transitions(
            1,
            Transitions(vec![Transition::new(
                TransitionProperty::All,
                0.3,
                TimingFunction::Spring(SpringConfig::default())
            )])
        ),
        Err(EngineError::Transition(
            TransitionError::SpringDeclaresDuration
        ))
    );
    assert_eq!(
        engine.set_transitions(
            1,
            Transitions(vec![ease(TransitionProperty::All, -1.0, Easing::Linear)])
        ),
        Err(EngineError::Transition(TransitionError::NegativeDuration))
    );
    let nine = (0..9)
        .map(|_| ease(TransitionProperty::All, 1.0, Easing::Linear))
        .collect();
    assert_eq!(
        engine.set_transitions(1, Transitions(nine)),
        Err(EngineError::Transition(TransitionError::TooMany))
    );
}
