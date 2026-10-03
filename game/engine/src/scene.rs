//! Dimensioned scene components and explicit scene functions.
use crate::{Affine3A, Component, Entity, Quat, Vec3, World};

/// Local pose; identity is an unmodified object, with forward along negative Z.
/// Upload layout: ten contiguous f32s (position xyz, rotation xyzw, scale xyz).
#[derive(Clone, Copy, Debug, PartialEq, Component)]
#[repr(C)]
pub struct Transform {
    /// Local translation.
    pub position: Vec3,
    /// Local orientation.
    pub rotation: Quat,
    /// Local axis scales.
    pub scale: Vec3,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}
impl Transform {
    /// Apply a model-local displacement through this transform's scale and rotation.
    pub fn translate_local(&mut self, displacement: Vec3) {
        self.position += self.rotation * (self.scale * displacement);
    }
    /// An identity pose translated to x, y, z.
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: Vec3::new(x, y, z),
            ..Self::default()
        }
    }
    /// Aim negative Z at target, with up selecting the roll.
    pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Self {
        let forward = (target - self.position).normalize();
        let right = forward.cross(up).normalize();
        assert!(
            forward.is_finite() && right.is_finite(),
            "look direction and up must span a plane"
        );
        self.rotation = Quat::from_mat3(&glam::Mat3::from_cols(
            right,
            right.cross(forward),
            -forward,
        ));
        self
    }
    /// Set local axis scales; a scalar supplies a uniform scale.
    pub fn with_scale(mut self, scale: impl IntoScale) -> Self {
        self.scale = scale.into_scale();
        self
    }
    fn affine(self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}
/// Scalar or per-axis input to Transform::with_scale.
pub trait IntoScale {
    /// Convert into per-axis scales.
    fn into_scale(self) -> Vec3;
}
impl IntoScale for f32 {
    fn into_scale(self) -> Vec3 {
        Vec3::splat(self)
    }
}
impl IntoScale for Vec3 {
    fn into_scale(self) -> Vec3 {
        self
    }
}
impl IntoScale for [f32; 3] {
    fn into_scale(self) -> Vec3 {
        Vec3::from_array(self)
    }
}

/// Hierarchy edge. Descendants of a dead parent leave at the end of the tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Component)]
pub struct Parent(pub Entity);

/// Camera projection; integer scaling treats one world unit as one logical pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, crate::Data)]
pub enum Projection {
    /// Perspective using Camera::fov_y_degrees.
    #[default]
    Perspective,
    /// Orthographic world units per screen height, optionally snapped to whole pixels.
    Orthographic {
        /// Vertical world extent at the authored resolution.
        height: f32,
        /// Use whole viewport (CSS) pixels per world unit; below 1x, crop at 1x.
        integer_scale: bool,
    },
}
/// Perspective or orthographic camera, sharing projection math with spatial reads.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct Camera {
    /// Projection choice; always encoded, including perspective.
    pub projection: Projection,
    /// Vertical field of view in degrees.
    pub fov_y_degrees: f32,
    /// Near clipping distance.
    pub near: f32,
    /// Far clipping distance.
    pub far: f32,
    /// Whether this camera is eligible for presentation.
    pub active: bool,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective,
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
            active: true,
        }
    }
}

impl Camera {
    /// Orthographic vertical extent in world units, looking along negative Z.
    pub fn orthographic(height: f32) -> Self {
        Self {
            projection: Projection::Orthographic {
                height,
                integer_scale: false,
            },
            ..Self::default()
        }
    }
    /// Snap the orthographic ratio down to whole CSS pixels per world unit. Extra world
    /// area becomes visible at noninteger ratios; smaller canvases crop at 1x.
    pub fn integer_scale(mut self) -> Self {
        if let Projection::Orthographic { integer_scale, .. } = &mut self.projection {
            *integer_scale = true;
        }
        self
    }
    /// Validate camera selection in both the renderer and spatial reads.
    pub fn valid(self) -> bool {
        self.active
            && self.near.is_finite()
            && self.far.is_finite()
            && self.near > 0.
            && self.far > self.near
            && match self.projection {
                Projection::Perspective => self.fov_y_degrees > 0. && self.fov_y_degrees < 180.,
                Projection::Orthographic { height, .. } => height.is_finite() && height > 0.,
            }
    }
    /// Shared WebGPU-depth projection. Size is the CSS-pixel viewport on every
    /// surface, so layout/pick and rendering agree across display scales.
    pub fn matrix(self, size: crate::Vec2) -> glam::Mat4 {
        match self.projection {
            Projection::Perspective => glam::camera::rh::proj::directx::perspective(
                self.fov_y_degrees.to_radians(),
                size.x / size.y,
                self.near,
                self.far,
            ),
            Projection::Orthographic {
                height,
                integer_scale,
            } => {
                let height = if integer_scale {
                    size.y / crate::math::floor(size.y / height).max(1.)
                } else {
                    height
                };
                let width = height * size.x / size.y;
                glam::Mat4::from_cols_array(&[
                    2. / width,
                    0.,
                    0.,
                    0.,
                    0.,
                    2. / height,
                    0.,
                    0.,
                    0.,
                    0.,
                    1. / (self.near - self.far),
                    0.,
                    0.,
                    0.,
                    self.near / (self.near - self.far),
                    1.,
                ])
            }
        }
    }
}

/// Dimensioned geometry. Metres, Y up, centred on the origin; capsules and
/// cylinders run along Y and height is tip to tip. Planes lie in XZ facing +Y.
/// Transform scale stretches these dimensions on top; assets use their baked bounds.
#[derive(Clone, Debug, PartialEq, Component)]
pub enum Mesh {
    /// Box with full axis lengths.
    Box {
        /// Full dimensions.
        size: Vec3,
    },
    /// Sphere.
    Sphere {
        /// Radius.
        radius: f32,
    },
    /// Capsule.
    Capsule {
        /// Hemisphere radius.
        radius: f32,
        /// Total height.
        height: f32,
    },
    /// Rectangle in XZ.
    Plane {
        /// X extent.
        width: f32,
        /// Z extent.
        depth: f32,
    },
    /// Cylinder.
    Cylinder {
        /// Radius.
        radius: f32,
        /// Total height.
        height: f32,
    },
    /// A baked asset's stable name.
    Asset(String),
}
impl Default for Mesh {
    fn default() -> Self {
        Self::cube(1.0)
    }
}
impl Mesh {
    /// Equal-sided box.
    pub fn cube(size: f32) -> Self {
        Self::cuboid(Vec3::splat(size))
    }
    /// Box with full axis lengths.
    pub fn cuboid(size: Vec3) -> Self {
        Self::Box { size }
    }
    /// Sphere of the given radius.
    pub fn sphere(radius: f32) -> Self {
        Self::Sphere { radius }
    }
    /// Capsule with a hemisphere at each end.
    pub fn capsule(radius: f32, height: f32) -> Self {
        Self::Capsule { radius, height }
    }
    /// Cylinder with flat caps.
    pub fn cylinder(radius: f32, height: f32) -> Self {
        Self::Cylinder { radius, height }
    }
    /// Rectangle in XZ, facing +Y.
    pub fn plane(width: f32, depth: f32) -> Self {
        Self::Plane { width, depth }
    }
    /// Baked model by name.
    pub fn asset(name: impl Into<String>) -> Self {
        Self::Asset(name.into())
    }
    /// Refuse invalid dimensions by primitive name, before rendering or colliding.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        let (name, valid) = match self {
            Self::Box { size } => ("Box", size.is_finite() && size.min_element() > 0.0),
            Self::Sphere { radius } => ("Sphere", positive(*radius)),
            Self::Capsule { radius, height } => (
                "Capsule",
                positive(*radius) && positive(*height) && *height >= 2.0 * radius,
            ),
            Self::Cylinder { radius, height } => {
                ("Cylinder", positive(*radius) && positive(*height))
            }
            Self::Plane { width, depth } => ("Plane", positive(*width) && positive(*depth)),
            Self::Asset(_) => return Ok(()),
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "Mesh.{name}: invalid dimensions {}",
                crate::json::to_string(self).unwrap_or_else(|_| "non-finite".into())
            ))
        }
    }
}

/// Saved emissive intensity. The renderer samples this tween at frame time and
/// multiplies the authored material emissive. Writing that intensity into both
/// Glow and Material applies it twice; keep the material's authored color constant.
/// A soft highlight shoulder applies only to this component's emissive output.
#[derive(Clone, Debug, Default, Component)]
pub struct Glow(pub crate::Tween);

/// Saved point- or spot-light intensity multiplier, sampled by the renderer at
/// frame time. Negative spring overshoot clamps to zero; the authored light stays constant.
#[derive(Clone, Debug, Default, Component)]
pub struct Lit(pub crate::Spring);
impl Lit {
    /// Retarget once, preserving the current value and velocity.
    pub fn to(&mut self, now: crate::Now, intensity: f32) {
        self.0.set_target(now, intensity);
    }
}

/// Renderer-neutral surface properties: ten contiguous f32s including grid spacing.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
#[repr(C)]
pub struct Material {
    /// Linear RGBA base color.
    pub color: [f32; 4],
    /// Metal fraction, normally in [0, 1].
    pub metallic: f32,
    /// Surface roughness, normally in [0, 1].
    pub roughness: f32,
    /// Linear RGB emitted light.
    pub emissive: [f32; 3],
    /// World-space grid spacing; zero disables the procedural grid.
    pub grid_spacing: f32,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            color: [1.0; 4],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0; 3],
            grid_spacing: 0.0,
        }
    }
}

impl Material {
    /// Opaque linear RGB with the default surface properties.
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self {
            color: [r, g, b, 1.0],
            ..Self::default()
        }
    }
    /// A world-space grid on any surface; colour is the base, spacing is in metres.
    /// Lines use screen-space derivatives and fade to their average below a pixel.
    pub fn grid(color: [f32; 3], spacing: f32) -> Self {
        assert!(
            spacing.is_finite() && spacing > 0.0,
            "grid spacing must be positive and finite"
        );
        Self {
            grid_spacing: spacing,
            ..Self::rgb(color[0], color[1], color[2])
        }
    }
    /// Emissive black surface. HDR values bloom with the default Environment;
    /// this is a glowing mesh, not an extra halo shell or an unlit shader.
    /// Add a [`Glow`] component to animate its intensity without changing Material.
    pub fn glow(color: [f32; 3]) -> Self {
        Self::rgb(0.0, 0.0, 0.0).emissive(color[0], color[1], color[2])
    }
    /// Set linear RGB emitted light.
    pub fn emissive(mut self, r: f32, g: f32, b: f32) -> Self {
        self.emissive = [r, g, b];
        self
    }
    /// Set the metal fraction.
    pub fn metallic(mut self, m: f32) -> Self {
        self.metallic = m;
        self
    }
    /// Set surface roughness.
    pub fn rough(mut self, r: f32) -> Self {
        self.roughness = r;
        self
    }
}

/// Parallel light rays along the entity's negative Z axis. The renderer uses the
/// first two in entity order: the first is the sun (shadowed when `shadows`), the
/// second an unshadowed fill such as the moon.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct DirectionalLight {
    /// Linear RGB light color.
    pub color: [f32; 3],
    /// Illuminance in lux.
    pub illuminance: f32,
    /// Cast sun shadows; enabled by default.
    pub shadows: bool,
}
impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            illuminance: 10000.0,
            shadows: true,
        }
    }
}

/// An omnidirectional light at the entity's position.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct PointLight {
    /// Linear RGB light color.
    pub color: [f32; 3],
    /// Luminous intensity in candela: `d` metres away it delivers
    /// `intensity / d²` lux, on the same scale as `DirectionalLight::illuminance`.
    pub intensity: f32,
    /// Maximum influence distance.
    pub range: f32,
}
impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            intensity: 100.0,
            range: 10.0,
        }
    }
}

/// Marks a `PointLight` or `SpotLight` that casts shadows; lights are
/// unshadowed without it. The renderer shadows the nearest few marked lights
/// each frame (a spot takes one shadow view, a point light six of eight).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Component)]
pub struct LightShadows;

/// A cone of light along the entity's −Z, the way a camera looks: a flashlight
/// or a street lamp. Intensity is candela on the axis, as `PointLight`'s.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct SpotLight {
    /// Linear RGB light color.
    pub color: [f32; 3],
    /// Luminous intensity on the axis in candela.
    pub intensity: f32,
    /// Maximum influence distance.
    pub range: f32,
    /// Half-angle in radians inside which the cone is at full intensity.
    pub inner: f32,
    /// Half-angle in radians at which the cone reaches zero; at most π/2.
    pub outer: f32,
}
impl Default for SpotLight {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            intensity: 1000.0,
            range: 20.0,
            inner: 0.3,
            outer: 0.45,
        }
    }
}

/// Draws this entity in the camera's viewmodel layer: a first-person weapon or
/// hands. The renderer gives the layer the nearest slice of depth, so it is
/// never hidden inside walls, and it casts no shadows. Mark each part.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Component)]
pub struct ViewModel;

/// Explicit visibility; absent visibility is interpreted as visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub struct Visible(pub bool);
impl Default for Visible {
    fn default() -> Self {
        Self(true)
    }
}

impl World {
    /// Reap dead-parent children in entity order, repeating for orphaned chains.
    /// Sim calls this once after Game::tick and before propagate.
    pub fn reap_orphans(&mut self) {
        // Reaping and propagation use this entity scratch sequentially.
        let mut orphans = std::mem::take(&mut self.hierarchy.entities);
        loop {
            orphans.clear();
            for (e, p) in self.query::<&Parent>().iter() {
                if !self.contains(p.0) {
                    orphans.push(e);
                }
            }
            if orphans.is_empty() {
                break;
            }
            for &e in &orphans {
                self.despawn(e);
            }
        }
        self.hierarchy.entities = orphans;
    }
    /// Resolve only parented entities, reusing indexed scratch and chain stamps.
    /// Stale parents act as roots; parents without Transform contribute identity.
    /// Runtime cycles lose the highest-index edge, with one journal line per cycle.
    pub fn propagate(&mut self) {
        self.resolve_hierarchy(false)
            .expect("runtime cycles are repaired");
    }
    pub(crate) fn validate_hierarchy(
        &mut self,
        r: &mut dyn crate::Reader,
    ) -> Result<(), crate::DataError> {
        let count = self.storage::<Parent>().map_or(0, |s| s.len());
        if count != 0 {
            let slots = self.alive_mask.len() * 64;
            crate::data::limits::reserve(r, &mut self.hierarchy.nodes, slots)?;
            crate::data::limits::reserve(r, &mut self.hierarchy.entities, count)?;
            crate::data::limits::reserve(r, &mut self.hierarchy.path, count)?;
            crate::data::limits::reserve(r, &mut self.hierarchy.broken, count)?;
        }
        self.resolve_hierarchy(true)
    }
    fn resolve_hierarchy(&mut self, reject: bool) -> Result<(), crate::DataError> {
        if self.storage::<Parent>().is_none_or(|s| s.is_empty()) {
            return Ok(());
        }
        let mut h = std::mem::take(&mut self.hierarchy);
        let result = h.resolve(self, reject);
        for &e in &h.broken {
            self.remove::<Parent>(e);
            self.log(format_args!(
                "transform hierarchy cycle: #{} treated as root",
                e.index()
            ));
        }
        self.hierarchy = h;
        result
    }
    /// World pose: a root reads its local Transform directly, without propagation.
    /// Parented poses reflect the last propagate call.
    #[track_caller]
    pub fn global(&self, e: Entity) -> Option<Affine3A> {
        // Socket-derived gameplay poses are tick-boundary reads, not stored hierarchy nodes.
        if self.attachments.is_some() {
            return self.current_global(e);
        }
        if !self.has::<Parent>(e) {
            let at = std::panic::Location::caller();
            return self
                .copied_at::<Transform>(e, at)
                .map(|local| local.affine());
        }
        self.hierarchy
            .nodes
            .get(e.index() as usize)
            .filter(|n| n.entity == e && n.done == self.hierarchy.stamp)
            .map(|n| n.global)
    }
    /// Resolve the current local poses through the parent chain, before propagation.
    #[track_caller]
    pub fn current_global(&self, e: Entity) -> Option<Affine3A> {
        self.current_global_at(e, std::panic::Location::caller())
    }
    pub(crate) fn current_global_at(&self, e: Entity, at: crate::storage::At) -> Option<Affine3A> {
        self.current_global_depth_at(e, self.len() + 1, at)
    }
    #[track_caller]
    pub(crate) fn current_global_depth(&self, e: Entity, remaining: usize) -> Option<Affine3A> {
        self.current_global_depth_at(e, remaining, std::panic::Location::caller())
    }
    // Each link leases one row at a time, reported at the author's call.
    fn current_global_depth_at(
        &self,
        e: Entity,
        remaining: usize,
        caller: crate::storage::At,
    ) -> Option<Affine3A> {
        if remaining == 0 {
            return None;
        }
        if let Some(pose) = self
            .attachments
            .and_then(|attachments| (attachments.pose)(self, e, remaining))
        {
            return Some(pose);
        }
        let mut pose = self.copied_at::<Transform>(e, caller)?.affine();
        let mut at = e;
        for _ in 0..self.len() {
            let Some(parent) = self
                .copied_at::<Parent>(at, caller)
                .map(|p| p.0)
                .filter(|p| self.contains(*p))
            else {
                return Some(pose);
            };
            if let Some(parent_pose) = self
                .attachments
                .and_then(|attachments| (attachments.pose)(self, parent, remaining - 1))
            {
                return Some(parent_pose * pose);
            }
            pose = self
                .copied_at::<Transform>(parent, caller)
                .map_or(Affine3A::IDENTITY, |t| t.affine())
                * pose;
            at = parent;
        }
        None // A runtime cycle has not yet been repaired by propagate.
    }
    /// Set a local pose, refresh parented globals and mark this entity fresh.
    pub fn teleport(&mut self, e: Entity, transform: Transform) {
        if self.insert(e, transform) {
            self.mark_fresh(e);
            for (_, follow) in self.query::<&mut Follow>().iter() {
                if follow.target.resolve(self) == Some(e) {
                    follow.initialized = false;
                }
            }
            self.propagate();
        }
    }
}

#[derive(Default)]
pub(crate) struct Hierarchy {
    nodes: Vec<Node>,
    entities: Vec<Entity>,
    path: Vec<Entity>,
    broken: Vec<Entity>,
    stamp: u32,
}
#[derive(Default)]
struct Node {
    entity: Entity,
    parent: Option<Entity>,
    present: u32,
    visiting: u32,
    done: u32,
    global: Affine3A,
}
impl Hierarchy {
    fn resolve(&mut self, w: &World, reject: bool) -> Result<(), crate::DataError> {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.nodes.clear();
            self.stamp = 1;
        }
        let stamp = self.stamp;
        self.entities.clear();
        self.broken.clear();
        // Resolution runs under `&mut World`: one whole-column hold replaces a row
        // lease per node. @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
        let poses = w.pages::<Transform>();
        let local = |e: Entity| {
            poses
                .row(e.index() as usize)
                .map_or(Affine3A::IDENTITY, |t| t.affine())
        };
        for (e, p) in w.query::<&Parent>().iter() {
            let i = e.index() as usize;
            if i >= self.nodes.len() {
                self.nodes.resize_with(i + 1, Node::default);
            }
            let n = &mut self.nodes[i];
            n.entity = e;
            n.parent = w.contains(p.0).then_some(p.0);
            n.present = stamp;
            self.entities.push(e);
        }
        for i in 0..self.entities.len() {
            let start = self.entities[i];
            if self.nodes[start.index() as usize].done == stamp {
                continue;
            }
            self.path.clear();
            let mut e = start;
            let mut base = loop {
                let Some(n) = self
                    .nodes
                    .get_mut(e.index() as usize)
                    .filter(|n| n.present == stamp)
                else {
                    break local(e);
                };
                if n.done == stamp {
                    break n.global;
                }
                if n.visiting == stamp {
                    let begin = self.path.iter().position(|&p| p == e).unwrap();
                    let root = *self.path[begin..].iter().max_by_key(|e| e.index()).unwrap();
                    if reject {
                        return Err(crate::DataError::new(format!(
                            "Parent cycle at #{}",
                            root.index()
                        )));
                    }
                    self.nodes[root.index() as usize].parent = None;
                    self.broken.push(root);
                    for e in self.path.drain(..) {
                        self.nodes[e.index() as usize].visiting = 0;
                    }
                    e = start;
                    continue;
                }
                n.visiting = stamp;
                self.path.push(e);
                if let Some(parent) = n.parent {
                    e = parent;
                } else {
                    break Affine3A::IDENTITY;
                }
            };
            while let Some(e) = self.path.pop() {
                base *= local(e);
                let n = &mut self.nodes[e.index() as usize];
                n.global = base;
                n.done = stamp;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hierarchy_refreshes_removed_recycled_and_wrapped_entries() {
        let mut w = World::new(60, 0);
        let root = w.spawn(Transform::at(1., 0., 0.));
        let child = w.spawn((Transform::at(2., 0., 0.), Parent(root)));
        w.propagate();
        assert_eq!(w.global(child).unwrap().translation.x, 3.);

        w.remove::<Parent>(child);
        w.get_mut::<Transform>(root).unwrap().position.x = 5.;
        w.propagate();
        assert_eq!(w.global(child).unwrap().translation.x, 2.);
        w.insert(child, Parent(root));
        w.propagate();
        assert_eq!(w.global(child).unwrap().translation.x, 7.);

        w.hierarchy.stamp = u32::MAX;
        w.propagate();
        assert_eq!(w.global(child).unwrap().translation.x, 7.);

        w.despawn(child);
        let replacement = w.spawn((Transform::at(9., 0., 0.), Parent(root)));
        assert_eq!(replacement.index(), child.index());
        assert!(w.global(replacement).is_none());
        w.propagate();
        assert_eq!(w.global(replacement).unwrap().translation.x, 14.);
        assert!(w.global(child).is_none());

        w.remove::<Transform>(root);
        w.propagate();
        assert_eq!(w.global(replacement).unwrap().translation.x, 9.);
    }
    #[test]
    fn parent_scratch_is_reused_and_flat_worlds_allocate_none() {
        let mut w = World::new(60, 0);
        let entities: Vec<_> = (0..10_000)
            .map(|_| w.spawn(Transform::at(1.0, 0.0, 0.0)))
            .collect();
        w.propagate();
        assert_eq!(w.hierarchy.nodes.capacity(), 0);
        assert_eq!(w.hierarchy.entities.capacity(), 0);
        for i in 1..6 {
            w.insert(entities[i], Parent(entities[i - 1]));
        }
        w.propagate();
        let buffers = |h: &Hierarchy| {
            (
                h.nodes.as_ptr(),
                h.entities.as_ptr(),
                h.path.as_ptr(),
                h.broken.as_ptr(),
            )
        };
        let first = buffers(&w.hierarchy);
        for _ in 0..10 {
            w.propagate();
        }
        assert_eq!(buffers(&w.hierarchy), first);
        assert_eq!(w.global(entities[5]).unwrap().translation.x, 6.0);
    }
}

/// Changes to this entity's components do not keep the seekable clock awake.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Ambient;

/// A direct follower target, or a name resolved by lowest living index.
#[derive(Clone, Debug, crate::Data)]
pub enum FollowTarget {
    /// Direct entity handle; no name scan.
    Entity(Entity),
    /// A name which follows the same resolution rule as World::named.
    Name(String),
}
impl Default for FollowTarget {
    fn default() -> Self {
        Self::Entity(Entity::default())
    }
}
impl From<Entity> for FollowTarget {
    fn from(e: Entity) -> Self {
        Self::Entity(e)
    }
}
impl From<&str> for FollowTarget {
    fn from(s: &str) -> Self {
        Self::Name(s.into())
    }
}
impl From<String> for FollowTarget {
    fn from(s: String) -> Self {
        Self::Name(s)
    }
}
impl FollowTarget {
    fn resolve(&self, w: &World) -> Option<Entity> {
        match self {
            Self::Entity(e) => w.contains(*e).then_some(*e),
            Self::Name(name) => w.named(name),
        }
    }
}

/// A saved camera follower, stepped by the scene after each game tick.
/// The engine places followers after setup and setup-argument rebuilds. Restore
/// places uninitialized followers while preserving saved, initialized poses and
/// smoothing, so restoring never advances the simulation by an extra step.
#[derive(Clone, Debug, Default, Component)]
pub struct Follow {
    /// Target handle or name. A changed name resolution reinitializes the follow.
    pub target: FollowTarget,
    resolved: Option<Entity>,
    /// Camera displacement from the target.
    pub offset: Vec3,
    /// Aim displacement from the followed position.
    pub look_at_offset: Vec3,
    /// Exponential time constant in seconds; zero snaps immediately.
    pub lag: f32,
    initialized: bool,
}
impl Follow {
    /// Follow the entity with this name; initial placement snaps exactly.
    pub fn new(target: impl Into<FollowTarget>) -> Self {
        Self {
            target: target.into(),
            ..Self::default()
        }
    }
    /// Camera displacement from the target.
    pub fn offset(mut self, x: f32, y: f32, z: f32) -> Self {
        self.offset = Vec3::new(x, y, z);
        self
    }
    /// Aim displacement from the followed position.
    pub fn look_at_offset(mut self, x: f32, y: f32, z: f32) -> Self {
        self.look_at_offset = Vec3::new(x, y, z);
        self
    }
    /// Exponential time constant in seconds.
    pub fn lag(mut self, seconds: f32) -> Self {
        assert!(seconds.is_finite() && seconds >= 0.0);
        self.lag = seconds;
        self
    }
}
/// Optionally step followers earlier in your tick for explicit ordering.
/// The automatic scene step will not step them twice in the same tick.
/// Step followers in entity order. The look direction follows the eased position,
/// so both translation and rotation stop exactly. Missing targets leave the pose alone.
#[track_caller]
pub fn follow(world: &World) {
    if world.in_tick && world.followed.replace(true) {
        return;
    }
    follow_inner(world, false);
}
pub(crate) fn place_followers(world: &World) {
    follow_inner(world, true);
}
#[track_caller]
fn follow_inner(world: &World, placement_only: bool) {
    for (e, follow) in world.query::<&mut Follow>().iter() {
        let target = follow.target.resolve(world);
        if target != follow.resolved {
            follow.initialized = false;
            follow.resolved = target;
        }
        if placement_only && follow.initialized {
            continue;
        }
        let Some(target) = target else {
            continue;
        };
        if target == e {
            continue;
        }
        let Some(target_pose) = world.current_global(target) else {
            continue;
        };
        let target = Vec3::from(target_pose.translation) + follow.offset;
        let Some(current) = world.current_global(e) else {
            continue;
        };
        let next = if follow.initialized {
            crate::math::ease(
                Vec3::from(current.translation),
                target,
                follow.lag,
                world.dt(),
            )
        } else {
            target
        };
        let aim = next - follow.offset + follow.look_at_offset;
        let (scale, rotation, _) = current.to_scale_rotation_translation();
        let mut pose = Transform {
            position: next,
            rotation,
            scale,
        };
        if next != aim {
            let forward = (aim - next).normalize();
            let previous_up = if follow.initialized {
                rotation * Vec3::Y
            } else {
                Vec3::Y
            };
            let transported = previous_up - forward * previous_up.dot(forward);
            let up = if transported.length_squared() > 1e-12 {
                transported.normalize()
            } else if forward.cross(Vec3::Y).length_squared() > 1e-6 {
                Vec3::Y
            } else {
                Vec3::Z
            };
            pose = pose.looking_at(aim, up);
        }
        let caller = std::panic::Location::caller();
        if let Some(parent) = world
            .get::<Parent>(e)
            .and_then(|p| world.current_global_at(p.0, caller))
        {
            let (scale, rotation, position) =
                (parent.inverse() * pose.affine()).to_scale_rotation_translation();
            pose = Transform {
                position,
                rotation,
                scale,
            };
        }
        if let Some(mut transform) = world.get_mut::<Transform>(e) {
            *transform = pose;
        }
        follow.initialized = true;
    }
}

#[cfg(test)]
mod camera_patch_regression {
    use super::*;
    #[test]
    fn perspective_patch_replaces_orthographic_projection() {
        let mut camera = Camera::orthographic(180.).integer_scale();
        crate::bin::read_into(&crate::bin::to_vec(&Camera::default()), &mut camera).unwrap();
        assert_eq!(camera, Camera::default());
        camera = Camera::orthographic(180.);
        crate::json::read_into(
            &crate::json::to_string(&Camera::default()).unwrap(),
            &mut camera,
        )
        .unwrap();
        assert_eq!(camera, Camera::default());
    }
}

#[cfg(test)]
mod e10_tests {
    use crate::*;
    struct Following;
    impl Game for Following {
        const ID: &'static str = "e10-follow";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let player = w.spawn_named("player", Transform::default());
            w.spawn_named(
                "camera",
                (Transform::default(), Follow::new(player).offset(0., 2., 3.)),
            );
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.require_mut::<Transform>("player").position.x += 1.;
        }
    }
    #[test]
    fn scene_steps_follow_after_game_tick_and_restore_does_not_step() {
        let mut sim = Sim::<Following>::new(()).unwrap();
        sim.run(1000.);
        assert_eq!(
            sim.world().require::<Transform>("camera").position.x,
            sim.world().require::<Transform>("player").position.x
        );
        let save = sim.save().unwrap();
        let mut restored = Sim::<Following>::new(()).unwrap();
        restored.restore(&save).unwrap();
        assert_eq!(restored.save().unwrap(), save);
        sim.run(1000.);
        restored.run(1000.);
        assert_eq!(sim.save().unwrap(), restored.save().unwrap());
    }
    #[test]
    fn count_and_proximity_name_are_read_only_and_ignore_despawned_entities() {
        let mut w = World::new(60, 0);
        let a = w.spawn_named("one", (Transform::default(), Mesh::cube(1.)));
        let b = w.spawn_named("two", (Transform::at(1., 0., 0.), Mesh::cube(2.)));
        let hash = w.hash();
        assert_eq!(w.count::<Mesh>(|_| true), 2);
        let (entity, _mesh) = w.nearest_xz_mut::<Mesh>("one", 2., |_| true).unwrap();
        assert_eq!(w.name(entity), Some("two"));
        drop(_mesh);
        assert_eq!(w.hash(), hash);
        w.despawn(a);
        assert_eq!(w.count::<Mesh>(|_| true), 1);
        w.despawn(b);
        assert_eq!(w.count::<Mesh>(|_| true), 0);
    }
}
