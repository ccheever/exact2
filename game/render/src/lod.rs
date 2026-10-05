//! Level-of-detail selection: one level per entity per frame, from the entity's
//! displayed position, with hysteresis. Presentation only: never saved or hashed.
use std::collections::BTreeMap;

/// Most levels one `ModelLod` entity may declare, its own model included.
pub(crate) const MAX_LEVELS: usize = 8;
/// A level changes only this fraction past its threshold, so an entity at a
/// boundary does not flicker between levels.
const HYSTERESIS: f32 = 0.05;
/// Selected for an entity drawn at no level (beyond `hide`).
pub(crate) const HIDDEN: u8 = u8::MAX;

/// One `ModelLod` entity as batched: where each level takes over, where it hides,
/// and which levels have resident draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lod {
    pub slot: u32,
    /// This entity's records, contiguous: `record..record + count`.
    pub record: u32,
    pub count: u32,
    /// Level `i` draws from `starts[i]` metres; `starts[0]` is zero.
    pub starts: [f32; MAX_LEVELS],
    pub levels: u8,
    pub hide: f32,
    /// Bit `i`: level `i` has draws (its model is resident).
    pub present: u8,
}

/// Check a `ModelLod`: finite positive distances in increasing order, a hide
/// distance beyond them, and at most `MAX_LEVELS` levels.
pub(crate) fn validate(lod: &exact_game::ModelLod) -> Result<(), String> {
    if lod.levels.len() + 1 > MAX_LEVELS {
        return Err(format!("ModelLod: at most {} levels", MAX_LEVELS - 1));
    }
    let mut last = 0.;
    for l in &lod.levels {
        if !(l.distance.is_finite() && l.distance > last) {
            return Err(format!(
                "ModelLod: level `{}` distance {} must be finite and beyond {last}",
                l.model, l.distance
            ));
        }
        last = l.distance;
    }
    if let Some(hide) = lod.hide {
        if !(hide.is_finite() && hide > last) {
            return Err(format!(
                "ModelLod: hide {hide} must be finite and beyond {last}"
            ));
        }
    }
    Ok(())
}

impl Lod {
    /// The level to draw at `distance`, given the level drawn last frame.
    pub fn select(&self, distance: f32, previous: Option<u8>) -> u8 {
        let target = |d: f32| {
            if d >= self.hide {
                return HIDDEN;
            }
            (1..self.levels as usize)
                .rev()
                .find(|&i| d >= self.starts[i])
                .unwrap_or(0) as u8
        };
        let wanted = match previous {
            None => target(distance),
            Some(p) => {
                // Move only once past the threshold by the hysteresis margin.
                let near = target(distance * (1. + HYSTERESIS));
                let far = target(distance * (1. - HYSTERESIS));
                let rank = |l: u8| if l == HIDDEN { u8::MAX } else { l };
                if rank(far) > rank(p) {
                    far
                } else if rank(near) < rank(p) {
                    near
                } else {
                    p
                }
            }
        };
        self.resident(wanted)
    }
    /// The nearest level with draws, while a level's model streams in.
    fn resident(&self, level: u8) -> u8 {
        if level == HIDDEN || self.present & (1 << level) != 0 {
            return level;
        }
        (1..self.levels)
            .flat_map(|step| [level.checked_sub(step), Some(level + step)])
            .flatten()
            .find(|&l| l < self.levels && self.present & (1 << l) != 0)
            .unwrap_or(level)
    }
}

/// The renderer's level state: batched entries and each record's level, the
/// level each entity drew last, and which records are hidden this frame.
#[derive(Default)]
pub(crate) struct Levels {
    pub entries: Vec<Lod>,
    pub records: Vec<u8>,
    // Per entry: the level drawn last frame (none before its first frame).
    selected: Vec<Option<u8>>,
    /// Per record: not this frame's level.
    pub hidden: Vec<bool>,
    /// `hidden` changed since the cull and skinning last read it.
    pub changed: bool,
}
impl Levels {
    /// New batches: entities keep the level they drew, by slot.
    pub fn set(&mut self, records: &[u8], entries: &[Lod]) {
        let previous: BTreeMap<u32, Option<u8>> = (self.entries.iter().map(|e| e.slot))
            .zip(self.selected.iter().copied())
            .collect();
        self.records.clear();
        self.records.extend_from_slice(records);
        self.entries.clear();
        self.entries.extend_from_slice(entries);
        self.selected.clear();
        self.selected.extend(
            entries
                .iter()
                .map(|e| previous.get(&e.slot).copied().flatten()),
        );
        self.hidden.clear();
        self.hidden.resize(records.len(), false);
        for (e, level) in self.entries.iter().zip(&self.selected) {
            if let Some(level) = *level {
                for i in e.record as usize..(e.record + e.count) as usize {
                    self.hidden[i] = self.records[i] != level;
                }
            }
        }
        self.changed = true;
    }
    /// Choose every entity's level from its distance (`distance(entry)`) and
    /// mark the records of other levels hidden.
    pub fn select(&mut self, distance: impl Fn(&Lod) -> f32) {
        for (e, selected) in self.entries.iter().zip(&mut self.selected) {
            let level = e.select(distance(e), *selected);
            if *selected == Some(level) {
                continue;
            }
            *selected = Some(level);
            self.changed = true;
            for i in e.record as usize..(e.record + e.count) as usize {
                self.hidden[i] = self.records[i] != level;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lod() -> Lod {
        let mut starts = [0.; MAX_LEVELS];
        starts[1] = 10.;
        starts[2] = 20.;
        Lod {
            slot: 3,
            record: 0,
            count: 3,
            starts,
            levels: 3,
            hide: 40.,
            present: 0b111,
        }
    }
    #[test]
    fn one_level_per_entity_with_hysteresis_and_streaming_fallback() {
        let l = lod();
        assert_eq!(l.select(9.9, None), 0);
        assert_eq!(l.select(10.1, None), 1);
        // Inside the 5% band the drawn level holds, either side of the threshold.
        assert_eq!(l.select(10.3, Some(0)), 0);
        assert_eq!(l.select(9.7, Some(1)), 1);
        assert_eq!(l.select(10.6, Some(0)), 1);
        assert_eq!(l.select(9.4, Some(1)), 0);
        assert_eq!(l.select(41., Some(2)), 2);
        assert_eq!(l.select(43., Some(2)), HIDDEN);
        assert_eq!(l.select(39., Some(HIDDEN)), HIDDEN);
        assert_eq!(l.select(37., Some(HIDDEN)), 2);
        // A far jump crosses several levels at once.
        assert_eq!(l.select(25., Some(0)), 2);
        // Level 1 still streaming: the nearest resident level draws instead.
        let streaming = Lod {
            present: 0b101,
            ..l
        };
        assert_eq!(streaming.select(15., None), 0);
        let mut levels = Levels::default();
        levels.set(&[0, 1, 2], &[l]);
        levels.select(|_| 15.);
        assert_eq!(levels.hidden, [true, false, true]);
        levels.changed = false;
        levels.select(|_| 15.5);
        assert!(!levels.changed, "the same level is no change");
    }
    #[test]
    fn invalid_levels_are_refused_by_name() {
        let lod = |distances: &[f32], hide| exact_game::ModelLod {
            levels: distances
                .iter()
                .map(|&distance| exact_game::LodLevel {
                    distance,
                    model: "low.model".into(),
                })
                .collect(),
            hide,
        };
        assert!(validate(&lod(&[10., 20.], Some(30.))).is_ok());
        assert!(validate(&lod(&[20., 10.], None)).is_err());
        assert!(validate(&lod(&[f32::NAN], None)).is_err());
        assert!(validate(&lod(&[-1.], None)).is_err());
        assert!(validate(&lod(&[10.], Some(5.))).is_err());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod gpu_tests {
    use exact_game::{Mesh, ModelLod, Pose, Transform, World};
    use exact_gpu::wgpu;
    // One 32×32 frame from the eye (0, 1, camera), looking at the origin.
    fn draw(renderer: &mut crate::Renderer, camera: f32) {
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let eye = glam::Vec3::new(0., 1., camera);
        let frame = crate::FrameInput {
            view: glam::camera::rh::view::look_at_mat4(eye, glam::Vec3::ZERO, glam::Vec3::Y),
            proj: glam::camera::rh::proj::directx::perspective(1., 1., 0.1, 500.),
            camera_position: eye,
            ..Default::default()
        };
        renderer.draw(&texture.create_view(&Default::default()), (32, 32), &frame);
    }
    // A skinned fox near, the same geometry unskinned (a rock) from 10 m.
    fn crowd_models(renderer: &mut crate::Renderer) -> Pose {
        let fox = crate::test_model::skinned_model();
        let mut rock = fox.clone();
        rock.skins.clear();
        rock.clips.clear();
        for node in &mut rock.nodes {
            node.skin = None;
        }
        for mesh in &mut rock.meshes {
            mesh.joints.clear();
            mesh.weights.clear();
        }
        renderer.prepare_model("fox.model", &fox).unwrap();
        renderer.prepare_model("rock.model", &rock).unwrap();
        let mut pose = Pose::default();
        pose.local = exact_game::animation::bind_pose(&fox);
        pose.previous = pose.local.clone();
        pose
    }
    fn lod() -> ModelLod {
        ModelLod {
            levels: vec![exact_game::LodLevel {
                distance: 10.,
                model: "rock.model".into(),
            }],
            hide: None,
        }
    }
    #[test]
    fn far_levels_skip_their_skinning_and_direct_draws_keep_level_zero() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let pose = crowd_models(&mut renderer);
        let mut w = World::new(60, 0);
        w.spawn((Transform::default(), Mesh::asset("fox.model"), pose, lod()));
        let mut feed = crate::Feed::default();
        feed.feed(&w, &mut renderer).unwrap();
        draw(&mut renderer, 3.);
        let near = renderer.models.skinning.as_ref().unwrap().active_jobs();
        assert!(near > 0, "the near level skins");
        draw(&mut renderer, 50.);
        let far = renderer.models.skinning.as_ref().unwrap().active_jobs();
        assert_eq!(far, 0, "a far level skins nothing");
        // Without indirect draws there is no per-instance cull: level 0 only.
        renderer.cull.indirect_execution = false;
        renderer.cull.epoch += 1;
        draw(&mut renderer, 50.);
        draw(&mut renderer, 50.);
        assert!(renderer.cull.direct);
        assert!(!renderer.cull.groups.is_empty());
        assert!(renderer
            .cull
            .groups
            .iter()
            .all(|g| renderer.batches[g.batch].level == 0));
        // Level 0 is the level selected, so the level it draws is the one skinned
        // and no record of it is hidden from the blended pass.
        assert!(renderer
            .levels
            .records
            .iter()
            .zip(&renderer.levels.hidden)
            .all(|(&level, &hidden)| hidden == (level != 0)));
        let direct = renderer.models.skinning.as_ref().unwrap().active_jobs();
        assert_eq!(direct, near, "direct draws skin level 0 at any distance");
    }
    #[test]
    fn a_level_change_uploads_only_what_changed() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let pose = crowd_models(&mut renderer);
        // Eight foxes in a line, the first farthest from the camera.
        let mut w = World::new(60, 0);
        for i in 0..8 {
            let z = -0.5 * (7 - i) as f32;
            w.spawn((
                Transform::at(0., 0., z),
                Mesh::asset("fox.model"),
                pose.clone(),
                lod(),
            ));
        }
        let mut feed = crate::Feed::default();
        feed.feed(&w, &mut renderer).unwrap();
        draw(&mut renderer, 3.);
        draw(&mut renderer, 3.);
        let jobs = renderer.models.skinning.as_ref().unwrap().active_jobs();
        assert_eq!(jobs, 8, "every fox is near and skins");
        let before = (
            renderer.cull.hidden_bytes,
            renderer.models.skinning.as_ref().unwrap().job_bytes,
        );
        // Only the first fox crosses 10 m (and its 5% band); the rest hold.
        draw(&mut renderer, 7.2);
        assert!(
            renderer.levels.hidden[0],
            "the first fox's near level hides"
        );
        assert_eq!(renderer.levels.hidden.iter().filter(|&&h| h).count(), 8);
        let skinning = renderer.models.skinning.as_ref().unwrap();
        assert_eq!(skinning.active_jobs(), 7);
        let records = renderer.cull.hidden_bytes - before.0;
        let jobs = skinning.job_bytes - before.1;
        eprintln!("one level change: {records} record bytes, {jobs} job bytes");
        // Its two records' hidden words, and the one job moved into its place.
        assert!(records <= 2 * 32, "{records} record bytes");
        assert!(jobs <= 16, "{jobs} job bytes");
        // Through every swap the list dispatches exactly the drawn records' jobs.
        for camera in [3., 7.2, 50., 3., 8., 7.2, 3.] {
            draw(&mut renderer, camera);
            let drawn: Vec<usize> = (0..renderer.models.records.len())
                .filter(|&r| renderer.models.records[r].skin.is_some())
                .filter(|&r| !renderer.levels.hidden[r])
                .collect();
            let skinning = renderer.models.skinning.as_ref().unwrap();
            assert_eq!(skinning.dispatched_records(), drawn, "camera at {camera}");
        }
    }
}
