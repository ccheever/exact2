use crate::{FrameInput, PointLightInput, Shadows, Sun};
use exact_game::{Camera, DirectionalLight, Entity, Parent, PointLight, Transform, World};
use glam::{Mat4, Vec3};

pub(crate) fn pose(w: &World, e: Entity) -> Option<Transform> {
    let (scale, rotation, position) = w.global(e)?.to_scale_rotation_translation();
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
    for _ in 0..=w.len() {
        if w.fresh().contains(&e) {
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
            if next_tick {
                self.prev = self.curr;
            }
            self.curr = curr;
            if snap(w, self.entity, parent_changed) {
                self.prev = curr;
            }
        }
    }
    fn at(self, alpha: f32) -> Transform {
        interpolate([self.prev, self.curr], alpha)
    }
}
struct Light {
    history: History,
    light: PointLight,
    selected: bool,
}
#[derive(Default)]
pub(super) struct Scene {
    versions: Option<[u64; 3]>,
    pub(super) attachments: Attachments,
    camera: Option<(History, Camera)>,
    sun: Option<(History, DirectionalLight)>,
    lights: Vec<Light>,
    selected: [usize; 16],
    count: usize,
    output: [PointLightInput; 16],
}
impl Scene {
    #[cfg(test)]
    pub(super) fn lights_for_test(&self) -> Vec<Entity> {
        self.selected[..self.count]
            .iter()
            .map(|&i| self.lights[i].history.entity)
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
        self.attachments = Attachments::default();
        self.camera = None;
        self.sun = None;
        self.lights.clear();
        self.count = 0;
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
        if old.is_none_or(|v| v[1] != versions[1]) || structure {
            self.sun = w.query::<&DirectionalLight>().iter().find_map(|(e, s)| {
                pose(w, e).map(|t| {
                    (
                        self.sun
                            .filter(|(h, _)| h.entity == e)
                            .map_or(History::new(e, t), |(h, _)| h),
                        *s,
                    )
                })
            });
        }
        if let Some((history, _)) = &mut self.sun {
            history.update(w, next_tick, parent_changed);
        }
        if old.is_none_or(|v| v[2] != versions[2]) || structure {
            // Entity order allows an in-place merge: reuse matching histories, insert
            // only newly authored lights. No fresh vector on value-only light edits.
            let mut at = 0;
            for (e, light) in w.query::<&PointLight>().iter() {
                let Some(t) = pose(w, e) else {
                    continue;
                };
                while at < self.lights.len() && self.lights[at].history.entity.index() < e.index() {
                    self.lights.remove(at);
                }
                if self.lights.get(at).is_some_and(|l| l.history.entity == e) {
                    self.lights[at].light = *light;
                } else {
                    self.lights.insert(
                        at,
                        Light {
                            history: History::new(e, t),
                            light: *light,
                            selected: false,
                        },
                    );
                }
                at += 1;
            }
            self.lights.truncate(at);
        }
        if next_tick || moved || structure || old != Some(versions) {
            self.count = 0;
            self.attachments.frame(1.);
            let camera = self.camera.map_or(Vec3::ZERO, |(h, _)| {
                displayed(&self.attachments.output, h.entity, h.curr).position
            });
            let mut distances = [f32::INFINITY; 16];
            for (i, light) in self.lights.iter_mut().enumerate() {
                light.history.update(w, next_tick, parent_changed);
                let distance = displayed(
                    &self.attachments.output,
                    light.history.entity,
                    light.history.curr,
                )
                .position
                .distance_squared(camera);
                // Selected lights keep membership until a challenger is >10% nearer.
                // Entity order breaks exact ties; this scan happens only at feed time.
                let score = if light.selected {
                    distance / 1.21
                } else {
                    distance
                };
                let at = distances.partition_point(|d| *d <= score);
                if at < 16 {
                    distances.copy_within(at..15, at + 1);
                    self.selected.copy_within(at..15, at + 1);
                    distances[at] = score;
                    self.selected[at] = i;
                    self.count = (self.count + 1).min(16);
                }
                light.selected = false;
            }
            self.selected[..self.count].sort_unstable_by(|a, b| {
                let distance = |i: usize| {
                    let h = self.lights[i].history;
                    displayed(&self.attachments.output, h.entity, h.curr)
                        .position
                        .distance_squared(camera)
                };
                distance(*a).total_cmp(&distance(*b)).then(a.cmp(b))
            });
            for &i in &self.selected[..self.count] {
                self.lights[i].selected = true;
            }
        }
        self.versions = Some(versions);
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
        let (camera_pose, mut camera) =
            self.camera
                .map_or((Transform::default(), Camera::default()), |(h, c)| {
                    (
                        displayed(&self.attachments.output, h.entity, h.at(alpha)),
                        c,
                    )
                });
        if !pixels {
            if let exact_game::Projection::Orthographic { integer_scale, .. } =
                &mut camera.projection
            {
                *integer_scale = false;
            }
        }
        // Only the selected sixteen histories are touched per frame.
        for i in 0..self.count {
            let l = &self.lights[self.selected[i]];
            let point = PointLightInput {
                position: displayed(
                    &self.attachments.output,
                    l.history.entity,
                    l.history.at(alpha),
                )
                .position,
                color: l.light.color.into(),
                intensity: l.light.intensity,
                range: l.light.range,
            };
            self.output[i] = point;
        }
        let sun = self.sun.map(|(h, s)| Sun {
            direction: (displayed(&self.attachments.output, h.entity, h.at(alpha)).rotation
                * -Vec3::Z)
                .normalize_or(-Vec3::Y),
            color: s.color.into(),
            // 10,000 lux maps to the renderer's default key radiance of 3.
            illuminance: s.illuminance * 0.0003,
            shadows: s.shadows.then(Shadows::default),
        });
        let e = w
            .try_resource::<exact_game::Environment>()
            .as_deref()
            .copied()
            .unwrap_or_default();
        FrameInput {
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
            points: &self.output[..self.count],
            environment: e,
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
struct Attachment {
    history: History,
    owner: History,
    chain: Vec<[Transform; 2]>,
    offset: Transform,
    model_digest: u64,
}
#[derive(Default)]
pub(crate) struct Attachments {
    owners: std::collections::BTreeMap<Entity, History>,
    items: Vec<Attachment>,
    pub(crate) diagnostics: std::collections::BTreeMap<Entity, String>,
    pub output: Vec<DisplayedAttachment>,
}
impl Attachments {
    fn warn(&mut self, e: Entity, message: impl FnOnce() -> String) {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.diagnostics.entry(e) {
            let message = message();
            // Presentation may run on only some hosts. Never put its diagnostics
            // in the simulation journal, which is part of continuation saves.
            #[cfg(target_arch = "wasm32")]
            web_sys::console::warn_1(&message.clone().into());
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{message}");
            entry.insert(message);
        }
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
        self.diagnostics.retain(|e, _| w.contains(*e));
        for owner in self.owners.values_mut() {
            owner.update(w, next_tick, parent_changed);
        }
        // Retain animated owners before attachments are spawned so a new charm
        // inherits the owner's displayed history, rather than snapping its bones.
        for (e, _) in w.query::<&exact_game::Pose>().iter() {
            if let Some(t) = pose(w, e) {
                self.owners.entry(e).or_insert_with(|| History::new(e, t));
            }
        }
        let mut at = 0;
        for (e, follow) in w.query::<&exact_game::SocketFollow>().iter() {
            let target = match &follow.target {
                exact_game::FollowTarget::Entity(e) => Some(*e),
                exact_game::FollowTarget::Name(n) => w.named(n),
            };
            let resolved = target
                .ok_or_else(|| format!("unresolved target {:?}", follow.target))
                .and_then(|target| {
                    exact_game::animation::socket_matrix(w, target, &follow.joint).map(|_| target)
                });
            let target = match resolved {
                Ok(target) => target,
                Err(error) => {
                    self.warn(e, || {
                        format!(
                            "SocketFollow `{}`: {error}; using authored Transform",
                            w.name(e).unwrap_or("unnamed")
                        )
                    });
                    continue;
                }
            };
            let (Some(home), Some(owner)) = (pose(w, e), pose(w, target)) else {
                self.warn(e, || {
                    format!(
                        "SocketFollow `{}`: unresolved Transform",
                        w.name(e).unwrap_or("unnamed")
                    )
                });
                continue;
            };
            let Ok(node) = exact_game::animation::socket_node(w, target, &follow.joint) else {
                continue;
            };
            let Some(mesh) = w.get::<exact_game::Mesh>(target) else {
                continue;
            };
            let exact_game::Mesh::Asset(name) = &*mesh else {
                continue;
            };
            let Some(model) = w.model(name) else { continue };
            while at < self.items.len() && self.items[at].history.entity.index() < e.index() {
                self.items.remove(at);
            }
            let fresh = initial
                || !self
                    .items
                    .get(at)
                    .is_some_and(|v| v.history.entity == e && v.owner.entity == target);
            if fresh {
                if self.items.get(at).is_some_and(|v| v.history.entity == e) {
                    self.items.remove(at);
                }
                self.items.insert(
                    at,
                    Attachment {
                        history: History::new(e, home),
                        owner: History::new(target, owner),
                        chain: vec![],
                        offset: follow.offset,
                        model_digest: exact_game::hash::of(model),
                    },
                );
            }
            let item = &mut self.items[at];
            item.history.update(w, next_tick, parent_changed);
            item.owner = *self
                .owners
                .entry(target)
                .or_insert_with(|| History::new(target, owner));
            let model_changed = if models_changed {
                let digest = exact_game::hash::of(model);
                let changed = item.model_digest != digest;
                item.model_digest = digest;
                changed
            } else {
                false
            };
            item.offset = follow.offset;
            item.chain.clear();
            let sampled = w.get::<exact_game::Pose>(target);
            let rest;
            let (prev, curr) = if let Some(p) = sampled.as_ref().filter(|p| {
                p.local.len() == model.nodes.len() * 10 && p.previous.len() == p.local.len()
            }) {
                (&p.previous[..], &p.local[..])
            } else {
                rest = exact_game::animation::bind_pose(model);
                (&rest[..], &rest[..])
            };
            let snap = initial || model_changed || snap(w, target, parent_changed);
            let mut node = Some(node);
            while let Some(i) = node {
                let start = i as usize * 10;
                let read = |p: &[f32]| Transform {
                    position: Vec3::from_slice(&p[start..]),
                    rotation: glam::Quat::from_slice(&p[start + 3..]).normalize(),
                    scale: Vec3::from_slice(&p[start + 7..]),
                };
                item.chain
                    .push([read(if snap { curr } else { prev }), read(curr)]);
                node = model.nodes[i as usize].parent;
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
        let owner = self
            .items
            .iter()
            .position(|v| v.history.entity == item.owner.entity)
            .map_or_else(
                || matrix(item.owner.at(alpha)),
                |i| self.matrix(i, alpha, remaining - 1),
            );
        item.chain.iter().fold(owner, |m, pair| {
            m * crate::skinning::interpolated_local(*pair, alpha)
        }) * matrix(item.offset)
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
