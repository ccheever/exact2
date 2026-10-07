//! Paint servers: `linearGradient` and `radialGradient`, resolved for one
//! shape into stops, a spread and one transform to the shape's user space.
//!
//! @ref LLP 1055.000 D3 (references), D7 (servers: defaults, `href`
//! templates, object bounding boxes); SVG 2 §14.2 (gradients), §14.2.3
//! (template inheritance)

use super::length::{Length, Viewport};
use super::transform::{self as tf, Affine};
use super::{Paint, TransformList};
use crate::generated::{NodeType, PropId, StyleId, StyleMask, StyleProps};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};
use crate::style::{ColorValue, Dimension};

/// A resolved gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    /// Linear or radial, in gradient units.
    pub kind: ServerKind,
    /// Stops: offsets in [0, 1], non-decreasing.
    pub stops: Vec<Stop>,
    /// `spreadMethod`.
    pub spread: Spread,
    /// Gradient space to the shape's user space: the bounding box's
    /// mapping (for `objectBoundingBox`) times `gradientTransform`.
    pub transform: Affine,
}

/// The gradient's geometry, in its own space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerKind {
    /// From `(x1, y1)` to `(x2, y2)`.
    Linear {
        /// Start.
        x1: f32,
        /// Start.
        y1: f32,
        /// End.
        x2: f32,
        /// End.
        y2: f32,
    },
    /// From the focal circle `(fx, fy, fr)` to the end circle `(cx, cy, r)`.
    Radial {
        /// End circle.
        cx: f32,
        /// End circle.
        cy: f32,
        /// End circle.
        r: f32,
        /// Focal circle.
        fx: f32,
        /// Focal circle.
        fy: f32,
        /// Focal circle.
        fr: f32,
    },
}

/// One stop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// Offset in [0, 1].
    pub offset: f32,
    /// `stop-color`, `currentcolor` resolved in the stop's own tree.
    pub color: ColorValue,
    /// `stop-opacity`.
    pub opacity: f32,
}

/// `spreadMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spread {
    /// Extend the end colours.
    Pad,
    /// Mirror.
    Reflect,
    /// Repeat.
    Repeat,
}

/// What a `url()` paint resolved to.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    /// A gradient.
    Server(Server),
    /// One colour (a gradient with one stop).
    Color(ColorValue, f32),
    /// Nothing paints: no stops, or a degenerate bounding box.
    Nothing,
    /// The reference did not name a paint server: use the fallback.
    Fallback,
}

/// Resolve `url(#id)` for the shape `from` whose fill box is `bbox`.
pub fn resolve(
    kernel: &Kernel,
    from: ViewId,
    id: &str,
    bbox: Option<(f32, f32, f32, f32)>,
    vp: Viewport,
) -> Resolved {
    let Some(target) = kernel.resolve_id(from, id).and_then(|t| kernel.node(t)) else {
        return Resolved::Fallback;
    };
    if !matches!(
        target.node_type,
        NodeType::SvgLinearGradient | NodeType::SvgRadialGradient
    ) {
        return Resolved::Fallback;
    }
    // The `href` chain, this gradient first (SVG 2 §14.2.3); a cycle ends
    // it, keeping what was gathered.
    let mut chain: Vec<NodeRef<'_>> = vec![target];
    loop {
        let last = chain.last().expect("the target");
        let next = last
            .props
            .str(PropId::Href)
            .and_then(|h| h.strip_prefix('#'))
            .and_then(|h| kernel.resolve_id(last.id, h))
            .and_then(|t| kernel.node(t))
            .filter(|t| {
                matches!(
                    t.node_type,
                    NodeType::SvgLinearGradient | NodeType::SvgRadialGradient
                )
            });
        match next {
            Some(n) if !chain.iter().any(|c| c.id == n.id) && chain.len() < 16 => chain.push(n),
            _ => break,
        }
    }
    let prop = |p: PropId| chain.iter().find_map(|n| n.props.str(p));
    let row = |s: StyleId| chain.iter().find(|n| n.style.mask.has(s));
    let obb = prop(PropId::GradientUnits) != Some("userSpaceOnUse");
    let stops = stops(kernel, &chain);
    match stops.as_slice() {
        [] => return Resolved::Nothing,
        [one] => return Resolved::Color(one.color, one.opacity),
        _ => {}
    }
    let (base, w, h) = if obb {
        match bbox {
            Some((x, y, w, h)) if w > 0.0 && h > 0.0 => ([w, 0.0, 0.0, h, x, y], 1.0, 1.0),
            // SVG 2: a server on a box with no width or height paints nothing.
            _ => return Resolved::Nothing,
        }
    } else {
        (tf::IDENTITY, vp.width, vp.height)
    };
    // A coordinate: a fraction of the box under objectBoundingBox (a
    // number or a percentage), else a length against the viewport.
    let coord = |text: Option<&str>, default: f32, basis: f32| -> f32 {
        match text.and_then(Length::parse) {
            Some(Length::Units(v)) => v,
            Some(Length::Percent(p)) => p / 100.0 * basis,
            None => default * basis,
        }
    };
    let dim = |d: Option<Dimension>, default: f32, basis: f32| match d {
        Some(Dimension::Points(v)) => v,
        Some(Dimension::Percent(p)) => p / 100.0 * basis,
        _ => default * basis,
    };
    let diag = if obb {
        1.0
    } else {
        ((w * w + h * h) / 2.0).sqrt()
    };
    let kind = if chain[0].node_type == NodeType::SvgLinearGradient {
        ServerKind::Linear {
            x1: coord(prop(PropId::X1), 0.0, w),
            y1: coord(prop(PropId::Y1), 0.0, h),
            x2: coord(prop(PropId::X2), 1.0, w),
            y2: coord(prop(PropId::Y2), 0.0, h),
        }
    } else {
        let cx = dim(row(StyleId::Cx).map(|n| n.style.cx), 0.5, w);
        let cy = dim(row(StyleId::Cy).map(|n| n.style.cy), 0.5, h);
        ServerKind::Radial {
            cx,
            cy,
            r: dim(row(StyleId::R).map(|n| n.style.r), 0.5, diag),
            fx: prop(PropId::Fx).map_or(cx, |t| coord(Some(t), 0.0, w)),
            fy: prop(PropId::Fy).map_or(cy, |t| coord(Some(t), 0.0, h)),
            fr: coord(prop(PropId::Fr), 0.0, diag),
        }
    };
    let gt = prop(PropId::GradientTransform)
        .and_then(TransformList::parse)
        .map_or(tf::IDENTITY, |t| t.matrix());
    let spread = match prop(PropId::SpreadMethod) {
        Some("reflect") => Spread::Reflect,
        Some("repeat") => Spread::Repeat,
        _ => Spread::Pad,
    };
    Resolved::Server(Server {
        kind,
        stops,
        spread,
        transform: tf::mul(base, gt),
    })
}

/// The first gradient in the chain with `stop` children gives the stops
/// (a gradient's own stops replace its template's). Offsets are numbers or
/// percentages, clamped to [0, 1] and made non-decreasing.
fn stops(kernel: &Kernel, chain: &[NodeRef<'_>]) -> Vec<Stop> {
    for g in chain {
        let stops: Vec<NodeRef<'_>> = g
            .children()
            .into_iter()
            .filter_map(|c| kernel.node(c))
            .filter(|c| c.node_type == NodeType::SvgStop)
            .collect();
        if stops.is_empty() {
            continue;
        }
        let mut last = 0.0f32;
        return stops
            .iter()
            .map(|s| {
                let offset = match s.props.str(PropId::Offset).and_then(Length::parse) {
                    Some(Length::Units(v)) => v,
                    Some(Length::Percent(p)) => p / 100.0,
                    None => 0.0,
                };
                last = offset.clamp(0.0, 1.0).max(last);
                let style: StyleProps = s.computed_style(StyleMask::INHERITED);
                let color = match &style.rare.stop_color {
                    Paint::CurrentColor => style.text_color,
                    Paint::Color(c) => *c,
                    _ => ColorValue::Fixed(crate::style::Color(0x0000_00ff)),
                };
                Stop {
                    offset: last,
                    color,
                    opacity: style.rare.stop_opacity.clamp(0.0, 1.0),
                }
            })
            .collect();
    }
    Vec::new()
}
