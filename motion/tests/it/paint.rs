//! Paint properties (LLP 1062): colours and `box-shadow` under `transition`
//! and `animation`, held to what a browser computes.

use exact_motion::{
    Animations, Change, Easing, Engine, Property, TimingFunction, TransitionProperty, Transitions,
    Value,
};

const NODE: u64 = 7;

fn rgba(r: u8, g: u8, b: u8, a: f64) -> Value {
    let unit = |c: u8| c as f64 / 255.0;
    Value::rgba(unit(r), unit(g), unit(b), a)
}

fn set(e: &mut Engine, property: Property, value: Value) {
    e.observe(Change {
        node: NODE,
        property,
        value,
        velocity: None,
    })
    .unwrap();
}

/// Straight channels 0–255, alpha 0–1: what computed style reads.
fn css(v: Value) -> [f64; 4] {
    let [r, g, b, a] = v.straight();
    [r * 255.0, g * 255.0, b * 255.0, a]
}

#[test]
fn the_shorthand_names_every_paint_property_and_box_shadow_names_both_halves() {
    let t = Transitions::parse(
        "background-color 200ms cubic-bezier(.32,.72,0,1), color 120ms ease, border-color 1s, box-shadow 320ms linear, tint-color 90ms",
    )
    .unwrap();
    let governs = |p: Property| t.matching(p).map(|d| d.duration);
    assert_eq!(governs(Property::BackgroundColor), Some(0.2));
    assert_eq!(governs(Property::Color), Some(0.12));
    for side in [
        Property::BorderTopColor,
        Property::BorderRightColor,
        Property::BorderBottomColor,
        Property::BorderLeftColor,
    ] {
        assert_eq!(governs(side), Some(1.0), "{side:?}");
    }
    assert_eq!(governs(Property::BoxShadow), Some(0.32));
    assert_eq!(governs(Property::ShadowColor), Some(0.32));
    assert_eq!(governs(Property::TintColor), Some(0.09));
    assert_eq!(governs(Property::Opacity), None);
    assert_eq!(t.0[2].property, TransitionProperty::BorderColor);
    // Its colour half has no name of its own, and a length is still refused.
    assert!(Transitions::parse("box-shadow-color 1s").is_err());
    assert!(Transitions::parse("width 1s").is_err());
}

#[test]
fn a_spring_never_drives_paint_as_the_web_leaves_it_out() {
    let t = Transitions::parse("background-color 200ms linear, all spring(180, 12, 1)").unwrap();
    // The web's CSS omits the spring, so the easing before it governs.
    let bg = t.matching(Property::BackgroundColor).unwrap();
    assert_eq!(bg.timing, TimingFunction::Easing(Easing::Linear));
    assert!(matches!(
        t.matching(Property::Translate).unwrap().timing,
        TimingFunction::Spring(_)
    ));
    let only = Transitions::parse("all spring(180, 12, 1)").unwrap();
    assert!(only.matching(Property::Color).is_none());
}

#[test]
fn colours_interpolate_premultiplied_as_chrome_does() {
    let mut e = Engine::new();
    e.set_transitions(
        NODE,
        Transitions::parse("background-color 1s linear").unwrap(),
    )
    .unwrap();
    set(&mut e, Property::BackgroundColor, rgba(255, 0, 0, 1.0));
    set(&mut e, Property::BackgroundColor, rgba(0, 0, 255, 0.5));
    // Chrome 153: red to half-transparent blue, weighted by each alpha.
    for (at, chrome) in [
        (0.25, [218.0, 0.0, 37.0, 0.875]),
        (0.5, [170.0, 0.0, 85.0, 0.753]),
        (0.75, [102.0, 0.0, 153.0, 0.627]),
    ] {
        e.advance(at).unwrap();
        let got = css(e.value(NODE, Property::BackgroundColor).unwrap());
        for i in 0..3 {
            assert!(
                (got[i] - chrome[i]).abs() <= 1.0,
                "{at}: {got:?} vs {chrome:?}"
            );
        }
        assert!(
            (got[3] - chrome[3]).abs() <= 1.0 / 255.0 + 1e-3,
            "{at}: {got:?}"
        );
    }
    // From transparent the hue never darkens: only alpha moves.
    let mut e = Engine::new();
    e.set_transitions(NODE, Transitions::parse("color 1s linear").unwrap())
        .unwrap();
    set(&mut e, Property::Color, Value::ZERO);
    set(&mut e, Property::Color, rgba(255, 0, 0, 1.0));
    e.advance(0.5).unwrap();
    assert_eq!(
        css(e.value(NODE, Property::Color).unwrap()),
        [255.0, 0.0, 0.0, 0.5]
    );
    e.advance(1.0).unwrap();
    assert_eq!(e.value(NODE, Property::Color), Some(rgba(255, 0, 0, 1.0)));
}

#[test]
fn a_shadow_from_none_grows_its_geometry_and_its_colour_together() {
    let mut e = Engine::new();
    e.set_transitions(NODE, Transitions::parse("box-shadow 1s linear").unwrap())
        .unwrap();
    // `none`: zero lengths, transparent — CSS's padding for the missing shadow.
    set(&mut e, Property::BoxShadow, Value::ZERO);
    set(&mut e, Property::ShadowColor, Value::ZERO);
    set(
        &mut e,
        Property::BoxShadow,
        Value::four(0.0, 4.0, 12.0, 0.0),
    );
    set(&mut e, Property::ShadowColor, rgba(0, 0, 0, 0.3));
    e.advance(0.5).unwrap();
    // Chrome 153 at 50%: `rgba(0, 0, 0, 0.153) 0px 2px 6px 0px`.
    assert_eq!(
        e.value(NODE, Property::BoxShadow),
        Some(Value::four(0.0, 2.0, 6.0, 0.0))
    );
    let [_, _, _, a] = css(e.value(NODE, Property::ShadowColor).unwrap());
    assert!((a - 0.153).abs() <= 1.0 / 255.0 + 1e-3, "{a}");
}

#[test]
fn a_value_carries_only_its_property_s_components() {
    let mut e = Engine::new();
    let refused = e.observe(Change {
        node: NODE,
        property: Property::BoxShadow,
        value: Value::four(1.0, 2.0, 3.0, 4.0),
        velocity: None,
    });
    assert!(
        refused.is_err(),
        "box-shadow's geometry has three components"
    );
    assert!(Value::four(1.0, 2.0, 3.0, 4.0).fits(Property::Color));
    assert!(!Value::new(1.0, 2.0).fits(Property::Opacity));
}

#[test]
fn keyframes_animate_colours_and_their_rule_reads_back() {
    let text = "k 1s linear @keyframes k{from{background-color:rgba(255,255,255,1)}to{--exact-tint:rgba(10,20,200,0.5);background-color:rgba(0,0,0,0)}}";
    let a = Animations::parse(text).unwrap();
    let rule = a.0[0].keyframes.rule("k");
    assert!(
        rule.contains("background-color:rgba(255,255,255,1)"),
        "{rule}"
    );
    assert!(rule.contains("--exact-tint:rgba(10,20,200,0.5)"), "{rule}");
    assert_eq!(Animations::parse(&a.text()).unwrap(), a);
    let mut e = Engine::new();
    set(&mut e, Property::BackgroundColor, rgba(0, 0, 0, 1.0));
    e.set_animations(NODE, a).unwrap();
    e.advance(0.5).unwrap();
    // White to transparent: white fading, not grey.
    assert_eq!(
        css(e.value(NODE, Property::BackgroundColor).unwrap()),
        [255.0, 255.0, 255.0, 0.5]
    );
    // `box-shadow` stays a transition's.
    assert!(Animations::parse("k 1s @keyframes k{to{box-shadow:0}}").is_err());
}
