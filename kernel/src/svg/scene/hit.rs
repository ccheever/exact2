//! Hit testing a resolved scene (LLP 1055.000 D17): the topmost element
//! under a point, by SVG 2's `pointer-events`.
//!
//! The fill is tested with the fill rule on the flattened path; the stroke
//! by distance to the flattened path, within half the stroke width. That
//! is round joins and caps: a miter's tip, a square cap's corners and dash
//! gaps are treated as stroke (declared, LLP 1055.000 §0). Clips bound hits;
//! masks, filters and opacity do not. An instance under a `use` hits as the
//! `use`, as its shadow tree retargets events.

use super::{Item, Kind, Scene, Shape};
use crate::generated::{FillRule, PointerEvents};
use crate::id::ViewId;
use crate::svg::transform::{self as tf};
use crate::svg::{Path, Seg};

impl Scene {
    /// The element under `point`, in the `svg`'s content box's space; `None`
    /// when the point hits no element (it may still hit the `svg` itself).
    pub fn hit(&self, point: (f32, f32)) -> Option<ViewId> {
        hit_items(&self.items, point, None)
    }
}

fn hit_items(items: &[Item], point: (f32, f32), owner: Option<ViewId>) -> Option<ViewId> {
    for item in items.iter().rev() {
        let Some(local) = tf::invert(item.ctm).map(|m| tf::apply(m, point)) else {
            continue;
        };
        if let Some(clip) = &item.clip {
            if !in_clip(clip, local) {
                continue;
            }
        }
        // A `use`'s instance hits as the `use`.
        let owner = if item.instance {
            owner.or(Some(item.id))
        } else {
            owner
        };
        let id = owner.unwrap_or(item.id);
        match &item.kind {
            Kind::Group(children) => {
                if let Some(hit) = hit_items(children, point, owner) {
                    return Some(hit);
                }
            }
            Kind::Viewport {
                rect,
                clip,
                children,
                ..
            } => {
                let inside = local.0 >= rect.0
                    && local.1 >= rect.1
                    && local.0 <= rect.0 + rect.2
                    && local.1 <= rect.1 + rect.3;
                if *clip && !inside {
                    continue;
                }
                if let Some(hit) = hit_items(children, point, owner) {
                    return Some(hit);
                }
            }
            Kind::Shape(shape) => {
                if shape_hit(shape, local, item.ctm, point) {
                    return Some(id);
                }
            }
            // Text hits by its runs' boxes once hosts report advances: owed.
            Kind::Text(_) => {}
        }
    }
    None
}

fn shape_hit(shape: &Shape, local: (f32, f32), ctm: tf::Affine, point: (f32, f32)) -> bool {
    use PointerEvents::*;
    let (fill_set, stroke_set) = shape.painted;
    let (fill, stroke) = match shape.pointer_events {
        None => return false,
        Auto | VisiblePainted => (shape.visible && fill_set, shape.visible && stroke_set),
        VisibleFill => (shape.visible, false),
        VisibleStroke => (false, shape.visible),
        Visible => (shape.visible, shape.visible),
        Painted => (fill_set, stroke_set),
        Fill => (true, false),
        Stroke => (false, true),
        All => (true, true),
        BoundingBox => {
            return shape.path.bounds().is_some_and(|(x, y, w, h)| {
                local.0 >= x && local.1 >= y && local.0 <= x + w && local.1 <= y + h
            })
        }
    };
    if fill && contains(&shape.path, local, shape.fill_rule == FillRule::Evenodd) {
        return true;
    }
    if stroke && shape.width > 0.0 {
        // A non-scaling stroke is measured in the content box's space.
        let (path, at) = if shape.non_scaling {
            (shape.path.transformed(ctm), point)
        } else {
            (shape.path.clone(), local)
        };
        return near(&path, at, shape.width / 2.0);
    }
    false
}

fn in_clip(clip: &super::Clip, p: (f32, f32)) -> bool {
    clip.shapes.iter().any(|s| contains(&s.path, p, s.even_odd))
        && clip.then.as_deref().is_none_or(|t| in_clip(t, p))
}

/// The path's segments as straight lines (cubics in sixteen pieces).
fn lines(path: &Path) -> Vec<((f32, f32), (f32, f32))> {
    let mut out = Vec::new();
    let (mut cur, mut start) = ((0.0, 0.0), (0.0, 0.0));
    for seg in &path.0 {
        match *seg {
            Seg::Move(x, y) => {
                cur = (x, y);
                start = cur;
            }
            Seg::Line(x, y) => {
                out.push((cur, (x, y)));
                cur = (x, y);
            }
            Seg::Cubic(x1, y1, x2, y2, x, y) => {
                let mut prev = cur;
                for i in 1..=16 {
                    let t = i as f32 / 16.0;
                    let u = 1.0 - t;
                    let p = (
                        u * u * u * cur.0
                            + 3.0 * u * u * t * x1
                            + 3.0 * u * t * t * x2
                            + t * t * t * x,
                        u * u * u * cur.1
                            + 3.0 * u * u * t * y1
                            + 3.0 * u * t * t * y2
                            + t * t * t * y,
                    );
                    out.push((prev, p));
                    prev = p;
                }
                cur = (x, y);
            }
            Seg::Close => {
                out.push((cur, start));
                cur = start;
            }
        }
    }
    out
}

/// Whether `p` is inside the path's fill: the winding number (or its
/// parity) of the implicitly closed subpaths.
fn contains(path: &Path, p: (f32, f32), even_odd: bool) -> bool {
    let mut winding = 0i32;
    let mut edges = lines(path);
    // Close every subpath for the fill, as SVG fills an open one.
    let mut start = None;
    let mut last = None;
    for seg in &path.0 {
        match *seg {
            Seg::Move(x, y) => {
                if let (Some(s), Some(l)) = (start, last) {
                    edges.push((l, s));
                }
                start = Some((x, y));
                last = Some((x, y));
            }
            Seg::Line(x, y) | Seg::Cubic(_, _, _, _, x, y) => last = Some((x, y)),
            Seg::Close => last = start,
        }
    }
    if let (Some(s), Some(l)) = (start, last) {
        edges.push((l, s));
    }
    for (a, b) in edges {
        if a.1 <= p.1 {
            if b.1 > p.1 && cross(a, b, p) > 0.0 {
                winding += 1;
            }
        } else if b.1 <= p.1 && cross(a, b, p) < 0.0 {
            winding -= 1;
        }
    }
    if even_odd {
        winding % 2 != 0
    } else {
        winding != 0
    }
}

fn cross(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1)
}

/// Whether `p` is within `radius` of the path.
fn near(path: &Path, p: (f32, f32), radius: f32) -> bool {
    lines(path).into_iter().any(|(a, b)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx * dx + dy * dy;
        let t = if len > 0.0 {
            (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (cx, cy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
        cx * cx + cy * cy <= radius * radius
    })
}
