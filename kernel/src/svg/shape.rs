//! Basic shapes to paths, `pathLength`, and the `viewBox` transform.
//!
//! @ref LLP 1055 D1; SVG 2 §10 (basic shapes and the paths they are
//! equivalent to), §9.5.1 (`pathLength`), §8.2 (the viewBox equations)

use super::path::{parse_d, parse_points, Path, Seg};
use crate::generated::{NodeType, PropId};
use crate::props::PropList;
use crate::style::Dimension;
use crate::StyleProps;

/// The cubic control distance of a quarter circle.
const KAPPA: f32 = 0.552_284_8;

/// A shape node's path in user units, or `None` when it renders nothing
/// (`r` ≤ 0, a rect with no area, fewer than two points).
pub fn geometry(node_type: NodeType, props: &PropList, style: &StyleProps) -> Option<Path> {
    let f = |id: PropId| props.get(id).and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
    match node_type {
        NodeType::SvgPath => {
            let path = parse_d(props.str(PropId::D)?);
            (!path.0.is_empty()).then_some(path)
        }
        NodeType::SvgPolyline | NodeType::SvgPolygon => {
            let points = parse_points(props.str(PropId::Points)?);
            (points.len() >= 2 || (points.len() == 1 && node_type == NodeType::SvgPolygon))
                .then(|| Path::polyline(&points, node_type == NodeType::SvgPolygon))
        }
        NodeType::SvgLine => Some(Path(vec![
            Seg::Move(f(PropId::X1), f(PropId::Y1)),
            Seg::Line(f(PropId::X2), f(PropId::Y2)),
        ])),
        NodeType::SvgCircle => circle(style.cx, style.cy, style.r),
        NodeType::SvgRect => {
            let len = |d: Dimension| match d {
                Dimension::Points(v) => v,
                _ => 0.0,
            };
            let rx = props
                .get(PropId::Rx)
                .and_then(|v| v.as_float())
                .map(|v| v as f32);
            let ry = props
                .get(PropId::Ry)
                .and_then(|v| v.as_float())
                .map(|v| v as f32);
            rect(
                f(PropId::X),
                f(PropId::Y),
                len(style.width),
                len(style.height),
                rx,
                ry,
            )
        }
        _ => None,
    }
}

/// A circle as SVG 2 §10.3 draws it: from (cx + r, cy), toward positive y.
pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    if r.is_nan() || r <= 0.0 {
        return None;
    }
    let k = r * KAPPA;
    Some(Path(vec![
        Seg::Move(cx + r, cy),
        Seg::Cubic(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        Seg::Cubic(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        Seg::Cubic(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        Seg::Cubic(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        Seg::Close,
    ]))
}

/// A rect as SVG 2 §10.2 draws it, `auto` radii taking the other's value and
/// each clamped to half its side.
fn rect(x: f32, y: f32, w: f32, h: f32, rx: Option<f32>, ry: Option<f32>) -> Option<Path> {
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let (rx, ry) = match (rx.filter(|v| *v >= 0.0), ry.filter(|v| *v >= 0.0)) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, a),
        (None, Some(b)) => (b, b),
        (None, None) => (0.0, 0.0),
    };
    let (rx, ry) = (rx.min(w / 2.0), ry.min(h / 2.0));
    if rx <= 0.0 || ry <= 0.0 {
        return Some(Path(vec![
            Seg::Move(x, y),
            Seg::Line(x + w, y),
            Seg::Line(x + w, y + h),
            Seg::Line(x, y + h),
            Seg::Close,
        ]));
    }
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let (r, b) = (x + w, y + h);
    Some(Path(vec![
        Seg::Move(x + rx, y),
        Seg::Line(r - rx, y),
        Seg::Cubic(r - rx + kx, y, r, y + ry - ky, r, y + ry),
        Seg::Line(r, b - ry),
        Seg::Cubic(r, b - ry + ky, r - rx + kx, b, r - rx, b),
        Seg::Line(x + rx, b),
        Seg::Cubic(x + rx - kx, b, x, b - ry + ky, x, b - ry),
        Seg::Line(x, y + ry),
        Seg::Cubic(x, y + ry - ky, x + rx - kx, y, x + rx, y),
        Seg::Close,
    ]))
}

/// How much a user unit of dash or offset is along the path: the path's
/// length over its `pathLength`, or 1 without one (SVG 2 §9.5.1).
pub fn dash_scale(path: &Path, props: &PropList) -> f32 {
    match props.get(PropId::PathLength).and_then(|v| v.as_float()) {
        Some(authored) if authored > 0.0 && authored.is_finite() => {
            (path.length() / authored) as f32
        }
        _ => 1.0,
    }
}

/// A parsed `viewBox`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    /// min-x.
    pub x: f32,
    /// min-y.
    pub y: f32,
    /// width.
    pub width: f32,
    /// height.
    pub height: f32,
}

/// An `svg`'s `viewBox`: four numbers separated by whitespace and/or a
/// comma. A negative width or height invalidates it (`None`, as if absent).
pub fn view_box(props: &PropList) -> Option<ViewBox> {
    let n = parse_points(props.str(PropId::ViewBox)?);
    match n.as_slice() {
        [(x, y), (w, h)] if *w >= 0.0 && *h >= 0.0 => Some(ViewBox {
            x: *x,
            y: *y,
            width: *w,
            height: *h,
        }),
        _ => None,
    }
}

/// The affine `[a, b, c, d, e, f]` from user units to the content box of a
/// `width` × `height` viewport (SVG 2 §8.2), under `preserveAspectRatio`
/// (default `xMidYMid meet`). `None` when the view box has no area: SVG
/// then renders nothing.
pub fn view_box_transform(
    view_box: Option<ViewBox>,
    preserve: Option<&str>,
    width: f32,
    height: f32,
) -> Option<[f32; 6]> {
    let Some(vb) = view_box else {
        return Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    };
    if vb.width <= 0.0 || vb.height <= 0.0 {
        return None;
    }
    let mut words = preserve.unwrap_or("xMidYMid meet").split_whitespace();
    let align = words.next().unwrap_or("xMidYMid");
    let slice = words.next() == Some("slice");
    let (mut sx, mut sy) = (width / vb.width, height / vb.height);
    if align != "none" {
        let s = if slice { sx.max(sy) } else { sx.min(sy) };
        (sx, sy) = (s, s);
    }
    let mut tx = -vb.x * sx;
    let mut ty = -vb.y * sy;
    let (free_x, free_y) = (width - vb.width * sx, height - vb.height * sy);
    if align.contains("xMid") {
        tx += free_x / 2.0;
    } else if align.contains("xMax") {
        tx += free_x;
    }
    if align.contains("YMid") {
        ty += free_y / 2.0;
    } else if align.contains("YMax") {
        ty += free_y;
    }
    Some([sx, 0.0, 0.0, sy, tx, ty])
}
