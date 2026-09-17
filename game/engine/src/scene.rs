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

/// Perspective camera, in degrees and world-distance units.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct Camera {
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
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
            active: true,
        }
    }
}

/// Geometry identity; a renderer owns its actual vertices.
#[derive(Clone, Debug, Default, PartialEq, Component)]
pub enum Mesh {
    /// A unit cube centered on the origin.
    #[default]
    Cube,
    /// A sphere of radius one.
    Sphere,
    /// A Y-axis capsule.
    Capsule {
        /// Hemisphere radius.
        radius: f32,
        /// Length of the cylindrical segment.
        height: f32,
    },
    /// A square in the XZ plane.
    Plane {
        /// Side length.
        size: f32,
    },
    /// A Y-axis cylinder of radius one and height one.
    Cylinder,
    /// A baked asset's stable name.
    Asset(String),
}

/// Renderer-neutral surface properties: ten contiguous f32s including the pad.
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
    /// Explicit upload padding; transient and initialized to zero by default.
    #[data(skip)]
    pub pad: f32,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            color: [1.0; 4],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0; 3],
            pad: 0.0,
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
    /// Set opacity.
    pub fn alpha(mut self, a: f32) -> Self {
        self.color[3] = a;
        self
    }
}

/// Parallel light rays along the entity's negative Z axis.
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
    /// Luminous intensity in candela.
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

/// Explicit visibility; absent visibility is interpreted as visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub struct Visible(pub bool);
impl Default for Visible {
    fn default() -> Self {
        Self(true)
    }
}

impl World {
    /// Register the renderer's plain-data vocabulary before loading a scene.
    pub fn register_scene(&mut self) -> &mut Self {
        self.register::<Transform>()
            .register::<Parent>()
            .register::<Camera>()
            .register::<Mesh>()
            .register::<Material>()
            .register::<DirectionalLight>()
            .register::<PointLight>()
            .register::<Visible>()
            .register_resource::<crate::Environment>()
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
    pub fn global(&self, e: Entity) -> Option<Affine3A> {
        let local = self.get::<Transform>(e)?;
        if !self.has::<Parent>(e) {
            return Some(local.affine());
        }
        self.hierarchy
            .nodes
            .get(e.index() as usize)
            .filter(|n| n.entity == e && n.done == self.hierarchy.stamp)
            .map(|n| n.global)
    }
    /// Set a local pose, refresh parented globals and mark this entity fresh.
    pub fn teleport(&mut self, e: Entity, transform: Transform) {
        if self.insert(e, transform) {
            self.fresh.push(e);
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
    stamp: u64,
}
#[derive(Default)]
struct Node {
    entity: Entity,
    parent: Option<Entity>,
    present: u64,
    visiting: u64,
    done: u64,
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
                let local = || {
                    w.get::<Transform>(e)
                        .map_or(Affine3A::IDENTITY, |t| t.affine())
                };
                let Some(n) = self
                    .nodes
                    .get_mut(e.index() as usize)
                    .filter(|n| n.present == stamp)
                else {
                    break local();
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
                base *= w
                    .get::<Transform>(e)
                    .map_or(Affine3A::IDENTITY, |t| t.affine());
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
