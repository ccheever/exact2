//! What moves decides the rate a display runs it at (LLP 1061 D4): a change
//! of place or size wants the panel's full rate; a fade does not.

use crate::keyframed;
use exact_motion::{Animations, Change, Engine, Property, Transitions, Value};

fn engine() -> Engine {
    let mut engine = Engine::new();
    for (node, property) in [(1, Property::Opacity), (2, Property::Translate)] {
        let value = property.identity().unwrap();
        let change = Change {
            node,
            property,
            value,
            velocity: None,
        };
        engine.observe(change).unwrap();
    }
    engine.frame();
    engine
}

#[test]
fn a_breathing_fade_is_motion_but_not_spatial() {
    let mut e = engine();
    let breathe = "breathe 4.2s ease-in-out infinite @keyframes breathe{from{opacity:0.6}50%{opacity:1}to{opacity:0.6}}";
    e.set_animations(1, &keyframed(breathe).unwrap()).unwrap();
    e.advance(1.0).unwrap();
    assert!(!e.quiescent(), "an endless loop keeps frames coming");
    assert!(!e.spatial(), "but a fade needs no more than 60 Hz");
    // A float of the same length moves the box: the panel's full rate.
    let float = "float 4.2s ease-in-out infinite @keyframes float{from{translate:0px 0px}to{translate:0px 8px}}";
    e.set_animations(2, &keyframed(float).unwrap()).unwrap();
    assert!(e.spatial());
    e.set_animations(2, &Animations::NONE).unwrap();
    assert!(!e.spatial());
}

#[test]
fn a_transition_is_spatial_by_what_it_moves() {
    let mut e = engine();
    for node in [1, 2] {
        e.set_transitions(node, Transitions::parse("all 300ms ease").unwrap())
            .unwrap();
    }
    let change = |node, property, value| Change {
        node,
        property,
        value,
        velocity: None,
    };
    e.observe(change(1, Property::Opacity, Value::scalar(0.5)))
        .unwrap();
    assert!(!e.quiescent() && !e.spatial(), "a fade");
    e.observe(change(2, Property::Translate, Value::new(0.0, 40.0)))
        .unwrap();
    assert!(e.spatial(), "a slide");
    e.advance(1.0).unwrap();
    assert!(e.quiescent() && !e.spatial(), "both settled");
}
