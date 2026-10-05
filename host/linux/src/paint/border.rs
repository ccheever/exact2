//! A box's border as the web paints it (LLP 1053 G2): each side in its own
//! colour over the area between the border box's curve and the padding
//! box's, joined at the corners where Chrome joins them. The geometry is
//! Apple's (`host/apple/Sources/ExactKit/BorderPaint.swift`), written again
//! here because the painters share no language.
//!
//! The join: each side owns the quadrilateral from its two outer corners to
//! its two inner corners, so the colour boundary at a corner is the line
//! from the border box's corner to the padding box's — at an angle set by the
//! two adjacent widths (CSS Backgrounds 3 §5.5; Chromium's
//! `BoxBorderPainter::ClipBorderSidePolygon`). Where the padding box's corner
//! is rounded, the line runs on to the chord of that inner curve so the
//! quadrilateral covers all of the corner's border. Sides that share a colour
//! are one fill, so no seam shows where they meet.

use super::{Rect4, Shape};
use exact_kernel::corner::CornerShape;

/// One step of a path, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathOp {
    /// Start a subpath.
    Move(f32, f32),
    /// A line.
    Line(f32, f32),
    /// A cubic: two controls, then the end.
    Cubic(f32, f32, f32, f32, f32, f32),
    /// Close the subpath.
    Close,
}

impl PathOp {
    /// Every coordinate the step carries.
    pub fn points(&self) -> impl Iterator<Item = f32> {
        let v = match *self {
            PathOp::Move(x, y) | PathOp::Line(x, y) => vec![x, y],
            PathOp::Cubic(a, b, c, d, e, f) => vec![a, b, c, d, e, f],
            PathOp::Close => vec![],
        };
        v.into_iter()
    }
}

/// One colour's share of a border: `region` filled even-odd, inside `clip`
/// (non-zero) when there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderFill {
    /// The area to paint.
    pub region: Vec<PathOp>,
    /// The sides' quadrilaterals, when the region is the whole ring.
    pub clip: Option<Vec<PathOp>>,
    /// Straight RGBA.
    pub color: [u8; 4],
    /// When the region is exactly one round-cornered ring: its outer and
    /// inner rects and radii, for a backend that fills a ring directly.
    pub ring: Option<Ring>,
}

/// A ring between two rounded rects: (rect, radii) outer, then inner.
pub type Ring = ((Rect4, [(f32, f32); 4]), (Rect4, [(f32, f32); 4]));

/// Circle-to-cubic control distance for a quarter arc.
const K: f32 = 0.552_284_8;

/// CSS's radius reduction: every corner scaled by the one factor that keeps
/// two neighbours from overlapping an edge. Radii are (horizontal,
/// vertical), top-left, top-right, bottom-right, bottom-left.
pub fn reduced(radii: [(f32, f32); 4], w: f32, h: f32) -> [(f32, f32); 4] {
    let sums = [
        (radii[0].0 + radii[1].0, w),
        (radii[3].0 + radii[2].0, w),
        (radii[0].1 + radii[3].1, h),
        (radii[1].1 + radii[2].1, h),
    ];
    let factor = sums
        .iter()
        .filter(|(sum, _)| *sum > 0.0)
        .fold(1.0_f32, |f, (sum, edge)| f.min(edge.max(0.0) / sum));
    radii.map(|(x, y)| ((x * factor).max(0.0), (y * factor).max(0.0)))
}

/// A shape's outline: its corners' `corner-shape` from the kernel (LLP 1077
/// D1), else [`rounded_rect`].
pub fn shape_path(out: &mut Vec<PathOp>, shape: &Shape) {
    shaped_rect(out, shape.rect, shape.radii, shape.corners.as_ref());
}

/// [`rounded_rect`] with shaped corners, when there are any.
pub fn shaped_rect(
    out: &mut Vec<PathOp>,
    rect: Rect4,
    radii: [(f32, f32); 4],
    corners: Option<&CornerShape>,
) {
    let Some(corners) =
        corners.filter(|c| !c.is_round() && radii.iter().any(|r| r.0 > 0.0 && r.1 > 0.0))
    else {
        return rounded_rect(out, rect, radii);
    };
    for seg in exact_kernel::corner::outline(rect, radii, corners).0 {
        out.push(match seg {
            exact_kernel::svg::Seg::Move(x, y) => PathOp::Move(x, y),
            exact_kernel::svg::Seg::Line(x, y) => PathOp::Line(x, y),
            exact_kernel::svg::Seg::Cubic(a, b, c, d, e, f) => PathOp::Cubic(a, b, c, d, e, f),
            exact_kernel::svg::Seg::Close => PathOp::Close,
        });
    }
}

/// A rectangle with an elliptical radius per corner, clockwise on screen.
pub fn rounded_rect(out: &mut Vec<PathOp>, (x, y, w, h): Rect4, r: [(f32, f32); 4]) {
    let [tl, tr, br, bl] = r;
    let (right, bottom) = (x + w, y + h);
    out.push(PathOp::Move(x + tl.0, y));
    out.push(PathOp::Line(right - tr.0, y));
    out.push(PathOp::Cubic(
        right - tr.0 * (1.0 - K),
        y,
        right,
        y + tr.1 * (1.0 - K),
        right,
        y + tr.1,
    ));
    out.push(PathOp::Line(right, bottom - br.1));
    out.push(PathOp::Cubic(
        right,
        bottom - br.1 * (1.0 - K),
        right - br.0 * (1.0 - K),
        bottom,
        right - br.0,
        bottom,
    ));
    out.push(PathOp::Line(x + bl.0, bottom));
    out.push(PathOp::Cubic(
        x + bl.0 * (1.0 - K),
        bottom,
        x,
        bottom - bl.1 * (1.0 - K),
        x,
        bottom - bl.1,
    ));
    out.push(PathOp::Line(x, y + tl.1));
    out.push(PathOp::Cubic(
        x,
        y + tl.1 * (1.0 - K),
        x + tl.0 * (1.0 - K),
        y,
        x + tl.0,
        y,
    ));
    out.push(PathOp::Close);
}

fn polygon(out: &mut Vec<PathOp>, points: &[(f32, f32)]) {
    for (i, &(x, y)) in points.iter().enumerate() {
        out.push(if i == 0 {
            PathOp::Move(x, y)
        } else {
            PathOp::Line(x, y)
        });
    }
    out.push(PathOp::Close);
}

/// Where the line through `a`–`b` meets the line through `c`–`d`.
fn intersection(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> Option<(f32, f32)> {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() <= 1e-9 {
        return None;
    }
    let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / den;
    Some((a.0 + r.0 * t, a.1 + r.1 * t))
}

/// A border box's paint: `rect` the border box, `radii` its authored corner
/// radii (top-left, top-right, bottom-right, bottom-left), `widths` and
/// `colors` top, right, bottom, left. One fill per colour; none for a side
/// without width or alpha.
pub fn border_fills(shape: &Shape, widths: [f32; 4], colors: [[u8; 4]; 4]) -> Vec<BorderFill> {
    let (rect, radii, corners) = (shape.rect, shape.radii, shape.corners);
    let (x, y, w, h) = rect;
    let wd = widths.map(|v| v.max(0.0));
    if w <= 0.0 || h <= 0.0 || wd.iter().all(|v| *v <= 0.0) {
        return Vec::new();
    }
    let outer = reduced(radii, w, h);
    let inner: Rect4 = (
        x + wd[3],
        y + wd[0],
        (w - wd[3] - wd[1]).max(0.0),
        (h - wd[0] - wd[2]).max(0.0),
    );
    let less = |(a, b): (f32, f32), dx: f32, dy: f32| ((a - dx).max(0.0), (b - dy).max(0.0));
    let inner_radii = reduced(
        [
            less(outer[0], wd[3], wd[0]),
            less(outer[1], wd[1], wd[0]),
            less(outer[2], wd[1], wd[2]),
            less(outer[3], wd[3], wd[2]),
        ],
        inner.2,
        inner.3,
    );
    // Visible sides, grouped by colour.
    let mut groups: Vec<([u8; 4], Vec<usize>)> = Vec::new();
    for side in (0..4).filter(|&s| wd[s] > 0.0 && colors[s][3] > 0) {
        match groups.iter_mut().find(|(c, _)| *c == colors[side]) {
            Some((_, sides)) => sides.push(side),
            None => groups.push((colors[side], vec![side])),
        }
    }
    let sided = wd.iter().filter(|v| **v > 0.0).count();
    let mut ring = Vec::new();
    shaped_rect(&mut ring, rect, outer, corners.as_ref());
    shaped_rect(&mut ring, inner, inner_radii, corners.as_ref());
    if let [(color, sides)] = groups.as_slice() {
        if sides.len() == sided {
            // One colour for every side with width: no joins to draw.
            let round = corners.as_ref().is_none_or(|c| c.is_round());
            return vec![BorderFill {
                region: ring,
                clip: None,
                color: *color,
                ring: round.then_some(((rect, outer), (inner, inner_radii))),
            }];
        }
    }
    let square = outer.iter().all(|&(a, b)| a == 0.0 && b == 0.0);
    let exact = square && w >= wd[1] + wd[3] && h >= wd[0] + wd[2];
    let quads = side_quads(rect, inner, inner_radii, if exact { 0.0 } else { 1.0 });
    groups
        .into_iter()
        .map(|(color, sides)| {
            let mut union = Vec::new();
            for side in sides {
                polygon(&mut union, &quads[side]);
            }
            // Square corners: the quadrilaterals are the border exactly.
            if exact {
                BorderFill {
                    region: union,
                    clip: None,
                    color,
                    ring: None,
                }
            } else {
                BorderFill {
                    region: ring.clone(),
                    clip: Some(union),
                    color,
                    ring: None,
                }
            }
        })
        .collect()
}

/// Each side's quadrilateral (top, right, bottom, left): outer corner, inner
/// corner, inner corner, outer corner. Each corner's join line is pushed
/// `extend` points past both ends along itself, so a clip's own
/// antialiasing never lands on the border's outer or inner edge.
fn side_quads(
    (x, y, w, h): Rect4,
    (ix, iy, iw, ih): Rect4,
    inner_radii: [(f32, f32); 4],
    extend: f32,
) -> [[(f32, f32); 4]; 4] {
    let outer = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    let inner = [(ix, iy), (ix + iw, iy), (ix + iw, iy + ih), (ix, iy + ih)];
    // Unit vectors from each inner corner along its two edges, inward.
    let along = [
        ((1.0, 0.0), (0.0, 1.0)),
        ((-1.0, 0.0), (0.0, 1.0)),
        ((-1.0, 0.0), (0.0, -1.0)),
        ((1.0, 0.0), (0.0, -1.0)),
    ];
    let joins: [((f32, f32), (f32, f32)); 4] = std::array::from_fn(|c| {
        let o = outer[c];
        let mut p = inner[c];
        let (rx, ry) = inner_radii[c];
        if rx > 0.0 || ry > 0.0 {
            // The inner curve's chord, from its end on one edge to its end on the other.
            let ((ax, ay), (bx, by)) = along[c];
            let a = (p.0 + ax * rx, p.1 + ay * rx);
            let b = (p.0 + bx * ry, p.1 + by * ry);
            if let Some(hit) = intersection(o, p, a, b) {
                p = hit;
            }
        }
        let (dx, dy) = (p.0 - o.0, p.1 - o.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 && extend > 0.0 {
            let (ux, uy) = (dx / len * extend, dy / len * extend);
            ((o.0 - ux, o.1 - uy), (p.0 + ux, p.1 + uy))
        } else {
            (o, p)
        }
    });
    std::array::from_fn(|side| {
        let (a, b) = (joins[side], joins[(side + 1) % 4]);
        [a.0, a.1, b.1, b.0]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn border_fills(
        rect: Rect4,
        radii: [(f32, f32); 4],
        widths: [f32; 4],
        colors: [[u8; 4]; 4],
    ) -> Vec<BorderFill> {
        super::border_fills(
            &Shape {
                rect,
                radii,
                corners: None,
            },
            widths,
            colors,
        )
    }

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];

    #[test]
    fn one_colour_is_one_ring_and_four_are_four_parts() {
        let one = border_fills(
            (0., 0., 100., 70.),
            [(20., 20.); 4],
            [2., 10., 2., 10.],
            [R; 4],
        );
        assert_eq!(one.len(), 1);
        assert!(one[0].clip.is_none());
        let four = border_fills((0., 0., 100., 70.), [(20., 20.); 4], [10.; 4], [R, G, R, G]);
        assert_eq!(four.len(), 2, "sides that share a colour are one fill");
        assert!(four.iter().all(|f| f.clip.is_some()));
    }

    #[test]
    fn square_corners_are_exact_trapezoids_joined_on_the_diagonal() {
        let fills = border_fills(
            (0., 0., 100., 70.),
            [(0., 0.); 4],
            [4., 12., 20., 8.],
            [R, G, R, G],
        );
        assert!(fills.iter().all(|f| f.clip.is_none()));
        // The top side's join at the top-left corner runs to the padding box's corner.
        assert_eq!(fills[0].region[0], PathOp::Move(0., 0.));
        assert_eq!(fills[0].region[1], PathOp::Line(8., 4.));
    }

    #[test]
    fn transparent_or_zero_width_sides_paint_nothing() {
        let clear = [0, 0, 0, 0];
        assert_eq!(
            border_fills(
                (0., 0., 100., 70.),
                [(12., 12.); 4],
                [10., 10., 10., 10.],
                [R, clear, G, clear]
            )
            .len(),
            2
        );
        let bar = border_fills(
            (0., 0., 100., 70.),
            [(12., 12.); 4],
            [0., 0., 0., 6.],
            [R; 4],
        );
        assert_eq!(bar.len(), 1, "a left bar alone is its own ring");
    }

    #[test]
    fn radii_reduce_by_one_factor_as_css_does() {
        let r = reduced([(60., 60.), (60., 60.), (0., 0.), (0., 0.)], 100., 200.);
        assert_eq!(r[0], (50., 50.));
        let kept = reduced([(40., 40.), (10., 10.), (35., 35.), (0., 0.)], 100., 70.);
        assert_eq!(
            kept[0],
            (40., 40.),
            "a radius over half the height is CSS when its neighbours fit"
        );
    }
}
