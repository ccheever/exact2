//! A Contract child on an entity's plane. No text, widgets, or saved presentation.
use crate::{Component, Data, Transform, Vec2, Vec3};
use glam::Mat4;

/// Orientation of a child's plane; a fixed plane's front is local +Z.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Facing {
    /// Follow the displayed camera's right and up axes.
    #[default]
    Camera,
    /// Follow the entity's displayed rotation, with a visible front only.
    Fixed,
}

/// A direct Contract child, selected by `testId` or by its current order.
#[derive(Clone, Debug, PartialEq, Eq, Data)]
pub enum CanvasChild {
    /// Zero-based direct child index, useful for generated lists.
    Index(u16),
    /// The child's Contract `testId`, independent of its order.
    Name(String),
}
impl Default for CanvasChild {
    fn default() -> Self {
        Self::Index(0)
    }
}
impl From<u16> for CanvasChild {
    fn from(index: u16) -> Self {
        Self::Index(index)
    }
}
impl From<&str> for CanvasChild {
    fn from(name: &str) -> Self {
        Self::Name(name.into())
    }
}
impl From<String> for CanvasChild {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

/// Place a direct Contract canvas child on this entity.
#[derive(Clone, Debug, PartialEq, Component)]
pub struct Placed {
    /// Direct child name or index; the selector is retained in saves.
    pub child: CanvasChild,
    /// Width in world units; height follows the child's kernel aspect ratio.
    pub width: f32,
    /// Pivot from the lower-left corner; default bottom-centre.
    pub anchor: [f32; 2],
    /// Camera-facing name-plate or fixed signpost.
    pub facing: Facing,
    /// Local displacement from the entity origin.
    pub offset: Vec3,
}
impl Default for Placed {
    fn default() -> Self {
        Self {
            child: CanvasChild::default(),
            width: 1.,
            anchor: [0.5, 0.],
            facing: Facing::Camera,
            offset: Vec3::ZERO,
        }
    }
}
impl Placed {
    /// Select a direct Contract child by `testId` or zero-based index.
    pub fn child(child: impl Into<CanvasChild>) -> Self {
        Self {
            child: child.into(),
            ..Self::default()
        }
    }
    /// Set width in world units.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    /// Select how the plane faces.
    pub fn facing(mut self, facing: Facing) -> Self {
        self.facing = facing;
        self
    }
    /// Validate authored geometry before presentation.
    pub fn validate(&self) -> Result<(), String> {
        if matches!(&self.child, CanvasChild::Name(name) if name.is_empty()) {
            return Err("Placed: child name must not be empty".into());
        }
        if self.width.is_finite()
            && self.width > 0.
            && self.anchor.iter().all(|n| n.is_finite())
            && self.offset.is_finite()
        {
            Ok(())
        } else {
            Err(format!(
                "Placed child {:?}: expected finite positive width, anchor and offset",
                self.child
            ))
        }
    }
}

/// Derived presentation only: never saved or hashed with the component.
#[derive(Clone, Copy, Debug)]
pub struct PlacedPlane {
    /// Top-left, top-right, bottom-right, bottom-left world corners.
    pub corners: [Vec3; 4],
    /// Child-local points to canvas points, row major.
    pub homography: [f32; 9],
    /// Larger nearer the eye: negative view distance, shared by draw/hit order.
    pub depth: f32,
    /// Explicitly absent from presentation, including behind the near plane.
    pub hidden: bool,
    /// Native clip-volume visibility; a crossing plane still submits geometry.
    pub native_hidden: bool,
    /// Near/far inequalities in child coordinates (ax + by + c >= 0).
    pub clip_depth: [[f32; 3]; 2],
}
impl Placed {
    /// Project the displayed entity and camera poses into a host's canvas points.
    pub fn project(
        &self,
        pose: Transform,
        child_size: Vec2,
        view: Mat4,
        proj: Mat4,
        size: Vec2,
    ) -> PlacedPlane {
        self.project_affine(
            Mat4::from_scale_rotation_translation(pose.scale, pose.rotation, pose.position),
            child_size,
            view,
            proj,
            size,
        )
    }
    /// Project a full displayed affine pose, including hierarchy shear.
    pub fn project_affine(
        &self,
        pose: Mat4,
        child_size: Vec2,
        view: Mat4,
        proj: Mat4,
        size: Vec2,
    ) -> PlacedPlane {
        let camera = view.inverse();
        let origin = pose.transform_point3(self.offset);
        let (right, up) = match self.facing {
            Facing::Camera => (
                camera.x_axis.truncate().normalize() * pose.x_axis.truncate().length(),
                camera.y_axis.truncate().normalize() * pose.y_axis.truncate().length(),
            ),
            Facing::Fixed => (pose.x_axis.truncate(), pose.y_axis.truncate()),
        };
        let x = right * (self.width);
        let y = up * (self.width * child_size.y / child_size.x.max(f32::MIN_POSITIVE));
        let center = origin + x * (0.5 - self.anchor[0]) + y * (0.5 - self.anchor[1]);
        let corners = [
            center - x * 0.5 + y * 0.5,
            center + x * 0.5 + y * 0.5,
            center + x * 0.5 - y * 0.5,
            center - x * 0.5 - y * 0.5,
        ];
        let vp = proj * view;
        let clip = corners.map(|p| vp * p.extend(1.));
        // Homogeneous polygon clipping catches disjoint viewport corners and
        // keeps native geometry visible while it crosses the near/eye planes.
        let outside = !intersects_frustum(clip);
        let depth_clipped = clip.iter().any(|p| p.w <= 0. || p.z < 0. || p.z > p.w);
        let behind =
            self.facing == Facing::Fixed && x.cross(y).dot(camera.w_axis.truncate() - origin) <= 0.;
        let map =
            |p: glam::Vec4| Vec3::new((p.x + p.w) * size.x * 0.5, (p.w - p.y) * size.y * 0.5, p.w);
        // The plane is affine in homogeneous clip space. Its two edge differences
        // give the closed-form homography without a solve or a perspective divide.
        let a = map(clip[0]);
        let dx = (map(clip[1]) - a) / child_size.x;
        let dy = (map(clip[3]) - a) / child_size.y;
        let h = [dx.x, dy.x, a.x, dx.y, dy.y, a.y, dx.z, dy.z, a.z];
        let native_hidden = child_size.x <= 0.
            || child_size.y <= 0.
            || outside
            || behind
            || !h.iter().all(|n| n.is_finite())
            || x.length_squared() == 0.
            || y.length_squared() == 0.;
        let hidden = native_hidden || depth_clipped;
        let plane = |v: [f32; 4]| {
            [
                (v[1] - v[0]) / child_size.x,
                (v[3] - v[0]) / child_size.y,
                v[0],
            ]
        };
        let clip_depth = [plane(clip.map(|p| p.z)), plane(clip.map(|p| p.w - p.z))];
        PlacedPlane {
            corners,
            homography: if native_hidden { [0.; 9] } else { h },
            depth: view.transform_point3(center).z,
            hidden,
            native_hidden,
            clip_depth,
        }
    }
}

fn intersects_frustum(corners: [glam::Vec4; 4]) -> bool {
    let mut polygon = [glam::Vec4::ZERO; 12];
    polygon[..4].copy_from_slice(&corners);
    let mut count = 4;
    for plane in 0..7 {
        let distance = |p: glam::Vec4| match plane {
            0 => p.w - 1e-6,
            1 => p.z,
            2 => p.w - p.z,
            3 => p.x + p.w,
            4 => p.w - p.x,
            5 => p.y + p.w,
            _ => p.w - p.y,
        };
        let mut next = [glam::Vec4::ZERO; 12];
        let mut n = 0;
        let mut a = polygon[count - 1];
        let mut da = distance(a);
        for &b in &polygon[..count] {
            let db = distance(b);
            if (da >= 0.) != (db >= 0.) {
                next[n] = a.lerp(b, da / (da - db));
                n += 1;
            }
            if db >= 0. {
                next[n] = b;
                n += 1;
            }
            a = b;
            da = db;
        }
        if n == 0 {
            return false;
        }
        polygon = next;
        count = n;
    }
    true
}
