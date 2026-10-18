//! A transition a host plays (`Engine::play_transition`): the engine keeps
//! it for a settle, an unrelated change and a removal until it ends, and
//! gives it up when a change, a hold or a new path takes it.
use exact_motion::{
    Animations, Change, Easing, Engine, Keyframes, PathCommand, PathValue, Property,
    TimingFunction, Transition, TransitionProperty, Transitions, Value,
};

const NODE: u64 = (5 << 32) | 9;
const P: Property = Property::Opacity;

fn observe(e: &mut Engine, x: f64) {
    observe_value(e, P, Value::scalar(x));
}

fn observe_value(e: &mut Engine, property: Property, value: Value) {
    e.observe(Change {
        node: NODE,
        property,
        value,
        velocity: None,
    })
    .unwrap();
}

fn linear(e: &mut Engine) {
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::All,
            1.0,
            TimingFunction::Easing(Easing::Linear),
        )]),
    )
    .unwrap();
}

/// A one-second linear fade from 1 to 0, played from the start.
fn fading() -> Engine {
    let mut e = Engine::new();
    linear(&mut e);
    observe(&mut e, 1.0);
    e.frame();
    observe(&mut e, 0.0);
    assert!(e.play_transition(NODE, P).is_some());
    e.frame();
    e
}

#[test]
fn a_played_transition_settles_at_its_end() {
    let e = fading();
    assert!(e.quiescent(), "nothing for the display to drive");
    assert_eq!(e.settle_time(), Some(1.0), "an agent's settle waits for it");
}

#[test]
fn an_unchanged_target_keeps_it_played_until_it_ends() {
    let mut e = fading();
    e.advance(0.5).unwrap();
    // A commit that changes the node restates its rows
    observe(&mut e, 0.0);
    assert!(e.quiescent());
    assert_eq!(e.settle_time(), Some(1.0), "still played");
    e.advance(1.0).unwrap();
    assert!(!e.is_active(NODE, P), "ended, as played ones end");
    assert_eq!(e.settle_time(), None);
}

#[test]
fn a_removal_forgets_it() {
    let mut e = fading();
    e.remove(NODE);
    assert!(e.at_rest(NODE));
    assert_eq!(e.settle_time(), None);
    // Its property alone, retired and taken again settled
    let mut e = fading();
    assert!(e.remove_property(NODE, P));
    assert!(!e.is_played(NODE, P));
    e.set_transitions(NODE, Transitions(vec![])).unwrap();
    observe(&mut e, 1.0);
    assert!(e.at_rest(NODE));
}

#[test]
fn a_hold_takes_it_from_where_it_stands() {
    let mut e = fading();
    let held = e.begin_hold(NODE, P, 0.25, None).unwrap().unwrap();
    assert_eq!(
        held.value,
        Value::scalar(0.75),
        "not the target it presented"
    );
    assert!(!e.is_played(NODE, P));
    // A transform hold takes a played translate the same way
    let mut e = Engine::new();
    linear(&mut e);
    observe_value(&mut e, Property::Translate, Value::ZERO);
    observe_value(&mut e, Property::Scale, Value::scalar(1.0));
    e.frame();
    observe_value(&mut e, Property::Translate, Value::new(100.0, 0.0));
    assert!(e.play_transition(NODE, Property::Translate).is_some());
    let held = e.begin_transform_hold(NODE, 0.25, None).unwrap().unwrap();
    assert_eq!(held.translate().value, Value::new(25.0, 0.0));
    assert!(!e.is_played(NODE, Property::Translate));
}

#[test]
fn an_unchanged_colour_target_keeps_a_transition_between_colour_spaces() {
    let red = exact_motion::color::parse("#ff0000").unwrap();
    let mut e = Engine::new();
    linear(&mut e);
    observe_value(
        &mut e,
        Property::BackgroundColor,
        Value::oklab(0.5, 0.1, 0.1, 1.0),
    );
    e.frame();
    observe_value(&mut e, Property::BackgroundColor, red);
    e.advance(0.5).unwrap();
    let halfway = e.value(NODE, Property::BackgroundColor);
    // It runs in Oklab; the commit restates the target in sRGB
    observe_value(&mut e, Property::BackgroundColor, red);
    assert_eq!(e.value(NODE, Property::BackgroundColor), halfway);
    assert_eq!(e.settle_time(), Some(1.0), "not started again");
}

#[test]
fn its_end_shows_the_animation_it_covered() {
    let fade = Keyframes::parse("from{opacity:0}to{opacity:1}").unwrap();
    let mut row = Animations::parse("fade 2s linear -1s paused").unwrap();
    row.resolve(|name| (name == "fade").then_some(&fade));
    let mut e = fading();
    e.set_animations(NODE, &row).unwrap();
    e.frame();
    e.advance(1.0).unwrap();
    let shown = e
        .frame()
        .into_iter()
        .find(|p| p.node == NODE && p.property == P)
        .map(|p| p.value.x);
    assert_eq!(shown, Some(0.5), "the paused animation, a second in");
}

#[test]
fn a_new_path_takes_its_curve_back() {
    let path = |y: f64| {
        PathValue(vec![
            PathCommand {
                verb: b'M',
                args: [6.0, y, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
            PathCommand {
                verb: b'L',
                args: [18.0, 24.0 - y, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
        ])
    };
    let line = PathValue(vec![PathCommand {
        verb: b'M',
        args: [0.0; 7],
    }]);
    for next in [path(9.0), line] {
        let mut e = Engine::new();
        e.set_transitions(NODE, Transitions::parse("d 400ms linear").unwrap())
            .unwrap();
        e.observe_path(NODE, Some(path(9.0)));
        e.observe_path(NODE, Some(path(15.0)));
        assert!(e.play_transition(NODE, Property::D).is_some());
        assert!(e.played_path(NODE).is_some());
        // Turned back (the engine samples it), or a path it cannot move to
        e.observe_path(NODE, Some(next));
        assert!(e.played_path(NODE).is_none());
        assert!(!e.is_played(NODE, Property::D));
    }
}
