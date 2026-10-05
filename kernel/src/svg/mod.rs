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
use crate::gradient::ColorText;
use crate::style::{Color, ColorValue};

pub mod filter;
pub mod length;
mod path;
pub mod refs;
pub mod scene;
pub mod server;
mod shape;
pub mod transform;

pub use length::{Length, Viewport};
pub use path::{parse_d, parse_d_whole, parse_points, Path, Seg};
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
                | NodeType::SvgDefs
                | NodeType::SvgLinearGradient
                | NodeType::SvgRadialGradient
                | NodeType::SvgStop
                | NodeType::SvgUse
                | NodeType::SvgSymbol
                | NodeType::SvgClipPath
                | NodeType::SvgText
                | NodeType::SvgTSpan
                | NodeType::SvgMarker
                | NodeType::SvgMask
                | NodeType::SvgPattern
                | NodeType::SvgForeignObject
                | NodeType::SvgFilter
                | NodeType::SvgFe
        )
    }

    /// A shape that draws a path.
    pub fn is_svg_shape(self) -> bool {
        matches!(
            self,
            NodeType::SvgPath
                | NodeType::SvgPolyline
                | NodeType::SvgPolygon
                | NodeType::SvgCircle
                | NodeType::SvgLine
                | NodeType::SvgRect
                | NodeType::SvgEllipse
        )
    }

    /// Whether the element renders where it stands: definitions (`defs`,
    /// gradients, `stop`, `symbol`) render only through a reference, and a
    /// `foreignObject` only on the web (LLP 1055.000 D13): a native host's
    /// scene has none.
    pub fn renders(self) -> bool {
        !matches!(
            self,
            NodeType::SvgDefs
                | NodeType::SvgLinearGradient
                | NodeType::SvgRadialGradient
                | NodeType::SvgStop
                | NodeType::SvgSymbol
                | NodeType::SvgClipPath
                | NodeType::SvgTSpan
                | NodeType::SvgMarker
                | NodeType::SvgMask
                | NodeType::SvgPattern
                | NodeType::SvgForeignObject
                | NodeType::SvgFilter
                | NodeType::SvgFe
        )
    }

    /// An SVG element that holds SVG elements: `svg` (root or nested) and `g`.
    pub fn is_svg_container(self) -> bool {
        matches!(
            self,
            NodeType::Svg
                | NodeType::SvgGroup
                | NodeType::SvgViewport
                | NodeType::SvgDefs
                | NodeType::SvgSymbol
                | NodeType::SvgLinearGradient
                | NodeType::SvgRadialGradient
                | NodeType::SvgClipPath
                | NodeType::SvgText
                | NodeType::SvgTSpan
                | NodeType::SvgMarker
                | NodeType::SvgMask
                | NodeType::SvgPattern
                | NodeType::SvgFilter
                | NodeType::SvgFe
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
            NodeType::SvgDefs => "defs",
            NodeType::SvgLinearGradient => "linearGradient",
            NodeType::SvgRadialGradient => "radialGradient",
            NodeType::SvgStop => "stop",
            NodeType::SvgUse => "use",
            NodeType::SvgSymbol => "symbol",
            NodeType::SvgClipPath => "clipPath",
            NodeType::SvgText => "text",
            NodeType::SvgTSpan => "tspan",
            NodeType::SvgMarker => "marker",
            NodeType::SvgMask => "mask",
            NodeType::SvgPattern => "pattern",
            NodeType::SvgForeignObject => "foreignObject",
            NodeType::SvgFilter => "filter",
            NodeType::SvgFe => "fe",
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

/// SVG paint (`fill`, `stroke`, `stop-color`): `none`, `currentcolor`, a
/// colour, or a paint server `url(#id)` with an optional fallback (LLP
/// 1055.000 D3, D7). Only same-document references: an external URL is
/// refused.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Paint {
    /// No paint.
    #[default]
    None,
    /// The element's computed `color`.
    CurrentColor,
    /// A colour, fixed or `light-dark()`.
    Color(ColorValue),
    /// A paint server by id, and what paints when it does not resolve.
    Url(Box<str>, PaintFallback),
}

/// What a `url()` paint falls back to when its server is missing or of the
/// wrong type (SVG 2 §13.2): `none` unless the author gave one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaintFallback {
    /// None given: nothing paints.
    #[default]
    Default,
    /// `none`.
    None,
    /// `currentcolor`.
    CurrentColor,
    /// A colour.
    Color(ColorValue),
}

impl Paint {
    /// `fill`'s initial value.
    pub const BLACK: Paint = Paint::Color(ColorValue::Fixed(Color(0x0000_00ff)));
    /// `lighting-color`'s initial value.
    pub const WHITE: Paint = Paint::Color(ColorValue::Fixed(Color(0xffff_ffff)));

    /// CSS's grammar for the subset.
    pub fn parse(css: &str) -> Option<Paint> {
        let t = css.trim();
        if t.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("url(")) {
            let close = t.find(')')?;
            let inner = t[4..close].trim().trim_matches(|c| c == '"' || c == '\'');
            let id = inner.strip_prefix('#').filter(|id| !id.is_empty())?;
            let fallback = match Paint::parse(&t[close + 1..]) {
                _ if t[close + 1..].trim().is_empty() => PaintFallback::Default,
                Some(Paint::None) => PaintFallback::None,
                Some(Paint::CurrentColor) => PaintFallback::CurrentColor,
                Some(Paint::Color(c)) => PaintFallback::Color(c),
                _ => return None,
            };
            return Some(Paint::Url(id.into(), fallback));
        }
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
        self.text(ColorText::Css)
    }

    /// The wire form: [`Self::css`], with every reference kept (LLP 1095 D1).
    pub fn wire(&self) -> String {
        self.text(ColorText::Wire)
    }

    fn text(&self, mode: ColorText) -> String {
        match self {
            Paint::None => "none".into(),
            Paint::CurrentColor => "currentcolor".into(),
            Paint::Color(c) => {
                let mut out = String::new();
                crate::gradient::color_text(&mut out, *c, mode);
                out
            }
            Paint::Url(id, fallback) => {
                let tail = match fallback {
                    PaintFallback::Default => String::new(),
                    PaintFallback::None => " none".into(),
                    PaintFallback::CurrentColor => " currentcolor".into(),
                    PaintFallback::Color(c) => format!(" {}", Paint::Color(*c).text(mode)),
                };
                format!("url(#{id}){tail}")
            }
        }
    }

    /// The paint a `url()` falls back to, or `None` when nothing paints.
    pub fn fallback(&self) -> Option<Paint> {
        match self {
            Paint::Url(_, PaintFallback::CurrentColor) => Some(Paint::CurrentColor),
            Paint::Url(_, PaintFallback::Color(c)) => Some(Paint::Color(*c)),
            Paint::Url(..) => None,
            other => Some(other.clone()),
        }
    }

    /// The colour to paint under an appearance, with `currentcolor`
    /// resolved against `color`; `None` paints nothing.
    pub fn resolve(&self, color: ColorValue, dark: bool) -> Option<Color> {
        match self {
            Paint::None | Paint::Url(..) => None,
            Paint::CurrentColor => Some(color.resolve(dark)),
            Paint::Color(c) => Some(c.resolve(dark)),
        }
    }
}

/// SVG 2 `paint-order`: the order fill (0), stroke (1) and markers (2)
/// paint in; `normal` is `[0, 1, 2]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintOrder(pub [u8; 3]);

impl Default for PaintOrder {
    fn default() -> Self {
        PaintOrder([0, 1, 2])
    }
}

impl PaintOrder {
    /// `normal`, or up to three distinct keywords; the rest follow in
    /// their normal order.
    pub fn parse(css: &str) -> Option<PaintOrder> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("normal") {
            return Some(PaintOrder::default());
        }
        let mut order: Vec<u8> = Vec::new();
        for w in t.split_whitespace() {
            let v = match w.to_ascii_lowercase().as_str() {
                "fill" => 0,
                "stroke" => 1,
                "markers" => 2,
                _ => return None,
            };
            if order.contains(&v) {
                return None;
            }
            order.push(v);
        }
        if order.is_empty() {
            return None;
        }
        for v in 0..3 {
            if !order.contains(&v) {
                order.push(v);
            }
        }
        Some(PaintOrder([order[0], order[1], order[2]]))
    }

    /// The value as CSS reads it.
    pub fn css(&self) -> String {
        if *self == PaintOrder::default() {
            return "normal".into();
        }
        self.0
            .iter()
            .map(|v| ["fill", "stroke", "markers"][*v as usize])
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// SVG `marker-start`, `marker-mid` and `marker-end`: `none`, or a
/// `marker` by `url(#id)` (LLP 1055.000 D9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkerRef(pub Option<Box<str>>);

impl MarkerRef {
    /// `none` or `url(#id)`.
    pub fn parse(css: &str) -> Option<MarkerRef> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") {
            return Some(MarkerRef(None));
        }
        let inner = t
            .get(..4)
            .filter(|p| p.eq_ignore_ascii_case("url("))
            .and_then(|_| t[4..].strip_suffix(')'))?;
        let id = inner.trim().trim_matches(|c| c == '"' || c == '\'');
        let id = id.strip_prefix('#').filter(|id| !id.is_empty())?;
        Some(MarkerRef(Some(id.into())))
    }

    /// The value as CSS reads it.
    pub fn css(&self) -> String {
        match &self.0 {
            Some(id) => format!("url(#{id})"),
            None => "none".into(),
        }
    }

    /// The id it names.
    pub fn url(&self) -> Option<&str> {
        self.0.as_deref()
    }
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
