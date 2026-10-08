//! Motion starts at the first frame that shows it (LLP 1003.001): with the
//! rule on, a curve an author's commit begins waits at its start until the
//! next presented frame, which starts it there; inputs keep their own clock.

use exact_motion::{
    Change, Easing, Engine, HoldEnd, Property, SpringConfig, TimingFunction, Transition,
    TransitionProperty, Transitions, Value,
};

const NODE: u64 = 7;

fn engine(seconds: f64, timing: TimingFunction) -> Engine {
    let mut e = Engine::new();
    e.set_start_on_frame(true, 0.0).unwrap();
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::All,
            seconds,
            timing,
        )]),
    )
    .unwrap();
    observe(&mut e, 0.0);
    e
}

fn linear() -> TimingFunction {
    TimingFunction::Easing(Easing::Linear)
}

fn observe(e: &mut Engine, value: f64) {
    e.observe(Change {
        node: NODE,
        property: Property::Opacity,
        value: Value::scalar(value),
        velocity: None,
    })
    .unwrap();
}

fn opacity(e: &Engine) -> f64 {
    e.value(NODE, Property::Opacity).unwrap().x
}

#[test]
fn a_curve_begun_between_frames_starts_at_the_next_frame() {
    let mut e = engine(1.0, linear());
    e.advance(0.004).unwrap();
    observe(&mut e, 1.0);
    // The commit's own work and the wait for the frame show nothing.
    e.advance(0.012).unwrap();
    assert_eq!(opacity(&e), 0.0);
    e.present_frame(0.030).unwrap();
    assert_eq!(opacity(&e), 0.0, "the first frame shows the start");
    e.advance(0.040).unwrap();
    e.present_frame(0.050).unwrap();
    assert!((opacity(&e) - 0.020).abs() < 1e-12, "then one interval");
    assert_eq!(e.settle_time(), Some(1.030));
}

#[test]
fn a_pending_curve_is_not_moved_or_retired_by_another_commit() {
    let mut e = engine(0.005, linear());
    observe(&mut e, 1.0);
    e.advance(0.010).unwrap();
    assert!(!e.quiescent(), "a 5 ms curve still waits for its frame");
    assert_eq!(opacity(&e), 0.0);
    e.present_frame(0.016).unwrap();
    assert_eq!(opacity(&e), 0.0);
    e.present_frame(0.0185).unwrap();
    assert!((opacity(&e) - 0.5).abs() < 1e-9);
}

#[test]
fn an_interruption_before_the_frame_starts_from_where_it_stood() {
    let mut e = engine(1.0, linear());
    observe(&mut e, 1.0);
    e.advance(0.010).unwrap();
    observe(&mut e, 0.5);
    e.present_frame(0.020).unwrap();
    e.present_frame(0.520).unwrap();
    assert!(
        (opacity(&e) - 0.25).abs() < 1e-9,
        "0 → 0.5 over 1 s, half way"
    );
}

#[test]
fn a_delay_is_kept_and_a_negative_delay_keeps_its_skip() {
    for (delay, at) in [(0.1, 0.0), (-0.25, 0.25)] {
        let mut e = Engine::new();
        e.set_start_on_frame(true, 0.0).unwrap();
        let mut t = Transition::new(TransitionProperty::All, 1.0, linear());
        t.delay = delay;
        e.set_transitions(NODE, Transitions(vec![t])).unwrap();
        observe(&mut e, 0.0);
        observe(&mut e, 1.0);
        e.advance(0.010).unwrap();
        e.present_frame(0.020).unwrap();
        assert!((opacity(&e) - at).abs() < 1e-9, "delay {delay}");
        e.present_frame(0.520).unwrap();
        assert!((opacity(&e) - (at + (0.5 - delay.max(0.0)).max(0.0))).abs() < 1e-9);
    }
}

#[test]
fn input_keeps_its_own_clock_behind_the_frame() {
    let mut e = engine(1.0, linear());
    e.present_frame(1.000).unwrap();
    // A touch whose time is before the frame the display just presented.
    let held = e
        .begin_hold(NODE, Property::Opacity, 0.990, None)
        .unwrap()
        .unwrap();
    assert!(e
        .update_hold(held.token, 0.995, Value::scalar(0.3))
        .unwrap());
    assert!(e.end_hold(held.token, 0.998, HoldEnd::Cancel).unwrap());
}

#[test]
fn a_fling_keeps_its_release_instant_and_a_cancel_waits() {
    let spring = TimingFunction::Spring(SpringConfig::default());
    let release = |end: HoldEnd, on: bool| {
        let mut e = engine(0.0, spring.clone());
        e.set_start_on_frame(on, 0.0).unwrap();
        observe(&mut e, 1.0);
        e.advance(0.5).unwrap();
        if on {
            e.present_frame(0.5).unwrap();
        }
        let held = e
            .begin_hold(NODE, Property::Opacity, 0.5, None)
            .unwrap()
            .unwrap();
        e.update_hold(held.token, 0.51, Value::scalar(0.2)).unwrap();
        e.end_hold(held.token, 0.52, end).unwrap();
        e.advance(0.53).unwrap();
        if on {
            e.present_frame(0.54).unwrap();
        } else {
            e.advance(0.54).unwrap();
        }
        opacity(&e)
    };
    let fling = HoldEnd::Release {
        velocity: Value::scalar(3.0),
    };
    assert_eq!(
        release(fling, true),
        release(fling, false),
        "v·Δt from the release"
    );
    let cancel = release(HoldEnd::Cancel, true);
    assert!(
        (cancel - 0.2).abs() < 1e-12,
        "a cancel's first frame is its start: {cancel}"
    );
}

#[test]
fn an_exit_ends_its_own_plays_after_the_frame_starts_them() {
    let mut e = engine(1.0, linear());
    let exit = crate::keyframed("fade 300ms @keyframes fade{to{opacity:0}}").unwrap();
    observe(&mut e, 1.0);
    e.advance(0.010).unwrap();
    e.play_exit(NODE, &exit).unwrap();
    assert_eq!(e.exit_end(NODE, 1), None, "no destroy while it waits");
    e.present_frame(0.020).unwrap();
    assert_eq!(e.exit_end(NODE, 1), Some(0.320));
    assert_eq!(e.curve_start(NODE, Property::Opacity), Some(0.020));
}

#[test]
fn a_lowered_play_keeps_the_clock_until_its_frame() {
    let mut e = Engine::new();
    e.set_start_on_frame(true, 0.0).unwrap();
    e.set_lowered(true);
    let spin = crate::keyframed("spin 1s @keyframes spin{to{opacity:0}}").unwrap();
    e.set_animations(NODE, &spin).unwrap();
    e.advance(0.010).unwrap();
    assert!(!e.quiescent(), "the display link runs to start it");
    assert_eq!(e.animation_plays(NODE)[0].local(0.010), 0.0);
    assert_eq!(e.present_frame(0.020).unwrap(), vec![NODE]);
    assert_eq!(e.animation_plays(NODE)[0].start, 0.020);
    assert!(e.quiescent(), "Core Animation plays it from here");
}

#[test]
fn joins_to_an_idle_clock_before_the_frame_move_with_its_origin() {
    let row = || {
        crate::keyframed(
            "pulse 800ms infinite alternate @keyframes pulse{from{opacity:0.4}to{opacity:1}}",
        )
        .unwrap()
    };
    let mut e = Engine::new();
    e.set_start_on_frame(true, 0.0).unwrap();
    for node in [3, 4] {
        e.set_animation_clock(node, Some("Pulse"));
    }
    e.advance(0.010).unwrap();
    e.set_animations(3, &row()).unwrap();
    e.advance(0.015).unwrap();
    e.set_animations(4, &row()).unwrap();
    assert!(!e.quiescent());
    e.present_frame(0.030).unwrap();
    let starts: Vec<f64> = [3, 4].map(|n| e.animation_plays(n)[0].start).to_vec();
    assert_eq!(starts, vec![0.030, 0.030], "one phase, from the frame");
}

#[test]
fn turning_the_rule_off_starts_what_waits_there() {
    let mut e = engine(1.0, linear());
    observe(&mut e, 1.0);
    e.advance(0.010).unwrap();
    e.set_start_on_frame(false, 0.025).unwrap();
    e.advance(0.525).unwrap();
    assert!((opacity(&e) - 0.5).abs() < 1e-9);
}
