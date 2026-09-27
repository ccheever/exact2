//! Markers (LLP 1055.000 D9; SVG 2 §11.6): the `marker`s a shape's
//! `marker-start`, `marker-mid` and `marker-end` name, instanced at its
//! vertices.
//!
//! A vertex is the end of each authored path command (an arc drawn as
//! several cubics is one), the start of each subpath, and for a closed
//! subpath the point `Z` returns to. Its direction bisects the incoming and
//! outgoing tangents; a closed subpath's start takes the closing segment as
//! its incoming one. Markers apply to `path`, `line`, `polyline` and
//! `polygon`, as Chrome draws them (SVG 2 would add the other shapes).
//!
//! A marker's content inherits from the marker's own ancestors, not from
//! the shape; it sits in a viewport of `markerWidth` × `markerHeight`
//! (scaled by `stroke-width` under `markerUnits="strokeWidth"`), mapped by
//! its view box, with `refX`/`refY` on the vertex.

use super::{Item, Kind, Resolver, Transform, Viewport};
use crate::generated::{NodeType, Overflow, PropId, StyleMask, StyleProps};
use crate::kernel::NodeRef;
use crate::svg::length::{self, Length};
use crate::svg::path::{parse_d_commands, Path, Seg};
use crate::svg::transform::{self as tf};
use crate::svg::{view_box, view_box_transform};

/// A vertex: where a marker sits and the directions (degrees) in and out.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vertex {
    at: (f32, f32),
    into: Option<f32>,
    out: Option<f32>,
}

impl Vertex {
    /// The bisecting direction, or whichever one there is.
    fn angle(&self) -> f32 {
        match (self.into, self.out) {
            (Some(a), Some(b)) => {
                let mut d = b - a;
                while d > 180.0 {
                    d -= 360.0;
                }
                while d <= -180.0 {
                    d += 360.0;
                }
                a + d / 2.0
            }
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => 0.0,
        }
    }
}

fn direction(from: (f32, f32), to: (f32, f32)) -> Option<f32> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    (dx != 0.0 || dy != 0.0).then(|| dy.atan2(dx).to_degrees())
}

/// A segment's end point, and its directions at its start and at its end.
type Tangents = ((f32, f32), Option<f32>, Option<f32>);

/// A drawing segment's [`Tangents`].
fn tangents(from: (f32, f32), seg: Seg) -> Option<Tangents> {
    match seg {
        Seg::Line(x, y) => {
            let d = direction(from, (x, y));
            Some(((x, y), d, d))
        }
        Seg::Cubic(x1, y1, x2, y2, x, y) => {
            let (c1, c2, p) = ((x1, y1), (x2, y2), (x, y));
            let start = direction(from, c1)
                .or_else(|| direction(from, c2))
                .or_else(|| direction(from, p));
            let end = direction(c2, p)
                .or_else(|| direction(c1, p))
                .or_else(|| direction(from, p));
            Some((p, start, end))
        }
        Seg::Move(..) | Seg::Close => None,
    }
}

/// The vertices of `path`, whose authored commands end at `ends`.
fn vertices(path: &Path, ends: &[usize]) -> Vec<Vertex> {
    let mut out: Vec<Vertex> = Vec::new();
    let mut cur = (0.0, 0.0);
    // The subpath's start vertex, and its first outgoing direction.
    let mut start: Option<usize> = None;
    let mut first_out: Option<f32> = None;
    let mut from = 0;
    for &end in ends {
        let segs = &path.0[from.min(end)..end];
        from = end;
        let Some(first) = segs.first() else { continue };
        match *first {
            Seg::Move(x, y) if segs.len() == 1 => {
                cur = (x, y);
                start = Some(out.len());
                first_out = None;
                out.push(Vertex {
                    at: cur,
                    into: None,
                    out: None,
                });
            }
            Seg::Close => {
                let Some(s) = start else { continue };
                let home = out[s].at;
                // A zero-length close keeps the direction it arrived with.
                let dir = direction(cur, home).or(out.last().and_then(|v| v.into));
                if let Some(last) = out.last_mut() {
                    last.out = dir;
                }
                out[s].into = dir;
                out.push(Vertex {
                    at: home,
                    into: dir,
                    out: first_out,
                });
                cur = home;
            }
            _ => {
                let mut into = None;
                let mut began = false;
                for seg in segs {
                    if let Seg::Move(x, y) = *seg {
                        // Drawing after a close continues from its start.
                        cur = (x, y);
                        continue;
                    }
                    let Some((p, s_dir, e_dir)) = tangents(cur, *seg) else {
                        continue;
                    };
                    if !began {
                        began = true;
                        if let Some(last) = out.last_mut() {
                            last.out = s_dir;
                        }
                        if first_out.is_none() {
                            first_out = s_dir;
                        }
                    }
                    into = e_dir.or(into);
                    cur = p;
                }
                out.push(Vertex {
                    at: cur,
                    into,
                    out: None,
                });
            }
        }
    }
    out
}

/// `orient`: `auto`, `auto-start-reverse`, or an angle.
enum Orient {
    Auto { reverse_start: bool },
    Angle(f32),
}

fn orient(text: Option<&str>) -> Orient {
    let t = text.unwrap_or("0").trim();
    match t {
        "auto" => {
            return Orient::Auto {
                reverse_start: false,
            }
        }
        "auto-start-reverse" => {
            return Orient::Auto {
                reverse_start: true,
            }
        }
        _ => {}
    }
    let split = t
        .find(|c: char| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
        .unwrap_or(t.len());
    let (n, unit) = t.split_at(split);
    let v = exact_num::parse_f64(n).ok().map(|v| v as f32);
    let deg = match unit {
        "" | "deg" => v,
        "rad" => v.map(f32::to_degrees),
        "grad" => v.map(|g| g * 0.9),
        "turn" => v.map(|t| t * 360.0),
        _ => None,
    };
    Orient::Angle(deg.filter(|d| d.is_finite()).unwrap_or(0.0))
}

impl Resolver<'_, '_> {
    /// The marker instances of a shape, in its user space.
    pub(super) fn markers(
        &mut self,
        node: &NodeRef<'_>,
        style: &StyleProps,
        path: &Path,
        vp: Viewport,
        ctm: tf::Affine,
    ) -> Vec<Item> {
        let refs = [&style.marker_start, &style.marker_mid, &style.marker_end];
        if refs.iter().all(|m| m.url().is_none()) {
            return Vec::new();
        }
        let ends: Vec<usize> = match node.node_type {
            NodeType::SvgPath => match node.props.str(PropId::D) {
                Some(d) => parse_d_commands(d).1,
                None => return Vec::new(),
            },
            NodeType::SvgLine | NodeType::SvgPolyline | NodeType::SvgPolygon => {
                (1..=path.0.len()).collect()
            }
            _ => return Vec::new(),
        };
        let vertices = vertices(path, &ends);
        let last = vertices.len().saturating_sub(1);
        let mut out = Vec::new();
        for (i, v) in vertices.iter().enumerate() {
            let which = if i == 0 {
                0
            } else if i == last {
                2
            } else {
                1
            };
            let Some(id) = refs[which].url() else {
                continue;
            };
            let Some(marker) = self
                .kernel
                .resolve_id(node.id, id)
                .and_then(|m| self.kernel.node(m))
                .filter(|m| m.node_type == NodeType::SvgMarker)
            else {
                continue;
            };
            if let Some(item) = self.marker(node, style, &marker, *v, i == 0, i, vp, ctm) {
                out.push(item);
            }
        }
        out
    }

    /// One marker instance at `v`.
    #[allow(clippy::too_many_arguments)]
    fn marker(
        &mut self,
        shape: &NodeRef<'_>,
        style: &StyleProps,
        marker: &NodeRef<'_>,
        v: Vertex,
        first: bool,
        index: usize,
        vp: Viewport,
        ctm: tf::Affine,
    ) -> Option<Item> {
        let props = marker.props;
        let size = |id: PropId, basis: f32| {
            props
                .str(id)
                .and_then(Length::parse)
                .map_or(3.0, |l| l.resolve(basis))
        };
        let (w, h) = (
            size(PropId::MarkerWidth, vp.width),
            size(PropId::MarkerHeight, vp.height),
        );
        if !(w > 0.0 && h > 0.0) {
            return None;
        }
        let vb = view_box(props);
        let view = view_box_transform(vb, props.str(PropId::PreserveAspectRatio), w, h)?;
        let (rx, ry) = (
            length::prop(props.str(PropId::RefX), vb.map_or(w, |b| b.width)),
            length::prop(props.str(PropId::RefY), vb.map_or(h, |b| b.height)),
        );
        let reference = tf::apply(view, (rx, ry));
        let units = if props.str(PropId::MarkerUnits) == Some("userSpaceOnUse") {
            1.0
        } else {
            style.stroke_width.max(0.0)
        };
        let angle = match orient(props.str(PropId::Orient)) {
            Orient::Auto { reverse_start } if reverse_start && first => v.angle() + 180.0,
            Orient::Auto { .. } => v.angle(),
            Orient::Angle(a) => a,
        };
        let place = tf::mul(
            tf::translate(v.at.0, v.at.1),
            tf::mul(
                tf::rotate(angle),
                tf::mul(
                    tf::scale(units, units),
                    tf::translate(-reference.0, -reference.1),
                ),
            ),
        );
        // A marker inside its own content draws nothing more.
        let salt = ((index as u64 + 1) << 32) | shape.id as u64;
        if self.uses.contains(&(marker.id as u64)) || self.uses.len() > 16 {
            return None;
        }
        let inherited = marker.computed_style(StyleMask::INHERITED);
        let mut mstyle = marker.style.clone();
        mstyle.copy_rows(&inherited, StyleMask::INHERITED.minus(marker.style.mask));
        let clip = mstyle.overflow_x != Overflow::Visible || mstyle.overflow_y != Overflow::Visible;
        let inner = match vb {
            Some(b) => Viewport {
                width: b.width,
                height: b.height,
            },
            None => Viewport {
                width: w,
                height: h,
            },
        };
        let at = tf::mul(ctm, place);
        self.uses.push(salt);
        self.uses.push(marker.id as u64);
        let children = self.children(marker, &mstyle, inner, tf::mul(at, view));
        let uid = self.uid(marker.id);
        self.uses.pop();
        self.uses.pop();
        Some(Item {
            id: shape.id,
            uid,
            key: shape.key,
            opacity: 1.0,
            transform: Some(Transform {
                origin: (0.0, 0.0),
                translate: (0.0, 0.0),
                rotate: 0.0,
                scale: 1.0,
                matrix: place,
            }),
            ctm: at,
            clip: None,
            mask: None,
            instance: true,
            kind: Kind::Viewport {
                rect: (0.0, 0.0, w, h),
                view: Some(view),
                clip,
                children,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angles(d: &str) -> Vec<(f32, f32, f32)> {
        let (path, ends) = parse_d_commands(d);
        vertices(&path, &ends)
            .iter()
            .map(|v| (v.at.0, v.at.1, (v.angle() * 100.0).round() / 100.0))
            .collect()
    }

    #[test]
    fn vertices_bisect_and_close() {
        // An open corner: in at 0°, out at 90°, the middle bisects to 45°.
        assert_eq!(
            angles("M0 0 L10 0 L10 10"),
            vec![(0.0, 0.0, 0.0), (10.0, 0.0, 45.0), (10.0, 10.0, 90.0)]
        );
        // A closed triangle: the start takes the closing segment's direction
        // (180°) against the first (0°); the `Z` vertex is the start again.
        let closed = angles("M0 0 H10 V10 Z");
        assert_eq!(closed.len(), 4);
        assert_eq!((closed[0].0, closed[0].1), (0.0, 0.0));
        assert_eq!(closed[3].0, 0.0);
        assert_eq!(closed[0].2, closed[3].2);
        // An arc is one command: one vertex at its end.
        assert_eq!(angles("M0 0 A10 10 0 0 1 20 0").len(), 2);
    }
}
