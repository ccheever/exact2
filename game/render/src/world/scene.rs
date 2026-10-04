use crate::{FrameInput, LightInput, Shadows, Sun, MAX_LIGHTS};
use exact_game::{
    Camera, DirectionalLight, Entity, LightShadows, Parent, PointLight, SpotLight, Transform, World,
};
use glam::{Mat4, Vec3};

/// Photometric units to renderer radiance, one scale for both: 10,000 lux of sun
/// and a 10,000 cd light seen from 1 m (10,000 lux there) map to the default key
/// radiance of 3. Exposure applies after.
pub const PHOTOMETRIC_SCALE: f32 = 0.0003;

pub(crate) fn pose(w: &World, e: Entity) -> Option<Transform> {
    let mut global = w.global(e)?;
    // The presentation offset moves the drawn pose only (exact_game::Offset).
    if let Some(offset) = w.get::<exact_game::Offset>(e) {
        let t = offset.0;
        global = global
            * glam::Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position);
    }
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
// A point light is a spot whose cone is the whole sphere.
#[derive(Clone, Copy)]
struct Emitter {
    color: [f32; 3],
    intensity: f32,
    range: f32,
    // Cosines of the inner and outer half-angles; a point light has none.
    cone: Option<[f32; 2]>,
    shadows: bool,
}
impl Emitter {
    fn point(light: &PointLight, shadows: bool) -> Self {
        Self {
            color: light.color,
            intensity: light.intensity,
            range: light.range,
            cone: None,
            shadows,
        }
    }
    #[allow(clippy::manual_clamp)] // clamp would keep NaN
    fn spot(light: &SpotLight, shadows: bool) -> Self {
        // `max` maps NaN to zero, which `clamp` would keep.
        let outer = light.outer.max(0.).min(std::f32::consts::FRAC_PI_2);
        let outer_cos = exact_game::math::cos(outer);
        // The shader's smoothstep needs a nonempty edge.
        let inner_cos = exact_game::math::cos(light.inner.max(0.).min(outer)).max(outer_cos + 1e-4);
        Self {
            color: light.color,
            intensity: light.intensity,
            range: light.range,
            cone: Some([inner_cos, outer_cos]),
            shadows,
        }
    }
}
struct Light {
    history: History,
    light: Emitter,
    lit: Option<exact_game::Lit>,
}
#[derive(Default)]
pub(super) struct Scene {
    versions: Option<[u64; 6]>,
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
        if old.is_none_or(|v| v[1] != versions[1]) || structure {
            // The first two posed directional lights: the sun, then a fill.
            let (sun, fill) = (self.sun, self.fill);
            let keep = |e: Entity, t: Transform| {
                [sun, fill]
                    .into_iter()
                    .flatten()
                    .find(|(h, _)| h.entity == e)
                    .map_or(History::new(e, t), |(h, _)| h)
            };
            let mut query = w.query::<&DirectionalLight>();
            let mut posed = query
                .iter()
                .filter_map(|(e, s)| pose(w, e).map(|t| (keep(e, t), *s)));
            self.sun = posed.next();
            self.fill = posed.next();
        }
        for (history, _) in [&mut self.sun, &mut self.fill].into_iter().flatten() {
            history.update(w, next_tick, parent_changed);
        }
        if old.is_none_or(|v| v[2..] != versions[2..]) || structure {
            // Compact departures once; keep each kind's histories in entity order
            // and append arrivals. Value-only edits reuse the buffer without sorting.
            let mut kept = 0;
            let mut spots = 0;
            let spot_at = self.first_spot;
            let mut index = 0;
            self.lights.retain(|l| {
                let spot = index >= spot_at;
                index += 1;
                let e = l.history.entity;
                let live = w.global(e).is_some()
                    && if spot {
                        w.has::<SpotLight>(e)
                    } else {
                        w.has::<PointLight>(e)
                    };
                if live {
                    kept += 1;
                    spots += usize::from(spot);
                }
                live
            });
            let points = kept - spots;
            let mut fresh = Vec::new();
            let mut at = 0;
            for (e, light) in w.query::<&PointLight>().iter() {
                let emitter = Emitter::point(light, w.has::<LightShadows>(e));
                let lit = w.get::<exact_game::Lit>(e).as_deref().cloned();
                if at < points && self.lights[at].history.entity == e {
                    (self.lights[at].light, self.lights[at].lit) = (emitter, lit);
                    at += 1;
                } else if let Some(t) = pose(w, e) {
                    fresh.push((
                        false,
                        Light {
                            history: History::new(e, t),
                            light: emitter,
                            lit,
                        },
                    ));
                }
            }
            let mut at = points;
            for (e, light) in w.query::<&SpotLight>().iter() {
                let emitter = Emitter::spot(light, w.has::<LightShadows>(e));
                let lit = w.get::<exact_game::Lit>(e).as_deref().cloned();
                if at < kept && self.lights[at].history.entity == e {
                    (self.lights[at].light, self.lights[at].lit) = (emitter, lit);
                    at += 1;
                } else if let Some(t) = pose(w, e) {
                    fresh.push((
                        true,
                        Light {
                            history: History::new(e, t),
                            light: emitter,
                            lit,
                        },
                    ));
                }
            }
            self.first_spot = points;
            if !fresh.is_empty() {
                let mut all: Vec<_> = self
                    .lights
                    .drain(..)
                    .enumerate()
                    .map(|(i, l)| (i >= points, l))
                    .chain(fresh)
                    .collect();
                all.sort_by_key(|(spot, l)| (*spot, l.history.entity.index()));
                self.first_spot = all.iter().filter(|(spot, _)| !spot).count();
                self.lights.extend(all.into_iter().map(|(_, l)| l));
            }
        }
        if next_tick || moved || structure || old != Some(versions) {
            self.selected.clear();
            self.attachments.frame(1.);
            let camera = self.camera.map_or(Vec3::ZERO, |(h, _)| {
                displayed(&self.attachments.output, h.entity, h.curr).position
            });
            for (i, light) in self.lights.iter_mut().enumerate() {
                light.history.update(w, next_tick, parent_changed);
                // Eligibility is evaluated at the same tick endpoint as distance.
                let intensity = light.light.intensity
                    * light
                        .lit
                        .as_ref()
                        .map_or(1., |lit| lit.0.value(w.now()).max(0.));
                if !intensity.is_finite()
                    || intensity <= 0.
                    || !light.light.range.is_finite()
                    || light.light.range <= 0.
                {
                    continue;
                }
                let distance = displayed(
                    &self.attachments.output,
                    light.history.entity,
                    light.history.curr,
                )
                .position
                .distance_squared(camera);
                // Selection depends only on current state; list order (points, then
                // spots, each in entity order) breaks ties.
                self.selected.push((distance, i));
            }
            self.selected
                .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            self.dropped = self.selected.len().saturating_sub(MAX_LIGHTS);
            // Frames then fill the output without allocating.
            self.output.reserve(self.selected.len().min(MAX_LIGHTS));
        }
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
        let e = w
            .try_resource::<exact_game::Environment>()
            .as_deref()
            .copied()
            .unwrap_or_default();
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
            }),
            ambient_occlusion: w
                .try_resource::<exact_game::AmbientOcclusion>()
                .as_deref()
                .copied(),
            timestamps: None,
            attachments: &self.attachments.output,
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

/// One displayed attachment plus the ordinary histories restored after submission.
#[derive(Clone, Copy)]
pub struct DisplayedAttachment {
    /// Entity whose drawing, lights and placed children use this pose.
    pub entity: Entity,
    /// World pose composed from interpolated local joints.
    pub pose: Transform,
    /// Full affine map; geometry must retain this instead of decomposing shear.
    pub matrix: Mat4,
}
// Owner transforms must be interpolated locally before hierarchy composition:
// decomposing a global matrix loses shear under non-uniform ancestors.
struct Owner {
    // First valid follower this feed; emits the owner override in item order.
    first_attachment: Option<u32>,
    chain: Vec<History>,
}
impl Owner {
    fn new(w: &World, entity: Entity) -> Self {
        let mut owner = Self {
            first_attachment: None,
            chain: Vec::new(),
        };
        owner.update(w, entity, false, true);
        owner
    }
    fn update(&mut self, w: &World, entity: Entity, next_tick: bool, parent_changed: bool) {
        // Keep leaf-to-root history in place; ancestor edits snap the changed chain.
        let mut at = entity;
        let mut length = 0;
        for _ in 0..=w.len() {
            let curr = w
                .get::<Transform>(at)
                .as_deref()
                .copied()
                .unwrap_or_default();
            if self.chain.get(length).is_none_or(|h| h.entity != at) {
                if let Some(found) = self.chain[length..].iter().position(|h| h.entity == at) {
                    self.chain.swap(length, length + found);
                } else {
                    self.chain.insert(length, History::new(at, curr));
                }
            }
            let h = &mut self.chain[length];
            if next_tick {
                h.prev = h.curr;
            }
            h.curr = curr;
            if snap(w, at, parent_changed) {
                h.prev = curr;
            }
            length += 1;
            let Some(parent) = w.get::<Parent>(at).map(|p| p.0).filter(|e| w.contains(*e)) else {
                break;
            };
            at = parent;
        }
        self.chain.truncate(length);
    }
}
#[derive(Default)]
pub(crate) struct DiagnosticRegistry {
    world: Option<exact_game::WorldId>,
    messages: std::collections::BTreeMap<(Entity, &'static str), String>,
}
impl std::ops::Deref for DiagnosticRegistry {
    type Target = std::collections::BTreeMap<(Entity, &'static str), String>;
    fn deref(&self) -> &Self::Target {
        &self.messages
    }
}
impl std::ops::DerefMut for DiagnosticRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.messages
    }
}
impl DiagnosticRegistry {
    pub(crate) fn restored(&mut self, w: &World) {
        self.world = Some(w.id());
    }
    pub(crate) fn observe(&mut self, w: &World) {
        if self.world.as_ref() != Some(&w.id()) {
            self.messages.clear();
            self.restored(w);
        }
        self.messages.retain(|(e, _), _| w.contains(*e));
    }
}
pub(crate) type AttachmentDiagnostics = std::rc::Rc<std::cell::RefCell<DiagnosticRegistry>>;
struct Attachment {
    history: History,
    owner: Entity,
    chain: Vec<[Transform; 2]>,
    offset: Transform,
    model_digest: u64,
}
#[derive(Default)]
pub(crate) struct Attachments {
    owners: std::collections::BTreeMap<Entity, Owner>,
    // Feed keeps one generation per entity index, in index order.
    items: Vec<Attachment>,
    pub(crate) diagnostics: AttachmentDiagnostics,
    pub output: Vec<DisplayedAttachment>,
    pub(crate) model_digests: std::collections::BTreeMap<String, u64>,
}
impl Attachments {
    pub fn reset(&mut self) {
        self.owners.clear();
        self.items.clear();
        self.output.clear();
    }
    fn warn(&mut self, e: Entity, identity: &'static str, message: impl FnOnce() -> String) {
        let message = message();
        let mut diagnostics = self.diagnostics.borrow_mut();
        if diagnostics.get(&(e, identity)) == Some(&message) {
            return;
        }
        // Host-only diagnostics never enter continuation saves.
        #[cfg(target_arch = "wasm32")]
        web_sys::console::warn_1(&message.clone().into());
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{message}");
        diagnostics.insert((e, identity), message);
    }

    pub fn feed(
        &mut self,
        w: &World,
        initial: bool,
        next_tick: bool,
        parent_changed: bool,
        models_changed: bool,
    ) {
        if initial {
            self.owners.clear();
        }
        self.owners.retain(|e, _| w.contains(*e));
        self.diagnostics.borrow_mut().observe(w);
        for (&entity, owner) in &mut self.owners {
            owner.first_attachment = None;
            owner.update(w, entity, next_tick, parent_changed);
        }
        // Retain animated owners before attachments are spawned so a new charm
        // inherits the owner's displayed history, rather than snapping its bones.
        for (e, _) in w.query::<&exact_game::Pose>().iter() {
            if w.has::<Transform>(e) {
                self.owners.entry(e).or_insert_with(|| Owner::new(w, e));
            }
        }
        let mut at = 0;
        for (e, follow) in w.query::<&exact_game::SocketFollow>().iter() {
            let target = match &follow.target {
                exact_game::FollowTarget::Entity(e) => Some(*e),
                exact_game::FollowTarget::Name(n) => w.named(n),
            };
            while at < self.items.len() && self.items[at].history.entity.index() < e.index() {
                self.items.remove(at);
            }
            let resolved = target
                .ok_or_else(|| format!("unresolved target {:?}", follow.target))
                .and_then(|target| {
                    exact_game::animation::socket_node(w, target, &follow.joint)
                        .map(|node| (target, node))
                });
            let (target, node) = match resolved {
                Ok(target) => target,
                Err(error) => {
                    self.warn(e, "socket", || {
                        format!(
                            "SocketFollow `{}`: {error}; using authored Transform",
                            w.name(e).unwrap_or("unnamed")
                        )
                    });
                    continue;
                }
            };
            let (Some(home), Some(_)) = (pose(w, e), w.get::<Transform>(target)) else {
                self.warn(e, "transform", || {
                    format!(
                        "SocketFollow `{}`: unresolved Transform",
                        w.name(e).unwrap_or("unnamed")
                    )
                });
                continue;
            };
            let Some(mesh) = w.get::<exact_game::Mesh>(target) else {
                continue;
            };
            let exact_game::Mesh::Asset(name) = &*mesh else {
                continue;
            };
            let Some(model) = w.model(name) else { continue };
            let fresh = initial
                || !self
                    .items
                    .get(at)
                    .is_some_and(|v| v.history.entity == e && v.owner == target);
            if fresh {
                if self.items.get(at).is_some_and(|v| v.history.entity == e) {
                    self.items.remove(at);
                }
                self.items.insert(
                    at,
                    Attachment {
                        history: History::new(e, home),
                        owner: target,
                        chain: vec![],
                        offset: follow.offset,
                        model_digest: self
                            .model_digests
                            .get(name)
                            .copied()
                            .unwrap_or(w.model_revision()),
                    },
                );
            }
            let item = &mut self.items[at];
            item.history
                .update_to(home, next_tick, snap(w, e, parent_changed));
            self.owners
                .entry(target)
                .or_insert_with(|| Owner::new(w, target))
                .first_attachment
                .get_or_insert(e.index());
            let model_changed = if models_changed {
                let digest = self
                    .model_digests
                    .get(name)
                    .copied()
                    .unwrap_or(w.model_revision());
                let changed = item.model_digest != digest;
                item.model_digest = digest;
                changed
            } else {
                false
            };
            item.offset = follow.offset;
            item.chain.clear();
            let sampled = w.get::<exact_game::Pose>(target).filter(|p| {
                p.local.len() == model.nodes.len() * 10 && p.previous.len() == p.local.len()
            });
            let snap = initial
                || model_changed
                || exact_game::animation::socket_stale(w, target)
                || snap(w, target, parent_changed);
            let nodes = std::iter::successors(Some(node), |&i| model.nodes[i as usize].parent);
            if let Some(p) = &sampled {
                let prev = if snap { &p.local } else { &p.previous };
                item.chain.extend(nodes.map(|i| {
                    let start = i as usize * 10;
                    let read = |values: &[f32]| Transform {
                        position: Vec3::from_slice(&values[start..]),
                        rotation: glam::Quat::from_slice(&values[start + 3..]).normalize(),
                        scale: Vec3::from_slice(&values[start + 7..]),
                    };
                    [read(prev), read(&p.local)]
                }));
            } else {
                item.chain.extend(nodes.map(|i| {
                    let (scale, rotation, position) =
                        Mat4::from_cols_array(&model.nodes[i as usize].transform)
                            .to_scale_rotation_translation();
                    // Match bind_pose's normalization followed by the pose reader's,
                    // without allocating or decomposing unrelated model nodes.
                    [Transform {
                        position,
                        rotation: rotation.normalize().normalize(),
                        scale,
                    }; 2]
                }));
            }
            item.chain.reverse();
            at += 1;
        }
        self.items.truncate(at);
        self.output.clear();
        self.output.reserve(self.items.len());
    }
    fn matrix(&self, index: usize, alpha: f32, remaining: usize) -> Mat4 {
        let item = &self.items[index];
        if remaining == 0 {
            return matrix(item.history.at(alpha));
        }
        let owner = self.owner_matrix(&self.owners[&item.owner], alpha, remaining);
        item.chain.iter().fold(owner, |m, pair| {
            m * crate::skinning::interpolated_local(*pair, alpha)
        }) * matrix(item.offset)
    }
    fn owner_matrix(&self, owner: &Owner, alpha: f32, remaining: usize) -> Mat4 {
        owner.chain.iter().rev().fold(Mat4::IDENTITY, |m, h| {
            self.items
                .binary_search_by_key(&h.entity, |v| v.history.entity)
                .ok()
                .map_or_else(
                    || m * matrix(h.at(alpha)),
                    |i| self.matrix(i, alpha, remaining - 1),
                )
        })
    }
    pub fn frame(&mut self, alpha: f32) {
        self.output.clear();
        for (i, item) in self.items.iter().enumerate() {
            let matrix = self.matrix(i, alpha, self.items.len());
            let (scale, rotation, position) = matrix.to_scale_rotation_translation();
            self.output.push(DisplayedAttachment {
                entity: item.history.entity,
                matrix,
                pose: Transform {
                    scale,
                    rotation,
                    position,
                },
            });
        }
        // The skinned owner must use the same affine chain as its charm; otherwise
        // the owner mesh would still flatten ancestor shear in its TRS arena.
        for item in &self.items {
            let owner = &self.owners[&item.owner];
            if owner.chain.len() < 2
                || owner.first_attachment != Some(item.history.entity.index())
                || self
                    .items
                    .binary_search_by_key(&item.owner, |v| v.history.entity)
                    .is_ok()
            {
                continue;
            }
            let matrix = self.owner_matrix(owner, alpha, self.items.len());
            let (scale, rotation, position) = matrix.to_scale_rotation_translation();
            self.output.push(DisplayedAttachment {
                entity: item.owner,
                matrix,
                pose: Transform {
                    scale,
                    rotation,
                    position,
                },
            });
        }
    }
}
fn matrix(t: Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}
pub(crate) fn displayed(
    attachments: &[DisplayedAttachment],
    entity: Entity,
    fallback: Transform,
) -> Transform {
    attachments
        .iter()
        .find(|v| v.entity == entity)
        .map_or(fallback, |v| v.pose)
}

pub(crate) fn displayed_matrix(
    attachments: &[DisplayedAttachment],
    entity: Entity,
    fallback: Transform,
) -> Mat4 {
    attachments
        .iter()
        .find(|a| a.entity == entity)
        .map_or_else(|| matrix(fallback), |a| a.matrix)
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    struct Rig;
    impl exact_game::Game for Rig {
        const ID: &'static str = "attachment-owner";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    #[test]
    fn attachments_share_one_owner_and_the_delivered_model_identity() {
        let model = crate::test_model::skinned_model();
        let before = crate::models::model_hash_count();
        let digest = crate::models::model_digest(&model);
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset("rig.model", Ok(exact_game::asset::Content::Model(model)))
            .unwrap();
        let w = sim.world_mut();
        let owner = w.spawn_named(
            "rig",
            (Transform::default(), exact_game::Mesh::asset("rig.model")),
        );
        for _ in 0..3 {
            w.spawn((
                Transform::default(),
                exact_game::SocketFollow::new("rig", "joint"),
            ));
        }
        w.propagate();
        let mut a = Attachments::default();
        a.model_digests.insert("rig.model".into(), digest);
        a.feed(w, true, true, true, true);
        assert_eq!(a.owners.len(), 1);
        assert_eq!(a.items.len(), 3);
        assert!(a
            .items
            .iter()
            .all(|v| v.owner == owner && v.model_digest == digest));
        // A residency identity is supplied, never rediscovered from model bytes.
        a.model_digests.insert("rig.model".into(), digest ^ 1);
        a.feed(w, false, true, false, true);
        assert!(a.items.iter().all(|v| v.model_digest == digest ^ 1));
        assert_eq!(crate::models::model_hash_count(), before + 1);
    }

    #[test]
    fn bind_fallback_matches_sampled_rest_for_affine_joint_chains() {
        use exact_game::{asset, Mesh, Pose, SocketFollow};
        let model = asset::Model {
            nodes: (0..16)
                .map(|i| {
                    let mut transform = Mat4::from_scale_rotation_translation(
                        Vec3::new(if i % 3 == 0 { -1.1 } else { 1.1 }, 0.9, 1.),
                        glam::Quat::from_rotation_y(i as f32 * 0.173),
                        Vec3::new(0.1, i as f32 * 0.02, -0.03),
                    );
                    transform.y_axis += transform.x_axis * 0.15;
                    asset::Node {
                        name: format!("joint-{i}"),
                        parent: (i > 0 && i < 10).then(|| i - 1),
                        transform: transform.to_cols_array(),
                        ..Default::default()
                    }
                })
                .collect(),
            ..Default::default()
        };
        let rest = exact_game::animation::bind_pose(&model);
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset("rig.model", Ok(asset::Content::Model(model)))
            .unwrap();
        let w = sim.world_mut();
        let owner = w.spawn((Transform::at(2., 1., 0.), Mesh::asset("rig.model")));
        w.spawn((
            Transform::default(),
            SocketFollow::new(owner, "joint-9").offset(Transform::at(0.2, 0.3, 0.)),
        ));
        let mut pose = Pose::default();
        pose.local = rest.clone();
        pose.previous = rest;
        w.insert(owner, pose.clone());
        let capture = |w: &World| {
            let saved = w.save();
            let mut attachments = Attachments::default();
            attachments.feed(w, true, true, true, true);
            assert_eq!(attachments.items[0].chain.len(), 10);
            let mut bits = Vec::new();
            for pair in &attachments.items[0].chain {
                for t in pair {
                    bits.extend(t.position.to_array().map(f32::to_bits));
                    bits.extend(t.rotation.to_array().map(f32::to_bits));
                    bits.extend(t.scale.to_array().map(f32::to_bits));
                }
            }
            for alpha in [0., 0.25, 0.75, 1.] {
                attachments.frame(alpha);
                bits.extend(
                    attachments.output[0]
                        .matrix
                        .to_cols_array()
                        .map(f32::to_bits),
                );
            }
            assert_eq!(w.save(), saved);
            bits
        };
        let expected = capture(w);
        w.remove::<Pose>(owner);
        assert_eq!(capture(w), expected);
        pose.previous.pop();
        w.insert(owner, pose.clone());
        assert_eq!(capture(w), expected);
        pose.local.pop();
        w.insert(owner, pose);
        assert_eq!(capture(w), expected);
    }

    #[test]
    fn attachment_lookup_keeps_entity_order_and_rejects_reused_slots() {
        use exact_game::{FollowTarget, Mesh, SocketFollow};
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset(
            "rig.model",
            Ok(exact_game::asset::Content::Model(
                crate::test_model::skinned_model(),
            )),
        )
        .unwrap();
        let w = sim.world_mut();
        let root = w.spawn((Transform::at(10., 0., 0.), Mesh::asset("rig.model")));
        let old = w.spawn((
            Transform::at(77., 0., 0.),
            Mesh::asset("rig.model"),
            SocketFollow::new(root, "joint"),
        ));
        let stale = Owner::new(w, old);
        let middle = w.spawn((Transform::default(), SocketFollow::new(root, "joint")));
        let tail = w.spawn((Transform::default(), SocketFollow::new(old, "joint")));
        w.propagate();
        let mut a = Attachments::default();
        a.feed(w, true, true, true, true);
        a.frame(0.5);
        assert_eq!(
            displayed(&a.output, tail, Transform::default()).position.x,
            10.
        );

        w.despawn(old);
        let mut socket = SocketFollow::new(root, "joint");
        socket.offset = Transform::at(4., 0., 0.);
        let replacement = w.spawn((Transform::at(3., 0., 0.), Mesh::asset("rig.model"), socket));
        assert_eq!(replacement.index(), old.index());
        assert_ne!(replacement, old);
        w.get_mut::<SocketFollow>(tail).unwrap().target = FollowTarget::Entity(replacement);
        w.get_mut::<SocketFollow>(middle).unwrap().joint = "missing".into();
        w.propagate();
        a.feed(w, false, true, false, false);
        assert_eq!(
            a.items.iter().map(|v| v.history.entity).collect::<Vec<_>>(),
            [replacement, tail]
        );
        a.frame(0.5);
        assert_eq!(
            displayed(&a.output, tail, Transform::default()).position.x,
            14.
        );
        assert_eq!(
            a.owner_matrix(&stale, 0.5, a.items.len()),
            matrix(Transform::at(77., 0., 0.))
        );

        w.get_mut::<SocketFollow>(middle).unwrap().joint = "joint".into();
        w.remove::<SocketFollow>(replacement);
        a.feed(w, false, false, false, false);
        assert_eq!(
            a.items.iter().map(|v| v.history.entity).collect::<Vec<_>>(),
            [middle, tail]
        );
        a.frame(0.5);
        assert_eq!(
            displayed(&a.output, tail, Transform::default()).position.x,
            3.
        );
    }

    #[test]
    fn parented_owner_overrides_follow_current_attachments_once() {
        use exact_game::{Mesh, SocketFollow};
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset(
            "rig.model",
            Ok(exact_game::asset::Content::Model(
                crate::test_model::skinned_model(),
            )),
        )
        .unwrap();
        let w = sim.world_mut();
        let parent = w.spawn(Transform::at(100., 0., 0.));
        let a = w.spawn((
            Transform::at(1., 0., 0.),
            Parent(parent),
            Mesh::asset("rig.model"),
        ));
        let b = w.spawn((
            Transform::at(2., 0., 0.),
            Parent(parent),
            Mesh::asset("rig.model"),
        ));
        let first_b = w.spawn((Transform::default(), SocketFollow::new(b, "joint")));
        let first_a = w.spawn((Transform::default(), SocketFollow::new(a, "joint")));
        let last_b = w.spawn((Transform::default(), SocketFollow::new(b, "joint")));
        let mut attachments = Attachments::default();
        let mut check = |w: &mut World, expected: &[Entity]| {
            w.propagate();
            attachments.feed(w, false, true, true, false);
            for alpha in [0., 0.5, 1.] {
                attachments.frame(alpha);
                assert_eq!(
                    attachments
                        .output
                        .iter()
                        .map(|v| v.entity)
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        };
        // Owner order follows the first referring attachment, not the map key.
        check(w, &[first_b, first_a, last_b, b, a]);
        w.despawn(first_b);
        check(w, &[first_a, last_b, a, b]);
        w.get_mut::<SocketFollow>(first_a).unwrap().joint = "missing".into();
        check(w, &[last_b, b]);
        w.get_mut::<SocketFollow>(first_a).unwrap().joint = "joint".into();
        check(w, &[first_a, last_b, a, b]);
        w.remove::<Parent>(b);
        check(w, &[first_a, last_b, a]);
        // An owner that is itself attached already has its composed override.
        w.insert(a, SocketFollow::new(b, "joint"));
        check(w, &[a, first_a, last_b]);
        w.remove::<SocketFollow>(first_a);
        w.remove::<SocketFollow>(last_b);
        check(w, &[a]);
        w.remove::<SocketFollow>(a);
        check(w, &[]);
    }

    #[test]
    fn owner_updates_reuse_one_chain_allocation() {
        let mut w = World::new(60, 0);
        let parent = w.spawn(Transform::at(2., 0., 0.));
        let child = w.spawn((Transform::at(1., 0., 0.), Parent(parent)));
        let mut owner = Owner::new(&w, child);
        let allocation = owner.chain.as_ptr();
        for _ in 0..100 {
            owner.update(&w, child, true, false);
            assert_eq!(owner.chain.as_ptr(), allocation);
            assert_eq!(owner.chain.len(), 2);
        }
        w.remove::<Parent>(child);
        owner.update(&w, child, true, true);
        assert_eq!(owner.chain.len(), 1);
        assert_eq!(owner.chain.as_ptr(), allocation);
    }

    #[test]
    fn distinct_diagnostic_identities_do_not_replace_each_other() {
        let mut w = World::new(60, 0);
        let e = w.spawn(Transform::default());
        let mut a = Attachments::default();
        a.warn(e, "socket", || "missing joint".into());
        a.warn(e, "stale", || "stale pose".into());
        a.warn(e, "socket", || "missing target".into());
        a.warn(e, "stale", || "stale pose".into());
        let registry = a.diagnostics.borrow();
        assert_eq!(registry.len(), 2);
        assert_eq!(registry[&(e, "socket")], "missing target");
        assert_eq!(registry[&(e, "stale")], "stale pose");
    }
}

#[cfg(test)]
#[path = "../../tests/light_selection/mod.rs"]
mod light_selection_tests;
