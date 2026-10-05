//! Paint properties (LLP 1062): colours and `box-shadow` under `transition`
//! and `animation`, held to what a browser computes.

use crate::keyframed;
use exact_motion::{
    Change, Engine, Keyframes, Property, TimingFunction, TransitionProperty, Transitions, Value,
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

/// LLP 1062 D3: a spring on paint is its curve from rest, a `linear()`
/// easing over its settle time — what the web's CSS plays — so it
/// interrupts as CSS does, from where it is, and every host agrees.
#[test]
fn a_spring_on_paint_plays_its_curve_from_rest() {
    let t = Transitions::parse("background-color 200ms linear, all spring(180, 12, 1)").unwrap();
    let spring = t.matching(Property::BackgroundColor).unwrap();
    assert!(matches!(spring.timing, TimingFunction::Spring(_)));
    let config = exact_motion::SpringConfig {
        stiffness: 180.0,
        damping: 12.0,
        mass: 1.0,
    };
    let (duration, easing) = config.easing();
    let governed = spring.governing(Property::BackgroundColor);
    assert_eq!(governed.duration, duration);
    assert_eq!(governed.timing, TimingFunction::Easing(easing.clone()));
    // A compositor row keeps its physics.
    assert_eq!(spring.governing(Property::Translate), *spring);
    // The curve overshoots and settles, as the spring does from rest.
    let peak = (0..=100)
        .map(|i| easing.progress(i as f64 / 100.0))
        .fold(f64::MIN, f64::max);
    assert!(peak > 1.05, "{peak}");
    assert_eq!(easing.progress(1.0), 1.0);
    let mut e = Engine::new();
    e.set_transitions(NODE, t.clone()).unwrap();
    let red = Value::rgba(1.0, 0.0, 0.0, 1.0);
    let blue = Value::rgba(0.0, 0.0, 1.0, 1.0);
    let change = |value| Change {
        node: NODE,
        property: Property::BackgroundColor,
        value,
        velocity: None,
    };
    e.observe(change(red)).unwrap();
    e.observe(change(blue)).unwrap();
    for at in [0.05, 0.1, 0.2] {
        e.advance(at).unwrap();
        let p = easing.progress(at / duration);
        let want = red.lerp(blue, p);
        let got = e.sampled_value(NODE, Property::BackgroundColor).unwrap();
        assert!(
            (got - want).components().iter().all(|c| c.abs() < 1e-9),
            "{at}: {got:?} vs {want:?}"
        );
    }
    // Interrupted, it starts again from where it is, with no velocity.
    let here = e.sampled_value(NODE, Property::BackgroundColor).unwrap();
    e.observe(change(red)).unwrap();
    assert_eq!(e.sampled_value(NODE, Property::BackgroundColor), Some(here));
    assert!(e
        .spring_descriptor(NODE, Property::BackgroundColor)
        .is_none());
    e.advance(0.2 + duration).unwrap();
    assert_eq!(e.sampled_value(NODE, Property::BackgroundColor), Some(red));
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
        let got = css(e.sampled_value(NODE, Property::BackgroundColor).unwrap());
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
        css(e.sampled_value(NODE, Property::Color).unwrap()),
        [255.0, 0.0, 0.0, 0.5]
    );
    e.advance(1.0).unwrap();
    assert_eq!(
        e.sampled_value(NODE, Property::Color),
        Some(rgba(255, 0, 0, 1.0))
    );
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
        e.sampled_value(NODE, Property::BoxShadow),
        Some(Value::four(0.0, 2.0, 6.0, 0.0))
    );
    let [_, _, _, a] = css(e.sampled_value(NODE, Property::ShadowColor).unwrap());
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
    let a = keyframed(text).unwrap();
    let rule = a.0[0].keyframes.css();
    assert!(
        rule.contains("background-color:rgba(255, 255, 255, 1)"),
        "{rule}"
    );
    assert!(
        rule.contains("--exact-tint:rgba(10, 20, 200, 0.5)"),
        "{rule}"
    );
    assert_eq!(Keyframes::parse(&rule).unwrap(), a.0[0].keyframes);
    let mut e = Engine::new();
    set(&mut e, Property::BackgroundColor, rgba(0, 0, 0, 1.0));
    e.set_animations(NODE, &a).unwrap();
    e.advance(0.5).unwrap();
    // White to transparent: white fading, not grey.
    assert_eq!(
        css(e.sampled_value(NODE, Property::BackgroundColor).unwrap()),
        [255.0, 255.0, 255.0, 0.5]
    );
    // A shadow is geometry and a colour.
    assert!(keyframed("k 1s @keyframes k{to{box-shadow:0}}").is_err());
}

/// LLP 1062 D9: a keyframe's `light-dark()` colour takes the appearance its
/// animation starts under, as Chrome 153 resolves the rule once: a flip
/// leaves a playing animation's colours, and a host correcting its boot
/// guess re-resolves them in place, keeping the start.
#[test]
fn a_light_dark_keyframe_takes_the_appearance_it_starts_under() {
    let text = "lit 1s linear both @keyframes lit{from{color:light-dark(rgba(79,102,87,1),rgba(183,201,172,1))}to{color:light-dark(rgba(23,27,23,1),rgba(245,245,236,1))}}";
    let a = keyframed(text).unwrap();
    let from = &a.0[0].keyframes.0[0];
    assert_eq!(from.values, [(Property::Color, rgba(79, 102, 87, 1.0))]);
    assert_eq!(from.dark, [(Property::Color, rgba(183, 201, 172, 1.0))]);
    let rule = a.0[0].keyframes.css();
    assert!(
        rule.contains("color:light-dark(rgba(79, 102, 87, 1), rgba(183, 201, 172, 1))"),
        "{rule}"
    );
    assert_eq!(Keyframes::parse(&rule).unwrap(), a.0[0].keyframes);
    let mut e = Engine::new();
    set(&mut e, Property::Color, rgba(0, 0, 0, 1.0));
    e.set_animations(NODE, &a).unwrap();
    e.advance(0.5).unwrap();
    let light = [51.0, 64.5, 55.0, 1.0];
    let dark = [214.0, 223.0, 204.0, 1.0];
    assert_eq!(css(e.sampled_value(NODE, Property::Color).unwrap()), light);
    e.frame();
    // A flip: the playing animation keeps what it started with (Chrome).
    e.set_dark(true, false);
    assert!(e.frame().is_empty());
    assert_eq!(css(e.sampled_value(NODE, Property::Color).unwrap()), light);
    // Re-rendering the same row restarts nothing, so still light.
    e.set_animations(NODE, &a).unwrap();
    assert_eq!(css(e.sampled_value(NODE, Property::Color).unwrap()), light);
    // A correction of the boot guess: in place, at the same moment.
    e.set_dark(true, true);
    assert_eq!(e.frame().len(), 1, "the colour repaints");
    assert_eq!(css(e.sampled_value(NODE, Property::Color).unwrap()), dark);
    // An animation that starts under dark is dark.
    let mut e = Engine::new();
    e.set_dark(true, false);
    set(&mut e, Property::Color, rgba(0, 0, 0, 1.0));
    e.set_animations(NODE, &a).unwrap();
    e.advance(0.5).unwrap();
    assert_eq!(css(e.sampled_value(NODE, Property::Color).unwrap()), dark);
    // A dark value is a colour's, and only beside a light one.
    assert!(
        keyframed("k 1s @keyframes k{to{opacity:light-dark(rgba(0,0,0,1),rgba(1,1,1,1))}}")
            .is_err()
    );
}

/// `box-shadow` in a keyframe: its geometry and colour together, one
/// declaration in the rule, a `light-dark()` colour kept (LLP 1062).
#[test]
fn box_shadow_keyframes_round_trip_and_play() {
    let text = "glow 1s linear both @keyframes glow{from{box-shadow:0px 0px 0px rgba(0,0,0,0)}to{box-shadow:0px 16px 24px light-dark(rgba(29,78,216,1),rgba(255,255,255,0.5))}}";
    let list = keyframed(text).unwrap();
    let keyframes = &list.0[0].keyframes;
    assert!(keyframes
        .properties()
        .ends_with(&[Property::BoxShadow, Property::ShadowColor]));
    let to = &keyframes.0[1];
    assert!(to
        .values
        .contains(&(Property::BoxShadow, Value::four(0.0, 16.0, 24.0, 0.0))));
    assert_eq!(to.dark[0].0, Property::ShadowColor);
    // The rule writes it back as one declaration, and reads back the same.
    let rule = keyframes.css();
    assert!(
        rule.contains(
            "box-shadow:0px 16px 24px light-dark(rgba(29, 78, 216, 1), rgba(255, 255, 255, 0.5))"
        ),
        "{rule}"
    );
    assert!(!rule.contains("box-shadow-color"), "{rule}");
    assert_eq!(Keyframes::parse(&rule).unwrap(), *keyframes);
    // Half a shadow is refused.
    assert!(keyframed("k 1s @keyframes k{to{box-shadow-color:rgba(0,0,0,1)}}").is_err());
    let mut e = Engine::new();
    set(&mut e, Property::BoxShadow, Value::ZERO);
    set(&mut e, Property::ShadowColor, Value::ZERO);
    e.set_animations(NODE, &list).unwrap();
    e.advance(0.5).unwrap();
    assert_eq!(
        e.sampled_value(NODE, Property::BoxShadow),
        Some(Value::four(0.0, 8.0, 12.0, 0.0))
    );
    let shade = css(e.sampled_value(NODE, Property::ShadowColor).unwrap());
    assert!(
        (shade[2] - 216.0).abs() < 0.5 && (shade[3] - 0.5).abs() < 1e-9,
        "{shade:?}"
    );
    // Dark, from its start: the pair's other half.
    let mut night = Engine::new();
    night.set_dark(true, false);
    set(&mut night, Property::ShadowColor, Value::ZERO);
    night
        .set_animations(NODE, &keyframed(text).unwrap())
        .unwrap();
    night.advance(1.0).unwrap();
    let shade = css(night.sampled_value(NODE, Property::ShadowColor).unwrap());
    assert_eq!(shade, [255.0, 255.0, 255.0, 0.5]);
}

/// LLP 1100 D2.
#[test]
fn a_modern_colour_moves_in_oklab() {
    use exact_motion::Value;
    let red = Value::rgba8(255, 0, 0, 255);
    let blue = Value::rgba8(0, 0, 255, 255);
    let ok_blue = blue.to_oklab();
    assert!(ok_blue.oklab);
    let [r, g, b, a] = ok_blue.straight();
    assert!((r - 0.0).abs() < 1e-6 && (g - 0.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6 && a == 1.0);
    let legacy = red.lerp(blue, 0.5).straight();
    let modern = red.lerp(ok_blue, 0.5);
    assert!(modern.oklab, "a modern endpoint takes the pair into Oklab");
    let modern = modern.straight();
    assert!((legacy[0] - 0.5).abs() < 1e-9 && (legacy[2] - 0.5).abs() < 1e-9);
    let lum = |c: [f64; 4]| c[0] + c[1] + c[2];
    assert!(lum(modern) > lum(legacy), "{modern:?} vs {legacy:?}");
    // A modern colour outside sRGB keeps its value until it is read.
    let p3_red = Value::oklab(0.6486, 0.2716, 0.1325, 1.0);
    let (lin, _) = p3_red.linear_srgb();
    assert!(lin[0] > 1.0, "outside sRGB, unclipped: {lin:?}");
    assert_eq!(p3_red.to_rgba8()[0], 255, "an 8-bit reader gets the clip");
}
