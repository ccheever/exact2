use crate::{
    geometry::{collide, unit, world_pose, Geometry},
    Collider, Hit, Shape,
};
use exact_game::{Entity, Transform, Vec3, World};

fn sphere_ray(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Option<(f32, Vec3)> {
    let p = o - c;
    let b = p.dot(d);
    let q = p.length_squared() - r * r;
    if q <= 0.0 {
        return Some((0.0, unit(p, -d)));
    }
    let disc = b * b - q;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (t >= 0.0).then_some((t, unit(p + d * t, -d)))
}
fn ray(g: Geometry, o: Vec3, d: Vec3) -> Option<(f32, Vec3)> {
    match g {
        Geometry::Box { center, axes, half } => {
            let p = o - center;
            let mut near = 0.0f32;
            let mut far = f32::INFINITY;
            let mut n = -d;
            for i in 0..3 {
                let x = p.dot(axes[i]);
                let v = d.dot(axes[i]);
                if v.abs() < 1e-12 {
                    if x.abs() > half[i] {
                        return None;
                    }
                    continue;
                }
                let a = (-half[i] - x) / v;
                let b = (half[i] - x) / v;
                if a.min(b) > near {
                    near = a.min(b);
                    n = axes[i] * if v > 0.0 { -1.0 } else { 1.0 };
                }
                far = far.min(a.max(b));
                if near > far {
                    return None;
                }
            }
            Some((near, n))
        }
        Geometry::Round { a, b, radius: r } => {
            let segment = b - a;
            let length = segment.length();
            if length < 1e-8 {
                return sphere_ray(o, d, a, r);
            }
            let axis = segment / length;
            let p = o - a;
            let nearest = a + axis * p.dot(axis).clamp(0.0, length);
            if (o - nearest).length_squared() <= r * r {
                return Some((0.0, unit(o - nearest, -d)));
            }
            let dp = d - axis * d.dot(axis);
            let pp = p - axis * p.dot(axis);
            let qa = dp.length_squared();
            let qb = pp.dot(dp);
            let qc = pp.length_squared() - r * r;
            let mut best = None;
            if qa > 1e-12 && qb * qb - qa * qc >= 0.0 {
                let t = (-qb - (qb * qb - qa * qc).sqrt()) / qa;
                let h = (p + d * t).dot(axis);
                if t >= 0.0 && h >= 0.0 && h <= length {
                    best = Some((t, unit(pp + dp * t, -d)));
                }
            }
            for c in [a, b] {
                if let Some(hit) = sphere_ray(o, d, c, r) {
                    if best.is_none_or(|old| hit.0 < old.0) {
                        best = Some(hit);
                    }
                }
            }
            best
        }
    }
}
/// Nearest collider along a ray; `dir` is normalized internally and `max` is metres.
/// Sensors participate. A ray starting inside returns distance zero.
pub fn raycast(world: &World, origin: Vec3, dir: Vec3, max: f32, mask: u32) -> Option<Hit> {
    if !origin.is_finite() || !dir.is_finite() || dir.length_squared() < 1e-16 || max < 0.0 {
        return None;
    }
    let dir = unit(dir, Vec3::X);
    let mut best: Option<Hit> = None;
    for (e, c) in world.query::<&Collider>().iter() {
        if c.layer & mask == 0 {
            continue;
        }
        if let Some((distance, normal)) =
            ray(Geometry::new(&c.shape, world_pose(world, e)), origin, dir)
        {
            if distance <= max && best.is_none_or(|h| distance < h.distance) {
                best = Some(Hit {
                    entity: e,
                    distance,
                    point: origin + dir * distance,
                    normal,
                });
            }
        }
    }
    best
}
/// Colliders touching or penetrating the supplied world pose, in entity order.
pub fn overlap(world: &World, shape: &Shape, pose: Transform, mask: u32) -> Vec<Entity> {
    let g = Geometry::new(shape, pose);
    world
        .query::<&Collider>()
        .iter()
        .filter_map(|(e, c)| {
            (c.layer & mask != 0
                && collide(g, Geometry::new(&c.shape, world_pose(world, e))).separation() <= 0.0)
                .then_some(e)
        })
        .collect()
}
fn advance(
    shape: &Shape,
    mut pose: Transform,
    motion: Vec3,
    target: Geometry,
) -> Option<(f32, Vec3, Vec3)> {
    let origin = pose.position;
    let mut t = 0.0;
    for _ in 0..40 {
        pose.position = origin + motion * t;
        let patch = collide(Geometry::new(shape, pose), target);
        let p = patch
            .points
            .iter()
            .min_by(|a, b| a.separation.total_cmp(&b.separation))?;
        let speed = motion.dot(patch.normal);
        if p.separation <= 1e-5 {
            if t == 0.0 && p.separation >= -1e-5 && speed <= 1e-8 {
                return None;
            }
            return Some((t, p.b, -patch.normal));
        }
        if speed <= 1e-8 {
            return None;
        }
        let next = t + p.separation / speed;
        if next > 1.0 + 1e-6 {
            return None;
        }
        if next <= t {
            return Some((t, p.b, -patch.normal));
        }
        t = next.min(1.0);
    }
    None
}
/// First collision of a sphere or capsule translated by `motion` in world metres.
/// Uses conservative advancement; rotation is held fixed. Box sweeps are P2.
/// Initial penetration returns distance zero. Sensors participate.
pub fn sweep(
    world: &World,
    shape: &Shape,
    pose: Transform,
    motion: Vec3,
    mask: u32,
) -> Option<Hit> {
    sweep_filtered(world, shape, pose, motion, mask, None, false)
}
pub(crate) fn sweep_filtered(
    world: &World,
    shape: &Shape,
    pose: Transform,
    motion: Vec3,
    mask: u32,
    exclude: Option<Entity>,
    solid: bool,
) -> Option<Hit> {
    assert!(
        !matches!(shape, Shape::Box { .. }),
        "physics: box sweeps are not implemented (P2)"
    );
    assert!(motion.is_finite(), "physics: sweep motion must be finite");
    let mut best: Option<Hit> = None;
    let length = motion.length();
    let (lo, hi) = Geometry::new(shape, pose).aabb();
    let lo = lo + motion.min(Vec3::ZERO);
    let hi = hi + motion.max(Vec3::ZERO);
    for (e, c) in world.query::<&Collider>().iter() {
        if Some(e) == exclude || c.layer & mask == 0 || (solid && c.sensor) {
            continue;
        }
        let target = Geometry::new(&c.shape, world_pose(world, e));
        let (tl, th) = target.aabb();
        if !(lo.cmple(th + Vec3::splat(1e-5)).all() && tl.cmple(hi + Vec3::splat(1e-5)).all()) {
            continue;
        }
        if let Some((t, point, normal)) = advance(shape, pose, motion, target) {
            let distance = t * length;
            if best.is_none_or(|h| distance < h.distance) {
                best = Some(Hit {
                    entity: e,
                    distance,
                    point,
                    normal,
                });
            }
        }
    }
    best
}
