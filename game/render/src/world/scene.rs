//! What a frame is seen from and lit by: displayed poses and their tick
//! histories, the camera and its `MouseLook`, the environment, and the frame's
//! inputs; the lights (`lights`) and socket attachments (`attachments`) apart.
use crate::{FrameInput, LightInput, Shadows, Sun, MAX_LIGHTS};
use exact_game::{
    Camera, DirectionalLight, DrawnLight, Entity, LightShadows, Parent, PointLight, SpotLight,
    Transform, World,
};
use glam::{Mat4, Vec3};

/// Photometric units to renderer radiance, one scale for both: 10,000 lux of sun
/// and a 10,000 cd light seen from 1 m (10,000 lux there) map to the default key
/// radiance of 3. Exposure applies after.
pub const PHOTOMETRIC_SCALE: f32 = 0.0003;

mod attachments;
mod lights;
pub use attachments::DisplayedAttachment;
pub(crate) use attachments::{displayed, displayed_matrix, AttachmentDiagnostics, Attachments};
use lights::Light;

/// The displayed global: drawn(parent) · local · offset. Presentation offsets
/// (exact_game::Offset) move an entity and everything under it; without one on
/// the chain this is the simulated global, exactly.
pub(crate) fn drawn(w: &World, e: Entity) -> Option<glam::Affine3A> {
    let affine = |t: Transform| {
        glam::Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position)
    };
    let offset = |e: Entity| w.get::<exact_game::Offset>(e).map(|o| affine(o.0));
    // Most chains carry no offset: no allocation for them.
    let mut at = e;
    let mut any = false;
    for _ in 0..=w.len() {
        any |= w.has::<exact_game::Offset>(at);
        match w.get::<Parent>(at) {
            Some(p) if !any => at = p.0,
            _ => break,
        }
    }
    if !any {
        return w.global(e);
    }
    // The parent chain, bounded even if tools edit a cycle.
    let mut chain = vec![e];
    while let Some(p) = w.get::<Parent>(*chain.last().unwrap()) {
        if chain.len() > w.len() {
            break;
        }
        chain.push(p.0);
    }
    // The offset nearest the root: above it the simulated global holds.
    let Some(first) = (0..chain.len()).rev().find(|&i| offset(chain[i]).is_some()) else {
        return w.global(e);
    };
    let mut global = w.global(chain[first])? * offset(chain[first]).unwrap();
    for &below in chain[..first].iter().rev() {
        let local = affine(*w.get::<Transform>(below)?);
        global = global * local * offset(below).unwrap_or(glam::Affine3A::IDENTITY);
    }
    Some(global)
}
pub(crate) fn pose(w: &World, e: Entity) -> Option<Transform> {
    let global = drawn(w, e)?;
    let (scale, rotation, position) = global.to_scale_rotation_translation();
    Some(Transform {
        position,
        rotation: glam::Quat::from_vec4(
            glam::Vec4::from_array(rotation.to_array())
                .try_normalize()
                .unwrap_or(glam::Vec4::W),
        ),
        scale,
    })
}
// Bounded even if tools edit a cycle before propagation.
pub(crate) fn snap(w: &World, mut e: Entity, parent_changed: bool) -> bool {
    if parent_changed {
        return true;
    }
    // Without a fresh pose, no ancestor can require an interpolation reset.
    if w.fresh().is_empty() {
        return false;
    }
    for _ in 0..=w.len() {
        if w.is_fresh(e) {
            return true;
        }
        let Some(parent) = w.get::<Parent>(e) else {
            return false;
        };
        e = parent.0;
    }
    false
}
#[derive(Clone, Copy)]
struct History {
    entity: Entity,
    prev: Transform,
    curr: Transform,
}
impl History {
    fn new(entity: Entity, curr: Transform) -> Self {
        Self {
            entity,
            prev: curr,
            curr,
        }
    }
    fn update(&mut self, w: &World, next_tick: bool, parent_changed: bool) {
        if let Some(curr) = pose(w, self.entity) {
            self.update_to(curr, next_tick, snap(w, self.entity, parent_changed));
        }
    }
    fn update_to(&mut self, curr: Transform, next_tick: bool, snap: bool) {
        if next_tick {
            self.prev = self.curr;
        }
        self.curr = curr;
        if snap {
            self.prev = curr;
        }
    }
    fn at(self, alpha: f32) -> Transform {
        interpolate([self.prev, self.curr], alpha)
    }
}
#[derive(Default)]
pub(super) struct Scene {
    versions: Option<[u64; 8]>,
    pub(super) attachments: Attachments,
    camera: Option<(History, Camera)>,
    // The camera's MouseLook, its revision and camera, and its descendants.
    look: Option<(exact_game::MouseLook, u64, Vec<History>)>,
    /// Pointer motion the next frame's MouseLook turns by (`Sim::unshown_motion`).
    pub(super) unshown: glam::Vec2,
    sun: Option<(History, DirectionalLight)>,
    fill: Option<(History, DirectionalLight)>,
    // Point lights, then spot lights, each in entity order.
    lights: Vec<Light>,
    first_spot: usize,
    // Eligible lights by (distance, list order); the first MAX_LIGHTS are drawn.
    selected: Vec<(f32, usize)>,
    dropped: usize,
    output: Vec<LightInput>,
    map: Option<exact_game::EnvironmentMap>,
}
impl Scene {
    #[cfg(test)]
    pub(super) fn lights_for_test(&self) -> Vec<Entity> {
        self.selected
            .iter()
            .take(MAX_LIGHTS)
            .map(|&(_, i)| self.lights[i].history.entity)
            .collect()
    }
    pub fn trace_camera(&self, alpha: f32) -> [f64; 3] {
        self.camera.map_or([0.; 3], |(h, _)| {
            crate::trace::mix(
                h.prev.position.to_array(),
                h.curr.position.to_array(),
                alpha,
            )
        })
    }
    pub fn reset(&mut self) {
        self.versions = None;
        self.attachments.reset();
        self.camera = None;
        self.look = None;
        self.sun = None;
        self.fill = None;
        self.lights.clear();
        self.first_spot = 0;
        self.selected.clear();
        self.dropped = 0;
        self.map = None;
    }
    pub fn feed(
        &mut self,
        w: &World,
        next_tick: bool,
        moved: bool,
        structure: bool,
        parent_changed: bool,
    ) {
        let versions = [
            w.revision::<Camera>(),
            w.revision::<DirectionalLight>(),
            w.revision::<PointLight>(),
            w.revision::<exact_game::Lit>(),
            w.revision::<SpotLight>(),
            w.revision::<LightShadows>(),
            w.revision::<exact_game::Visible>(),
            w.revision::<DrawnLight>(),
        ];
        let old = self.versions;
        if old.is_none_or(|v| v[0] != versions[0]) || structure {
            let camera = w
                .query::<&Camera>()
                .iter()
                .find_map(|(e, c)| c.valid().then(|| pose(w, e).map(|t| (e, t, *c))).flatten());
            self.camera = camera.map(|(e, t, c)| {
                (
                    self.camera
                        .filter(|(h, _)| h.entity == e)
                        .map_or(History::new(e, t), |(h, _)| h),
                    c,
                )
            });
        }
        if let Some((history, _)) = &mut self.camera {
            history.update(w, next_tick, parent_changed);
        }
        self.feed_look(w, next_tick, structure || parent_changed, parent_changed);
        self.feed_lights(w, versions, next_tick, moved, structure, parent_changed);
        // Cloned only when it changes.
        let map = w.try_resource::<exact_game::EnvironmentMap>();
        if self.map.as_ref() != map.as_deref() {
            self.map = map.as_deref().cloned();
        }
        self.versions = Some(versions);
    }
    pub(super) fn has_look(&self) -> bool {
        self.look.is_some()
    }
    // The camera's MouseLook and the histories of everything under it, kept as
    // the camera's own is; the descendants are found again when the hierarchy
    // or the component changes.
    fn feed_look(&mut self, w: &World, next_tick: bool, changed: bool, parent_changed: bool) {
        let camera = self.camera.map(|(h, _)| h.entity);
        let Some((camera, look)) =
            camera.and_then(|e| w.get::<exact_game::MouseLook>(e).map(|l| (e, *l)))
        else {
            self.look = None;
            return;
        };
        let revision = w.revision::<exact_game::MouseLook>();
        if changed || self.look.as_ref().is_none_or(|(_, r, _)| *r != revision) {
            let old = self.look.take().map_or_else(Vec::new, |(_, _, kids)| kids);
            let mut kids = Vec::new();
            for (e, _) in w.query::<&Parent>().iter() {
                let mut at = e;
                for _ in 0..=w.len() {
                    let Some(parent) = w.get::<Parent>(at).map(|p| p.0) else {
                        break;
                    };
                    if parent == camera {
                        if let Some(t) = pose(w, e) {
                            kids.push(
                                old.iter()
                                    .copied()
                                    .find(|h| h.entity == e)
                                    .unwrap_or(History::new(e, t)),
                            );
                        }
                        break;
                    }
                    at = parent;
                }
            }
            self.look = Some((look, revision, kids));
        }
        if let Some((current, _, kids)) = &mut self.look {
            *current = look;
            for h in kids {
                h.update(w, next_tick, parent_changed);
            }
        }
    }
    pub fn frame(
        &mut self,
        w: &World,
        alpha: f32,
        size: glam::Vec2,
        pixels: bool,
    ) -> FrameInput<'_> {
        let alpha = alpha.clamp(0.0, 1.0);
        self.attachments.frame(alpha);
        let (mut camera_pose, mut camera) =
            self.camera
                .map_or((Transform::default(), Camera::default()), |(h, c)| {
                    (
                        displayed(&self.attachments.output, h.entity, h.at(alpha)),
                        c,
                    )
                });
        // MouseLook: the drawn camera and everything under it turn about the eye
        // by the motion no drawn tick shows yet. Presentation only.
        let motion = std::mem::take(&mut self.unshown);
        if let Some((look, _, kids)) = self.look.as_ref().filter(|_| motion != glam::Vec2::ZERO) {
            let turn = look.turn(camera_pose.rotation, motion);
            let eye = camera_pose.position;
            let about =
                Mat4::from_translation(eye) * Mat4::from_quat(turn) * Mat4::from_translation(-eye);
            camera_pose.rotation = (turn * camera_pose.rotation).normalize();
            for h in kids {
                let out = &mut self.attachments.output;
                let matrix = about * displayed_matrix(out, h.entity, h.at(alpha));
                let (scale, rotation, position) = matrix.to_scale_rotation_translation();
                let turned = DisplayedAttachment {
                    entity: h.entity,
                    pose: Transform {
                        scale,
                        rotation,
                        position,
                    },
                    matrix,
                };
                match out.iter_mut().find(|a| a.entity == h.entity) {
                    Some(a) => *a = turned,
                    None => out.push(turned),
                }
            }
        }
        if !pixels {
            if let exact_game::Projection::Orthographic { integer_scale, .. } =
                &mut camera.projection
            {
                *integer_scale = false;
            }
        }
        let seconds = if w.tick() == 0 {
            0.
        } else {
            (w.tick() as f64 - 1. + alpha as f64) / w.hz() as f64
        };
        // Only the selected histories are touched per frame.
        self.output.clear();
        for &(_, i) in self.selected.iter().take(MAX_LIGHTS) {
            let l = &self.lights[i];
            let pose = displayed(
                &self.attachments.output,
                l.history.entity,
                l.history.at(alpha),
            );
            self.output.push(LightInput {
                position: pose.position,
                color: l.light.color.into(),
                intensity: PHOTOMETRIC_SCALE
                    * l.light.intensity
                    * l.lit
                        .as_ref()
                        .map_or(1., |lit| lit.0.value_at(seconds, w.hz()).max(0.)),
                range: l.light.range,
                direction: (pose.rotation * -Vec3::Z).normalize_or(-Vec3::Z),
                cone: l.light.cone,
                shadows: l.light.shadows,
            });
        }
        let directional = |(h, s): (History, DirectionalLight), shadows: bool| Sun {
            direction: (displayed(&self.attachments.output, h.entity, h.at(alpha)).rotation
                * -Vec3::Z)
                .normalize_or(-Vec3::Y),
            color: s.color.into(),
            illuminance: s.illuminance * PHOTOMETRIC_SCALE,
            shadows: (shadows && s.shadows).then(Shadows::default),
        };
        let sun = self.sun.map(|s| directional(s, true));
        let fill = self.fill.map(|s| directional(s, false));
        // The drawn camera's own sky, else the world's.
        let view = (self.camera)
            .and_then(|(h, _)| w.get::<exact_game::DrawnEnvironment>(h.entity).map(|v| *v));
        let e = view.map_or_else(
            || {
                w.try_resource::<exact_game::Environment>()
                    .as_deref()
                    .copied()
                    .unwrap_or_default()
            },
            |v| v.environment,
        );
        FrameInput {
            glows: &[],
            seconds: if w.tick() == 0 {
                0.
            } else {
                (w.tick() as f64 - 1. + alpha.clamp(0., 1.) as f64) / w.hz() as f64
            },
            view: Mat4::from_scale_rotation_translation(
                camera_pose.scale,
                camera_pose.rotation,
                camera_pose.position,
            )
            .inverse(),
            proj: camera.matrix(size),
            camera_position: camera_pose.position,
            alpha,
            sun,
            fill,
            lights: &self.output,
            lights_dropped: self.dropped,
            environment: e,
            environment_map: self.map.as_ref().map(|m| crate::EnvironmentMapInput {
                texture: &m.texture,
                intensity: m.intensity,
                rgbm: m.rgbm,
                visible: m.visible,
                rotation: m.rotation,
            }),
            ambient_occlusion: view.map_or_else(
                || {
                    w.try_resource::<exact_game::AmbientOcclusion>()
                        .as_deref()
                        .copied()
                },
                |v| v.ambient_occlusion,
            ),
            timestamps: None,
            attachments: &self.attachments.output,
            headroom: 1.0,
        }
    }
}

// Matches transform.wgsl/model.wgsl: shortest-path normalized linear quaternion
// interpolation, local scale and translation first, then transform the point.
pub(crate) fn interpolate([a, b]: [Transform; 2], alpha: f32) -> Transform {
    let t = alpha.clamp(0., 1.);
    let q = if a.rotation.dot(b.rotation) < 0. {
        -b.rotation
    } else {
        b.rotation
    };
    let q =
        glam::Vec4::from_array(a.rotation.to_array()).lerp(glam::Vec4::from_array(q.to_array()), t);
    Transform {
        position: a.position.lerp(b.position, t),
        scale: a.scale.lerp(b.scale, t),
        rotation: glam::Quat::from_vec4(q.try_normalize().unwrap_or(glam::Vec4::W)),
    }
}

#[cfg(test)]
#[path = "../../tests/light_selection/mod.rs"]
mod light_selection_tests;
