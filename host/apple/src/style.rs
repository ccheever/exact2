//! Style rows → a typed dictionary, keyed by row name.
//!
//! @ref LLP 1008 §2
//!
//! Every set row of a node becomes one entry, read through the kernel's
//! generated `StyleProps::get`: dimensions as numbers in points, `"auto"`, or
//! `{"pct": n}`; colors as `[r,g,b,a]` bytes; enums as their CSS spelling;
//! `vec2` as `[x,y]`; numbers as numbers. The four motion targets
//! (`translate`, `scale`, `rotate`, `opacity`) are left out: a presenter
//! applies their *presentation* values from `present` ops, never the style.
//! Rows a presenter cannot use yet are named, not guessed.

use exact_kernel::{Dimension, RowValue, StyleId, StyleProps};
use std::fmt::Write as _;

/// A row this host does not lower (and why).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The row.
    pub row: StyleId,
    /// Why.
    pub reason: &'static str,
}

/// The style dictionary for a node's set rows, as a JSON object, plus what
/// was skipped.
pub fn style_json(style: &StyleProps) -> (String, Vec<Skipped>) {
    let mut out = String::from("{");
    let mut skipped = Vec::new();
    let mut first = true;
    for id in style.mask.iter() {
        let name = id.name();
        let value = match style.get(id) {
            RowValue::Dimension(d) => match d {
                Dimension::Auto => "\"auto\"".to_string(),
                Dimension::Points(p) => num(p),
                Dimension::Percent(p) => format!("{{\"pct\":{}}}", num(p)),
            },
            RowValue::Color(c) => format!("[{},{},{},{}]", c.r(), c.g(), c.b(), c.a()),
            RowValue::Enum(e) => format!("\"{e}\""),
            RowValue::Vec2(v) => format!("[{},{}]", num(v.x), num(v.y)),
            RowValue::Number(n) => num(n as f32),
            RowValue::Transitions(_) => continue, // the engine's, not the presenter's
            RowValue::Color2(_) | RowValue::Tracks(_) | RowValue::Placement(_) => {
                skipped.push(Skipped {
                    row: id,
                    reason: "gradient and grid rows are not lowered in v1",
                });
                continue;
            }
        };
        if matches!(name, "translate" | "scale" | "rotate" | "opacity") {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "\"{name}\":{value}");
    }
    out.push('}');
    (out, skipped)
}

/// Shortest exact decimal for a number: `24`, not `24.0`; `0.5`.
pub fn num(n: f32) -> String {
    if n.fract() == 0.0 && n.abs() < 1e9 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}
