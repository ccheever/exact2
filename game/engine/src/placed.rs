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

/// Place a direct canvas child, in Contract order, on this entity.
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct Placed {
    /// Direct child index; zero is usually the unplaced HUD.
    pub child: u16,
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
            child: 0,
            width: 1.,
            anchor: [0.5, 0.],
            facing: Facing::Camera,
            offset: Vec3::ZERO,
        }
    }
}
impl Placed {
    /// Select a direct Contract child.
    pub fn child(child: u16) -> Self {
        Self {
            child,
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
    pub fn validate(self) -> Result<(), String> {
        if self.width.is_finite()
            && self.width > 0.
            && self.anchor.iter().all(|n| n.is_finite())
            && self.offset.is_finite()
        {
            Ok(())
        } else {
            Err(format!(
                "Placed child {}: expected finite positive width, anchor and offset",
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
}
impl Placed {
    /// Project the displayed entity and camera poses into a host's canvas points.
    pub fn project(
        self,
        pose: Transform,
        child_size: Vec2,
        view: Mat4,
        proj: Mat4,
        size: Vec2,
    ) -> PlacedPlane {
        let camera = view.inverse();
        let origin = pose.position + pose.rotation * (pose.scale * self.offset);
        let (right, up) = match self.facing {
            Facing::Camera => (
                camera.x_axis.truncate().normalize(),
                camera.y_axis.truncate().normalize(),
            ),
            Facing::Fixed => (pose.rotation * Vec3::X, pose.rotation * Vec3::Y),
        };
        let x = right * (self.width * pose.scale.x.abs());
        let y = up
            * (self.width * child_size.y / child_size.x.max(f32::MIN_POSITIVE)
                * pose.scale.y.abs());
        let center = origin + x * (0.5 - self.anchor[0]) + y * (0.5 - self.anchor[1]);
        let corners = [
            center - x * 0.5 + y * 0.5,
            center + x * 0.5 + y * 0.5,
            center + x * 0.5 - y * 0.5,
            center - x * 0.5 - y * 0.5,
        ];
        let vp = proj * view;
        let clip = corners.map(|p| vp * p.extend(1.));
        // Reject only a shared side plane. Covering quads can have every corner
        // outside different sides. Depth crossings are all-or-nothing on every
        // host, so CSS never receives a map through the eye's vanishing line.
        let outside = clip.iter().all(|p| p.x < -p.w)
            || clip.iter().all(|p| p.x > p.w)
            || clip.iter().all(|p| p.y < -p.w)
            || clip.iter().all(|p| p.y > p.w);
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
        let hidden = child_size.x <= 0.
            || child_size.y <= 0.
            || depth_clipped
            || [
                (0., 0.),
                (child_size.x, 0.),
                (child_size.x, child_size.y),
                (0., child_size.y),
            ]
            .iter()
            .any(|&(x, y)| h[6] * x + h[7] * y + h[8] <= 0.)
            || outside
            || behind
            || !h.iter().all(|n| n.is_finite())
            || x.length_squared() == 0.
            || y.length_squared() == 0.;
        PlacedPlane {
            corners,
            homography: if hidden { [0.; 9] } else { h },
            depth: view.transform_point3(center).z,
            hidden,
        }
    }
}
