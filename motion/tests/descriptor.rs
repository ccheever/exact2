//! Web playback compares a fixed-size curve descriptor before lowering frames.
use exact_motion::{
    Change, Easing, Engine, HoldEnd, Property, SpringConfig, SpringDescriptor, TimingFunction,
    Transition, TransitionProperty, Transitions, Value,
};

const NODE: u64 = 1;
const PROP: Property = Property::Translate;

fn setup() -> Engine {
    let mut e = Engine::new();
    e.set_transitions(NODE, declaration(SpringConfig::default()))
        .unwrap();
    target(&mut e, Value::ZERO);
    e
}

fn declaration(config: SpringConfig) -> Transitions {
    let mut t = Transition::new(TransitionProperty::All, 0.0, TimingFunction::Spring(config));
    t.delay = 0.2;
    Transitions(vec![t])
}

fn target(e: &mut Engine, value: Value) {
    e.observe(Change {
        node: NODE,
        property: PROP,
        value,
        velocity: None,
    })
    .unwrap();
}

#[test]
fn descriptor_is_copyable_and_unchanged_by_seeks_or_unrelated_holds() {
    fn copyable<T: Copy>(value: T) -> T {
        value
    }
    let mut e = setup();
    target(&mut e, Value::new(100.0, 20.0));
    let expected = copyable(e.spring_descriptor(NODE, PROP).unwrap());
    assert_eq!(
        expected,
        SpringDescriptor {
            start: 0.2,
            from: Value::ZERO,
            target: Value::new(100.0, 20.0),
            velocity: Value::ZERO,
            config: SpringConfig::default(),
        }
    );
    e.observe(Change {
        node: NODE,
        property: Property::Opacity,
        value: Value::scalar(1.0),
        velocity: None,
    })
    .unwrap();
    let hold = e
        .begin_hold(NODE, Property::Opacity, 0.0, None)
        .unwrap()
        .unwrap();
    for n in 1..=60 {
        e.update_hold(hold.token, n as f64 / 120.0, Value::scalar(0.5))
            .unwrap();
        assert_eq!(e.spring_descriptor(NODE, PROP), Some(expected));
    }
    let frames = e.spring_frames(NODE, PROP).unwrap();
    assert_eq!(frames.start, expected.start);
    assert_eq!(frames.values[0], expected.from);
    assert_eq!(*frames.values.last().unwrap(), expected.target);
}

#[test]
fn same_clock_same_target_releases_distinguish_velocity_and_config() {
    let mut e = setup();
    let release = |e: &mut Engine, velocity| {
        let hold = e
            .begin_hold(NODE, PROP, 0.0, Some(Value::ZERO))
            .unwrap()
            .unwrap();
        assert!(e.spring_descriptor(NODE, PROP).is_none());
        e.end_hold(
            hold.token,
            0.0,
            HoldEnd::Release {
                velocity: Value::new(velocity, 0.0),
            },
        )
        .unwrap();
        e.spring_descriptor(NODE, PROP).unwrap()
    };
    let first = release(&mut e, 100.0);
    let second = release(&mut e, 200.0);
    assert_eq!(
        (first.start, first.target, first.from),
        (second.start, second.target, second.from)
    );
    assert_ne!(first, second);
    assert_ne!(first.velocity, second.velocity);
    e.set_transitions(
        NODE,
        declaration(SpringConfig {
            stiffness: 200.0,
            ..SpringConfig::default()
        }),
    )
    .unwrap();
    assert_eq!(
        e.spring_descriptor(NODE, PROP),
        Some(second),
        "a declaration does not rewrite a running curve"
    );
    let third = release(&mut e, 200.0);
    assert_ne!(second, third);
    assert_ne!(second.config, third.config);
}

#[test]
fn descriptor_absent_for_unknown_held_settled_removed_and_easing_properties() {
    let mut e = setup();
    assert!(e.spring_descriptor(999, PROP).is_none());
    assert!(e.spring_descriptor(NODE, PROP).is_none());
    target(&mut e, Value::new(100.0, 20.0));
    assert!(e.spring_descriptor(NODE, PROP).is_some());
    let hold = e.begin_hold(NODE, PROP, 0.0, None).unwrap().unwrap();
    assert!(e.spring_descriptor(NODE, PROP).is_none());
    e.end_hold(hold.token, 0.0, HoldEnd::Cancel).unwrap();
    e.advance(e.settle_time().unwrap()).unwrap();
    assert!(e.spring_descriptor(NODE, PROP).is_none());
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::All,
            1.0,
            TimingFunction::Easing(Easing::Linear),
        )]),
    )
    .unwrap();
    target(&mut e, Value::ZERO);
    assert!(!e.quiescent());
    assert!(e.spring_descriptor(NODE, PROP).is_none());
    e.remove(NODE);
    assert!(e.spring_descriptor(NODE, PROP).is_none());
}
