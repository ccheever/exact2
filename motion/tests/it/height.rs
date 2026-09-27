//! Numeric height uses the same ownership and curves as compositor properties.
use exact_motion::{
    Change, Easing, Engine, EngineError, HoldEnd, Property, SpringConfig, TimingFunction,
    Transition, TransitionProperty, Transitions, Value,
};

const NODE: u64 = 7;
const HEIGHT: Property = Property::Height;

fn target(e: &mut Engine, property: Property, x: f64) {
    e.observe(Change {
        node: NODE,
        property,
        value: Value::scalar(x),
        velocity: None,
    })
    .unwrap();
}

fn spring(delay: f64) -> Transitions {
    Transitions(vec![Transition {
        property: TransitionProperty::All,
        duration: 0.0,
        delay,
        timing: TimingFunction::Spring(SpringConfig::default()),
    }])
}

#[test]
fn height_appends_wire_value_and_has_no_numeric_css_initial() {
    assert_eq!(Property::ALL.len(), 7);
    for (wire, p) in Property::ALL.into_iter().enumerate() {
        assert_eq!(Property::from_wire(wire as u8), Some(p));
        assert_eq!(Property::from_name(p.name()), Some(p));
    }
    assert_eq!(HEIGHT as u8, 4);
    assert_eq!(HEIGHT.name(), "height");
    assert_eq!(HEIGHT.components(), 1);
    assert_eq!(HEIGHT.identity(), None);
    assert_eq!(Property::Translate.identity(), Some(Value::ZERO));
    assert_eq!(Property::Scale.identity(), Some(Value::scalar(1.0)));
    assert_eq!(Property::Rotate.identity(), Some(Value::ZERO));
    assert_eq!(Property::Opacity.identity(), Some(Value::scalar(1.0)));
    assert_eq!(Property::from_wire(7), None);
    let parsed = Transitions::parse("height 200ms linear 50ms").unwrap();
    assert_eq!(parsed.0[0].property, TransitionProperty::Property(HEIGHT));
    let mut e = Engine::new();
    assert_eq!(e.value(NODE, HEIGHT), None);
    assert!(!e.is_active(NODE, HEIGHT));
    assert!(e
        .begin_hold(NODE, HEIGHT, f64::NAN, None)
        .unwrap()
        .is_none());
    assert_eq!(e.now(), 0.0);
}

#[test]
fn retirement_is_property_scoped_and_stale_before_clock_or_validation() {
    let mut e = Engine::new();
    e.set_transitions(NODE, spring(0.0)).unwrap();
    target(&mut e, HEIGHT, 180.0);
    target(&mut e, Property::Translate, 0.0);
    target(&mut e, Property::Translate, 90.0);
    let h = e.begin_hold(NODE, HEIGHT, 0.1, None).unwrap().unwrap();
    e.update_hold(h.token, 0.2, Value::scalar(210.0)).unwrap();
    let translate = e.value(NODE, Property::Translate);
    let curve = e.spring_descriptor(NODE, Property::Translate);
    assert!(e.remove_property(NODE, HEIGHT));
    assert!(!e.remove_property(NODE, HEIGHT));
    assert_eq!(e.now(), 0.2);
    assert_eq!(e.value(NODE, HEIGHT), None);
    assert_eq!(e.target(NODE, HEIGHT), None);
    assert!(!e.has_hold(h.token));
    assert!(!e.is_active(NODE, HEIGHT));
    assert_eq!(e.value(NODE, Property::Translate), translate);
    assert_eq!(e.spring_descriptor(NODE, Property::Translate), curve);
    assert!(e.frame().iter().all(|p| p.property != HEIGHT));
    assert!(!e
        .update_hold(h.token, f64::NAN, Value::new(f64::NAN, 1.0))
        .unwrap());
    assert!(!e
        .end_hold(
            h.token,
            -1.0,
            HoldEnd::Release {
                velocity: Value::new(f64::NAN, 2.0)
            }
        )
        .unwrap());
    assert_eq!(e.now(), 0.2);
    target(&mut e, HEIGHT, 300.0);
    assert!(
        !e.is_active(NODE, HEIGHT),
        "readoption has no before-change value"
    );
    let new = e.begin_hold(NODE, HEIGHT, 0.2, None).unwrap().unwrap();
    assert_ne!(new.token, h.token);
    e.update_hold(new.token, 0.2, Value::scalar(200.0)).unwrap();
    e.end_hold(new.token, 0.2, HoldEnd::Cancel).unwrap();
    assert!(
        e.spring_descriptor(NODE, HEIGHT).is_some(),
        "declaration survives property retirement"
    );
}

#[test]
fn height_catch_latest_target_delay_and_velocity_keep_existing_hold_semantics() {
    let mut e = Engine::new();
    e.set_transitions(NODE, spring(0.0)).unwrap();
    target(&mut e, HEIGHT, 640.0);
    // A host catches the actual CSS height after a max-height constraint.
    let h = e
        .begin_hold(NODE, HEIGHT, 0.0, Some(Value::scalar(400.0)))
        .unwrap()
        .unwrap();
    assert_eq!(h.value, Value::scalar(400.0));
    assert_eq!(e.target(NODE, HEIGHT), Some(Value::scalar(640.0)));
    assert!(e.is_active(NODE, HEIGHT));
    assert!(e.is_held(NODE, HEIGHT));
    assert!(e.quiescent(), "holds do not request ticks");
    e.frame();
    target(&mut e, HEIGHT, 500.0);
    target(&mut e, HEIGHT, 400.0);
    e.set_transitions(NODE, spring(0.25)).unwrap();
    assert!(e.frame().is_empty());
    e.end_hold(
        h.token,
        0.0,
        HoldEnd::Release {
            velocity: Value::scalar(-200.0),
        },
    )
    .unwrap();
    assert!(!e.is_held(NODE, HEIGHT));
    assert!(
        e.is_active(NODE, HEIGHT),
        "equal target with velocity is still active"
    );
    e.advance(0.1).unwrap();
    assert_eq!(e.value(NODE, HEIGHT), e.target(NODE, HEIGHT));
    assert!(
        e.is_active(NODE, HEIGHT),
        "delay is active despite equality"
    );
    e.advance(0.26).unwrap();
    assert!(e.value(NODE, HEIGHT).unwrap().x < 400.0);
    e.advance(e.settle_time().unwrap()).unwrap();
    assert_eq!(e.value(NODE, HEIGHT), Some(Value::scalar(400.0)));
    assert!(!e.is_active(NODE, HEIGHT));
}

#[test]
fn height_invalid_shape_does_not_replace_live_hold_and_easing_is_active() {
    let mut e = Engine::new();
    target(&mut e, HEIGHT, 200.0);
    let old = e.begin_hold(NODE, HEIGHT, 0.0, None).unwrap().unwrap();
    assert_eq!(
        e.begin_hold(NODE, HEIGHT, 1.0, Some(Value::new(220.0, 1.0))),
        Err(EngineError::InvalidValueShape)
    );
    assert_eq!(
        e.end_hold(
            old.token,
            1.0,
            HoldEnd::Release {
                velocity: Value::new(0.0, 1.0)
            }
        ),
        Err(EngineError::InvalidValueShape)
    );
    assert!(e.has_hold(old.token));
    assert_eq!(e.now(), 0.0);
    e.update_hold(old.token, 0.0, Value::scalar(100.0)).unwrap();
    let new = e.begin_hold(NODE, HEIGHT, 0.0, None).unwrap().unwrap();
    assert_eq!(new.value, Value::scalar(100.0));
    assert!(!e.has_hold(old.token));
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::Property(HEIGHT),
            1.0,
            TimingFunction::Easing(Easing::Linear),
        )]),
    )
    .unwrap();
    e.end_hold(new.token, 0.0, HoldEnd::Cancel).unwrap();
    assert!(e.is_active(NODE, HEIGHT));
    assert!(e.spring_descriptor(NODE, HEIGHT).is_none());
    e.advance(0.5).unwrap();
    assert_eq!(e.value(NODE, HEIGHT), Some(Value::scalar(150.0)));
    e.advance(1.0).unwrap();
    assert!(!e.is_active(NODE, HEIGHT));
}
