//! A style row as the agent prints it: CSS text, the same words the web writes.
//!
//! A length is a number of CSS pixels, `"auto"`, `"50%"`, or
//! `"env(safe-area-inset-top)"`. A colour is `"#rrggbb"` (`"#rrggbbaa"` when
//! translucent), `"light-dark(#…, #…)"`, or the CSS text of a wide or profiled
//! colour. An enum is its CSS name, a vector is `[x, y]`, and a clip path is
//! its canonical text. The engine's and the grid's rows are named, not spelled.

use super::{num, quote};
use exact_kernel::{Color, ColorValue, Dimension, Edge, RowValue, StyleId};
use std::fmt::Write as _;

pub(super) fn row_json(row: StyleId, v: RowValue<'_>, out: &mut String) {
    // CSS's unbounded maximum is `none`, as the web writes it (LLP 1102 §3.11).
    if matches!(row, StyleId::MaxWidth | StyleId::MaxHeight)
        && matches!(v, RowValue::Dimension(Dimension::Auto))
    {
        return out.push_str("\"none\"");
    }
    let hex = |c: Color| -> String {
        if c.a() == 255 {
            format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
        }
    };
    match v {
        RowValue::Dimension(Dimension::Viewport(unit, n)) => {
            quote(&format!("{}{}", num(n as f64), unit.name()), out)
        }
        RowValue::Dimension(Dimension::Auto) => out.push_str("\"auto\""),
        RowValue::Dimension(Dimension::Points(p)) => {
            let _ = write!(out, "{}", num(p as f64));
        }
        RowValue::Dimension(Dimension::Percent(p)) => quote(&format!("{}%", num(p as f64)), out),
        RowValue::Dimension(Dimension::Calc(p, plus)) => quote(
            &format!(
                "calc({}% {} {}px)",
                num(p as f64),
                if plus < 0.0 { "-" } else { "+" },
                num(plus.abs() as f64)
            ),
            out,
        ),
        RowValue::Dimension(Dimension::Env(edge, offset)) => {
            let edge = match edge {
                Edge::Top => "top",
                Edge::Right => "right",
                Edge::Bottom => "bottom",
                Edge::Left => "left",
            };
            let text = if offset == 0.0 {
                format!("env(safe-area-inset-{edge})")
            } else {
                format!(
                    "calc(env(safe-area-inset-{edge}) + {}px)",
                    num(offset as f64)
                )
            };
            quote(&text, out)
        }
        RowValue::Dimension(Dimension::Segment(var, x, y, offset)) => {
            let var = var.name();
            let text = if offset == 0.0 {
                format!("env(viewport-segment-{var} {x} {y})")
            } else {
                format!(
                    "calc(env(viewport-segment-{var} {x} {y}) {} {}px)",
                    if offset < 0.0 { "-" } else { "+" },
                    num(offset.abs() as f64)
                )
            };
            quote(&text, out)
        }
        RowValue::LineHeight(v) => match v {
            exact_kernel::LineHeight::Number(n) => {
                let _ = write!(out, "{}", num(n as f64));
            }
            _ => quote(&v.css(), out),
        },
        RowValue::Number(n) => {
            let _ = write!(out, "{}", num(n));
        }
        RowValue::Color(c) | RowValue::ColorValue(ColorValue::Fixed(c)) => quote(&hex(c), out),
        RowValue::ColorValue(ColorValue::LightDark(l, d)) => {
            quote(&format!("light-dark({}, {})", hex(l), hex(d)), out)
        }
        // @ref LLP 1095 D7 — a reference reports what it names.
        RowValue::ColorValue(ColorValue::Role(id)) => quote(
            exact_kernel::style::roles::role_of(id).map_or("transparent", |r| r.name),
            out,
        ),
        RowValue::ColorValue(ColorValue::Platform(id)) => {
            match exact_kernel::style::roles::platform(id) {
                Some(p) => quote(&p.text, out),
                None => quote("transparent", out),
            }
        }
        // @ref LLP 1100 D2 — a colour in its own space reports as CSS writes it.
        RowValue::ColorValue(ColorValue::Wide(id)) => match exact_kernel::style::wide::wide(id) {
            Some(w) => quote(&w.text, out),
            None => quote("transparent", out),
        },
        RowValue::ColorValue(c @ (ColorValue::Moving(..) | ColorValue::Profiled(_))) => {
            let mut text = String::new();
            exact_kernel::gradient::color_css(&mut text, c);
            quote(&text, out)
        }
        RowValue::Enum(name) => quote(name, out),
        RowValue::Vec2(v) => {
            let _ = write!(out, "[{},{}]", num(v.x as f64), num(v.y as f64));
        }
        RowValue::ClipPath(p) => quote(&p.css(), out),
        RowValue::AspectRatio(r) => quote(&r.css(), out),
        RowValue::DragTimeline(d) => quote(&d.css(), out),
        RowValue::AnimationTimeline(t) => quote(&t.css(), out),
        RowValue::AnimationRange(r) => quote(&r.css(), out),
        RowValue::TimelineScope(s) => quote(&s.css(), out),
        RowValue::BackgroundImage(g) | RowValue::MaskImage(g) => quote(&g.css(), out),
        RowValue::TextShadow(s) => quote(&s.css(), out),
        RowValue::BoxShadow(s) => quote(&s.css(), out),
        RowValue::CornerShape(c) => quote(&c.css(), out),
        RowValue::RotateAxis(a) => quote(&a.css(), out),
        RowValue::SymbolPalette(p) => quote(&p.css(), out),
        RowValue::ShapeOutside(p) => quote(&p.css(), out),
        RowValue::Transitions(_) => quote("(transition)", out),
        RowValue::Paint(p) => quote(&p.css(), out),
        RowValue::DashArray(d) => quote(&d.css(), out),
        RowValue::Transform(t) => quote(&t.css(), out),
        RowValue::TransformOrigin(t) => quote(&t.css(), out),
        RowValue::PaintOrder(p) => quote(&p.css(), out),
        RowValue::Marker(m) => quote(&m.css(), out),
        RowValue::Filter(f) => quote(&f.css(), out),
        RowValue::BackdropFilter(f) => quote(&f.css(), out),
        RowValue::Animations(a) => quote(&a.css(), out),
        RowValue::Color2(_) | RowValue::Tracks(_) | RowValue::Placement(_) => quote("(grid)", out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unbounded_maximum_prints_none_as_css_does() {
        let print = |row, v| {
            let mut s = String::new();
            row_json(row, v, &mut s);
            s
        };
        let auto = || RowValue::Dimension(Dimension::Auto);
        assert_eq!(print(StyleId::MaxWidth, auto()), "\"none\"");
        assert_eq!(print(StyleId::MaxHeight, auto()), "\"none\"");
        assert_eq!(print(StyleId::Width, auto()), "\"auto\"");
    }
}
