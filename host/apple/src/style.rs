//! Style rows → a typed dictionary, keyed by row name.
//!
//! @ref LLP 1008 §2
//!
//! Every set row of a node becomes one entry, read through the kernel's
//! generated `StyleProps::get`: dimensions as numbers in points (an `env()`
//! length resolved against the kernel's environment — the presenter sees
//! points, and a change of the insets re-sends the dictionary), `"auto"`, or
//! `{"pct": n}` (`{"pct": n, "px": m}` for a `calc()` of both); colors as
//! `[r,g,b,a]` bytes; enums as their CSS spelling;
//! `vec2` as `[x,y]`; numbers as numbers. The four motion targets
//! (`translate`, `scale`, `rotate`, `opacity`) are left out: a presenter
//! applies their *presentation* values from `present` ops, never the style.
//! Rows a presenter cannot use yet are named, not guessed.

use exact_kernel::style::ColorValue;
use exact_kernel::{
    Dimension, Env, NodeRef, NodeType, Overflow, RowValue, StyleId, StyleMask, StyleProps,
    StyleValue,
};
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
pub fn style_json(style: &StyleProps, env: &Env) -> (String, Vec<Skipped>) {
    let mut out = String::from("{");
    let mut skipped = Vec::new();
    let mut first = true;
    for id in style.mask.iter() {
        let name = id.name();
        // LLP 1043.000 M3: the presenter will use resolved shapes.
        if matches!(id, StyleId::WrapFlow | StyleId::ShapeMargin) {
            continue;
        }
        let value = match style.get(id) {
            RowValue::ShapeOutside(_) => continue, // LLP 1043.000 M3
            // Layout only (LLP 1053 G1): the kernel sizes the box.
            RowValue::AspectRatio(_) => continue,
            RowValue::Dimension(d) => match d.resolve(env) {
                Dimension::Auto => "\"auto\"".to_string(),
                Dimension::Points(p) => num(p),
                Dimension::Percent(p) => format!("{{\"pct\":{}}}", num(p)),
                Dimension::Calc(p, x) => {
                    format!("{{\"pct\":{},\"px\":{}}}", num(p), num(x))
                }
                Dimension::Env(..) => unreachable!("resolved"),
            },
            RowValue::Color(c) => format!("[{},{},{},{}]", c.r(), c.g(), c.b(), c.a()),
            // A colour a row holds (LLP 1034 D1/D2). A fixed one crosses as
            // the four channels it always did; a `light-dark()` pair crosses
            // as both, because the presenter resolves it against the
            // *owning view's* appearance and must re-resolve when that
            // changes. Flattening here would be the kernel choosing, which
            // is exactly what D2 forbids.
            RowValue::ColorValue(ColorValue::Fixed(c)) => {
                format!("[{},{},{},{}]", c.r(), c.g(), c.b(), c.a())
            }
            RowValue::ColorValue(ColorValue::LightDark(l, d)) => format!(
                "[[{},{},{},{}],[{},{},{},{}]]",
                l.r(),
                l.g(),
                l.b(),
                l.a(),
                d.r(),
                d.g(),
                d.b(),
                d.a()
            ),
            RowValue::ClipPath(p) => {
                let commands: Vec<_> = p
                    .commands()
                    .iter()
                    .map(|(command, values)| {
                        let values = values.iter().map(|n| num(*n)).collect::<Vec<_>>().join(",");
                        format!("[\"{command}\",[{values}]]")
                    })
                    .collect();
                format!("[{}]", commands.join(","))
            }
            RowValue::Enum(e) => format!("\"{e}\""),
            RowValue::Vec2(v) => format!("[{},{}]", num(v.x), num(v.y)),
            RowValue::LineHeight(v) => match v {
                exact_kernel::LineHeight::Number(n) => num(n),
                _ => format!("\"{}\"", v.css()),
            },
            RowValue::Number(n) => num(n as f32),
            RowValue::Transitions(_) => continue, // the engine's, not the presenter's
            // @ref LLP 1055 D4/D7 — the `svg` scene and CA specs carry these.
            RowValue::Paint(_)
            | RowValue::DashArray(_)
            | RowValue::Transform(_)
            | RowValue::TransformOrigin(_)
            | RowValue::Animations(_) => continue,
            RowValue::Color2(_) | RowValue::Tracks(_) | RowValue::Placement(_) => {
                skipped.push(Skipped {
                    row: id,
                    reason: "grid rows are not lowered in v1",
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

/// A node's effective overflow per axis — the kernel's own rule
/// (`StyleProps::to_taffy`): a `ScrollView`/`List` scrolls on y unless its
/// row says otherwise, and an unset x follows a non-visible y (CSS Overflow
/// §3). The presenter scrolls and clips from these, never from the node
/// type.
pub fn effective_overflow(node: &NodeRef<'_>) -> (Overflow, Overflow) {
    let s = node.style;
    let y = if s.mask.has(StyleId::OverflowY) {
        s.overflow_y
    } else if node.node_type.scrolls_by_default() {
        Overflow::Scroll
    } else {
        Overflow::Visible
    };
    let mut x = if s.mask.has(StyleId::OverflowX) {
        s.overflow_x
    } else {
        Overflow::Visible
    };
    let mut y = y;
    // Symmetric, as the kernel computes: a `visible` axis beside a
    // non-visible one is scrollable (CSS's `auto`; the schema has no `auto`).
    if x == Overflow::Visible && y != Overflow::Visible {
        x = Overflow::Scroll;
    } else if y == Overflow::Visible && x != Overflow::Visible {
        y = Overflow::Scroll;
    }
    (x, y)
}

/// The style dictionary with CSS inheritance resolved and effective
/// overflow: derived values that must reach the presenter even when no row
/// is set. A text node or an editor gets every inherited row's computed
/// value — the font rows, alignment and colour it measures and paints with,
/// as a `<span>` in a `<div>` would — and any other node its computed colour.
/// A row resolved to its initial value stays out (the presenter carries
/// CSS's defaults), except colour, which always crosses. The kernel touches
/// the descendants an inherited change reaches (LLP 1035.000 D4), so this is
/// re-sent by the ordinary update path, never re-derived per frame.
pub fn style_json_for(node: &NodeRef<'_>, env: &Env) -> (String, Vec<Skipped>) {
    let rows = if matches!(
        node.node_type,
        NodeType::Text | NodeType::TextInput | NodeType::Image
    ) {
        StyleMask::INHERITED
    } else {
        StyleMask::of(StyleId::TextColor)
    };
    let mut computed = node.computed_style(rows);
    computed.mask.set(StyleId::TextColor);
    // The presenter must inset editors/images and paint the same border area
    // that the kernel laid out. Authored widths survive separately in the node.
    let widths = computed.border_widths();
    for (id, width) in [
        StyleId::BorderWidthTop,
        StyleId::BorderWidthRight,
        StyleId::BorderWidthBottom,
        StyleId::BorderWidthLeft,
    ]
    .into_iter()
    .zip(widths)
    {
        if width != 0.0 || computed.mask.has(id) {
            computed
                .set_dynamic(id, &StyleValue::Number(width as f64))
                .unwrap();
        }
    }
    if widths.iter().any(|width| *width > 0.0) {
        let [top, right, bottom, left] = computed.border_colors(computed.text_color);
        computed.border_color_top = Some(top);
        computed.border_color_right = Some(right);
        computed.border_color_bottom = Some(bottom);
        computed.border_color_left = Some(left);
        for id in [
            StyleId::BorderColorTop,
            StyleId::BorderColorRight,
            StyleId::BorderColorBottom,
            StyleId::BorderColorLeft,
        ] {
            computed.mask.set(id);
        }
    }
    let (mut json, skipped) = style_json(&computed, env);
    let (x, y) = effective_overflow(node);
    let name = |o: Overflow| match o {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
    };
    if x != Overflow::Visible || y != Overflow::Visible {
        let head = format!(
            "{{\"overflow_x\":\"{}\",\"overflow_y\":\"{}\"",
            name(x),
            name(y)
        );
        json = if json == "{}" {
            head + "}"
        } else {
            head + "," + &json[1..]
        };
    }
    (json, skipped)
}

/// Shortest exact decimal for a number: `24`, not `24.0`; `0.5`.
pub fn num(n: f32) -> String {
    if n.fract() == 0.0 && n.abs() < 1e9 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    #[test]
    fn exclusion_rows_wait_for_resolved_shape_batches() {
        let mut s = StyleProps::default();
        for (id, value) in [
            (StyleId::WrapFlow, "both"),
            (StyleId::ShapeOutside, "circle()"),
            (StyleId::ShapeMargin, "8px"),
        ] {
            s.set_dynamic(id, &StyleValue::Text(value.into())).unwrap();
        }
        assert_eq!(style_json(&s, &Env::default()), ("{}".into(), vec![]));
    }
}
