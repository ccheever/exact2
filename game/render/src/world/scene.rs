use crate::{Bloom, Environment, Fog, FrameInput, PointLightInput, Shadows, Sun};
use exact_game::{Camera, DirectionalLight, Entity, Parent, PointLight, Transform, World};
use glam::{Mat4, Vec3};

pub(super) fn pose(w: &World, e: Entity) -> Option<Transform> {
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
pub(super) fn snap(w: &World, mut e: Entity, parent_changed: bool) -> bool {
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
        Transform {
            position: self.prev.position.lerp(self.curr.position, alpha),
            rotation: self.prev.rotation.slerp(self.curr.rotation, alpha),
            scale: self.prev.scale.lerp(self.curr.scale, alpha),
        }
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
    pub fn reset(&mut self) {
        self.versions = None;
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
            let camera = w.query::<&Camera>().iter().find_map(|(e, c)| {
                (c.active
                    && c.near > 0.0
                    && c.far > c.near
                    && c.fov_y_degrees > 0.0
                    && c.fov_y_degrees < 180.0)
                    .then(|| pose(w, e).map(|t| (e, t, *c)))
                    .flatten()
            });
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
            let camera = self.camera.map_or(Vec3::ZERO, |(h, _)| h.curr.position);
            let mut distances = [f32::INFINITY; 16];
            for (i, light) in self.lights.iter_mut().enumerate() {
                light.history.update(w, next_tick, parent_changed);
                let distance = light.history.curr.position.distance_squared(camera);
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
                    self.lights[i]
                        .history
                        .curr
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
    pub fn frame(&mut self, w: &World, alpha: f32, aspect: f32) -> FrameInput<'_> {
        let alpha = alpha.clamp(0.0, 1.0);
        let (camera_pose, camera) = self
            .camera
            .map_or((Transform::default(), Camera::default()), |(h, c)| {
                (h.at(alpha), c)
            });
        // Only the selected sixteen histories are touched per frame.
        for i in 0..self.count {
            let l = &self.lights[self.selected[i]];
            let point = PointLightInput {
                position: l.history.at(alpha).position,
                color: l.light.color.into(),
                intensity: l.light.intensity,
                range: l.light.range,
            };
            self.output[i] = point;
        }
        let sun = self.sun.map(|(h, s)| Sun {
            direction: (h.at(alpha).rotation * -Vec3::Z).normalize_or(-Vec3::Y),
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
            proj: glam::camera::rh::proj::directx::perspective(
                camera.fov_y_degrees.to_radians(),
                aspect,
                camera.near,
                camera.far,
            ),
            camera_position: camera_pose.position,
            alpha,
            sun,
            points: &self.output[..self.count],
            environment: Environment {
                zenith: e.zenith,
                horizon: e.horizon,
                ground: e.ground,
                ambient: e.ambient,
                sun_disc: e.sun_disc,
                fog: e.fog.map(|f| Fog {
                    color: f.color,
                    density: f.density,
                    height_falloff: f.height_falloff,
                }),
            },
            exposure: e.exposure,
            bloom: e.bloom.map(|b| Bloom {
                threshold: b.threshold,
                intensity: b.intensity,
                radius: b.radius,
            }),
            timestamps: None,
        }
    }
}
