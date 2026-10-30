use exact_motion::{Engine, Property, Value};

fn animation(timing: &str) -> exact_motion::Animation {
    crate::keyframed(&format!(
        "blink {timing} @keyframes blink{{from{{opacity:1}}to{{opacity:0}}}}"
    ))
    .unwrap()
    .0
    .remove(0)
}

#[test]
fn step_deadlines_follow_the_sample_and_stop_after_the_last_iteration() {
    for position in ["jump-start", "jump-end", "jump-none", "jump-both"] {
        let a = animation(&format!("1s steps(4, {position}) 2"));
        for t in [0., 0.1, 0.25, 0.5, 0.9, 1., 1.75] {
            let next = a
                .next_change(t, Property::Opacity, Value::scalar(1.), false)
                .unwrap();
            assert!(next > t);
            assert_eq!(
                a.sample((t + next) / 2., Property::Opacity, Value::scalar(1.)),
                a.sample(t, Property::Opacity, Value::scalar(1.)),
                "{position} at {t} until {next}"
            );
        }
        assert_eq!(
            a.next_change(2., Property::Opacity, Value::scalar(1.), false),
            None
        );
    }
}

#[test]
fn lowered_and_paused_plays_request_no_sample_clock() {
    let a = crate::keyframed("blink 1s steps(2) infinite @keyframes blink{to{opacity:0}}").unwrap();
    let mut e = Engine::new();
    e.set_animations(1, &a).unwrap();
    assert_eq!(e.next_change(), Some(0.5));
    e.set_lowered(true);
    assert_eq!(e.next_change(), None);
}

#[test]
fn reverse_steps_cross_the_boundary_without_a_display_period_of_delay() {
    let a = animation("1s steps(2, jump-none) infinite reverse");
    let mut t = 0.;
    let mut changes = Vec::new();
    let mut value = a.sample(t, Property::Opacity, Value::scalar(1.));
    for _ in 0..12 {
        t = a
            .next_change(t, Property::Opacity, Value::scalar(1.), false)
            .unwrap();
        let next = a.sample(t, Property::Opacity, Value::scalar(1.));
        if next != value {
            changes.push(t);
            value = next;
        }
        if changes.len() == 3 {
            break;
        }
    }
    assert_eq!(changes.len(), 3, "{changes:?}");
    for (actual, expected) in changes.into_iter().zip([0.5, 1., 1.5]) {
        assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
    }
}
