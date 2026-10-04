//! Clock timelines (LLP 1055.002): animations on one share a phase, a lone
//! one starts at its first keyframe, a late joiner's start is cut short and
//! never its end, and a resume rejoins.

use exact_motion::{Animations, Engine, Keyframes, NamedTimeline};

const LOCK: u64 = 3;
const ENGINE: u64 = 4;

fn row(text: &str) -> Animations {
    let pulse = Keyframes::parse("from{opacity:0.4}to{opacity:1}").unwrap();
    let mut a = Animations::parse(text).unwrap();
    a.resolve(|name| (name == "pulse").then_some(&pulse));
    a
}

fn start(e: &Engine, node: u64) -> f64 {
    e.animation_plays(node)[0].start
}

fn opacity(e: &Engine, node: u64) -> f64 {
    let play = &e.animation_plays(node)[0];
    let local = play.local(e.now());
    play.animation
        .sample_in(
            local,
            exact_motion::Property::Opacity,
            exact_motion::Value::scalar(1.0),
            false,
        )
        .unwrap()
        .x
}

#[test]
fn a_lone_animation_starts_now_and_a_joiner_takes_its_phase() {
    let mut e = Engine::new();
    e.advance(10.0).unwrap();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Pending"));
    }
    e.set_animations(LOCK, &row("pulse 800ms ease-in-out infinite alternate"))
        .unwrap();
    assert_eq!(start(&e, LOCK), 10.0, "idle: the origin is now");
    e.advance(10.3).unwrap();
    e.set_animations(ENGINE, &row("pulse 800ms ease-in-out infinite alternate"))
        .unwrap();
    // An alternating cycle is 1.6 s: the boundary at or before 10.3 is 10.0.
    assert_eq!(start(&e, ENGINE), 10.0);
    for t in [10.45, 11.0, 12.7, 31.9] {
        e.advance(t).unwrap();
        assert_eq!(opacity(&e, LOCK), opacity(&e, ENGINE), "in step at {t}");
    }
}

#[test]
fn off_a_clock_animations_keep_their_own_starts() {
    let mut e = Engine::new();
    e.set_animations(LOCK, &row("pulse 800ms infinite alternate"))
        .unwrap();
    e.advance(0.3).unwrap();
    e.set_animations(ENGINE, &row("pulse 800ms infinite alternate"))
        .unwrap();
    assert_eq!(start(&e, ENGINE), 0.3);
}

#[test]
fn a_finite_joiner_ends_on_a_cycle_boundary() {
    let mut e = Engine::new();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Pending"));
    }
    e.set_animations(LOCK, &row("pulse 1s infinite")).unwrap();
    e.advance(5.25).unwrap();
    e.set_animations(ENGINE, &row("pulse 1s 3")).unwrap();
    let play = &e.animation_plays(ENGINE)[0];
    assert_eq!(play.start, 5.0);
    // Visible from 5.25 to 8.0: between n - 1 and n iterations, ending
    // where three iterations end.
    assert_eq!(play.start + play.animation.end_time(), 8.0);
}

#[test]
fn a_join_on_a_boundary_starts_on_it() {
    let mut e = Engine::new();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Pending"));
    }
    e.set_animations(LOCK, &row("pulse 800ms infinite alternate"))
        .unwrap();
    // 4.8 % 1.6 is a hair under 1.6 in f64: the boundary is 4.8, not 3.2.
    e.advance(4.8).unwrap();
    e.set_animations(ENGINE, &row("pulse 800ms 1 alternate"))
        .unwrap();
    assert_eq!(start(&e, ENGINE), 4.8);
}

#[test]
fn a_timeline_left_idle_starts_over() {
    let mut e = Engine::new();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Pending"));
    }
    e.set_animations(LOCK, &row("pulse 1s 2")).unwrap();
    e.advance(2.0).unwrap(); // LOCK's ended exactly: the timeline is idle
    e.advance(2.7).unwrap();
    e.set_animations(ENGINE, &row("pulse 1s infinite")).unwrap();
    assert_eq!(start(&e, ENGINE), 2.7);
}

#[test]
fn a_paused_member_keeps_the_timeline_busy_and_resume_rejoins() {
    let mut e = Engine::new();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Pending"));
    }
    e.set_animations(LOCK, &row("pulse 1s infinite")).unwrap();
    e.set_animations(ENGINE, &row("pulse 1s infinite")).unwrap();
    e.advance(0.4).unwrap();
    e.set_animations(ENGINE, &row("pulse 1s infinite paused"))
        .unwrap();
    e.set_animations(LOCK, &Animations::NONE).unwrap();
    e.advance(7.9).unwrap();
    // ENGINE is paused, so the timeline stayed busy: origin 0.
    e.set_animations(LOCK, &row("pulse 1s infinite")).unwrap();
    assert_eq!(start(&e, LOCK), 7.0);
    e.advance(8.2).unwrap();
    e.set_animations(ENGINE, &row("pulse 1s infinite running"))
        .unwrap();
    assert_eq!(start(&e, ENGINE), 8.0, "a resume rejoins the phase");
    assert_eq!(opacity(&e, LOCK), opacity(&e, ENGINE));
}

#[test]
fn different_durations_meet_at_common_boundaries() {
    let mut e = Engine::new();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Activity"));
    }
    e.set_animations(LOCK, &row("pulse 800ms infinite"))
        .unwrap();
    e.advance(1.0).unwrap();
    e.set_animations(ENGINE, &row("pulse 1.6s infinite"))
        .unwrap();
    assert_eq!(start(&e, ENGINE), 0.0);
}

#[test]
fn a_node_moved_onto_an_idle_timeline_starts_it_over() {
    let mut e = Engine::new();
    e.set_animation_clock(LOCK, Some("Pending"));
    e.set_animations(LOCK, &row("pulse 1s 2")).unwrap();
    e.advance(0.2).unwrap();
    e.set_animations(ENGINE, &row("pulse 1s infinite")).unwrap();
    e.advance(10.3).unwrap();
    // Pending went idle at 2.0 with its origin at 0: ENGINE's own play,
    // kept from 0.2, must neither hold it busy nor keep its old phase.
    e.set_animation_clock(ENGINE, Some("Pending"));
    e.set_animations(ENGINE, &row("pulse 1s infinite")).unwrap();
    assert_eq!(start(&e, ENGINE), 10.3);
}

#[test]
fn leaving_a_drag_timeline_onto_a_clock_takes_its_phase() {
    let mut e = Engine::new();
    for node in [LOCK, ENGINE] {
        e.set_animation_clock(node, Some("Pending"));
    }
    e.set_animations(LOCK, &row("pulse 1s infinite")).unwrap();
    e.set_animation_timeline(ENGINE, Some((NamedTimeline::Missing, [0.0, 1.0])));
    e.set_animations(ENGINE, &row("pulse 1s infinite")).unwrap();
    e.advance(5.3).unwrap();
    e.set_animation_timeline(ENGINE, None);
    assert_eq!(start(&e, ENGINE), 5.0);
    assert_eq!(opacity(&e, LOCK), opacity(&e, ENGINE));
}

#[test]
fn removing_a_node_forgets_its_clock() {
    let mut e = Engine::new();
    e.set_animation_clock(LOCK, Some("Pending"));
    e.remove(LOCK);
    assert_eq!(e.animation_clock(LOCK), None);
}
