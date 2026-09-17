use crate::{Affine3A, Component, Entity, Quat, Vec3, World};
use std::collections::{BTreeMap, BTreeSet};

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

/// Hierarchy edge. Despawning a parent also despawns its descendants.
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

/// Parallel light rays along the entity's negative Z axis.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct DirectionalLight {
    /// Linear RGB light color.
    pub color: [f32; 3],
    /// Illuminance in lux.
    pub illuminance: f32,
}
impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            illuminance: 10000.0,
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
    }
    /// Rebuild global poses, following arbitrary-depth parent chains iteratively.
    /// A stale parent is a root; a parent without Transform contributes identity.
    /// Cycles panic with the offending index instead of hanging or overflowing.
    pub fn propagate(&mut self) {
        let local: BTreeMap<_, _> = self
            .query::<&Transform>()
            .iter()
            .map(|(e, t)| (e, t.affine()))
            .collect();
        let parents: BTreeMap<_, _> = self
            .query::<&Parent>()
            .iter()
            .map(|(e, p)| (e, p.0))
            .collect();
        let mut computed = BTreeMap::new();
        for &start in local.keys() {
            if computed.contains_key(&start) {
                continue;
            }
            let mut path = Vec::new();
            let mut visiting = BTreeSet::new();
            let mut e = start;
            let mut base = loop {
                if let Some(&g) = computed.get(&e) {
                    break g;
                }
                assert!(
                    visiting.insert(e),
                    "transform hierarchy cycle at #{}",
                    e.index()
                );
                path.push(e);
                if let Some(&parent) = parents.get(&e).filter(|&&p| self.contains(p)) {
                    e = parent;
                } else {
                    break Affine3A::IDENTITY;
                }
            };
            while let Some(e) = path.pop() {
                base *= local.get(&e).copied().unwrap_or(Affine3A::IDENTITY);
                computed.insert(e, base);
            }
        }
        computed.retain(|e, _| local.contains_key(e));
        if self.propagated_tick != Some(self.tick()) {
            self.previous = std::mem::take(&mut self.globals);
            self.propagated_tick = Some(self.tick());
        }
        for (&e, &g) in &computed {
            self.previous.entry(e).or_insert(g);
        }
        self.previous.retain(|e, _| computed.contains_key(e));
        self.globals = computed;
    }
    /// Cached world-space pose after propagate.
    pub fn global(&self, e: Entity) -> Option<Affine3A> {
        self.globals.get(&e).copied()
    }
    /// Interpolate affine columns between ticks, retaining shear and zero scales.
    /// Alpha is clamped; endpoints return the original matrices bit-for-bit.
    pub fn global_lerp(&self, e: Entity, alpha: f32) -> Option<Affine3A> {
        let now = self.global(e)?;
        let old = self.previous.get(&e).copied().unwrap_or(now);
        if alpha <= 0.0 {
            return Some(old);
        }
        if alpha >= 1.0 {
            return Some(now);
        }
        Some(Affine3A {
            matrix3: old.matrix3 * (1.0 - alpha) + now.matrix3 * alpha,
            translation: old.translation * (1.0 - alpha) + now.translation * alpha,
        })
    }
    /// Set a local pose and snap both cached ticks for it and its descendants.
    pub fn teleport(&mut self, e: Entity, transform: Transform) {
        self.insert(e, transform);
        self.propagate();
        let mut edges: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
        for (child, p) in self.query::<&Parent>().iter() {
            edges.entry(p.0).or_default().push(child);
        }
        let mut stack = vec![e];
        let mut seen = BTreeSet::new();
        while let Some(e) = stack.pop() {
            if !seen.insert(e) {
                continue;
            }
            if let Some(g) = self.global(e) {
                self.previous.insert(e, g);
            }
            if let Some(children) = edges.get(&e) {
                stack.extend(children);
            }
        }
    }
}
