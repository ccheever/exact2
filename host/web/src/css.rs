//! Style rows → CSS declarations.
//!
//! @ref `rules/RULES.md` §Scope (the web is the standard: a row's CSS name is
//! the row's name with `-` for `_`, and its value the CSS value; the few rows
//! that are not one CSS property each are listed here by hand)
//! @ref LLP 1002 D2 (`transition` as CSS; a spring is the one declared
//! deviation and is not emitted as CSS)
//!
//! Every set row of a node becomes one declaration, read through the
//! kernel's generated `StyleProps::get`, so a row added to `schema.json`
//! reaches the page with no change here unless it needs a unit or a name
//! this table does not know — in which case it is skipped and named in
//! [`Skipped`], never guessed.

use exact_kernel::style::ColorValue;
use exact_kernel::{Color, Dimension, RowValue, StyleId, StyleProps};
use exact_motion::{
    Easing, StepPosition, TimingFunction, Transition, TransitionProperty, Transitions,
};
use std::fmt::Write as _;

/// Rows this host knows it does not lower (and why), so an author sees a
/// reason instead of silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The row.
    pub row: StyleId,
    /// Why.
    pub reason: &'static str,
}

/// The `cssText` for a node's set rows, plus what was skipped.
pub fn css_text(style: &StyleProps, font_names: &[String]) -> (String, Vec<Skipped>) {
    let mut out = String::new();
    let mut skipped = Vec::new();
    let mut shadow: Option<(f32, f32, f32, Color, f32)> = None;
    for id in style.mask.iter() {
        let value = style.get(id);
        let name = id.name();
        match (name, &value) {
            // Rows that compose into one CSS property.
            ("shadow_offset", RowValue::Vec2(v)) => {
                let s = shadow.get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0));
                s.0 = v.x;
                s.1 = v.y;
            }
            ("shadow_radius", RowValue::Number(n)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .2 = *n as f32
            }
            // A shadow is composed into one `box-shadow` string here rather
            // than emitted as its own declaration, so its colour is resolved
            // rather than handed over: CSS has no way to say "this shadow's
            // colour is scheme-aware" inside a composed value. A pair on a
            // shadow takes its light half (LLP 1034 §5).
            ("shadow_color", RowValue::Color(c)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .3 = *c
            }
            ("shadow_color", RowValue::ColorValue(v)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .3 = v.resolve(false)
            }
            ("shadow_opacity", RowValue::Number(n)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .4 = *n as f32
            }
            ("transition", RowValue::Transitions(t)) => {
                let (text, spring_skipped) = transition_css(t);
                if !text.is_empty() {
                    let _ = write!(out, "transition:{text};");
                }
                if spring_skipped {
                    skipped.push(Skipped {
                        row: id,
                        reason: "spring transitions are lowered to keyframes by the host, not to CSS `transition`",
                    });
                }
            }
            ("font_family", RowValue::Number(index)) => {
                if let Some(family) = font_names.get(*index as usize) {
                    let value = if is_generic_family(family) {
                        family.clone()
                    } else {
                        css_string(family)
                    };
                    let _ = write!(out, "font-family:{value};");
                } else {
                    skipped.push(Skipped {
                        row: id,
                        reason: "font stack id is absent from the plan catalog",
                    });
                }
            }
            ("font_variant_numeric", _) | ("line_clamp", _) | ("tint_color", _) => {
                skipped.push(Skipped {
                    row: id,
                    reason: "not lowered in v1",
                })
            }
            ("grid_template_columns", _)
            | ("grid_template_rows", _)
            | ("grid_column", _)
            | ("grid_row", _)
            | ("grid_auto_flow", _)
            | ("justify_items", _) => skipped.push(Skipped {
                row: id,
                reason: "grid rows are not lowered in v1",
            }),
            _ => match declaration(name, &value) {
                Some((prop, val)) => {
                    let _ = write!(out, "{prop}:{val};");
                }
                None => skipped.push(Skipped {
                    row: id,
                    reason: "no CSS lowering for this row's codec",
                }),
            },
        }
    }
    if let Some((x, y, radius, color, opacity)) = shadow {
        let c = Color::rgba(
            color.r(),
            color.g(),
            color.b(),
            (color.a() as f32 * opacity.clamp(0.0, 1.0)) as u8,
        );
        let _ = write!(
            out,
            "box-shadow:{}px {}px {}px {};",
            num(x),
            num(y),
            num(radius),
            rgba(c)
        );
    }
    (out, skipped)
}

fn is_generic_family(value: &str) -> bool {
    matches!(
        value,
        "system-ui"
            | "ui-sans-serif"
            | "sans-serif"
            | "ui-serif"
            | "serif"
            | "ui-monospace"
            | "monospace"
            | "ui-rounded"
    )
}

fn css_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\a "),
            '\r' => out.push_str("\\d "),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\{:x} ", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One row → one declaration, by the CSS rule for its name and codec.
fn declaration(name: &str, value: &RowValue<'_>) -> Option<(String, String)> {
    let prop = match name {
        "text_color" => "color".to_string(),
        "position_type" => "position".to_string(),
        n if n.starts_with("border_radius_") => format!(
            "border-{}-radius",
            n.trim_start_matches("border_radius_").replace('_', "-")
        ),
        n if n.starts_with("border_width_") => {
            format!("border-{}-width", n.trim_start_matches("border_width_"))
        }
        n if n.starts_with("border_color_") => {
            format!("border-{}-color", n.trim_start_matches("border_color_"))
        }
        "backdrop_blur" => "backdrop-filter".to_string(),
        n => n.replace('_', "-"),
    };
    let val = match value {
        RowValue::Dimension(d) => dimension(*d),
        RowValue::Color(c) => rgba(*c),
        // The browser resolves this one (LLP 1034 D2): handed the function
        // it does so per element against the inherited `color-scheme`, with
        // no work of ours and no repaint pass. This is the whole reason the
        // kernel keeps the pair instead of flattening it.
        RowValue::ColorValue(ColorValue::Fixed(c)) => rgba(*c),
        RowValue::ColorValue(ColorValue::LightDark(l, d)) => {
            format!("light-dark({}, {})", rgba(*l), rgba(*d))
        }
        RowValue::Enum(e) => e.to_string(),
        RowValue::Vec2(v) => match name {
            "translate" => format!("{}px {}px", num(v.x), num(v.y)),
            _ => return None,
        },
        RowValue::Number(n) => match name {
            "flex_grow" | "flex_shrink" | "opacity" | "z_index" | "aspect_ratio"
            | "font_weight" | "scale" | "shadow_opacity" => num(*n as f32),
            "rotate" => format!("{}deg", num(*n as f32)),
            "backdrop_blur" => format!("blur({}px)", num(*n as f32)),
            _ => format!("{}px", num(*n as f32)),
        },
        RowValue::Color2(_)
        | RowValue::Tracks(_)
        | RowValue::Placement(_)
        | RowValue::Transitions(_) => return None,
    };
    Some((prop, val))
}

/// A `transition` row as CSS; `true` when a spring was left out.
pub fn transition_css(t: &Transitions) -> (String, bool) {
    let mut parts = Vec::new();
    let mut spring = false;
    for tr in &t.0 {
        match &tr.timing {
            TimingFunction::Spring(_) => spring = true,
            TimingFunction::Easing(e) => parts.push(format!(
                "{} {}s {} {}s",
                transition_property(tr),
                num(tr.duration as f32),
                easing_css(e),
                num(tr.delay as f32)
            )),
        }
    }
    (parts.join(","), spring)
}

fn transition_property(tr: &Transition) -> &'static str {
    match tr.property {
        TransitionProperty::All => "all",
        TransitionProperty::Property(p) => p.name(),
    }
}

/// A CSS `<easing-function>` from the motion crate's spelling.
pub fn easing_css(e: &Easing) -> String {
    match e {
        Easing::Linear => "linear".into(),
        Easing::Ease => "ease".into(),
        Easing::EaseIn => "ease-in".into(),
        Easing::EaseOut => "ease-out".into(),
        Easing::EaseInOut => "ease-in-out".into(),
        Easing::CubicBezier { x1, y1, x2, y2 } => format!(
            "cubic-bezier({},{},{},{})",
            num(*x1 as f32),
            num(*y1 as f32),
            num(*x2 as f32),
            num(*y2 as f32)
        ),
        Easing::Steps { count, position } => format!(
            "steps({count},{})",
            match position {
                StepPosition::JumpStart => "jump-start",
                StepPosition::JumpEnd => "jump-end",
                StepPosition::JumpNone => "jump-none",
                StepPosition::JumpBoth => "jump-both",
            }
        ),
        Easing::PiecewiseLinear(stops) => format!(
            "linear({})",
            stops
                .iter()
                .map(|s| format!(
                    "{} {}%",
                    num(s.output as f32),
                    num((s.input * 100.0) as f32)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn dimension(d: Dimension) -> String {
    match d {
        Dimension::Auto => "auto".into(),
        Dimension::Points(p) => format!("{}px", num(p)),
        Dimension::Percent(p) => format!("{}%", num(p)),
        // The browser resolves the inset itself (under `viewport-fit=cover`,
        // which the glue sets from the root's prop; zero otherwise).
        Dimension::Env(edge, plus) => {
            if plus == 0.0 {
                format!("env(safe-area-inset-{})", edge.name())
            } else {
                format!(
                    "calc(env(safe-area-inset-{}) {} {}px)",
                    edge.name(),
                    if plus < 0.0 { "-" } else { "+" },
                    num(plus.abs())
                )
            }
        }
    }
}

/// `rgba(r,g,b,a)` with the alpha as a fraction.
pub fn rgba(c: Color) -> String {
    format!(
        "rgba({},{},{},{})",
        c.r(),
        c.g(),
        c.b(),
        num(c.a() as f32 / 255.0)
    )
}

/// Shortest exact decimal for a number: `24`, not `24.0`; `0.5`; `1.2`.
pub fn num(n: f32) -> String {
    if n.fract() == 0.0 && n.abs() < 1e9 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n}");
        s
    }
}
