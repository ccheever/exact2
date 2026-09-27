//! Exit animations and layout transitions in the engine (LLP 1063).

use exact_motion::{AnimationError, Animations, Change, Engine, Property, Transitions, Value};

const NODE: u64 = 3;
const FADE: &str = "@keyframes fade{from{opacity:1}to{opacity:0}}";

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn at(engine: &mut Engine, node: u64, x: f64, y: f64) {
    engine
        .observe(Change {
            node,
            property: Property::Layout,
            value: Value::new(x, y),
            velocity: None,
        })
        .unwrap();
}

#[test]
fn a_layout_change_is_first_seen_then_moves_under_its_own_row_only() {
    let mut engine = Engine::new();
    engine
        .set_transitions(NODE, Transitions::parse("all 1s linear").unwrap())
        .unwrap();
    at(&mut engine, NODE, 0.0, 100.0);
    // `transition: all` is not a layout transition: the box jumps.
    at(&mut engine, NODE, 0.0, 40.0);
    assert!(engine.quiescent());
    assert_eq!(
        engine.value(NODE, Property::Layout),
        Some(Value::new(0.0, 40.0))
    );

    engine
        .set_layout_transition(NODE, &Transitions::parse("1s linear").unwrap())
        .unwrap();
    at(&mut engine, NODE, 0.0, 0.0);
    engine.advance(0.25).unwrap();
    close(engine.value(NODE, Property::Layout).unwrap().y, 30.0);
    close(engine.settle_time().unwrap(), 1.0);
    // Interrupted, it starts from where it is, not where it was laid out.
    at(&mut engine, NODE, 0.0, 60.0);
    close(engine.value(NODE, Property::Layout).unwrap().y, 30.0);
    engine.advance(1.25).unwrap();
    close(engine.value(NODE, Property::Layout).unwrap().y, 60.0);
    assert!(engine.quiescent());
}

#[test]
fn a_declaration_naming_a_property_does_not_cover_layout() {
    let mut engine = Engine::new();
    engine
        .set_layout_transition(NODE, &Transitions::parse("opacity 1s").unwrap())
        .unwrap();
    at(&mut engine, NODE, 0.0, 0.0);
    at(&mut engine, NODE, 10.0, 0.0);
    assert!(engine.quiescent());
    assert!(Property::from_name("layout").is_none(), "never authorable");
}

#[test]
fn an_exit_restarts_even_under_the_entry_keyframes_and_reports_its_end() {
    let mut engine = Engine::new();
    engine
        .observe(Change {
            node: NODE,
            property: Property::Opacity,
            value: Value::scalar(1.0),
            velocity: None,
        })
        .unwrap();
    let entry = Animations::parse(&format!("fade 1s linear {FADE}")).unwrap();
    engine.set_animations(NODE, entry.clone()).unwrap();
    engine.advance(0.5).unwrap();
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.5);
    // The same keyframes as the exit: set_animations alone would continue.
    let end = engine.restart_animations(NODE, entry).unwrap();
    close(end, 1.5);
    close(engine.value(NODE, Property::Opacity).unwrap().x, 1.0);
    close(engine.settle_time().unwrap(), 1.5);
    engine.advance(1.5).unwrap();
    assert!(engine.quiescent());
}

#[test]
fn an_exit_must_end() {
    for endless in ["fade 1s infinite", "fade 1s paused"] {
        let a = Animations::parse(&format!("{endless} {FADE}")).unwrap();
        assert_eq!(a.validate(), Ok(()));
        assert_eq!(
            a.validate_ending(),
            Err(AnimationError::Endless),
            "{endless}"
        );
    }
    let a = Animations::parse(&format!("fade 200ms 100ms 2 {FADE}")).unwrap();
    assert_eq!(a.validate_ending(), Ok(()));
    close(a.end_time(), 0.5);
}
