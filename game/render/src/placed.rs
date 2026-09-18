//! Plane geometry and derived child placements, separate from saved game state.
use crate::{quads, world::scene, FrameInput, RenderError};
use exact_game::{Camera, Parent, Placed, Transform, World};
use exact_gpu::{wgpu, Placement};
#[cfg(not(target_arch = "wasm32"))]
use glam::Vec3;
use glam::{Mat4, Vec2};

#[derive(Default)]
pub(crate) struct Child {
    pub frame: [f32; 4],
    pub texture: Option<wgpu::TextureView>,
    pub plane: Option<Plane>,
}
#[derive(Clone, Copy)]
pub(crate) struct Plane {
    pub placement: Placement,
    #[cfg(not(target_arch = "wasm32"))]
    pub center: Vec3,
    #[cfg(not(target_arch = "wasm32"))]
    pub x: Vec3,
    #[cfg(not(target_arch = "wasm32"))]
    pub y: Vec3,
}
#[derive(Default)]
pub(crate) struct Placements {
    pub children: Vec<Child>,
    items: Vec<quads::Item<Placed>>,
    claims: Vec<u16>,
    cameras: Vec<quads::Item<Camera>>,
    attachments: scene::Attachments,
    stamp: Option<(u64, u64, u64)>,
}
impl Placements {
    pub fn child(&mut self, index: usize, texture: Option<&wgpu::TextureView>, frame: [f32; 4]) {
        if index > self.children.len() {
            return;
        }
        if index == self.children.len() {
            self.children.push(Child::default());
        }
        self.children[index].frame = frame;
        self.children[index].texture = texture.cloned();
    }
    pub fn feed(&mut self, w: &World) -> Result<(), RenderError> {
        let next = (
            w.presentation_generation(),
            w.tick(),
            w.revision::<Parent>(),
        );
        let initial = self.stamp.is_none_or(|old| old.0 != next.0);
        if w.query::<&Placed>().iter().next().is_none() {
            self.items.clear();
            self.claims.clear();
            self.cameras.clear();
            self.attachments = scene::Attachments::default();
            self.stamp = Some(next);
            return Ok(());
        }
        quads::feed(
            w,
            &mut self.items,
            initial,
            self.stamp.is_none_or(|old| old.1 != next.1),
            self.stamp.is_some_and(|old| old.2 != next.2),
        );
        quads::feed(
            w,
            &mut self.cameras,
            initial,
            self.stamp.is_none_or(|old| old.1 != next.1),
            self.stamp.is_some_and(|old| old.2 != next.2),
        );
        self.attachments.feed(
            w,
            initial,
            self.stamp.is_none_or(|old| old.1 != next.1),
            self.stamp.is_some_and(|old| old.2 != next.2),
            false,
        );
        self.stamp = Some(next);
        self.claims.clear();
        for (_, value) in w.query::<&Placed>().iter() {
            value.validate().map_err(RenderError::scene)?;
            if self.claims.contains(&value.child) {
                return Err(RenderError::scene(format!(
                    "Placed child {} has multiple owners",
                    value.child
                )));
            }
            self.claims.push(value.child);
        }
        Ok(())
    }
    pub fn frame(&mut self, input: &FrameInput<'_>, size: Vec2) {
        for child in &mut self.children {
            child.plane = None;
        }
        // An invisible owner or one without a pose is explicitly hidden, never
        // returned to its kernel frame by the visible-item filter.
        for &index in &self.claims {
            if let Some(child) = self.children.get_mut(usize::from(index)) {
                child.plane = Some(Plane {
                    placement: Placement {
                        hidden: true,
                        homography: [0.; 9],
                        depth: 0.,
                    },
                    #[cfg(not(target_arch = "wasm32"))]
                    center: Vec3::ZERO,
                    #[cfg(not(target_arch = "wasm32"))]
                    x: Vec3::ZERO,
                    #[cfg(not(target_arch = "wasm32"))]
                    y: Vec3::ZERO,
                });
            }
        }
        for item in &self.items {
            let Some(child) = self.children.get_mut(usize::from(item.value.child)) else {
                continue;
            };
            child.plane = Some(project(
                item.value,
                input.displayed(item.entity, scene::interpolate(item.poses, input.alpha)),
                child.frame,
                input.view,
                input.proj,
                size,
            ));
        }
    }
    // Headless hosts consume the same retained tick pair and display alpha as
    // device hosts, including the camera. Current-pose sampling is one tick ahead.
    pub fn headless(&mut self, size: Vec2, alpha: f32) {
        let mut attachments = std::mem::take(&mut self.attachments);
        attachments.frame(alpha);
        if let Some(camera) = self.cameras.iter().find(|c| c.value.valid()) {
            let pose = scene::displayed(
                &attachments.output,
                camera.entity,
                scene::interpolate(camera.poses, alpha),
            );
            let view = Mat4::from_rotation_translation(pose.rotation, pose.position).inverse();
            self.frame(
                &FrameInput {
                    view,
                    proj: camera.value.matrix(size),
                    alpha,
                    attachments: &attachments.output,
                    ..Default::default()
                },
                size,
            );
        }
        self.attachments = attachments;
    }
    pub fn placement(&self, index: usize) -> Option<Placement> {
        self.children.get(index)?.plane.map(|p| p.placement)
    }
    pub fn status(&self, w: &World, request: &str, reply: &mut String) {
        #[derive(Default, exact_game::Data)]
        struct Request {
            op: String,
            entity: String,
        }
        let Ok(q) = exact_game::json::from_str::<Request>(request) else {
            return;
        };
        if q.op != "state" || !reply.ends_with("}}") {
            return;
        }
        let Some(e) = w.resolve(&q.entity) else {
            return;
        };
        let Some(value) = w.get::<Placed>(e) else {
            return;
        };
        let p = self.placement(usize::from(value.child));
        reply.truncate(reply.len() - 2);
        reply.push_str(&format!(
            ",\"placed\":{{\"child\":{},\"hidden\":{},\"depth\":{}}}}}}}",
            value.child,
            p.is_none_or(|p| p.hidden),
            p.map_or(0., |p| p.depth)
        ));
    }
}

// Geometry stays host-independent in the engine; only the ABI conversion lives here.
pub(crate) fn project(
    value: Placed,
    pose: Transform,
    frame: [f32; 4],
    view: Mat4,
    proj: Mat4,
    size: Vec2,
) -> Plane {
    let p = value.project(pose, Vec2::new(frame[2], frame[3]), view, proj, size);
    Plane {
        placement: Placement {
            hidden: p.hidden,
            homography: p.homography,
            depth: p.depth,
        },
        #[cfg(not(target_arch = "wasm32"))]
        center: (p.corners[0] + p.corners[2]) * 0.5,
        #[cfg(not(target_arch = "wasm32"))]
        x: p.corners[1] - p.corners[0],
        #[cfg(not(target_arch = "wasm32"))]
        y: p.corners[0] - p.corners[3],
    }
}

#[cfg(test)]
#[path = "placed_tests.rs"]
mod tests;
