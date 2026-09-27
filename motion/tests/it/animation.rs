//! Keyframe animations: the row's text, CSS's timing model, and the engine
//! under the seekable clock (LLP 1057).

use exact_motion::{
    AnimationError, Animations, Change, Direction, Easing, Engine, EngineError, FillMode,
    Keyframes, ParseError, PlayState, Property, TimingFunction, Transition, TransitionProperty,
    Transitions, Value,
};

const NODE: u64 = 9;
const BREATHE: &str =
    "@keyframes breathe{from{opacity:0.4;scale:0.9}50%{opacity:1;animation-timing-function:linear}to{opacity:0.4}}";

fn animations(shorthand: &str) -> Animations {
    Animations::parse(&format!("{shorthand} {BREATHE}")).unwrap()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn opacity(engine: &Engine) -> f64 {
    engine.value(NODE, Property::Opacity).unwrap().x
}

/// An engine that knows the node's four rows at their CSS initial values.
fn engine() -> Engine {
    let mut engine = Engine::new();
    for property in [
        Property::Translate,
        Property::Scale,
        Property::Rotate,
        Property::Opacity,
    ] {
        engine
            .observe(Change {
                node: NODE,
                property,
                value: property.identity().unwrap(),
                velocity: None,
            })
            .unwrap();
    }
    engine.frame();
    engine
}

#[test]
fn the_shorthand_takes_every_longhand_in_any_order() {
    let a = animations("infinite 250ms breathe alternate ease-in both 1s paused");
    let a = &a.0[0];
    assert_eq!(a.keyframes.name, "breathe");
    close(a.duration, 0.25);
    close(a.delay, 1.0);
    assert_eq!(a.easing, Easing::EaseIn);
    assert!(a.iterations.is_infinite());
    assert_eq!(a.direction, Direction::Alternate);
    assert_eq!(a.fill, FillMode::Both);
    assert_eq!(a.play_state, PlayState::Paused);
    // CSS's defaults: 0s, ease, 0s, 1, normal, none, running.
    let a = &animations("breathe").0[0];
    assert_eq!(
        (a.duration, a.easing.clone(), a.delay, a.iterations),
        (0.0, Easing::Ease, 0.0, 1.0)
    );
    assert_eq!(
        (a.direction, a.fill, a.play_state),
        (Direction::Normal, FillMode::None, PlayState::Running)
    );
    // The first `none` is the fill mode; only a second is the name.
    assert_eq!(animations("none breathe 1s").0[0].fill, FillMode::None);
    assert_eq!(animations("none none").0.len(), 0);
    assert_eq!(Animations::parse("none").unwrap(), Animations::NONE);
    assert_eq!(animations("breathe 1s, breathe 2s 1").0.len(), 2);
}

#[test]
fn keyframes_sort_merge_and_name_their_blocks() {
    let k = Keyframes::parse(
        "@keyframes k{to{opacity:1}0%,50%{opacity:0;translate:4px}50%{translate:1px 2px;rotate:90deg}}",
    )
    .unwrap();
    let offsets: Vec<f64> = k.blocks.iter().map(|b| b.offset).collect();
    assert_eq!(offsets, [0.0, 0.5, 1.0]);
    assert_eq!(
        k.blocks[1].values,
        [
            (Property::Opacity, Value::scalar(0.0)),
            (Property::Translate, Value::new(1.0, 2.0)),
            (Property::Rotate, Value::scalar(90.0)),
        ]
    );
    assert_eq!(
        k.blocks[0].values[1],
        (Property::Translate, Value::new(4.0, 0.0))
    );
}

#[test]
fn the_row_text_reads_back_as_the_same_row() {
    for shorthand in [
        "breathe 1.6s ease-in-out infinite",
        "breathe 300ms cubic-bezier(0.4, 0, 0.2, 1) -100ms 2.5 alternate-reverse forwards",
        "breathe 1s steps(4, jump-both), breathe 2s linear(0, 0.7 20%, 1) paused",
    ] {
        let a = animations(shorthand);
        assert_eq!(Animations::parse(&a.text()).unwrap(), a, "{}", a.text());
    }
    assert_eq!(
        animations("breathe 1.6s ease-in-out infinite").text(),
        "breathe 1.6s ease-in-out 0s infinite normal none running \
         @keyframes breathe{0%{opacity:0.4;scale:0.9;}50%{opacity:1;animation-timing-function:linear;}100%{opacity:0.4;}}"
    );
}

#[test]
fn what_css_refuses_is_refused_by_name() {
    assert_eq!(
        Animations::parse("pulse 1s"),
        Err(ParseError::UnknownKeyframes("pulse".into()))
    );
    assert_eq!(
        Animations::parse(&format!("breathe 1s spring(100, 10, 1) {BREATHE}")),
        Err(ParseError::SpringInAnimation)
    );
    assert!(matches!(
        Animations::parse(&format!("breathe 1s 2s 3s {BREATHE}")),
        Err(ParseError::BadShape(_))
    ));
    assert_eq!(
        Animations::parse(&format!("breathe -1s {BREATHE}")),
        Err(ParseError::InvalidAnimation(
            AnimationError::NegativeDuration
        ))
    );
    assert_eq!(
        Keyframes::parse("@keyframes k{0%{width:1}}"),
        Err(ParseError::UnknownProperty("width".into()))
    );
    assert_eq!(
        Keyframes::parse("@keyframes k{0%{height:1}}"),
        Err(ParseError::InvalidAnimation(AnimationError::NotAnimatable(
            Property::Height
        )))
    );
    assert_eq!(
        Keyframes::parse("@keyframes k{150%{opacity:1}}"),
        Err(ParseError::InvalidAnimation(
            AnimationError::OffsetOutOfRange
        ))
    );
}

#[test]
fn the_timing_function_eases_each_interval_and_a_keyframe_may_name_its_own() {
    // `ease-in` over 0%→50%, the 50% keyframe's `linear` over 50%→100%.
    let a = &animations("breathe 2s ease-in").0[0];
    let under = Value::scalar(1.0);
    let at = |p| a.value(Property::Opacity, p, under, false).unwrap().x;
    close(at(0.0), 0.4);
    close(at(0.25), 0.4 + 0.6 * Easing::EaseIn.progress(0.5));
    close(at(0.5), 1.0);
    close(at(0.75), 0.7);
    close(at(1.0), 0.4);
    // `scale` is set only at 0%: 100% is the underlying value.
    close(
        a.value(Property::Scale, 0.5, under, false).unwrap().x,
        0.9 + 0.1 * Easing::EaseIn.progress(0.5),
    );
    close(
        a.value(Property::Scale, 1.0, Value::scalar(2.0), false)
            .unwrap()
            .x,
        2.0,
    );
    // A property no keyframe sets is not the animation's.
    assert_eq!(a.value(Property::Rotate, 0.5, under, false), None);
}

#[test]
fn delay_fill_count_and_direction_follow_the_web_animations_timing_model() {
    let progress = |shorthand: &str, local: f64| animations(shorthand).0[0].progress(local);
    // Delay: nothing applies before it unless filling backwards.
    assert_eq!(progress("breathe 1s 1s", 0.5), None);
    assert_eq!(progress("breathe 1s 1s backwards", 0.5), Some(0.0));
    // After the end: nothing, unless filling forwards, where the last
    // iteration's end holds.
    assert_eq!(progress("breathe 1s", 1.0), None);
    assert_eq!(progress("breathe 1s forwards", 5.0), Some(1.0));
    assert_eq!(progress("breathe 1s 2.5 forwards", 9.0), Some(0.5));
    // Alternate plays odd iterations backwards; reverse all of them.
    assert_eq!(progress("breathe 1s 3 alternate", 1.25), Some(0.75));
    assert_eq!(progress("breathe 1s 3 alternate-reverse", 1.25), Some(0.25));
    assert_eq!(progress("breathe 1s reverse", 0.25), Some(0.75));
    // A negative delay starts partway through.
    assert_eq!(progress("breathe 1s -0.25s", 0.0), Some(0.25));
    // Endless: never after.
    assert_eq!(progress("breathe 1s infinite", 1000.5), Some(0.5));
}

#[test]
fn an_animation_plays_over_the_row_and_the_clock_is_a_seek() {
    let mut stepped = engine();
    let mut jumped = engine();
    for e in [&mut stepped, &mut jumped] {
        e.set_animations(NODE, animations("breathe 2s linear infinite"))
            .unwrap();
    }
    close(opacity(&stepped), 0.4);
    for n in 1..=20 {
        stepped.advance(n as f64 * 0.125).unwrap();
    }
    jumped.advance(2.5).unwrap();
    close(opacity(&stepped), 0.7);
    assert_eq!(opacity(&stepped).to_bits(), opacity(&jumped).to_bits());
    // The frame carries the animated properties, never an untouched one.
    let frame = jumped.frame();
    assert!(frame
        .iter()
        .any(|p| p.property == Property::Opacity && p.value.x == opacity(&stepped)));
    assert!(frame.iter().all(|p| p.property != Property::Rotate));
    // Endless: the host keeps its frames running, but settling never waits.
    assert!(!jumped.quiescent());
    assert_eq!(jumped.settle_time(), None);
}

#[test]
fn an_unchanged_row_never_restarts_and_a_new_name_does() {
    let mut e = engine();
    e.set_animations(NODE, animations("breathe 2s linear infinite"))
        .unwrap();
    e.advance(0.5).unwrap();
    close(opacity(&e), 0.7);
    // A re-render with the same value: nothing restarts, nothing is dirty.
    e.frame();
    e.set_animations(NODE, animations("breathe 2s linear infinite"))
        .unwrap();
    assert!(e.frame().is_empty());
    close(opacity(&e), 0.7);
    // CSS: a new duration on the same keyframes applies as if it always had
    // it, from the original start.
    e.set_animations(NODE, animations("breathe 4s linear infinite"))
        .unwrap();
    close(opacity(&e), 0.4 + 0.6 * 0.25);
    // Other keyframes are another animation: it starts now.
    let other = Animations::parse("pulse 1s linear @keyframes pulse{from{opacity:0}to{opacity:1}}")
        .unwrap();
    e.set_animations(NODE, other).unwrap();
    close(opacity(&e), 0.0);
    e.advance(1.0).unwrap();
    close(opacity(&e), 0.5);
}

#[test]
fn removing_an_animation_returns_the_row_to_its_own_value() {
    let mut e = engine();
    e.set_animations(NODE, animations("breathe 2s linear infinite"))
        .unwrap();
    e.advance(0.5).unwrap();
    e.frame();
    e.set_animations(NODE, Animations::NONE).unwrap();
    let frame = e.frame();
    assert!(frame
        .iter()
        .any(|p| p.property == Property::Opacity && p.value == Value::scalar(1.0)));
    close(opacity(&e), 1.0);
    assert!(e.quiescent());
}

#[test]
fn a_finite_animation_settles_and_fill_decides_what_remains() {
    let mut e = engine();
    e.advance(1.0).unwrap();
    e.set_animations(NODE, animations("breathe 1s linear 0.5s 2"))
        .unwrap();
    assert_eq!(e.settle_time(), Some(3.5));
    // In the delay without backwards fill: the row's own value.
    close(opacity(&e), 1.0);
    e.advance(e.settle_time().unwrap()).unwrap();
    assert!(e.quiescent());
    close(opacity(&e), 1.0);
    // Forwards fill keeps the final keyframe once it ends.
    let mut e = engine();
    e.set_animations(NODE, animations("breathe 1s linear forwards"))
        .unwrap();
    e.advance(5.0).unwrap();
    assert!(e.quiescent());
    close(opacity(&e), 0.4);
}

#[test]
fn a_paused_animation_holds_and_resumes_where_it_stopped() {
    let mut e = engine();
    e.set_animations(NODE, animations("breathe 2s linear infinite"))
        .unwrap();
    e.advance(0.5).unwrap();
    e.set_animations(NODE, animations("breathe 2s linear infinite paused"))
        .unwrap();
    assert!(e.quiescent());
    e.advance(10.0).unwrap();
    close(opacity(&e), 0.7);
    e.set_animations(NODE, animations("breathe 2s linear infinite running"))
        .unwrap();
    e.advance(10.5).unwrap();
    close(opacity(&e), 1.0);
}

#[test]
fn an_animation_wins_over_a_transition_on_the_same_property_while_it_runs() {
    let mut e = engine();
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::All,
            1.0,
            TimingFunction::Easing(Easing::Linear),
        )]),
    )
    .unwrap();
    e.set_animations(NODE, animations("breathe 2s linear"))
        .unwrap();
    e.observe(Change {
        node: NODE,
        property: Property::Opacity,
        value: Value::scalar(0.0),
        velocity: None,
    })
    .unwrap();
    e.advance(0.5).unwrap();
    close(opacity(&e), 0.7);
    // The transition ran underneath and has ended; the animation's end
    // uncovers the row's own value.
    e.advance(2.0).unwrap();
    close(opacity(&e), 0.0);
}

#[test]
fn an_invalid_row_is_refused_and_changes_nothing() {
    let mut e = engine();
    let mut bad = animations("breathe 1s");
    bad.0[0].iterations = -1.0;
    assert_eq!(
        e.set_animations(NODE, bad),
        Err(EngineError::Animation(AnimationError::NegativeIterations))
    );
    close(opacity(&e), 1.0);
}
