//! SVG 2 shapes: the paint and dash values, and the one geometry every host
//! paints.
//!
//! @ref LLP 1055 D1 (the subset), D2 (presentation properties as rows), D3
//! (geometry parsed once, in Rust)
//!
//! Every host is Rust at its boundary, so path data, points, basic shapes and
//! the `viewBox` transform are computed here and nowhere else: no host parses
//! SVG, and no two hosts can disagree about an arc.

use crate::generated::NodeType;
use crate::style::{Color, ColorValue};
use std::fmt::Write as _;

pub mod length;
mod path;
pub mod scene;
mod shape;
pub mod transform;

pub use length::{Length, Viewport};
pub use path::{parse_d, parse_points, Path, Seg};
pub use scene::{Item, Kind, Scene, Shape, Transform};
pub use shape::{circle, dash_scale, ellipse, geometry, view_box, view_box_transform, ViewBox};
pub use transform::{Affine, TransformList, TransformOrigin};

impl NodeType {
    /// An element inside an `svg`: `g` or a shape (LLP 1055 D3). Never laid
    /// out as a box; painted by its `svg` from [`geometry`].
    pub fn is_svg_element(self) -> bool {
        matches!(
            self,
            NodeType::SvgGroup
                | NodeType::SvgPath
                | NodeType::SvgPolyline
                | NodeType::SvgPolygon
                | NodeType::SvgCircle
                | NodeType::SvgLine
                | NodeType::SvgRect
                | NodeType::SvgEllipse
                | NodeType::SvgViewport
        )
    }

    /// A shape that draws a path.
    pub fn is_svg_shape(self) -> bool {
        self.is_svg_element() && !self.is_svg_container()
    }

    /// An SVG element that holds SVG elements: `svg` (root or nested) and `g`.
    pub fn is_svg_container(self) -> bool {
        matches!(
            self,
            NodeType::Svg | NodeType::SvgGroup | NodeType::SvgViewport
        )
    }

    /// The SVG element name.
    pub fn svg_tag(self) -> Option<&'static str> {
        Some(match self {
            NodeType::Svg => "svg",
            NodeType::SvgGroup => "g",
            NodeType::SvgPath => "path",
            NodeType::SvgPolyline => "polyline",
            NodeType::SvgPolygon => "polygon",
            NodeType::SvgCircle => "circle",
            NodeType::SvgLine => "line",
            NodeType::SvgRect => "rect",
            NodeType::SvgEllipse => "ellipse",
            NodeType::SvgViewport => "svg",
            _ => return None,
        })
    }
}

/// An `svg` box's natural aspect ratio, width over height, from a view box
/// with area (LLP 1055.000 D4); `None` for any other node.
pub(crate) fn natural_ratio(arena: &crate::arena::NodeArena, slot: u32) -> Option<f32> {
    if arena.node_type(slot) != NodeType::Svg {
        return None;
    }
    let vb = view_box(arena.props(slot))?;
    (vb.width > 0.0 && vb.height > 0.0).then(|| vb.width / vb.height)
}

/// SVG paint (`fill`, `stroke`): `none`, `currentcolor`, or a colour.
/// Paint servers (`url(#…)`) are refused (LLP 1055 D12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Paint {
    /// No paint.
    #[default]
    None,
    /// The element's computed `color`.
    CurrentColor,
    /// A colour, fixed or `light-dark()`.
    Color(ColorValue),
}

impl Paint {
    /// `fill`'s initial value.
    pub const BLACK: Paint = Paint::Color(ColorValue::Fixed(Color(0x0000_00ff)));

    /// CSS's grammar for the subset.
    pub fn parse(css: &str) -> Option<Paint> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") {
            return Some(Paint::None);
        }
        if t.eq_ignore_ascii_case("currentcolor") {
            return Some(Paint::CurrentColor);
        }
        if let Some(pair) = ColorValue::parse_light_dark(t) {
            return Some(Paint::Color(pair));
        }
        // Named colours other than `transparent` are refused, as on every
        // colour row (`Color::parse`).
        Color::parse(t).map(|c| Paint::Color(ColorValue::Fixed(c)))
    }

    /// The value as CSS reads it.
    pub fn css(&self) -> String {
        match self {
            Paint::None => "none".into(),
            Paint::CurrentColor => "currentcolor".into(),
            Paint::Color(ColorValue::Fixed(c)) => hex(*c),
            Paint::Color(ColorValue::LightDark(a, b)) => {
                format!("light-dark({}, {})", hex(*a), hex(*b))
            }
        }
    }

    /// The colour to paint under an appearance, with `currentcolor`
    /// resolved against `color`; `None` paints nothing.
    pub fn resolve(&self, color: ColorValue, dark: bool) -> Option<Color> {
        match self {
            Paint::None => None,
            Paint::CurrentColor => Some(color.resolve(dark)),
            Paint::Color(c) => Some(c.resolve(dark)),
        }
    }
}

fn hex(c: Color) -> String {
    let mut s = String::with_capacity(9);
    let _ = write!(s, "#{:08x}", c.0);
    s
}

/// SVG `stroke-dasharray`: `none` (empty) or non-negative lengths in user
/// units. An odd list repeats to make it even, as SVG 2 §13.5.7 says; a
/// negative value makes the whole value invalid.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DashArray(pub Vec<f32>);

impl DashArray {
    /// Most entries a list may carry.
    pub const MAX: usize = 64;

    /// CSS's grammar: `none`, or numbers (optionally `px`) separated by
    /// commas and/or whitespace.
    pub fn parse(css: &str) -> Option<DashArray> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") {
            return Some(DashArray::default());
        }
        let mut out = Vec::new();
        for part in t
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|p| !p.is_empty())
        {
            let n = part.strip_suffix("px").unwrap_or(part);
            let v = exact_num::parse_f64(n).ok()? as f32;
            if !v.is_finite() || v < 0.0 {
                return None;
            }
            out.push(v);
        }
        (!out.is_empty() && out.len() <= Self::MAX).then_some(DashArray(out))
    }

    /// The value as CSS reads it.
    pub fn css(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let parts: Vec<String> = self
            .0
            .iter()
            .map(|v| exact_num::Shortest(*v as f64).to_string())
            .collect();
        parts.join(" ")
    }

    /// The pattern a painter draws: the list repeated to even length, each
    /// entry multiplied by `scale` (the path's length over `pathLength`).
    /// Empty when there is no dashing: `none`, or a pattern that sums to 0.
    pub fn pattern(&self, scale: f32) -> Vec<f32> {
        if self.0.iter().sum::<f32>() <= 0.0 {
            return Vec::new();
        }
        let mut out: Vec<f32> = self.0.iter().map(|v| v * scale).collect();
        if out.len() % 2 == 1 {
            out.extend_from_within(..);
        }
        out
    }
}

#[cfg(test)]
mod tests;
