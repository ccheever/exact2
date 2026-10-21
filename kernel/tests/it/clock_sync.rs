//! A commit's clock joins wait for all of its rows (LLP 1055.002 D6,
//! "Within a commit"): `MotionSync::apply` holds them, so the order a
//! commit's nodes are applied in does not pick a phase.

use exact_kernel::MotionSync;
use exact_motion::{Animations, Engine, Keyframes};

fn row(text: &str) -> Animations {
    let pulse = Keyframes::parse("from{opacity:0.4}to{opacity:1}").unwrap();
    let mut a = Animations::parse(text).unwrap();
    a.resolve(|name| (name == "pulse").then_some(&pulse));
    a
}

#[test]
fn a_commit_that_swaps_the_only_member_starts_the_clock_over() {
    let (old, new) = (3, 4);
    let mut e = Engine::new();
    let first = MotionSync {
        clocks: vec![(old, Some("Pending".into()))],
        animations: vec![(old, row("pulse 800ms infinite alternate"))],
        ..Default::default()
    };
    first.apply(&mut e).unwrap();
    e.advance(1.2).unwrap();
    // A created node's row comes before a touched node's: the new member is
    // applied while the old one still plays.
    let second = MotionSync {
        clocks: vec![(new, Some("Pending".into())), (old, Some("Pending".into()))],
        animations: vec![
            (new, row("pulse 800ms 1 alternate")),
            (old, Animations::NONE),
        ],
        ..Default::default()
    };
    second.apply(&mut e).unwrap();
    assert_eq!(
        e.animation_plays(new)[0].start,
        1.2,
        "not 0.0, ended at once"
    );
}

/// A renewed node (LLP 1078) is forgotten and heard again as new. One the
/// engine holds only settled values for, restated with nothing that moves,
/// is left in place: its values end the same and only those that changed are
/// presented. One restated with a transition is forgotten, so no transition
/// runs from the values it had.
#[test]
fn a_bare_renewed_node_at_rest_is_not_forgotten_and_one_with_a_transition_is() {
    use exact_motion::{
        Change, Easing, Property, TimingFunction, Transition, TransitionProperty, Transitions,
        Value,
    };
    let change = |node: u64, property: Property, value: f64| Change {
        node,
        property,
        value: Value::scalar(value),
        velocity: None,
    };
    let restated = |node: u64, opacity: f64, row: Transitions| MotionSync {
        removed: vec![node],
        renewed: vec![node],
        transitions: vec![(node, row)],
        changes: vec![
            change(node, Property::Scale, 1.0),
            change(node, Property::Opacity, opacity),
        ],
        ..MotionSync::default()
    };
    let mut engine = Engine::new();
    for node in [1, 2] {
        restated(node, 1.0, Transitions::NONE)
            .apply(&mut engine)
            .unwrap();
    }
    assert_eq!(engine.frame().len(), 4, "each new node's values, once");

    // Bare, at rest: observed in place.
    restated(1, 0.5, Transitions::NONE)
        .apply(&mut engine)
        .unwrap();
    let presented: Vec<_> = engine
        .frame()
        .into_iter()
        .map(|p| (p.node, p.property))
        .collect();
    assert_eq!(
        presented,
        [(1, Property::Opacity)],
        "what changed, not all it holds"
    );
    assert_eq!(engine.value(1, Property::Opacity), Some(Value::scalar(0.5)));

    // Restated with a transition: forgotten first, so the new value is where
    // it starts, not where a transition from the old one ends.
    let fade = Transitions(vec![Transition::new(
        TransitionProperty::All,
        0.3,
        TimingFunction::Easing(Easing::Linear),
    )]);
    restated(2, 0.25, fade.clone()).apply(&mut engine).unwrap();
    assert_eq!(
        engine.value(2, Property::Opacity),
        Some(Value::scalar(0.25))
    );
    assert_eq!(engine.frame().len(), 2, "heard as new: every value");
    // And a node that has a transition row is not at rest: renewed bare, it
    // is forgotten too, and nothing runs from 0.25.
    restated(2, 1.0, Transitions::NONE)
        .apply(&mut engine)
        .unwrap();
    assert_eq!(engine.value(2, Property::Opacity), Some(Value::scalar(1.0)));
    engine.advance(0.1).unwrap();
    assert_eq!(engine.value(2, Property::Opacity), Some(Value::scalar(1.0)));
}
