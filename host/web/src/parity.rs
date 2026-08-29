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

use crate::css::transition_css;
use exact_motion::{Change, Engine, Property, TimingFunction, Transitions, Value};
use std::fmt::Write as _;

/// The band a browser sample may differ from the engine's by: computed
/// style serializes to about six significant digits, and cubic-bezier
/// solvers differ in their last bits.
pub const TOLERANCE: f64 = 1e-3;

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
    /// The value before the script starts (set with no transition).
    pub initial: Value,
    /// The script, in time order.
    pub steps: Vec<Step>,
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
        initial: from,
        steps,
    }
}

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
    // A spring, on the grid (24/240 = 0.1 s) and between grid points.
    out.push(single(
        "spring",
        Property::Scale,
        "scale spring(180, 12, 1)",
        o(1.0),
        o(1.5),
        &[0.05, 0.1, 0.1020833333, 0.2, 0.35, 0.5, 0.8],
    ));
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
    let mut out = Vec::new();
    for step in &case.steps {
        match step {
            Step::Set { at, value } => {
                engine.advance(*at).expect("time moves forward");
                observe(&mut engine, *value);
            }
            Step::Sample { at } => {
                engine.advance(*at).expect("time moves forward");
                out.push((*at, engine.value(1, case.property).expect("observed")));
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
            "{{\"name\":\"{}\",\"property\":\"{}\",\"css\":\"{}\",\"initial\":[{},{}],\"steps\":[",
            case.name,
            case.property.name(),
            css,
            case.initial.x,
            case.initial.y
        );
        for (j, step) in case.steps.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            match step {
                Step::Set { at, value } => {
                    let _ = write!(s, "{{\"at\":{at},\"set\":[{},{}]}}", value.x, value.y);
                }
                Step::Sample { at } => {
                    let _ = write!(s, "{{\"at\":{at}}}");
                }
            }
        }
        s.push(']');
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

/// The fixture text for what a browser recorded: one `sample <case> <at>
/// <x> <y>` line per sample, in any order, after a `# recorded …` header.
/// [`check`] reads this.
pub fn fixture_header(recorder: &str) -> String {
    format!(
        "# exact motion parity fixture — the browser's samples of the cases in host/web/src/parity.rs\n# recorded by host/web/parity.mjs: {recorder}\n# `sample <case> <seconds> <x> <y>`; held by host/web/tests/parity.rs within {TOLERANCE}\n"
    )
}

/// Every disagreement between the engine and the fixture, as one line each;
/// empty when the engine matches the browser on every sample and no case
/// is missing from the fixture.
pub fn check(fixture: &str) -> Vec<String> {
    let mut recorded: Vec<(String, f64, Value)> = Vec::new();
    for line in fixture.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() == 5 && f[0] == "sample" {
            let parse = |s: &str| s.parse::<f64>().ok();
            if let (Some(at), Some(x), Some(y)) = (parse(f[2]), parse(f[3]), parse(f[4])) {
                recorded.push((f[1].to_string(), at, Value::new(x, y)));
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
            let dx = (browser.x - expected.x).abs();
            let dy = (browser.y - expected.y).abs();
            if dx > TOLERANCE || dy > TOLERANCE {
                out.push(format!(
                    "{}: at {at}s the browser shows ({}, {}), the engine ({}, {})",
                    case.name, browser.x, browser.y, expected.x, expected.y
                ));
            }
        }
    }
    out
}
