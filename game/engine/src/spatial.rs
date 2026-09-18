//! CPU geometry shared by layout and pick. The renderer is not a source of truth.
use crate::{math, Affine3A, Camera, Entity, Mesh, Vec2, Vec3, Visible, World};
use glam::Mat4;
pub(crate) mod index;
use index::Sight;

pub(crate) struct View {
    pub entity: Entity,
    pub pose: Affine3A,
    pub camera: Camera,
    pub size: Vec2,
}
impl View {
    pub fn new(w: &World, size: Vec2) -> Option<Self> {
        if size.min_element() <= 0.0 {
            return None;
        }
        w.query::<&Camera>().iter().find_map(|(e, c)| {
            (c.active
                && c.near > 0.0
                && c.far > c.near
                && c.fov_y_degrees > 0.0
                && c.fov_y_degrees < 180.0)
                .then(|| {
                    w.global(e)
                        .filter(|p| p.is_finite() && p.matrix3.determinant().abs() >= 1e-12)
                        .map(|pose| Self {
                            entity: e,
                            pose,
                            camera: *c,
                            size,
                        })
                })
                .flatten()
        })
    }
    fn projection(&self) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            self.camera.fov_y_degrees.to_radians(),
            self.size.x / self.size.y,
            self.camera.near,
            self.camera.far,
        )
    }
    pub fn ray(&self, p: Vec2) -> (Vec3, Vec3) {
        let t = math::tan(self.camera.fov_y_degrees.to_radians() * 0.5);
        let local = Vec3::new(
            (p.x / self.size.x * 2.0 - 1.0) * self.size.x / self.size.y * t,
            (1.0 - p.y / self.size.y * 2.0) * t,
            -1.0,
        );
        (
            self.pose.translation.into(),
            self.pose.transform_vector3(local).normalize(),
        )
    }
    pub fn screen(&self, corners: &[Vec3; 8]) -> Option<[f32; 4]> {
        let inv = self.pose.inverse();
        let points = corners.map(|p| inv.transform_point3(p));
        let mut clipped: Vec<_> = points
            .iter()
            .copied()
            .filter(|p| p.z <= -self.camera.near)
            .collect();
        // Clip box edges against near before perspective division; a box crossing
        // the eye must not invent an inverted or infinite screen rectangle.
        for i in 0..8 {
            for bit in [1, 2, 4] {
                let j = i ^ bit;
                if i >= j {
                    continue;
                }
                let a = points[i];
                let b = points[j];
                if (a.z < -self.camera.near) != (b.z < -self.camera.near) {
                    let t = (-self.camera.near - a.z) / (b.z - a.z);
                    clipped.push(a + (b - a) * t);
                }
            }
        }
        let mut lo = Vec2::splat(f32::INFINITY);
        let mut hi = Vec2::splat(f32::NEG_INFINITY);
        for p in clipped {
            let p = self.projection().project_point3(p);
            let p = Vec2::new(
                (p.x + 1.0) * 0.5 * self.size.x,
                (1.0 - p.y) * 0.5 * self.size.y,
            );
            lo = lo.min(p);
            hi = hi.max(p);
        }
        lo.is_finite()
            .then_some([lo.x, lo.y, hi.x - lo.x, hi.y - lo.y])
    }
    pub fn visibility(&self, corners: &[Vec3; 8], center: Vec3) -> (bool, bool, f32, f32) {
        let inv = self.pose.inverse();
        let c = inv.transform_point3(center);
        let clip = corners.map(|p| self.projection() * inv.transform_point3(p).extend(1.0));
        let outside = (0..6).any(|plane| {
            clip.iter().all(|p| match plane {
                0 => p.x < -p.w,
                1 => p.x > p.w,
                2 => p.y < -p.w,
                3 => p.y > p.w,
                4 => p.z < 0.0,
                _ => p.z > p.w,
            })
        });
        (
            !outside,
            corners.iter().all(|p| inv.transform_point3(*p).z >= 0.0),
            center.distance(self.pose.translation.into()),
            -c.z,
        )
    }
}
pub(crate) fn extent(mesh: Option<&Mesh>) -> Vec3 {
    match mesh {
        Some(Mesh::Sphere { radius }) => Vec3::splat(*radius),
        Some(Mesh::Capsule { radius, height }) => Vec3::new(*radius, height * 0.5, *radius),
        Some(Mesh::Plane { width, depth }) => Vec3::new(width * 0.5, 0.005, depth * 0.5),
        Some(Mesh::Cylinder { radius, height }) => Vec3::new(*radius, height * 0.5, *radius),
        Some(Mesh::Box { size }) => *size * 0.5,
        Some(Mesh::Asset(_)) => Vec3::splat(0.5),
        None => Vec3::ZERO,
    }
}
pub(crate) fn center(mesh: Option<&Mesh>) -> Vec3 {
    if matches!(mesh, Some(Mesh::Plane { .. })) {
        Vec3::new(0.0, -0.005, 0.0)
    } else {
        Vec3::ZERO
    }
}
pub(crate) fn bounds(w: &World, entity: Entity, mesh: Option<&Mesh>) -> (Vec3, Vec3) {
    if let Some(Mesh::Asset(name)) = mesh {
        if let Some(model) = w.model(name) {
            let bounds = w
                .get::<crate::Pose>(entity)
                .map_or_else(|| crate::animation::animated_bounds(model), |p| p.bounds);
            let lo = Vec3::from_slice(&bounds[..3]);
            let hi = Vec3::from_slice(&bounds[3..]);
            return ((hi - lo) * 0.5, (hi + lo) * 0.5);
        }
    }
    if let Some(b) = w.get::<crate::asset::ModelBounds>(entity) {
        let lo = Vec3::from_slice(&b.0[..3]);
        let hi = Vec3::from_slice(&b.0[3..]);
        return ((hi - lo) * 0.5, (hi + lo) * 0.5);
    }
    (extent(mesh), center(mesh))
}
pub(crate) fn unbounded(w: &World, e: Entity, mesh: Option<&Mesh>) -> bool {
    if w.get::<crate::asset::ModelBounds>(e)
        .is_some_and(|b| !crate::asset::valid_bounds(&b.0))
    {
        return true;
    }
    matches!(mesh, Some(Mesh::Asset(name)) if w.model(name).is_none() && w.get::<crate::asset::ModelBounds>(e).is_none())
}
pub(crate) fn corners(pose: Affine3A, half: Vec3, center: Vec3) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        pose.transform_point3(
            center
                + half
                    * Vec3::new(
                        if i & 1 == 0 { -1.0 } else { 1.0 },
                        if i & 2 == 0 { -1.0 } else { 1.0 },
                        if i & 4 == 0 { -1.0 } else { 1.0 },
                    ),
        )
    })
}
fn sphere_roots(o: Vec3, d: Vec3, center: Vec3, radius: f32) -> Option<[f32; 2]> {
    let o = o - center;
    let a = d.length_squared();
    let b = o.dot(d);
    let c = o.length_squared() - radius * radius;
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let root = math::sqrt(disc);
    Some([(-b - root) / a, (-b + root) / a])
}
fn sphere(o: Vec3, d: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    sphere_roots(o, d, center, radius)?
        .into_iter()
        .find(|t| *t >= 0.0)
}
fn slab(o: Vec3, d: Vec3, half: Vec3) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i].abs() > half[i] {
                return None;
            }
        } else {
            let a = (-half[i] - o[i]) / d[i];
            let b = (half[i] - o[i]) / d[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
    }
    if far < near || far < 0.0 {
        None
    } else {
        Some(if near >= 0.0 { near } else { far })
    }
}
fn capsule(o: Vec3, d: Vec3, radius: f32, height: f32) -> Option<f32> {
    let half = height * 0.5;
    let mut hit = None;
    // Test only the exposed hemispheres. Whole endpoint spheres return an
    // interior surface when the camera starts inside the capsule's cylinder.
    for sign in [-1.0, 1.0] {
        if let Some(roots) = sphere_roots(o, d, Vec3::Y * half * sign, radius) {
            for t in roots {
                if t >= 0.0 && (o.y + t * d.y) * sign >= half && hit.is_none_or(|old| t < old) {
                    hit = Some(t);
                }
            }
        }
    }
    let a = d.x * d.x + d.z * d.z;
    let b = o.x * d.x + o.z * d.z;
    let c = o.x * o.x + o.z * o.z - radius * radius;
    let disc = b * b - a * c;
    if a > 1e-12 && disc >= 0.0 {
        for t in [(-b - math::sqrt(disc)) / a, (-b + math::sqrt(disc)) / a] {
            if t >= 0.0 && (o.y + t * d.y).abs() <= half && hit.is_none_or(|h| t < h) {
                hit = Some(t);
            }
        }
    }
    hit
}
fn cylinder(o: Vec3, d: Vec3, radius: f32, height: f32) -> Option<f32> {
    let mut hit: Option<f32> = None;
    let mut accept = |t: f32| {
        if t >= 0.0 && hit.is_none_or(|old| t < old) {
            hit = Some(t);
        }
    };
    let a = d.x * d.x + d.z * d.z;
    let b = o.x * d.x + o.z * d.z;
    let c = o.x * o.x + o.z * o.z - radius * radius;
    let disc = b * b - a * c;
    if a > 1e-12 && disc >= 0.0 {
        for t in [(-b - math::sqrt(disc)) / a, (-b + math::sqrt(disc)) / a] {
            if (o.y + t * d.y).abs() <= height * 0.5 {
                accept(t);
            }
        }
    }
    if d.y.abs() > 1e-8 {
        for y in [-height * 0.5, height * 0.5] {
            let t = (y - o.y) / d.y;
            let p = o + d * t;
            if p.x * p.x + p.z * p.z <= radius * radius {
                accept(t);
            }
        }
    }
    hit
}
pub(crate) fn pick(w: &World, view: &View, point: Vec2) -> Option<(Entity, f32, Vec3)> {
    let (origin, direction) = view.ray(point);
    let mut hit = None;
    for (e, mesh) in w.query::<&Mesh>().iter() {
        let t = ray_hit(w, e, mesh, origin, direction);
        if let Some(t) = t {
            let p = origin + direction * t;
            let depth = -view.pose.inverse().transform_point3(p).z;
            if depth >= view.camera.near
                && depth <= view.camera.far
                && hit.is_none_or(|(_, old, _)| t < old)
            {
                hit = Some((e, t, p));
            }
        }
    }
    hit
}

fn ray_hit(w: &World, e: Entity, mesh: &Mesh, origin: Vec3, direction: Vec3) -> Option<f32> {
    if unbounded(w, e, Some(mesh)) || w.get::<Visible>(e).is_some_and(|v| !v.0) {
        return None;
    }
    let pose = w.global(e)?;
    if pose.matrix3.determinant().abs() < 1e-12 {
        return None;
    }
    let inv = pose.inverse();
    let o = inv.transform_point3(origin);
    let d = inv.transform_vector3(direction);
    let (half, center) = bounds(w, e, Some(mesh));
    shape_hit(mesh, o, d, half, center)
}
fn shape_hit(mesh: &Mesh, o: Vec3, d: Vec3, half: Vec3, center: Vec3) -> Option<f32> {
    if half.max_element() <= 0.0 {
        return None;
    }
    match mesh {
        Mesh::Cylinder { radius, height } => cylinder(o, d, *radius, *height),
        Mesh::Sphere { radius } => sphere(o, d, Vec3::ZERO, *radius),
        Mesh::Capsule { radius, height } => capsule(o, d, *radius, *height - 2.0 * radius),

        _ => slab(o - center, d, half),
    }
}

/// Eight corners, six face centres, and centre of the oriented bounds.
pub(crate) fn occlusion(
    sight: &mut Sight<'_>,
    origin: Vec3,
    corners: &[Vec3; 8],
) -> Result<(f32, Vec<Entity>), String> {
    let mut samples = [Vec3::ZERO; 15];
    samples[..8].copy_from_slice(corners);
    let mut n = 8;
    for bit in [1, 2, 4] {
        for side in [0, bit] {
            samples[n] = (0..8)
                .filter(|i| i & bit == side)
                .map(|i| corners[i])
                .sum::<Vec3>()
                * 0.25;
            n += 1;
        }
    }
    samples[14] = (corners[0] + corners[7]) * 0.5;
    let mut hidden = 0;
    let mut nearest: [Option<(Entity, f32)>; 4] = [None; 4];
    for sample in samples {
        let mut blocked = false;
        sight.segment(origin, sample, 2, |entity, distance| {
            blocked = true;
            let mut candidate = (entity, distance);
            if let Some(i) = nearest
                .iter()
                .position(|v| v.is_some_and(|(e, _)| e == entity))
            {
                candidate.1 = candidate.1.min(nearest[i].unwrap().1);
                for j in i..3 {
                    nearest[j] = nearest[j + 1];
                }
                nearest[3] = None;
            }
            for slot in &mut nearest {
                if slot.is_none_or(|(e, d)| distance_order(candidate.0, candidate.1, e, d).is_lt())
                {
                    let old = slot.replace(candidate);
                    if let Some(old) = old {
                        candidate = old;
                    } else {
                        break;
                    }
                }
            }
            false // Collect nearest four across every hit; LOS instead stops at one.
        })?;
        hidden += usize::from(blocked);
    }
    Ok((
        hidden as f32 / 15.0,
        nearest.into_iter().flatten().map(|(e, _)| e).collect(),
    ))
}
fn distance_order(a: Entity, x: f32, b: Entity, y: f32) -> std::cmp::Ordering {
    distance_key(x)
        .cmp(&distance_key(y))
        .then_with(|| a.index().cmp(&b.index()))
}

fn distance_key(distance: f32) -> u64 {
    (f64::from(distance) * 10_000.0).round() as u64
}
fn contains_origin(mesh: &Mesh, o: Vec3, half: Vec3, center: Vec3) -> bool {
    match mesh {
        Mesh::Sphere { radius } => o.length_squared() <= radius * radius,
        Mesh::Capsule { radius, height } => {
            let stem = (height * 0.5 - radius).max(0.0);
            (o - Vec3::Y * o.y.clamp(-stem, stem)).length_squared() <= radius * radius
        }
        Mesh::Cylinder { radius, height } => {
            o.x * o.x + o.z * o.z <= radius * radius && o.y.abs() <= height * 0.5
        }
        _ => (o - center).abs().cmple(half).all(),
    }
}

/// Screen-space rectangle in CSS pixels for the simulation's current viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}
impl ScreenRect {
    /// Center point for a pick query.
    pub fn center(self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}
/// Projected bounds of one entity at its current simulation pose.
#[derive(Clone, Copy, Debug)]
pub struct EntityLayout {
    /// Resolved entity handle.
    pub entity: Entity,
    /// Screen rectangle, clipped to the camera's near plane.
    pub screen: ScreenRect,
}
/// Nearest visible mesh intersected by a screen-space ray.
#[derive(Clone, Copy, Debug)]
pub struct PickHit {
    /// Hit entity.
    pub entity: Entity,
    /// World-space distance from the camera.
    pub distance: f32,
    /// World-space intersection.
    pub point: Vec3,
}
impl<G: crate::Game> crate::Sim<G> {
    /// Project an entity using the same geometry and viewport as agent layout.
    /// Missing entities, cameras and entirely near-clipped bounds return None.
    pub fn layout(&self, target: impl crate::Target) -> Option<EntityLayout> {
        let entity = target.entity(&self.world)?;
        let pose = self.world.global(entity).filter(|p| p.is_finite())?;
        let mesh = self.world.get::<Mesh>(entity);
        let (half, center) = bounds(&self.world, entity, mesh.as_deref());
        let view = View::new(&self.world, self.input.viewport)?;
        let [x, y, w, h] = view.screen(&corners(pose, half, center))?;
        Some(EntityLayout {
            entity,
            screen: ScreenRect { x, y, w, h },
        })
    }
    /// Pick at a CSS-pixel point using the agent's ray/mesh implementation.
    pub fn pick(&self, point: Vec2) -> Option<PickHit> {
        if !point.is_finite() {
            return None;
        }
        let view = View::new(&self.world, self.input.viewport)?;
        let (entity, distance, point) = pick(&self.world, &view, point)?;
        Some(PickHit {
            entity,
            distance,
            point,
        })
    }
}
