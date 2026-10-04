use super::*;

fn anim(text: &str, keyframes: &str) -> Animation {
    let mut a = Animations::parse(text).unwrap();
    let k = Keyframes::parse(keyframes).unwrap();
    a.resolve(|_| Some(&k));
    a.0.remove(0)
}

fn at(a: &Animation, t: f64, p: Property) -> Option<f64> {
    a.sample(t, p, Value::scalar(0.25)).map(|v| v.x)
}

fn close(a: Option<f64>, b: f64) {
    let a = a.expect("a value");
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn the_shorthand_assigns_parts_as_css_does() {
    let a = Animations::parse("pulse 1.2s ease-out 600ms infinite").unwrap();
    let p = &a.0[0];
    assert_eq!(p.name, "pulse");
    assert_eq!(p.duration, 1.2);
    assert_eq!(p.delay, 0.6);
    assert_eq!(p.easing, Easing::EaseOut);
    assert!(p.iterations.is_infinite());
    // A keyword that could be another longhand's goes there first; the name
    // is what is left, wherever it stands.
    let b = Animations::parse("ease ease 1s alternate both paused 3").unwrap();
    let q = &b.0[0];
    assert_eq!((q.name.as_str(), q.easing.clone()), ("ease", Easing::Ease));
    assert_eq!(q.direction, Direction::Alternate);
    assert_eq!(q.fill, FillMode::Both);
    assert!(q.paused);
    assert_eq!(q.iterations, 3.0);
    assert_eq!(Animations::parse("none").unwrap(), Animations::NONE);
    assert_eq!(Animations::parse("a 1s, b 2s").unwrap().0.len(), 2);
    assert!(Animations::parse("a 1s 2s 3s").is_err());
    assert!(Animations::parse("a spring(100, 10, 1) 1s").is_err());
    assert!(Animations::parse("a -1s").is_err());
    assert!(Animations::parse("a b").is_err());
}

#[test]
fn the_shorthand_round_trips_through_css() {
    let a = Animations::parse(
        "draw 600ms ease-out both, breathe 1.2s cubic-bezier(0, 0, 0.58, 1) -300ms infinite paused",
    )
    .unwrap();
    let again = Animations::parse(&a.css()).unwrap();
    assert_eq!(a, again);
}

#[test]
fn keyframes_parse_merge_and_order() {
    let k =
        Keyframes::parse("to{r:9px;opacity:0}from{r:3;opacity:.5}50%,75%{opacity:0.2}").unwrap();
    let offsets: Vec<f64> = k.0.iter().map(|f| f.offset).collect();
    assert_eq!(offsets, vec![0.0, 0.5, 0.75, 1.0]);
    assert_eq!(k.properties(), vec![Property::Opacity, Property::R]);
    // Equal offsets and timing functions merge; a later value wins.
    let m = Keyframes::parse("0%{opacity:1}0%{opacity:0;r:2}").unwrap();
    assert_eq!(m.0.len(), 1);
    assert_eq!(
        m.0[0].values,
        vec![
            (Property::Opacity, Value::scalar(0.0)),
            (Property::R, Value::scalar(2.0))
        ]
    );
    assert!(Keyframes::parse("from{color:reddish}").is_err());
    assert!(Keyframes::parse("from{height:3px}").is_err());
    assert!(Keyframes::parse("120%{opacity:1}").is_err());
    assert_eq!(Keyframes::parse(&k.css()).unwrap(), k);
    assert_eq!(
        k.css(),
        "0%{r:3px;opacity:0.5;}50%{opacity:0.2;}75%{opacity:0.2;}100%{r:9px;opacity:0;}"
    );
}

#[test]
fn phases_and_fill_modes() {
    let k = "from{opacity:0}to{opacity:1}";
    let none = anim("a 1s linear 0.5s", k);
    assert_eq!(
        at(&none, 0.25, Property::Opacity),
        None,
        "delay, no backwards fill"
    );
    close(at(&none, 1.0, Property::Opacity), 0.5);
    assert_eq!(
        at(&none, 1.5, Property::Opacity),
        None,
        "ended, no forwards fill"
    );
    let both = anim("a 1s linear 0.5s both", k);
    close(at(&both, 0.25, Property::Opacity), 0.0);
    close(at(&both, 9.0, Property::Opacity), 1.0);
    // A negative delay starts partway through.
    let early = anim("a 1s linear -0.25s", k);
    close(at(&early, 0.0, Property::Opacity), 0.25);
    assert_eq!(early.end_time(), 0.75);
}

#[test]
fn iterations_and_directions() {
    let k = "from{opacity:0}to{opacity:1}";
    let alt = anim("a 1s linear 3 alternate forwards", k);
    close(at(&alt, 0.25, Property::Opacity), 0.25);
    close(at(&alt, 1.25, Property::Opacity), 0.75);
    close(at(&alt, 2.25, Property::Opacity), 0.25);
    // After three iterations the last (forward) one fills at its end.
    close(at(&alt, 5.0, Property::Opacity), 1.0);
    let rev = anim("a 1s linear reverse", k);
    close(at(&rev, 0.25, Property::Opacity), 0.75);
    let ar = anim("a 1s linear 2 alternate-reverse forwards", k);
    close(at(&ar, 0.25, Property::Opacity), 0.75);
    close(at(&ar, 1.25, Property::Opacity), 0.25);
    close(at(&ar, 3.0, Property::Opacity), 1.0);
    // A fractional count ends partway through an iteration.
    let half = anim("a 1s linear 1.5 forwards", k);
    close(at(&half, 9.0, Property::Opacity), 0.5);
    // Infinite never ends.
    let inf = anim("a 1s linear infinite", k);
    assert!(inf.end_time().is_infinite());
    close(at(&inf, 1000.25, Property::Opacity), 0.25);
}

#[test]
fn implicit_endpoints_take_the_underlying_value() {
    // Only `to` is declared: `from` is the underlying 0.25.
    let a = anim("a 1s linear", "to{opacity:1}");
    close(at(&a, 0.0, Property::Opacity), 0.25);
    close(at(&a, 0.5, Property::Opacity), 0.625);
    // A property the rule does not name is not animated.
    assert_eq!(at(&a, 0.5, Property::R), None);
}

#[test]
fn the_timing_function_applies_per_keyframe_interval() {
    let k = "0%{opacity:0}50%{opacity:1;animation-timing-function:linear}100%{opacity:0}";
    let a = anim("a 2s ease-in", k);
    // First half: ease-in over the interval.
    close(at(&a, 0.5, Property::Opacity), Easing::EaseIn.progress(0.5));
    // Second half: the keyframe's own linear.
    close(at(&a, 1.5, Property::Opacity), 0.5);
}

#[test]
fn zero_duration_jumps_to_the_end_with_forwards_fill() {
    let k = "from{opacity:0}to{opacity:1}";
    let a = anim("a 0s forwards", k);
    close(at(&a, 0.0, Property::Opacity), 1.0);
    let b = anim("a 0s", k);
    assert_eq!(at(&b, 0.0, Property::Opacity), None);
}

#[test]
fn the_benchmark_draw_in_and_pulse() {
    let draw = anim(
        "draw 600ms ease-out both",
        "from{stroke-dashoffset:1}to{stroke-dashoffset:0}",
    );
    close(at(&draw, 0.0, Property::StrokeDashoffset), 1.0);
    close(at(&draw, 0.6, Property::StrokeDashoffset), 0.0);
    close(
        at(&draw, 0.3, Property::StrokeDashoffset),
        1.0 - Easing::EaseOut.progress(0.5),
    );
    let pulse = anim(
        "breathe 1200ms ease-out 600ms infinite",
        "from{r:3;opacity:.5}to{r:9;opacity:0}",
    );
    assert_eq!(at(&pulse, 0.3, Property::R), None, "waits for the draw-in");
    close(at(&pulse, 0.6, Property::R), 3.0);
    close(
        at(&pulse, 1.2, Property::R),
        3.0 + 6.0 * Easing::EaseOut.progress(0.5),
    );
    close(at(&pulse, 1.8, Property::R), 3.0);
}
