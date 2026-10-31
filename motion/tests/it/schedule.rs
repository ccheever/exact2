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
                a.sample(next.next_down(), Property::Opacity, Value::scalar(1.)),
                a.sample(t, Property::Opacity, Value::scalar(1.)),
                "{position} at {t} until {next}"
            );
            assert!(
                a.sample(next, Property::Opacity, Value::scalar(1.))
                    != a.sample(t, Property::Opacity, Value::scalar(1.))
                    || next.fract() == 0.,
                "{position} at {t}: {next} is neither a change nor an iteration edge"
            );
        }
        assert_eq!(
            a.next_change(2., Property::Opacity, Value::scalar(1.), false),
            None
        );
    }
}

#[test]
fn keyframe_holds_sleep_to_their_edge_and_continuous_segments_keep_sampling() {
    let a = crate::keyframed(
        "blink 1s linear infinite @keyframes blink{from{opacity:1}50%{opacity:1}to{opacity:0}}",
    )
    .unwrap()
    .0
    .remove(0);
    for t in [0., 0.25, 0.49] {
        let next = a
            .next_change(t, Property::Opacity, Value::scalar(1.), false)
            .unwrap();
        assert_eq!(next, 0.5);
        assert_eq!(
            a.sample(next.next_down(), Property::Opacity, Value::scalar(1.)),
            a.sample(t, Property::Opacity, Value::scalar(1.))
        );
    }
    assert_eq!(
        a.next_change(0.5, Property::Opacity, Value::scalar(1.), false),
        Some(0.5)
    );
    // Flat piecewise easing remains continuous demand conservatively.
    let a = animation("1s linear(0 0%, 0 50%, 1 100%) infinite");
    assert_eq!(
        a.next_change(0.25, Property::Opacity, Value::scalar(1.), false),
        Some(0.25)
    );
}

#[test]
fn alternate_repeats_keep_the_first_and_last_value_of_each_direction() {
    for direction in ["alternate", "alternate-reverse"] {
        let a = animation(&format!("1s steps(2, jump-none) 3 forwards {direction}"));
        let mut t = 0.;
        let mut changes = Vec::new();
        for _ in 0..24 {
            let Some(next) = a.next_change(t, Property::Opacity, Value::scalar(1.), false) else {
                break;
            };
            let before = a.sample(t, Property::Opacity, Value::scalar(1.));
            assert_eq!(
                a.sample(next.next_down(), Property::Opacity, Value::scalar(1.)),
                before,
                "{direction} {t} -> {next}"
            );
            let after = a.sample(next, Property::Opacity, Value::scalar(1.));
            if before != after {
                changes.push(next);
            } else {
                assert_eq!(next.fract(), 0., "{direction} {t} -> {next}");
            }
            t = next;
        }
        assert_eq!(changes.len(), 3, "{direction}: {changes:?}");
        for (got, expected) in changes.into_iter().zip([0.5, 1.5, 2.5]) {
            assert!((got - expected).abs() < 1e-12);
        }
        assert_eq!(t, 3.);
    }
}

#[test]
fn retargeting_and_competing_plays_recompute_the_earliest_deadline() {
    use exact_motion::{Change, Transitions};
    let mut e = Engine::new();
    let opacity = |value| Change {
        node: 1,
        property: Property::Opacity,
        value: Value::scalar(value),
        velocity: None,
    };
    e.set_transitions(1, Transitions::parse("opacity 1s steps(2)").unwrap())
        .unwrap();
    e.observe(opacity(1.)).unwrap();
    e.observe(opacity(0.)).unwrap();
    assert_eq!(e.next_change(), Some(0.5));
    e.advance(0.25).unwrap();
    e.observe(opacity(0.4)).unwrap();
    assert_eq!(e.next_change(), Some(0.75));
    let plays = crate::keyframed(
        "blink 1s steps(4) forwards @keyframes blink{from{opacity:1}to{opacity:0}}",
    )
    .unwrap();
    e.set_animations(2, &plays).unwrap();
    assert_eq!(e.next_change(), Some(0.5));
    e.advance(0.5).unwrap();
    assert_eq!(e.next_change(), Some(0.75));
    e.advance(1.25_f64.next_down()).unwrap();
    assert_ne!(
        e.sampled_value(2, Property::Opacity),
        Some(Value::scalar(0.))
    );
    assert_eq!(e.next_change(), Some(1.25));
    e.advance(1.25).unwrap();
    assert_eq!(
        e.sampled_value(2, Property::Opacity),
        Some(Value::scalar(0.))
    );
    assert_eq!(e.value(1, Property::Opacity), Some(Value::scalar(0.4)));
    assert_eq!(e.next_change(), None);
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
