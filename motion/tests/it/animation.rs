//! CSS animations in the engine (LLP 1055 D5, D7, D10): start on first sight,
//! restart only when the name comes back, re-time in place, pause and resume,
//! lowered hosts never sampled, and infinite animations never settle.

use exact_motion::{Animations, Change, Engine, Keyframes, Property, Value};

const NODE: u64 = 3;

fn row(text: &str) -> Animations {
    let fade = Keyframes::parse("from{opacity:0}to{opacity:1}").unwrap();
    let grow = Keyframes::parse("from{r:3}to{r:9}").unwrap();
    let mut a = Animations::parse(text).unwrap();
    a.resolve(|name| match name {
        "fade" => Some(&fade),
        "grow" => Some(&grow),
        _ => None,
    });
    a
}

fn opacity(e: &mut Engine) -> Option<f64> {
    e.frame()
        .into_iter()
        .find(|p| p.node == NODE && p.property == Property::Opacity)
        .map(|p| p.value.x)
}

fn observe(e: &mut Engine, property: Property, value: f64) {
    e.observe(Change {
        node: NODE,
        property,
        value: Value::scalar(value),
        velocity: None,
    })
    .unwrap();
}

#[test]
fn an_animation_starts_when_first_seen_and_samples_as_a_seek() {
    let mut e = Engine::new();
    e.advance(10.0).unwrap();
    observe(&mut e, Property::Opacity, 1.0);
    e.set_animations(NODE, &row("fade 1s linear")).unwrap();
    assert!(!e.quiescent());
    assert_eq!(opacity(&mut e), Some(0.0));
    e.advance(10.5).unwrap();
    assert_eq!(opacity(&mut e), Some(0.5));
    assert_eq!(e.settle_time(), Some(11.0));
    // After the end without a fill the property shows its own value again.
    e.advance(11.2).unwrap();
    assert_eq!(opacity(&mut e), Some(1.0));
    assert!(e.quiescent());
    assert_eq!(e.settle_time(), None);
}

#[test]
fn re_timing_keeps_the_start_and_only_a_new_name_restarts() {
    let mut e = Engine::new();
    e.set_animations(NODE, &row("fade 2s linear")).unwrap();
    e.advance(1.0).unwrap();
    // A longer duration re-times the running animation: same start.
    e.set_animations(NODE, &row("fade 4s linear")).unwrap();
    assert_eq!(e.animation_plays(NODE)[0].start, 0.0);
    assert_eq!(opacity(&mut e), Some(0.25));
    // The name leaves and comes back: a restart.
    e.set_animations(NODE, &Animations::NONE).unwrap();
    assert!(e.animation_plays(NODE).is_empty());
    e.set_animations(NODE, &row("fade 4s linear")).unwrap();
    assert_eq!(e.animation_plays(NODE)[0].start, 1.0);
    // A destroyed node forgets its animations.
    e.remove(NODE);
    assert!(e.animation_plays(NODE).is_empty());
}

#[test]
fn pause_holds_local_time_and_resume_continues() {
    let mut e = Engine::new();
    e.set_animations(NODE, &row("fade 2s linear")).unwrap();
    e.advance(0.5).unwrap();
    e.set_animations(NODE, &row("fade 2s linear paused"))
        .unwrap();
    e.advance(5.0).unwrap();
    assert_eq!(opacity(&mut e), Some(0.25));
    assert!(e.quiescent(), "a paused animation keeps nothing busy");
    e.set_animations(NODE, &row("fade 2s linear running"))
        .unwrap();
    e.advance(5.5).unwrap();
    assert_eq!(opacity(&mut e), Some(0.5));
    // Created paused with a negative delay: a fixed phase (BENCH_FREEZE).
    let mut f = Engine::new();
    f.set_animations(NODE, &row("fade 2s linear -1s paused"))
        .unwrap();
    f.advance(100.0).unwrap();
    assert_eq!(opacity(&mut f), Some(0.5));
}

#[test]
fn infinite_animations_never_settle_but_finite_ones_do() {
    let mut e = Engine::new();
    e.set_animations(
        NODE,
        &row("fade 1s linear 0.5s both, grow 1.2s ease-out 0.6s infinite"),
    )
    .unwrap();
    assert_eq!(e.settle_time(), Some(1.5));
    e.advance(1.5).unwrap();
    assert_eq!(e.settle_time(), None);
    assert!(!e.quiescent(), "the pulse still runs");
    // Mid-iteration, 800 iterations in: the same value as the first's middle.
    e.advance(0.6 + 1.2 * 800.0 + 0.6).unwrap();
    let r = e
        .frame()
        .into_iter()
        .find(|p| p.property == Property::R)
        .unwrap();
    let mid = 3.0 + 6.0 * exact_motion::Easing::EaseOut.progress(0.5);
    assert!((r.value.x - mid).abs() < 1e-6, "{}", r.value.x);
}

#[test]
fn a_lowered_engine_tracks_starts_but_never_samples() {
    let mut e = Engine::new();
    e.set_lowered(true);
    observe(&mut e, Property::Opacity, 1.0);
    e.frame();
    e.advance(2.0).unwrap();
    e.set_animations(NODE, &row("fade 1s linear infinite"))
        .unwrap();
    assert!(e.quiescent());
    assert_eq!(e.animation_plays(NODE)[0].start, 2.0);
    e.advance(2.5).unwrap();
    // The host's compositor plays it; the frame carries the property's own value.
    assert_eq!(opacity(&mut e), Some(1.0));
    assert_eq!(
        e.animated(NODE, Property::Opacity, Value::scalar(1.0)),
        Some(Value::scalar(0.5))
    );
}

#[test]
fn an_unknown_name_starts_nothing() {
    let mut e = Engine::new();
    e.set_animations(NODE, &row("nosuch 1s, fade 2s")).unwrap();
    assert_eq!(e.animation_plays(NODE).len(), 1);
    assert_eq!(e.animation_plays(NODE)[0].animation.name, "fade");
}

#[test]
fn per_property_lowering_samples_only_the_rest() {
    let mut e = Engine::new();
    e.set_lowered_properties(&[Property::Opacity]);
    e.set_animations(NODE, &row("fade 1s linear infinite"))
        .unwrap();
    assert!(e.quiescent(), "opacity is the compositor's");
    e.set_animations(
        NODE,
        &row("fade 1s linear infinite, grow 1s linear infinite"),
    )
    .unwrap();
    assert!(!e.quiescent(), "r is sampled");
    e.advance(0.5).unwrap();
    let frame = e.frame();
    assert!(frame.iter().any(|p| p.property == Property::R));
    assert!(frame.iter().all(|p| p.property != Property::Opacity));
}

#[test]
fn duplicate_names_are_two_animations_and_a_reorder_restarts_none() {
    let mut e = Engine::new();
    e.set_animations(NODE, &row("fade 1s, grow 1s")).unwrap();
    e.advance(0.5).unwrap();
    // Reordered: both keep their starts.
    e.set_animations(NODE, &row("grow 1s, fade 1s")).unwrap();
    assert!(e.animation_plays(NODE).iter().all(|p| p.start == 0.0));
    // A second `fade` is a new animation; the old one keeps its place.
    e.set_animations(NODE, &row("grow 1s, fade 1s, fade 1s"))
        .unwrap();
    let starts: Vec<f64> = e.animation_plays(NODE).iter().map(|p| p.start).collect();
    assert_eq!(
        starts,
        vec![0.0, 0.5, 0.0],
        "the last `fade` pairs with the old one"
    );
}

#[test]
fn a_running_transition_wins_over_an_animation() {
    use exact_motion::{Easing, TimingFunction, Transition, TransitionProperty, Transitions};
    let mut e = Engine::new();
    observe(&mut e, Property::Opacity, 1.0);
    e.set_transitions(
        NODE,
        Transitions(vec![Transition::new(
            TransitionProperty::All,
            1.0,
            TimingFunction::Easing(Easing::Linear),
        )]),
    )
    .unwrap();
    e.set_animations(NODE, &row("fade 10s linear")).unwrap();
    e.frame();
    observe(&mut e, Property::Opacity, 0.0);
    e.advance(0.5).unwrap();
    assert_eq!(
        opacity(&mut e),
        Some(0.5),
        "the transition's value, not the animation's 0.05"
    );
    e.advance(1.5).unwrap();
    assert_eq!(
        opacity(&mut e),
        Some(0.15),
        "the transition ended: the animation shows"
    );
}

/// Held to the browser (LLP 1002 D2, LLP 1055 §8): values read with
/// `getComputedStyle` in headless Chrome 154 from paused CSS animations at
/// a set `currentTime` (local time, delay included), rounded as Chrome prints.
#[test]
fn samples_match_chrome() {
    let breathe = Keyframes::parse("0%{r:3;opacity:0.5}100%{r:9;opacity:0}").unwrap();
    let k3 = Keyframes::parse(
        "0%{opacity:0}50%{opacity:1;animation-timing-function:linear}100%{opacity:0}",
    )
    .unwrap();
    let cases: [(&str, &Keyframes, f64, Property, f64); 17] = [
        (
            "k3 2s steps(4, jump-start) -500ms",
            &k3,
            0.0,
            Property::Opacity,
            0.75,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            0.0,
            Property::R,
            3.0,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            300.0,
            Property::R,
            3.0,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            600.0,
            Property::R,
            3.0,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            900.0,
            Property::R,
            5.26883,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            1500.0,
            Property::R,
            8.43921,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            2100.0,
            Property::R,
            5.26883,
        ),
        (
            "breathe 1200ms ease-out 600ms infinite",
            &breathe,
            900.0,
            Property::Opacity,
            0.310931,
        ),
        (
            "k3 2s ease-in 3 alternate both",
            &k3,
            250.0,
            Property::Opacity,
            0.0934647,
        ),
        (
            "k3 2s ease-in 3 alternate both",
            &k3,
            1000.0,
            Property::Opacity,
            1.0,
        ),
        (
            "k3 2s ease-in 3 alternate both",
            &k3,
            1500.0,
            Property::Opacity,
            0.5,
        ),
        (
            "k3 2s ease-in 3 alternate both",
            &k3,
            2500.0,
            Property::Opacity,
            0.5,
        ),
        (
            "k3 2s ease-in 3 alternate both",
            &k3,
            3500.0,
            Property::Opacity,
            0.315357,
        ),
        (
            "k3 2s ease-in 3 alternate both",
            &k3,
            7000.0,
            Property::Opacity,
            0.0,
        ),
        (
            "k3 2s ease-in 1.5 reverse forwards",
            &k3,
            500.0,
            Property::Opacity,
            0.5,
        ),
        (
            "k3 2s ease-in 1.5 reverse forwards",
            &k3,
            5000.0,
            Property::Opacity,
            1.0,
        ),
        (
            "k3 2s steps(4, jump-start) -500ms",
            &k3,
            300.0,
            Property::Opacity,
            1.0,
        ),
    ];
    for (text, rule, ms, property, chrome) in cases {
        let mut a = Animations::parse(text).unwrap();
        a.resolve(|_| Some(rule));
        let underlying = match property {
            Property::R => Value::scalar(3.0),
            _ => Value::scalar(1.0),
        };
        let v = a.0[0]
            .sample(ms / 1000.0, property, underlying)
            .map_or(underlying.x, |v| v.x);
        assert!(
            (v - chrome).abs() < 5e-6,
            "{text} at {ms} ms: {v} vs Chrome {chrome}"
        );
    }
}

/// Colour animations interpolate premultiplied sRGB (LLP 1055.000 D6),
/// pinned to headless Chrome 154's `getComputedStyle` at a paused
/// `currentTime` (alpha to Chrome's 8-bit rounding).
#[test]
fn colour_samples_match_chrome() {
    let c1 = Keyframes::parse("from{color:#ff0000}to{color:rgba(0,0,255,0.5)}").unwrap();
    let c2 =
        Keyframes::parse("from{background-color:#16a34a}to{background-color:#000000}").unwrap();
    let c3 = Keyframes::parse("from{fill:#ff0000}to{fill:transparent}").unwrap();
    let cases: [(&str, &Keyframes, Property, f64, [u8; 4]); 8] = [
        (
            "c 1s linear",
            &c1,
            Property::Color,
            250.0,
            [218, 0, 37, 223],
        ),
        (
            "c 1s linear",
            &c1,
            Property::Color,
            500.0,
            [170, 0, 85, 192],
        ),
        (
            "c 1s linear",
            &c1,
            Property::Color,
            750.0,
            [102, 0, 153, 160],
        ),
        (
            "c 400ms ease",
            &c2,
            Property::BackgroundColor,
            0.0,
            [22, 163, 74, 255],
        ),
        (
            "c 400ms ease",
            &c2,
            Property::BackgroundColor,
            100.0,
            [13, 96, 44, 255],
        ),
        (
            "c 400ms ease",
            &c2,
            Property::BackgroundColor,
            200.0,
            [4, 32, 15, 255],
        ),
        (
            "c 400ms ease",
            &c2,
            Property::BackgroundColor,
            300.0,
            [1, 6, 3, 255],
        ),
        ("c 1s linear", &c3, Property::Fill, 500.0, [255, 0, 0, 128]),
    ];
    for (text, k, p, ms, want) in cases {
        let mut a = Animations::parse(text).unwrap();
        a.resolve(|_| Some(k));
        let entry = &a.0[0];
        let got = entry
            .sample(ms / 1000.0, p, Value::ZERO)
            .unwrap()
            .to_rgba8();
        for c in 0..4 {
            assert!(
                (got[c] as i32 - want[c] as i32).abs() <= 1,
                "{text} at {ms} ms: {got:?} != Chrome {want:?}"
            );
        }
    }
}
