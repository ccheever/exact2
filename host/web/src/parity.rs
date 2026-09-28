//! The browser-driven parity harness: the browser is the oracle for
//! `exact-motion` (LLP 1002 D2, §5).
//!
//! A case is a `transition` row (as CSS text the host itself emits), an
//! initial value, and a script of target changes and sample times. A real
//! browser runs the cases (`host/web/parity.html`, driven by
//! `host/web/parity.mjs`) by seeking each transition with
//! `Animation.currentTime` — the same operation the engine's clock is (LLP
//! 1002 D3) — and records what it computed into a fixture. [`check`] drives
//! the engine through the same script and holds every sample to the
//! browser's within [`TOLERANCE`]. The fixture is checked in, so the check
//! is deterministic and needs no browser; the recorder re-records it.
//!
//! A spring case is the frames the engine lowers (`Engine::spring_frames`),
//! played by the browser through `Element.animate` exactly as the glue plays
//! them; its samples hold the *lowering* to the engine, midpoints included.
//!
//! A keyframe case (LLP 1055 D5) is an `animation` row: the page gets the
//! declaration and `@keyframes` rules the host emits, starts it at time zero
//! over the case's initial value, and seeks it the same way.
//!
//! Presence (LLP 1063) has no browser oracle: [`PRESENCE_SOURCE`] and
//! [`PRESENCE_STEPS`] drive one recorded timeline through the agent on each
//! host. `parity.mjs --presence` compares their presented surfaces and opacity.

use crate::css::transition_css;
use exact_motion::{Animations, Change, Engine, Property, TimingFunction, Transitions, Value};
use std::fmt::Write as _;

/// The band a browser sample may differ from the engine's by: computed
/// style serializes to about six significant digits, and cubic-bezier
/// solvers differ in their last bits.
pub const TOLERANCE: f64 = 1e-3;

/// A colour's band (LLP 1062): a browser keeps an interpolated colour in
/// 8-bit channels, alpha included, so a channel may sit a unit from the
/// engine's and alpha a step.
pub const COLOR_TOLERANCE: (f64, f64) = (1.0, 1.0 / 255.0 + 1e-3);

/// Fixed boxes avoid platform font metrics; the root's viewport offset is
/// subtracted by the recorder so native safe areas do not become motion.
pub const PRESENCE_SOURCE: &str = r##"keyframes leave
  from opacity=1
  to opacity=0

component PresenceTimeline
  state moved = false
  state large = false
  state shown = true
  action move writes moved
    moved = not moved
  action grow writes large
    large = not large
  action hide writes shown
    shown = false
  view
    column testId="root" width=300 height=500
      row height=30
        button "Move" testId="move" press=move width=80 height=30
        button "Grow" testId="grow" press=grow width=80 height=30
        button "Hide" testId="hide" press=hide width=80 height=30
      view height=(moved ? 120 : 20) flex-shrink=0
      when shown
        view testId="card" width=(large ? 180 : 100) height=(large ? 80 : 40) flex-shrink=0 background-color="#ff0000" layout-transition="1000ms linear" exit-animation="leave 400ms linear both"
      view testId="sibling" width=50 height=30 flex-shrink=0 background-color="#0000ff" layout-transition="1000ms linear"
"##;

/// Relative agent-clock advances, sampled after every operation. Resizing
/// the surface during a move and resizing the viewport are distinct cases.
pub const PRESENCE_STEPS: &str = r#"[
  {"name":"initial"},
  {"name":"move-start","tap":"move"},
  {"name":"move-100","advance":100},
  {"name":"size-start","tap":"grow"},
  {"name":"size-100","advance":100},
  {"name":"retarget-start","tap":"move"},
  {"name":"retarget-100","advance":100},
  {"name":"viewport-resize","resize":[460,500]},
  {"name":"after-resize","advance":100},
  {"name":"second-move","tap":"move"},
  {"name":"second-move-200","advance":200},
  {"name":"before-exit","advance":1000},
  {"name":"exit-start","tap":"hide"},
  {"name":"exit-100","advance":100},
  {"name":"exit-200","advance":100},
  {"name":"exit-ended","advance":201},
  {"name":"settled","advance":600}
]"#;

/// One step of a case's script, at a time in seconds.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// The target changes.
    Set {
        /// Seconds from the case's start.
        at: f64,
        /// The new target.
        value: Value,
    },
    /// Read the presentation value.
    Sample {
        /// Seconds from the case's start.
        at: f64,
    },
}

/// One case.
#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    /// A name, unique in the list.
    pub name: &'static str,
    /// The property sampled.
    pub property: Property,
    /// The node's `transition` row.
    pub transitions: Transitions,
    /// The node's `animation` row, started at time zero.
    pub animations: Animations,
    /// The value before the script starts (set with no transition).
    pub initial: Value,
    /// The script, in time order.
    pub steps: Vec<Step>,
    /// A drag timeline's `animation-range`, in points (LLP 1057.003): the
    /// animation follows a source's position instead of the clock, and each
    /// `Sample`'s `at` is that position. The browser's oracle is the same
    /// animation on a CSS scroll timeline, read at that scroll offset.
    pub timeline: Option<[f64; 2]>,
}

fn samples(at: &[f64]) -> Vec<Step> {
    at.iter().map(|t| Step::Sample { at: *t }).collect()
}

fn single(
    name: &'static str,
    property: Property,
    css: &str,
    from: Value,
    to: Value,
    at: &[f64],
) -> Case {
    let mut steps = vec![Step::Set { at: 0.0, value: to }];
    steps.extend(samples(at));
    Case {
        name,
        property,
        transitions: Transitions::parse(css).expect("a valid case"),
        animations: Animations::NONE,
        initial: from,
        steps,
        timeline: None,
    }
}

/// A shorthand then the `@keyframes name{…}` rules it names, resolved as
/// the runner resolves a row against its plan's table (LLP 1055 D5).
fn resolved(text: &str) -> Animations {
    let (head, rules) = text.split_at(text.find("@keyframes").unwrap_or(text.len()));
    let table: Vec<(String, exact_motion::Keyframes)> = rules
        .split("@keyframes")
        .skip(1)
        .map(|rule| {
            let open = rule.find('{').expect("a rule");
            let body = rule[open + 1..]
                .trim_end()
                .strip_suffix('}')
                .expect("a body");
            let k = exact_motion::Keyframes::parse(body).expect("a valid rule");
            (rule[..open].trim().to_string(), k)
        })
        .collect();
    let mut a = Animations::parse(head).expect("a valid case");
    let dropped = a.resolve(|n| table.iter().find(|(m, _)| m == n).map(|(_, k)| k));
    assert!(dropped.is_empty(), "{dropped:?}");
    a
}

/// A case's own name for a rule, so cases that reuse a name never share a
/// page's rule.
fn rule_name(case: &str, a: &exact_motion::Animation) -> String {
    format!(
        "{}-{}",
        a.name,
        case.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
    )
}

fn keyframes(
    name: &'static str,
    property: Property,
    text: &str,
    underlying: Value,
    at: &[f64],
) -> Case {
    Case {
        name,
        property,
        transitions: Transitions::NONE,
        animations: resolved(text),
        initial: underlying,
        steps: samples(at),
        timeline: None,
    }
}

/// A keyframe case held at a drag timeline's positions (LLP 1057.003):
/// `at` are the source's positions, in points, over `range`.
fn timeline(
    name: &'static str,
    property: Property,
    text: &str,
    underlying: Value,
    range: [f64; 2],
    at: &[f64],
) -> Case {
    Case {
        timeline: Some(range),
        ..keyframes(name, property, text, underlying, at)
    }
}

/// The node a timeline case's source is; the sampled node is 1.
const SOURCE: u64 = 2;

/// Every case, in fixture order.
pub fn cases() -> Vec<Case> {
    let o = |v: f64| Value::scalar(v);
    let mid = [0.1, 0.25, 0.5, 0.75, 0.9];
    let off_grid = [0.1, 0.3, 0.55, 0.7, 0.9];
    let mut out = vec![
        single(
            "linear",
            Property::Opacity,
            "opacity 1s linear",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "ease",
            Property::Opacity,
            "opacity 1s ease",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "ease-in",
            Property::Opacity,
            "opacity 1s ease-in",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "ease-out",
            Property::Opacity,
            "opacity 1s ease-out",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "ease-in-out",
            Property::Opacity,
            "opacity 1s ease-in-out",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "cubic-bezier",
            Property::Opacity,
            "opacity 1s cubic-bezier(0.4, 0, 0.2, 1)",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "steps-jump-start",
            Property::Opacity,
            "opacity 1s steps(4, jump-start)",
            o(0.0),
            o(1.0),
            &off_grid,
        ),
        single(
            "steps-jump-end",
            Property::Opacity,
            "opacity 1s steps(4, jump-end)",
            o(0.0),
            o(1.0),
            &off_grid,
        ),
        single(
            "steps-jump-none",
            Property::Opacity,
            "opacity 1s steps(4, jump-none)",
            o(0.0),
            o(1.0),
            &off_grid,
        ),
        single(
            "steps-jump-both",
            Property::Opacity,
            "opacity 1s steps(4, jump-both)",
            o(0.0),
            o(1.0),
            &off_grid,
        ),
        single(
            "linear-stops",
            Property::Opacity,
            "opacity 1s linear(0, 0.9 50%, 1)",
            o(0.0),
            o(1.0),
            &mid,
        ),
        single(
            "delay",
            Property::Opacity,
            "opacity 1s linear 0.5s",
            o(0.0),
            o(1.0),
            &[0.25, 0.75, 1.25, 1.6],
        ),
        single(
            "negative-delay",
            Property::Opacity,
            "opacity 1s linear -0.5s",
            o(0.0),
            o(1.0),
            &[0.0, 0.1, 0.25, 0.4, 0.6],
        ),
        single(
            "zero-duration",
            Property::Opacity,
            "opacity 0s linear",
            o(0.0),
            o(1.0),
            &[0.0, 0.1],
        ),
        single(
            "translate",
            Property::Translate,
            "translate 1s ease",
            Value::ZERO,
            Value::new(100.0, 50.0),
            &mid,
        ),
        single(
            "scale",
            Property::Scale,
            "scale 1s ease-out",
            o(1.0),
            o(2.0),
            &mid,
        ),
        single(
            "rotate",
            Property::Rotate,
            "rotate 1s ease-in",
            o(0.0),
            o(90.0),
            &mid,
        ),
        single(
            "all",
            Property::Scale,
            "all 1s linear",
            o(1.0),
            o(3.0),
            &mid,
        ),
    ];
    // CSS Transitions §3.2: reversing an ease-in-out 30% in shortens it.
    let mut reversing = single(
        "reversing",
        Property::Opacity,
        "opacity 1s ease-in-out",
        o(0.0),
        o(1.0),
        &[0.1, 0.25],
    );
    reversing.steps.push(Step::Set {
        at: 0.3,
        value: o(0.0),
    });
    reversing
        .steps
        .extend(samples(&[0.35, 0.45, 0.55, 0.7, 1.0]));
    out.push(reversing);
    // An interruption to a value that is not the reversing-adjusted start
    // runs the full duration from the current value.
    let mut interrupt = single(
        "interrupt",
        Property::Opacity,
        "opacity 1s linear",
        o(0.0),
        o(1.0),
        &[0.25],
    );
    interrupt.steps.push(Step::Set {
        at: 0.5,
        value: o(0.25),
    });
    interrupt.steps.extend(samples(&[0.75, 1.0, 1.25, 1.6]));
    out.push(interrupt);
    // Paint (LLP 1062): colours interpolate premultiplied, as CSS Color 4
    // says for legacy colours — a fade from transparent keeps its hue, and
    // one between alphas weights each colour by its own alpha.
    let rgba = |r: u8, g: u8, b: u8, a: f64| {
        let unit = |c: u8| c as f64 / 255.0;
        Value::rgba(unit(r), unit(g), unit(b), a)
    };
    out.extend([
        single(
            "color-premultiplied",
            Property::BackgroundColor,
            "background-color 1s linear",
            rgba(255, 0, 0, 1.0),
            rgba(0, 0, 255, 0.5),
            &mid,
        ),
        single(
            "color-from-transparent",
            Property::BackgroundColor,
            "background-color 1s ease",
            Value::ZERO,
            rgba(255, 0, 0, 1.0),
            &mid,
        ),
        single(
            "color-text",
            Property::Color,
            "color 1s cubic-bezier(0.32, 0.72, 0, 1)",
            rgba(17, 24, 39, 1.0),
            rgba(249, 115, 22, 1.0),
            &mid,
        ),
        single(
            "color-border-shorthand",
            Property::BorderTopColor,
            "border-color 1s ease-in-out",
            rgba(0, 128, 0, 1.0),
            rgba(255, 255, 255, 0.25),
            &mid,
        ),
        // A spring on paint is its curve from rest as `linear()` (LLP 1062 D3).
        single(
            "color-spring",
            Property::BackgroundColor,
            "background-color spring(180, 12, 1)",
            rgba(0, 0, 0, 1.0),
            rgba(255, 128, 0, 1.0),
            &[0.05, 0.1, 0.2, 0.3, 0.45],
        ),
        keyframes(
            "kf-color",
            Property::BackgroundColor,
            "k 1s ease-in-out infinite alternate @keyframes k{from{background-color:rgba(255,255,255,1)}to{background-color:rgba(10,20,200,0.5)}}",
            rgba(0, 0, 0, 1.0),
            &[0.1, 0.5, 0.9, 1.25],
        ),
    ]);
    let mut color_reversing = single(
        "color-reversing",
        Property::BackgroundColor,
        "background-color 1s ease-in-out",
        rgba(0, 0, 0, 1.0),
        rgba(255, 255, 255, 1.0),
        &[0.1],
    );
    color_reversing.steps.push(Step::Set {
        at: 0.3,
        value: rgba(0, 0, 0, 1.0),
    });
    color_reversing
        .steps
        .extend(samples(&[0.35, 0.5, 0.6, 1.0]));
    out.push(color_reversing);
    // A spring, on the grid (24/240 = 0.1 s) and between grid points.
    out.push(single(
        "spring",
        Property::Scale,
        "scale spring(180, 12, 1)",
        o(1.0),
        o(1.5),
        &[0.05, 0.1, 0.1020833333, 0.2, 0.35, 0.5, 0.8],
    ));
    let o = |v: f64| Value::scalar(v);
    out.extend([
        // The timing function eases each interval, not the iteration.
        keyframes(
            "kf-breathe",
            Property::Opacity,
            "b 1s ease-in-out infinite @keyframes b{from{opacity:0.4}50%{opacity:1}to{opacity:0.4}}",
            o(1.0),
            &[0.1, 0.25, 0.4, 0.6, 1.1, 2.3],
        ),
        // A keyframe's own easing governs the interval it starts.
        keyframes(
            "kf-keyframe-easing",
            Property::Opacity,
            "k 1s linear @keyframes k{0%{opacity:0;animation-timing-function:steps(3, jump-none)}40%{opacity:0.6;animation-timing-function:cubic-bezier(0.4, 0, 0.2, 1)}100%{opacity:1}}",
            o(1.0),
            &[0.1, 0.2, 0.3, 0.5, 0.7, 0.9],
        ),
        // Missing `from`/`to`: the underlying value; after the end with no
        // fill, the underlying value again.
        keyframes(
            "kf-implicit",
            Property::Opacity,
            "k 1s linear @keyframes k{50%{opacity:1}}",
            o(0.2),
            &[0.25, 0.5, 0.75, 1.2],
        ),
        keyframes(
            "kf-alternate-fill",
            Property::Opacity,
            "k 1s linear 0.5s 2 alternate both @keyframes k{from{opacity:0.1}to{opacity:0.9}}",
            o(1.0),
            &[0.2, 0.75, 1.25, 1.75, 3.0],
        ),
        keyframes(
            "kf-reverse-negative-delay",
            Property::Scale,
            "k 1s ease -0.25s reverse forwards @keyframes k{from{scale:0.5}to{scale:2}}",
            o(1.0),
            &[0.0, 0.25, 0.5, 0.8, 1.5],
        ),
        keyframes(
            "kf-translate",
            Property::Translate,
            "k 0.5s ease-out 1.5 alternate-reverse forwards @keyframes k{from{translate:0px 8px}to{translate:40px -8px}}",
            Value::ZERO,
            &[0.1, 0.3, 0.6, 0.7, 1.0],
        ),
        keyframes(
            "kf-rotate",
            Property::Rotate,
            "k 1s linear(0, 0.8 30%, 1) 2 @keyframes k{to{rotate:90deg}}",
            o(0.0),
            &[0.15, 0.3, 0.65, 1.15, 2.5],
        ),
        keyframes(
            "kf-zero-duration",
            Property::Opacity,
            "k 0s 3 forwards @keyframes k{from{opacity:0}to{opacity:0.5}}",
            o(1.0),
            &[0.0, 0.5],
        ),
    ]);
    // Drag timelines (LLP 1057.003): the range maps a position to progress
    // as a scroll timeline maps its offset, the delay and active interval
    // share it, and the animation's own timing (easing, fill, iterations,
    // direction) applies over it. An endless animation is no case: CSS shows
    // its end, the hosts hold its start, and the compiler refuses a literal
    // one (`lower-timeline-endless`).
    let fade = "@keyframes k{from{opacity:1}to{opacity:0}}";
    out.extend([
        timeline(
            "tl-linear",
            Property::Opacity,
            &format!("k 1s linear both {fade}"),
            o(1.0),
            [0.0, 300.0],
            &[0.0, 75.0, 150.0, 225.0, 300.0, 420.0],
        ),
        timeline(
            "tl-offset-ease",
            Property::Opacity,
            &format!("k 1s ease-in both {fade}"),
            o(1.0),
            [50.0, 250.0],
            &[0.0, 50.0, 100.0, 180.0, 250.0, 320.0],
        ),
        timeline(
            "tl-fill-none",
            Property::Opacity,
            &format!("k 1s linear {fade}"),
            o(0.5),
            [100.0, 200.0],
            &[40.0, 100.0, 150.0, 199.0, 260.0],
        ),
        timeline(
            "tl-delay",
            Property::Opacity,
            &format!("k 1s linear 1s both {fade}"),
            o(1.0),
            [0.0, 200.0],
            &[0.0, 50.0, 100.0, 150.0, 200.0],
        ),
        timeline(
            "tl-iterations",
            Property::Opacity,
            &format!("k 1s linear 2 alternate both {fade}"),
            o(1.0),
            [0.0, 200.0],
            &[0.0, 50.0, 100.0, 150.0, 200.0],
        ),
        timeline(
            "tl-translate",
            Property::Translate,
            "k 1s ease-out both @keyframes k{from{translate:0px 0px}to{translate:40px 20px}}",
            Value::ZERO,
            [0.0, 100.0],
            &[0.0, 25.0, 60.0, 100.0],
        ),
    ]);
    out
}

/// Drive the engine through a case's script; returns `(at, value)` for
/// every `Sample`.
pub fn engine_samples(case: &Case) -> Vec<(f64, Value)> {
    let mut engine = Engine::new();
    engine
        .set_transitions(1, case.transitions.clone())
        .expect("a valid case");
    let observe = |engine: &mut Engine, value: Value| {
        engine
            .observe(Change {
                node: 1,
                property: case.property,
                value,
                velocity: None,
            })
            .expect("finite");
    };
    observe(&mut engine, case.initial);
    engine
        .set_animations(1, &case.animations)
        .expect("a valid case");
    if let Some(range) = case.timeline {
        engine.set_drag_timeline(SOURCE, Some(("--t", false)));
        engine.set_animation_timeline(1, Some(("--t", range)));
    }
    let mut out = Vec::new();
    for step in &case.steps {
        match step {
            Step::Set { at, value } => {
                engine.advance(*at).expect("time moves forward");
                observe(&mut engine, *value);
            }
            Step::Sample { at } if case.timeline.is_some() => {
                engine
                    .observe(Change {
                        node: SOURCE,
                        property: Property::Translate,
                        value: Value::new(0.0, *at),
                        velocity: None,
                    })
                    .expect("finite");
                // A frame seeks the timeline, as a host's frame does.
                engine.frame();
                let value = engine.sampled_value(1, case.property);
                out.push((*at, value.expect("observed")));
            }
            Step::Sample { at } => {
                engine.advance(*at).expect("time moves forward");
                let value = engine.sampled_value(1, case.property);
                out.push((*at, value.expect("observed")));
            }
        }
    }
    out
}

/// A spring case's frames, as the glue would play them: the engine's own
/// lowering of the first `Set`.
pub fn spring_frames(case: &Case) -> Option<(f64, Vec<Value>)> {
    if !case
        .transitions
        .0
        .iter()
        .any(|t| matches!(t.timing, TimingFunction::Spring(_)))
    {
        return None;
    }
    let mut engine = Engine::new();
    engine
        .set_transitions(1, case.transitions.clone())
        .expect("a valid case");
    let mut observe = |value: Value| {
        engine
            .observe(Change {
                node: 1,
                property: case.property,
                value,
                velocity: None,
            })
            .expect("finite");
    };
    observe(case.initial);
    let Some(Step::Set { value, .. }) = case.steps.first() else {
        return None;
    };
    observe(*value);
    engine
        .spring_frames(1, case.property)
        .map(|f| (f.duration, f.values))
}

/// The cases as JSON for the page: `[{name, property, css, initial, steps,
/// frames?, duration?}]`, built by hand like the batch.
pub fn cases_json() -> String {
    let mut s = String::from("[");
    for (i, case) in cases().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let (css, _) = transition_css(&case.transitions);
        let _ = write!(
            s,
            "{{\"name\":\"{}\",\"property\":\"{}\",\"css\":\"{}\",\"initial\":{},\"steps\":[",
            case.name,
            case.property.css_name(),
            css,
            wire(case.property, case.initial)
        );
        for (j, step) in case.steps.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            match step {
                Step::Set { at, value } => {
                    let _ = write!(s, "{{\"at\":{at},\"set\":{}}}", wire(case.property, *value));
                }
                Step::Sample { at } => {
                    let _ = write!(s, "{{\"at\":{at}}}");
                }
            }
        }
        s.push(']');
        if !case.animations.0.is_empty() {
            s.push_str(",\"animation\":\"");
            let mut rules = Vec::new();
            for (i, a) in case.animations.0.iter().enumerate() {
                let mut named = a.clone();
                named.name = rule_name(case.name, a);
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&Animations(vec![named.clone()]).css());
                rules.push(format!(
                    "@keyframes {}{{{}}}",
                    named.name,
                    a.keyframes.css()
                ));
            }
            let _ = write!(s, "\",\"rules\":[\"{}\"]", rules.join("\",\""));
        }
        if let Some([start, end]) = case.timeline {
            let _ = write!(s, ",\"timeline\":[{start},{end}]");
        }
        if let Some((duration, frames)) = spring_frames(case) {
            let _ = write!(s, ",\"duration\":{},\"frames\":[", duration * 1000.0);
            for (k, v) in frames.iter().enumerate() {
                if k > 0 {
                    s.push(',');
                }
                let _ = write!(s, "[{},{}]", v.x, v.y);
            }
            s.push(']');
        }
        s.push('}');
    }
    s.push(']');
    s
}

/// A value as the page writes it: a colour straight, channels 0–255 and
/// alpha 0–1, as CSS spells it; anything else its two components.
fn wire(property: Property, value: Value) -> String {
    let [x, y, z, w] = sample_units(property, value);
    match property.is_color() {
        true => format!("[{x},{y},{z},{w}]"),
        false => format!("[{x},{y}]"),
    }
}

/// An engine value in the units a browser's computed style reads.
fn sample_units(property: Property, value: Value) -> [f64; 4] {
    match property.is_color() {
        true => {
            let [r, g, b, a] = value.straight();
            [r * 255.0, g * 255.0, b * 255.0, a]
        }
        false => value.components(),
    }
}

/// The fixture text for what a browser recorded: one `sample <case> <at>
/// <x> <y>` line per sample, in any order, after a `# recorded …` header.
/// [`check`] reads this.
pub fn fixture_header(recorder: &str) -> String {
    format!(
        "# exact motion parity fixture — the browser's samples of the cases in host/web/src/parity.rs\n# recorded by host/web/parity.mjs: {recorder}\n# `sample <case> <seconds> <x> <y>` (a `tl-` case: its source's position, px, for seconds); held by host/web/tests/it/parity.rs within {TOLERANCE}\n"
    )
}

/// Every disagreement between the engine and the fixture, as one line each;
/// empty when the engine matches the browser on every sample and no case
/// is missing from the fixture.
pub fn check(fixture: &str) -> Vec<String> {
    let mut recorded: Vec<(String, f64, Value)> = Vec::new();
    for line in fixture.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if (f.len() == 5 || f.len() == 7) && f[0] == "sample" {
            let n: Option<Vec<f64>> = f[2..].iter().map(|s| s.parse::<f64>().ok()).collect();
            if let Some(n) = n {
                let (z, w) = (
                    n.get(3).copied().unwrap_or(0.0),
                    n.get(4).copied().unwrap_or(0.0),
                );
                recorded.push((f[1].to_string(), n[0], Value::four(n[1], n[2], z, w)));
            }
        }
    }
    let mut out = Vec::new();
    for case in cases() {
        for (at, expected) in engine_samples(&case) {
            let Some((_, _, browser)) = recorded
                .iter()
                .find(|(n, t, _)| n == case.name && (t - at).abs() < 1e-9)
            else {
                out.push(format!("{}: no browser sample at {at}s", case.name));
                continue;
            };
            let expected = sample_units(case.property, expected);
            let browser = browser.components();
            let (channel, alpha) = match case.property.is_color() {
                true => COLOR_TOLERANCE,
                false => (TOLERANCE, TOLERANCE),
            };
            let off = (0..4).any(|i| {
                let band = if i == 3 { alpha } else { channel };
                (browser[i] - expected[i]).abs() > band
            });
            if off {
                out.push(format!(
                    "{}: at {at}s the browser shows {browser:?}, the engine {expected:?}",
                    case.name
                ));
            }
        }
    }
    out
}
