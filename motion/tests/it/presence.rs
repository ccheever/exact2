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
    sized(engine, node, x, y, 100.0, 20.0);
}

fn sized(engine: &mut Engine, node: u64, x: f64, y: f64, w: f64, h: f64) {
    engine
        .observe(Change {
            node,
            property: Property::Layout,
            value: Value::four(x, y, w, h),
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
        Some(Value::four(0.0, 40.0, 100.0, 20.0))
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
fn a_box_that_grows_grows_under_its_layout_transition() {
    let mut engine = Engine::new();
    engine
        .set_layout_transition(NODE, &Transitions::parse("1s linear").unwrap())
        .unwrap();
    sized(&mut engine, NODE, 0.0, 0.0, 100.0, 40.0);
    // An accordion opens: its size moves as its place does.
    sized(&mut engine, NODE, 0.0, 0.0, 100.0, 240.0);
    engine.advance(0.25).unwrap();
    let shown = engine.value(NODE, Property::Layout).unwrap();
    close(shown.z, 100.0);
    close(shown.w, 90.0);
    close(engine.settle_time().unwrap(), 1.0);
}

#[test]
fn a_spring_layout_rests_in_points_on_every_axis() {
    let mut engine = Engine::new();
    engine
        .set_layout_transition(NODE, &Transitions::parse("spring(300, 30, 1)").unwrap())
        .unwrap();
    sized(&mut engine, NODE, 0.0, 0.0, 100.0, 40.0);
    sized(&mut engine, NODE, 0.0, 300.0, 100.0, 41.0);
    let config = exact_motion::SpringConfig {
        stiffness: 300.0,
        damping: 30.0,
        mass: 1.0,
    };
    // The longest axis sets the end: 300 points, at the same rest threshold
    // the web lowers each move with.
    close(
        engine.settle_time().unwrap(),
        config.settle_time(-300.0, 0.0),
    );
    assert!(config.settle_time(-300.0, 0.0) > config.settle_time(-1.0, 0.0));
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
    let end = engine.play_exit(NODE, entry).unwrap();
    close(end, 1.5);
    close(engine.value(NODE, Property::Opacity).unwrap().x, 1.0);
    close(engine.settle_time().unwrap(), 1.5);
    engine.advance(1.5).unwrap();
    assert!(engine.quiescent());
}

#[test]
fn an_exit_composites_over_the_animations_the_node_already_plays() {
    const SPIN: &str = "@keyframes spin{from{rotate:0deg}to{rotate:360deg}}";
    let mut engine = Engine::new();
    for (property, value) in [(Property::Rotate, 0.0), (Property::Opacity, 1.0)] {
        engine
            .observe(Change {
                node: NODE,
                property,
                value: Value::scalar(value),
                velocity: None,
            })
            .unwrap();
    }
    let spin = Animations::parse(&format!("spin 1s linear infinite {SPIN}")).unwrap();
    engine.set_animations(NODE, spin).unwrap();
    engine.advance(0.25).unwrap();
    let exit = Animations::parse(&format!("fade 1s linear {FADE}")).unwrap();
    let end = engine.play_exit(NODE, exit).unwrap();
    close(end, 1.25);
    engine.advance(0.75).unwrap();
    // The spinner keeps spinning while it fades: neither replaces the other.
    close(engine.value(NODE, Property::Rotate).unwrap().x, 270.0);
    close(engine.value(NODE, Property::Opacity).unwrap().x, 0.5);
    // Only the exit is waited for: the endless spin is not.
    close(engine.settle_time().unwrap(), 1.25);
    let endless = Animations::parse(&format!("fade 1s infinite {FADE}")).unwrap();
    assert!(engine.play_exit(NODE, endless).is_err());
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
