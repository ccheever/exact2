//! CPU geometry shared by layout and pick. The renderer is not a source of truth.
use crate::{math, Affine3A, Camera, Entity, Mesh, Vec2, Vec3, Visible, World};
use glam::Mat4;

pub(crate) struct View {
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
            c.valid()
                .then(|| {
                    w.global(e).map(|pose| Self {
                        pose,
                        camera: *c,
                        size,
                    })
                })
                .flatten()
        })
    }
    fn projection(&self) -> Mat4 {
        self.camera.matrix(self.size)
    }
    pub fn ray(&self, p: Vec2) -> (Vec3, Vec3) {
        let ndc = Vec3::new(p.x / self.size.x * 2. - 1., 1. - p.y / self.size.y * 2., 0.);
        let inverse = self.projection().inverse();
        let near = inverse.project_point3(ndc);
        let far = inverse.project_point3(ndc.with_z(1.));
        let origin = if matches!(self.camera.projection, crate::Projection::Perspective) {
            Vec3::ZERO
        } else {
            near.with_z(0.)
        };
        (
            self.pose.transform_point3(origin),
            self.pose.transform_vector3(far - near).normalize(),
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
            c.z >= 0.0,
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
        Some(Mesh::Asset(_)) => Vec3::ZERO,
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
    if let Some(e) = w.get::<crate::Emitter>(entity).filter(|_| mesh.is_none()) {
        let lo = Vec3::from_slice(&e.bound[..3]);
        let hi = Vec3::from_slice(&e.bound[3..]);
        return ((hi - lo) * 0.5, (hi + lo) * 0.5);
    }
    if let Some(s) = w.get::<crate::Sprite>(entity).filter(|_| mesh.is_none()) {
        return (
            s.size.extend(0.001) * 0.5,
            ((crate::Vec2::splat(0.5) - s.anchor) * s.size).extend(0.),
        );
    }
    if let Some(Mesh::Asset(name)) = mesh {
        if let Some(model) = w.model(name) {
            let bounds = w
                .get::<crate::Pose>(entity)
                .map_or_else(|| crate::asset::pose::animated_bounds(model), |p| p.bounds);
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
        if w.get::<Visible>(e).is_some_and(|v| !v.0) {
            continue;
        }
        let Some(pose) = w.global(e) else {
            continue;
        };
        if pose.matrix3.determinant().abs() < 1e-12 {
            continue;
        }
        let inv = pose.inverse();
        let o = inv.transform_point3(origin);
        let d = inv.transform_vector3(direction);
        let t = match mesh {
            Mesh::Cylinder { radius, height } => cylinder(o, d, *radius, *height),
            Mesh::Sphere { radius } => sphere(o, d, Vec3::ZERO, *radius),
            Mesh::Capsule { radius, height } => capsule(o, d, *radius, *height - 2.0 * radius),
            Mesh::Asset(name)
                if w.model(name).is_none() && w.get::<crate::asset::ModelBounds>(e).is_none() =>
            {
                None
            }
            _ => {
                let (half, center) = bounds(w, e, Some(mesh));
                slab(o - center, d, half)
            }
        };
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
    for (e, sprite) in w.query::<&crate::Sprite>().iter() {
        if w.get::<Visible>(e).is_some_and(|v| !v.0) {
            continue;
        }
        let pose = displayed_bounds_pose(w, e, Some(view));
        if pose.matrix3.determinant().abs() < 1e-12 {
            continue;
        }
        let inv = pose.inverse();
        let o = inv.transform_point3(origin);
        let d = inv.transform_vector3(direction);
        if d.z.abs() < 1e-8 {
            continue;
        }
        let t = -o.z / d.z;
        let p = o + d * t;
        let lo = -sprite.anchor * sprite.size;
        let hi = lo + sprite.size;
        let point = origin + direction * t;
        let depth = -view.pose.inverse().transform_point3(point).z;
        if t >= 0.
            && p.x >= lo.x
            && p.x <= hi.x
            && p.y >= lo.y
            && p.y <= hi.y
            && depth >= view.camera.near
            && depth <= view.camera.far
            && hit.is_none_or(|(_, old, _)| t < old)
        {
            hit = Some((e, t, point));
        }
    }

    hit
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
pub(crate) struct Layout {
    pub pose: Affine3A,
    pub corners: [Vec3; 8],
    pub screen: Option<[f32; 4]>,
    pub visibility: Option<(bool, bool, f32, f32)>,
    pub unbounded: bool,
}
fn displayed_bounds_pose(w: &World, entity: Entity, view: Option<&View>) -> Affine3A {
    let pose = w.global(entity).unwrap_or(Affine3A::IDENTITY);
    if w.has::<crate::Sprite>(entity) {
        let (scale, _, position) = pose.to_scale_rotation_translation();
        let rotation = view.map_or(crate::Quat::IDENTITY, |v| {
            v.pose.to_scale_rotation_translation().1
        });
        Affine3A::from_scale_rotation_translation(scale.abs(), rotation, position)
    } else {
        pose
    }
}
pub(crate) fn layout(world: &World, viewport: Vec2, entity: Entity) -> Layout {
    let pose = displayed_bounds_pose(world, entity, View::new(world, viewport).as_ref());
    let mesh = world.get::<Mesh>(entity);
    let (half, center) = bounds(world, entity, mesh.as_deref());
    let corners = corners(pose, half, center);
    let view = View::new(world, viewport);
    Layout {
        pose,
        corners,
        screen: view.as_ref().and_then(|v| v.screen(&corners)),
        visibility: view
            .as_ref()
            .map(|v| v.visibility(&corners, pose.translation.into())),
        unbounded: matches!(mesh.as_deref(), Some(Mesh::Asset(name)) if world.model(name).is_none() && world.get::<crate::asset::ModelBounds>(entity).is_none()),
    }
}
impl<G: crate::Game> crate::Sim<G> {
    /// Project an entity using the same geometry and viewport as agent layout.
    /// Missing entities, cameras and entirely near-clipped bounds return None.
    pub fn layout(&self, target: impl crate::Target) -> Option<EntityLayout> {
        let entity = target.entity(&self.world)?;
        let [x, y, w, h] = layout(&self.world, self.input.viewport, entity).screen?;
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
