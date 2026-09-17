use crate::geometry::{segments, unit, Geometry, Patch, Point};
use exact_game::Vec3;

#[derive(Clone, Copy)]
struct BoxFrame {
    c: Vec3,
    u: [Vec3; 3],
    h: Vec3,
}
impl BoxFrame {
    fn new(g: Geometry) -> Self {
        let Geometry::Box {
            center: c,
            axes: u,
            half: h,
        } = g
        else {
            unreachable!()
        };
        Self { c, u, h }
    }
    fn radius(self, n: Vec3) -> f32 {
        self.h.x * self.u[0].dot(n).abs()
            + self.h.y * self.u[1].dot(n).abs()
            + self.h.z * self.u[2].dot(n).abs()
    }
    fn vertex(self, id: u32) -> Vec3 {
        let mut p = self.c;
        for i in 0..3 {
            p += self.u[i] * self.h[i] * if id & (1 << i) == 0 { -1.0 } else { 1.0 };
        }
        p
    }
    fn closest(self, p: Vec3) -> Vec3 {
        let d = p - self.c;
        self.c
            + self.u[0] * d.dot(self.u[0]).clamp(-self.h.x, self.h.x)
            + self.u[1] * d.dot(self.u[1]).clamp(-self.h.y, self.h.y)
            + self.u[2] * d.dot(self.u[2]).clamp(-self.h.z, self.h.z)
    }
    fn edge(self, axis: usize, n: Vec3) -> (Vec3, Vec3) {
        let mut c = self.c;
        for i in 0..3 {
            if i != axis {
                c += self.u[i] * self.h[i] * if self.u[i].dot(n) >= 0.0 { 1.0 } else { -1.0 };
            }
        }
        (
            c - self.u[axis] * self.h[axis],
            c + self.u[axis] * self.h[axis],
        )
    }
}
#[derive(Clone, Copy)]
struct Vertex {
    p: Vec3,
    id: u32,
}
fn clip(poly: Vec<Vertex>, n: Vec3, limit: f32, bit: u32) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(8);
    if poly.is_empty() {
        return out;
    }
    let mut a = poly[poly.len() - 1];
    let mut da = a.p.dot(n) - limit;
    for b in poly {
        let db = b.p.dot(n) - limit;
        if (da > 0.0) != (db > 0.0) {
            out.push(Vertex {
                p: a.p + (b.p - a.p) * (da / (da - db)),
                id: a.id | b.id | bit,
            });
        }
        if db <= 0.0 {
            out.push(b);
        }
        a = b;
        da = db;
    }
    out
}
fn face(a: BoxFrame, b: BoxFrame, axis: usize, n: Vec3, key: u32) -> Patch {
    let center = a.c + n * a.h[axis];
    let mut incident = 0;
    for i in 1..3 {
        if b.u[i].dot(n).abs() > b.u[incident].dot(n).abs() {
            incident = i;
        }
    }
    let sign = if b.u[incident].dot(n) > 0.0 {
        -1.0
    } else {
        1.0
    };
    let bc = b.c + b.u[incident] * b.h[incident] * sign;
    let j = (incident + 1) % 3;
    let k = (incident + 2) % 3;
    let mut poly = Vec::with_capacity(8);
    for (id, (x, y)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .into_iter()
        .enumerate()
    {
        poly.push(Vertex {
            p: bc + b.u[j] * (x * b.h[j]) + b.u[k] * (y * b.h[k]),
            id: 1 << id,
        });
    }
    let j = (axis + 1) % 3;
    let k = (axis + 2) % 3;
    for (bit, (t, h)) in [
        (a.u[j], a.h[j]),
        (-a.u[j], a.h[j]),
        (a.u[k], a.h[k]),
        (-a.u[k], a.h[k]),
    ]
    .into_iter()
    .enumerate()
    {
        poly = clip(poly, t, center.dot(t) + h, 1 << (bit + 4));
    }
    let prefix = (key << 16) | ((incident as u32) << 12) | ((sign > 0.0) as u32) << 11;
    let points = poly
        .into_iter()
        .map(|v| {
            let s = (v.p - center).dot(n);
            Point {
                a: v.p - n * s,
                b: v.p,
                separation: s,
                feature: prefix | v.id,
            }
        })
        .collect();
    Patch { normal: n, points }
}
// Deepest, longest span, largest triangle, then largest remaining triangle area.
// Deterministic feature-order ties avoid point-order chatter under exact alignment.
fn reduce(patch: &mut Patch) {
    patch.points.sort_by_key(|p| p.feature);
    patch
        .points
        .dedup_by(|a, b| (a.a - b.a).length_squared() < 1e-12);
    if patch.points.len() <= 4 {
        return;
    }
    let mut candidates = std::mem::take(&mut patch.points);
    let mut first = 0;
    for i in 1..candidates.len() {
        if candidates[i].separation < candidates[first].separation {
            first = i;
        }
    }
    patch.points.push(candidates.remove(first));
    while patch.points.len() < 4 {
        let mut best = 0;
        let mut score = -1.0;
        for (i, p) in candidates.iter().enumerate() {
            let v = p.a - patch.points[0].a;
            let s = if patch.points.len() == 1 {
                v.length_squared()
            } else if patch.points.len() == 2 {
                v.cross(patch.points[1].a - patch.points[0].a)
                    .length_squared()
            } else {
                (0..3)
                    .map(|j| {
                        ((patch.points[j].a - p.a).cross(patch.points[(j + 1) % 3].a - p.a))
                            .dot(patch.normal)
                            .abs()
                    })
                    .sum()
            };
            if s > score {
                score = s;
                best = i;
            }
        }
        patch.points.push(candidates.remove(best));
    }
    patch.points.sort_by_key(|p| p.feature);
}
fn separated(a: BoxFrame, b: BoxFrame) -> Patch {
    let mut best = (f32::INFINITY, a.c, b.c, 0);
    for i in 0..8 {
        for (x, y, id) in [
            (a.vertex(i), b.closest(a.vertex(i)), i),
            (a.closest(b.vertex(i)), b.vertex(i), 8 + i),
        ] {
            let d = (x - y).length_squared();
            if d < best.0 {
                best = (d, x, y, id);
            }
        }
    }
    for ia in 0..3 {
        for va in 0..8 {
            if va & (1 << ia) != 0 {
                continue;
            }
            for ib in 0..3 {
                for vb in 0..8 {
                    if vb & (1 << ib) != 0 {
                        continue;
                    }
                    let (x, y, _, _) = segments(
                        a.vertex(va),
                        a.vertex(va | (1 << ia)),
                        b.vertex(vb),
                        b.vertex(vb | (1 << ib)),
                    );
                    let d = (x - y).length_squared();
                    if d < best.0 {
                        best = (d, x, y, 32 + ia * 192 + va * 24 + ib * 8 + vb);
                    }
                }
            }
        }
    }
    Patch::one(
        best.1,
        best.2,
        unit(best.2 - best.1, Vec3::X),
        0x8000_0000 | best.3,
    )
}
pub(crate) fn box_box(ga: Geometry, gb: Geometry) -> Patch {
    let a = BoxFrame::new(ga);
    let b = BoxFrame::new(gb);
    let d = b.c - a.c;
    let mut gap = f32::NEG_INFINITY;
    let mut normal = Vec3::X;
    let mut feature = 0;
    for i in 0..15 {
        let axis = if i < 3 {
            a.u[i]
        } else if i < 6 {
            b.u[i - 3]
        } else {
            a.u[(i - 6) / 3].cross(b.u[(i - 6) % 3])
        };
        if axis.length_squared() < 1e-10 {
            continue;
        }
        let n = unit(axis, Vec3::X);
        let s = d.dot(n).abs() - a.radius(n) - b.radius(n);
        // Prefer face axes at essentially equal depth, giving four-point face contacts.
        if s > gap + 1e-6 {
            gap = s;
            normal = if d.dot(n) >= 0.0 { n } else { -n };
            feature = i;
        }
    }
    let mut patch = if feature < 3 {
        face(a, b, feature, normal, feature as u32)
    } else if feature < 6 {
        face(b, a, feature - 3, -normal, feature as u32).flip()
    } else {
        let i = (feature - 6) / 3;
        let j = (feature - 6) % 3;
        let (p, q) = a.edge(i, normal);
        let (r, s) = b.edge(j, -normal);
        let (x, y, _, _) = segments(p, q, r, s);
        Patch::one(x, y, normal, feature as u32 + 0x4000_0000)
    };
    if patch.points.is_empty() || (gap > 0.0 && feature >= 6) {
        return separated(a, b);
    }
    reduce(&mut patch);
    patch
}
