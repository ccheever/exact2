use crate::Shape;
use exact_game::{Affine3A, Entity, Parent, Quat, Transform, Vec3, World};

pub(crate) const SKIN: f32 = 0.001;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Geometry {
    Round {
        a: Vec3,
        b: Vec3,
        radius: f32,
    },
    Box {
        center: Vec3,
        axes: [Vec3; 3],
        half: Vec3,
    },
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Point {
    pub a: Vec3,
    pub b: Vec3,
    pub separation: f32,
    pub feature: u32,
}
#[derive(Clone, Debug)]
pub(crate) struct Patch {
    pub normal: Vec3,
    pub points: Vec<Point>,
}
impl Patch {
    pub fn one(a: Vec3, b: Vec3, normal: Vec3, feature: u32) -> Self {
        Self {
            normal,
            points: vec![Point {
                a,
                b,
                separation: (b - a).dot(normal),
                feature,
            }],
        }
    }
    pub fn flip(mut self) -> Self {
        self.normal = -self.normal;
        for p in &mut self.points {
            std::mem::swap(&mut p.a, &mut p.b);
        }
        self
    }
    pub fn separation(&self) -> f32 {
        self.points
            .iter()
            .map(|p| p.separation)
            .fold(f32::INFINITY, f32::min)
    }
}

pub(crate) fn unit(v: Vec3, fallback: Vec3) -> Vec3 {
    let l = v.length_squared();
    if l > 1.0e-16 {
        v / l.sqrt()
    } else {
        fallback
    }
}
pub(crate) fn basis(n: Vec3) -> (Vec3, Vec3) {
    let axis = if n.x.abs() < 0.57735 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let t = unit(n.cross(axis), Vec3::Z);
    (t, n.cross(t))
}
pub(crate) fn integrate(q: Quat, spin: Vec3, h: f32) -> Quat {
    let dq = Quat::from_xyzw(spin.x, spin.y, spin.z, 0.0) * q;
    (q + dq * (0.5 * h)).normalize()
}
// Queries must see game writes in this tick, before Sim's final propagation.
pub(crate) fn world_pose(world: &World, entity: Entity) -> Transform {
    let pose = world
        .get::<Transform>(entity)
        .map(|p| *p)
        .unwrap_or_default();
    if !world.has::<Parent>(entity) {
        return pose;
    }
    let mut affine =
        Affine3A::from_scale_rotation_translation(pose.scale, pose.rotation, pose.position);
    let mut e = entity;
    for _ in 0..world.len() {
        let Some(parent) = world.get::<Parent>(e) else {
            break;
        };
        if !world.contains(parent.0) {
            break;
        }
        e = parent.0;
        let p = world.get::<Transform>(e).map(|p| *p).unwrap_or_default();
        affine =
            Affine3A::from_scale_rotation_translation(p.scale, p.rotation, p.position) * affine;
        assert_ne!(
            e,
            entity,
            "physics: transform cycle at {}",
            world.name(entity).unwrap_or("unnamed")
        );
    }
    let (scale, rotation, position) = affine.to_scale_rotation_translation();
    let rebuilt = Affine3A::from_scale_rotation_translation(scale, rotation, position);
    assert!(
        (affine.matrix3.x_axis - rebuilt.matrix3.x_axis).length() < 1e-4
            && (affine.matrix3.y_axis - rebuilt.matrix3.y_axis).length() < 1e-4
            && (affine.matrix3.z_axis - rebuilt.matrix3.z_axis).length() < 1e-4,
        "physics: sheared collider {}",
        world.name(entity).unwrap_or("unnamed")
    );
    Transform {
        position,
        rotation,
        scale,
    }
}
impl Geometry {
    pub fn new(shape: &Shape, p: Transform) -> Self {
        assert!(
            p.position.is_finite()
                && p.rotation.is_finite()
                && p.scale.is_finite()
                && p.scale.min_element() > 0.0,
            "physics: invalid collider pose"
        );
        assert!(
            (p.rotation.length_squared() - 1.0).abs() < 1e-4,
            "physics: rotation must be normalized"
        );
        match *shape {
            Shape::Box { half } => {
                assert!(
                    half.is_finite() && half.min_element() > 0.0,
                    "physics: box extents must be positive"
                );
                Self::Box {
                    center: p.position,
                    axes: [
                        p.rotation * Vec3::X,
                        p.rotation * Vec3::Y,
                        p.rotation * Vec3::Z,
                    ],
                    half: half * p.scale,
                }
            }
            Shape::Sphere { radius } | Shape::Capsule { radius, .. } => {
                assert!(
                    radius.is_finite()
                        && radius > 0.0
                        && (p.scale.max_element() - p.scale.min_element()) < 1e-6,
                    "physics: round colliders need a positive radius and uniform scale"
                );
                let height = match *shape {
                    Shape::Capsule { height, .. } => height,
                    _ => 2.0 * radius,
                };
                assert!(
                    height.is_finite() && height >= 2.0 * radius,
                    "physics: capsule height includes hemispheres"
                );
                let d = p.rotation * Vec3::Y * ((height * 0.5 - radius) * p.scale.y);
                Self::Round {
                    a: p.position - d,
                    b: p.position + d,
                    radius: radius * p.scale.x,
                }
            }
        }
    }
    pub fn aabb(self) -> (Vec3, Vec3) {
        match self {
            Self::Round { a, b, radius } => (
                a.min(b) - Vec3::splat(radius),
                a.max(b) + Vec3::splat(radius),
            ),
            Self::Box { center, axes, half } => {
                let r = axes[0].abs() * half.x + axes[1].abs() * half.y + axes[2].abs() * half.z;
                (center - r, center + r)
            }
        }
    }
    pub fn radius(self) -> f32 {
        let (lo, hi) = self.aabb();
        (hi - lo).length() * 0.5
    }
    pub fn mass(self, mass: f32) -> (f32, Vec3) {
        let (volume, unit_inertia) = match self {
            Self::Box { half: h, .. } => (
                8.0 * h.x * h.y * h.z,
                Vec3::new(
                    h.y * h.y + h.z * h.z,
                    h.x * h.x + h.z * h.z,
                    h.x * h.x + h.y * h.y,
                ) / 3.0,
            ),
            Self::Round { a, b, radius: r } => {
                let h = (b - a).length();
                let cylinder = std::f32::consts::PI * r * r * h;
                let sphere = (4.0 / 3.0) * std::f32::consts::PI * r * r * r;
                let volume = cylinder + sphere;
                let axial = (cylinder * 0.5 * r * r + sphere * 0.4 * r * r) / volume;
                let radial = (cylinder * (r * r / 4.0 + h * h / 12.0)
                    + sphere * (0.4 * r * r + 0.25 * h * h + 0.375 * h * r))
                    / volume;
                (volume, Vec3::new(radial, axial, radial))
            }
        };
        let m = if mass > 0.0 { mass } else { volume * 1000.0 };
        (1.0 / m, Vec3::ONE / (unit_inertia * m))
    }
}

pub(crate) fn collide(a: Geometry, b: Geometry) -> Patch {
    match (a, b) {
        (
            Geometry::Round {
                a: a0,
                b: a1,
                radius: ra,
            },
            Geometry::Round {
                a: b0,
                b: b1,
                radius: rb,
            },
        ) => crate::round::round_round(a0, a1, ra, b0, b1, rb),
        (Geometry::Round { a, b, radius }, target @ Geometry::Box { .. }) => {
            crate::round::round_box(a, b, radius, target)
        }
        (target @ Geometry::Box { .. }, Geometry::Round { a, b, radius }) => {
            crate::round::round_box(a, b, radius, target).flip()
        }
        (a, b) => crate::boxes::box_box(a, b),
    }
}

pub(crate) fn segment(a: Vec3, b: Vec3, p: Vec3) -> (Vec3, f32) {
    let d = b - a;
    let t = if d.length_squared() > 1e-16 {
        ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + d * t, t)
}
pub(crate) fn segments(p: Vec3, q: Vec3, a: Vec3, b: Vec3) -> (Vec3, Vec3, f32, f32) {
    let d = q - p;
    let e = b - a;
    let r = p - a;
    let dd = d.dot(d);
    let ee = e.dot(e);
    let de = d.dot(e);
    if dd < 1e-16 {
        let (y, t) = segment(a, b, p);
        return (p, y, 0.0, t);
    }
    if ee < 1e-16 {
        let (x, s) = segment(p, q, a);
        return (x, a, s, 0.0);
    }
    let denom = dd * ee - de * de;
    let mut s = if denom > 1e-8 * dd * ee {
        ((de * e.dot(r) - d.dot(r) * ee) / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let t = (de * s + e.dot(r)) / ee;
    let t = if t < 0.0 {
        s = (-d.dot(r) / dd).clamp(0.0, 1.0);
        0.0
    } else if t > 1.0 {
        s = ((de - d.dot(r)) / dd).clamp(0.0, 1.0);
        1.0
    } else {
        t
    };
    (p + d * s, a + e * t, s, t)
}
pub(crate) fn endpoint(t: f32) -> u32 {
    if t <= 0.0 {
        0
    } else if t >= 1.0 {
        1
    } else {
        2
    }
}
