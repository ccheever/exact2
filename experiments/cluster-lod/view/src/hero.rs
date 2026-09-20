//! Resolve an authored screen anchor to the nearest real source triangle once, at load time.
use crate::scene::Camera;
use clod_format::{ORIGINAL, Reader};
use glam::{Mat4, Vec3};
pub fn target(
    reader: &Reader<'_>,
    base: Mat4,
    min: Vec3,
    max: Vec3,
    washington: bool,
) -> (Vec3, Vec3) {
    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    let fov = 45f32.to_radians();
    let eye = center + Vec3::new(0.85, -1.6, 0.85).normalize() * radius / (fov * 0.5).sin() * 1.1;
    let gaul = reader.header.source_sha256
        == [
            66, 70, 235, 208, 143, 170, 208, 211, 168, 58, 223, 157, 119, 225, 17, 122, 80, 158,
            152, 207, 233, 58, 254, 94, 60, 163, 106, 188, 30, 247, 194, 178,
        ];
    let anchor = if washington {
        [0.442, 0.277]
    } else if gaul {
        [0.379, 0.364]
    } else {
        [0.5, 0.45]
    };
    let camera = Camera::perspective(eye, center, 16.0 / 9.0, fov, 0.002, radius * 12.0 + 10.0);
    let far = camera.matrix.inverse().project_point3(Vec3::new(
        anchor[0] * 2.0 - 1.0,
        1.0 - anchor[1] * 2.0,
        0.9,
    ));
    let ray = (far - eye).normalize();
    let inverse = base.inverse();
    let origin = inverse.transform_point3(eye);
    let direction = inverse.transform_vector3(ray).normalize();
    let mut nearest = f32::INFINITY;
    for (id, c) in reader
        .clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| c.refined == ORIGINAL)
    {
        let center = Vec3::new(c.sphere[0], c.sphere[1], c.sphere[2]);
        let offset = center - origin;
        if offset.cross(direction).length_squared() > c.sphere[3] * c.sphere[3] {
            continue;
        }
        let (vertices, indices) = reader.geometry(id).expect("validated source geometry");
        for tri in indices.chunks_exact(3) {
            let a = Vec3::from_array(vertices[tri[0] as usize].position);
            let b = Vec3::from_array(vertices[tri[1] as usize].position);
            let c = Vec3::from_array(vertices[tri[2] as usize].position);
            let e1 = b - a;
            let e2 = c - a;
            let h = direction.cross(e2);
            let det = e1.dot(h);
            if det <= 1e-12 {
                continue;
            }
            let s = origin - a;
            let u = s.dot(h) / det;
            let q = s.cross(e1);
            let v = direction.dot(q) / det;
            let t = e2.dot(q) / det;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 && t < nearest {
                nearest = t;
            }
        }
    }
    let target = if nearest.is_finite() {
        base.transform_point3(origin + direction * nearest)
    } else {
        center
    };
    (target, (eye - target).normalize())
}

/// Median of all three edges of every original triangle (multiplicity intentional).
pub fn median_edge(reader: &Reader<'_>, scale: f32) -> f32 {
    let mut edges = Vec::with_capacity(reader.header.source_triangles as usize * 3);
    for (id, _) in reader
        .clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| c.refined == ORIGINAL)
    {
        let (vertices, indices) = reader.geometry(id).expect("validated geometry");
        for t in indices.chunks_exact(3) {
            for i in 0..3 {
                let a = Vec3::from_array(vertices[t[i] as usize].position);
                let b = Vec3::from_array(vertices[t[(i + 1) % 3] as usize].position);
                edges.push(a.distance_squared(b));
            }
        }
    }
    let middle = edges.len() / 2;
    edges
        .select_nth_unstable_by(middle, f32::total_cmp)
        .1
        .sqrt()
        * scale
}

/// Quintic Hermite interpolation: shared first derivatives, zero second derivatives.
/// Endpoint velocities are zero; the last segment is an intentional hold.
pub fn spline(t: f32, knots: &[(f32, Vec3)]) -> Vec3 {
    let i = knots
        .windows(2)
        .position(|w| t <= w[1].0)
        .unwrap_or(knots.len() - 2);
    let (a, p) = knots[i];
    let (b, q) = knots[i + 1];
    let h = b - a;
    let u = ((t - a) / h).clamp(0.0, 1.0);
    let velocity = |k: usize| {
        if k == 0
            || k + 1 == knots.len()
            || (k > 0 && knots[k].1 == knots[k - 1].1)
            || (k + 1 < knots.len() && knots[k].1 == knots[k + 1].1)
        {
            Vec3::ZERO
        } else {
            (knots[k + 1].1 - knots[k - 1].1) / (knots[k + 1].0 - knots[k - 1].0) * 0.7
        }
    };
    let v = velocity(i) * h;
    let w = velocity(i + 1) * h;
    let d = q - p;
    p + v * u
        + (d * 10.0 - v * 6.0 - w * 4.0) * u.powi(3)
        + (-d * 15.0 + v * 8.0 + w * 7.0) * u.powi(4)
        + (d * 6.0 - v * 3.0 - w * 3.0) * u.powi(5)
}

/// Closest point on each potentially closer original triangle, with sphere pruning.
pub fn surface_distance(reader: &Reader<'_>, eye: Vec3, mut best: f32) -> f32 {
    for (id, c) in reader
        .clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| c.refined == ORIGINAL)
    {
        let center = Vec3::new(c.sphere[0], c.sphere[1], c.sphere[2]);
        if eye.distance(center) - c.sphere[3] >= best {
            continue;
        }
        let (vertices, indices) = reader.geometry(id).expect("validated geometry");
        for t in indices.chunks_exact(3) {
            let [a, b, c] =
                [t[0], t[1], t[2]].map(|i| Vec3::from_array(vertices[i as usize].position));
            let ab = b - a;
            let ac = c - a;
            let ap = eye - a;
            let n = ab.cross(ac);
            let nn = n.length_squared();
            let mut d = f32::INFINITY;
            if nn > 0.0 {
                let projected = eye - n * (ap.dot(n) / nn);
                if [
                    (b - a).cross(projected - a),
                    (c - b).cross(projected - b),
                    (a - c).cross(projected - c),
                ]
                .iter()
                .all(|v| v.dot(n) >= 0.0)
                {
                    d = ap.dot(n).abs() / nn.sqrt();
                }
            }
            for (p, q) in [(a, b), (b, c), (c, a)] {
                let e = q - p;
                let u =
                    ((eye - p).dot(e) / e.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
                d = d.min(eye.distance(p + e * u));
            }
            best = best.min(d);
        }
    }
    best
}
