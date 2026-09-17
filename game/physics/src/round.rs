use crate::geometry::{endpoint, segment, segments, unit, Geometry, Patch, Point};
use exact_game::Vec3;

pub(crate) fn round_round(a: Vec3, b: Vec3, ra: f32, c: Vec3, d: Vec3, rb: f32) -> Patch {
    let (x, y, s, t) = segments(a, b, c, d);
    let n = unit(y - x, unit((b - a).cross(d - c), Vec3::X));
    let mut patch = Patch::one(x + n * ra, y - n * rb, n, endpoint(s) * 3 + endpoint(t));
    // Parallel capsules need the extent of their contact interval, not an arbitrary endpoint.
    if (b - a).length_squared() > 1e-12
        && (d - c).length_squared() > 1e-12
        && (b - a).cross(d - c).length_squared()
            < 1e-8 * (b - a).length_squared() * (d - c).length_squared()
    {
        for (i, p) in [a, b].into_iter().enumerate() {
            let (q, t) = segment(c, d, p);
            let diff = q - p;
            if (diff.length() - (y - x).length()).abs() < 1e-5 && (p - x).length_squared() > 1e-8 {
                patch.points.push(Point {
                    a: p + n * ra,
                    b: q - n * rb,
                    separation: diff.dot(n) - ra - rb,
                    feature: 16 + i as u32 * 3 + endpoint(t),
                });
            }
        }
    }
    patch
}

fn local(p: Vec3, c: Vec3, axes: [Vec3; 3]) -> Vec3 {
    let v = p - c;
    Vec3::new(v.dot(axes[0]), v.dot(axes[1]), v.dot(axes[2]))
}
fn unlocal(p: Vec3, c: Vec3, axes: [Vec3; 3]) -> Vec3 {
    c + axes[0] * p.x + axes[1] * p.y + axes[2] * p.z
}
fn face_id(p: Vec3, half: Vec3) -> u32 {
    let mut id = 0;
    for i in 0..3 {
        id = id * 3
            + if p[i] <= -half[i] {
                1
            } else if p[i] >= half[i] {
                2
            } else {
                0
            };
    }
    id
}
// Distance to an AABB is a piecewise quadratic along a segment. Examine the exact
// stationary point in every interval bounded by a crossing of one of its six planes.
fn segment_box(a: Vec3, b: Vec3, half: Vec3) -> (Vec3, Vec3, f32) {
    let d = b - a;
    let mut cuts = vec![0.0, 1.0];
    for i in 0..3 {
        if d[i].abs() > 1e-12 {
            for sign in [-1.0, 1.0] {
                let t = (sign * half[i] - a[i]) / d[i];
                if t > 0.0 && t < 1.0 {
                    cuts.push(t);
                }
            }
        }
    }
    cuts.sort_by(f32::total_cmp);
    let mut best = (f32::INFINITY, a, a.clamp(-half, half), 0.0);
    for w in cuts.windows(2) {
        let mid = a + d * ((w[0] + w[1]) * 0.5);
        let mut aa = 0.0;
        let mut bb = 0.0;
        for i in 0..3 {
            let face = if mid[i] < -half[i] {
                -half[i]
            } else if mid[i] > half[i] {
                half[i]
            } else {
                continue;
            };
            aa += d[i] * d[i];
            bb += d[i] * (a[i] - face);
        }
        let t = if aa > 0.0 {
            (-bb / aa).clamp(w[0], w[1])
        } else {
            w[0]
        };
        let p = a + d * t;
        let q = p.clamp(-half, half);
        let dist = (q - p).length_squared();
        if dist < best.0 {
            best = (dist, p, q, t);
        }
    }
    (best.1, best.2, best.3)
}

pub(crate) fn round_box(a: Vec3, b: Vec3, radius: f32, target: Geometry) -> Patch {
    let Geometry::Box { center, axes, half } = target else {
        unreachable!()
    };
    let p = local(a, center, axes);
    let q = local(b, center, axes);
    let (x, y, t) = segment_box(p, q, half);
    let diff = y - x;
    let (normal, point, face) = if diff.length_squared() > 1e-14 {
        (unit(diff, Vec3::X), y, face_id(y, half))
    } else {
        // When the segment intersects the box, choose the minimum translation over
        // face axes and edge × segment axes (the rounded polytope's SAT axes).
        let mid = (p + q) * 0.5;
        let extent = (q - p) * 0.5;
        let mut best = f32::INFINITY;
        let mut n = Vec3::X;
        let mut id = 0;
        for (i, axis) in [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::X.cross(extent),
            Vec3::Y.cross(extent),
            Vec3::Z.cross(extent),
        ]
        .into_iter()
        .enumerate()
        {
            if axis.length_squared() < 1e-12 {
                continue;
            }
            let axis = unit(axis, Vec3::X);
            let depth = half.dot(axis.abs()) + extent.dot(axis).abs() - mid.dot(axis).abs();
            if depth < best {
                best = depth;
                n = if mid.dot(axis) >= 0.0 { -axis } else { axis };
                id = i as u32 + 32;
            }
        }
        let support = if p.dot(n) > q.dot(n) { p } else { q };
        let surface = support - n * best;
        (n, surface, id)
    };
    let n = axes[0] * normal.x + axes[1] * normal.y + axes[2] * normal.z;
    let support = if diff.length_squared() > 1e-14 {
        x
    } else if p.dot(normal) > q.dot(normal) {
        p
    } else {
        q
    };
    let mut out = Patch::one(
        unlocal(support, center, axes) + n * radius,
        unlocal(point, center, axes),
        n,
        face * 4 + endpoint(t),
    );
    // A capsule side against a face has two supports. Preserve both when equally deep.
    if (b - a).length_squared() > 1e-12 && ((q - p).dot(normal)).abs() < 1e-5 {
        for (i, end) in [p, q].into_iter().enumerate() {
            let mut closest = end.clamp(-half, half);
            if diff.length_squared() <= 1e-14 {
                closest = end + normal * (point - support).dot(normal);
            }
            let sep = (closest - end).dot(normal) - radius;
            if (sep - out.points[0].separation).abs() < 1e-5
                && ((closest - end) - normal * (sep + radius)).length_squared() < 1e-8
            {
                let pa = unlocal(end, center, axes) + n * radius;
                if (pa - out.points[0].a).length_squared() > 1e-8 {
                    out.points.push(Point {
                        a: pa,
                        b: unlocal(closest, center, axes),
                        separation: sep,
                        feature: face * 4 + i as u32 + 256,
                    });
                }
            }
        }
    }
    out
}
