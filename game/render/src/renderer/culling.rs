//! Per-frame culling inputs: draw groups, the retained setup and view planes.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
use super::*;
use crate::cull::{Group, CAPSULE, GROUP_WORDS, KEEP_ALL, PLAIN, RECORD_WORDS, SKIN_WORDS};

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// Resolve this frame's draw groups (one per batch and winding) and upload a
    /// new setup only when they or the retained structure changed.
    pub(super) fn prepare_cull(&mut self, frame: &FrameInput<'_>, cascades: Option<&Cascades>) {
        self.cull_groups(frame, cascades.map_or(0, |c| c.count));
        self.local_groups();
        if self.cull.stale() || self.locals_stale() {
            self.write_cull_setup();
        } else if self.levels.changed {
            let hidden = &self.levels.hidden;
            self.cull.hide_records(&self.queue, hidden);
            for local in &mut self.local_culls {
                if !local.cull.groups.is_empty() {
                    local.cull.hide_records(&self.queue, hidden);
                }
            }
        }
        self.levels.changed = false;
        self.local_views();
        if self.cull.direct || self.cull.groups.is_empty() {
            return;
        }
        self.cull.bind(
            &self.device,
            &self.uniform,
            [&self.transforms[0].raw, &self.transforms[1].raw],
            &self.materials.raw,
            &self.slots.raw,
            &self.attachment_matrices.raw,
            self.models.skinning.as_ref().map(|s| &s.palette.raw),
        );
        if self.culled_key.0 != self.cull.compacted.raw || self.culled_key.1 != self.cull.window {
            self.rebind_culled();
        }
        let mut views = [frame.proj * frame.view; 1 + crate::shadows::MAX_CASCADES];
        let count = cascades.map_or(0, |c| c.count as usize);
        if let Some(c) = cascades {
            views[1..1 + count].copy_from_slice(&c.matrices[..count]);
        }
        self.cull.write_views(
            &self.queue,
            &views[..1 + count],
            self.slot_list.len() as u32,
        );
    }

    /// The frame's CPU culling work: retained groups, no allocation once warm.
    pub(crate) fn cull_groups(&mut self, frame: &FrameInput<'_>, cascades: u32) {
        let casters = ((1u32 << cascades) - 1) << 1;
        let mut groups = std::mem::take(&mut self.cull.groups);
        groups.clear();
        self.viewmodels = false;
        for (index, batch) in self.batches.iter().enumerate() {
            if batch.slots.is_empty() {
                continue;
            }
            // Direct draws have no per-instance cull: only the finest level.
            if self.cull.direct && batch.level != 0 {
                continue;
            }
            self.viewmodels |= batch.viewmodel;
            let material = if ASSETS {
                self.model_batches[index]
            } else {
                None
            };
            // Blended models draw one at a time in the ordered translucent pass.
            if material.is_some_and(|m| {
                self.models.materials[m.0].alpha == exact_game::asset::AlphaMode::Blend
            }) {
                continue;
            }
            let flags = 1
                | if batch.casts_shadows { casters } else { 0 }
                | if self.cull.keep_all { KEEP_ALL } else { 0 };
            for (range, mirrored) in self.winding_ranges(index, frame) {
                groups.push(Group {
                    batch: index,
                    range,
                    mirrored,
                    flags,
                });
            }
        }
        self.cull.groups = groups;
    }

    // Groups, then model records and skins; see cull.wgsl for the word layout.
    fn write_cull_setup(&mut self) {
        self.cull.begin_setup();
        let mut chunk = 0;
        for index in 0..self.cull.groups.len() {
            let group = &self.cull.groups[index];
            let mesh = &self.meshes[self.batches[group.batch].mesh.0];
            let head = self.cull.group_words(index, chunk);
            chunk += head[3];
            let flags = group.flags
                | if mesh.capsule { CAPSULE } else { 0 }
                | if mesh.plain { PLAIN } else { 0 };
            let words: [u32; GROUP_WORDS] = [
                head[0],
                head[1],
                head[2],
                head[3],
                mesh.indices.end - mesh.indices.start,
                mesh.indices.start,
                mesh.base_vertex as u32,
                flags,
                self.cull.region(index),
                mesh.reach.x.to_bits(),
                mesh.reach.y.to_bits(),
                mesh.reach.z.to_bits(),
                mesh.cap.to_bits(),
                0,
                0,
                0,
            ];
            self.cull.words.extend(words);
        }
        let records = self.cull.words.len() as u32;
        let mut skins = 0;
        if ASSETS {
            for (index, record) in self.models.records.iter().enumerate() {
                let mesh = &self.meshes[record.geometry.0];
                let (center, radius) = if record.skin.is_some() {
                    skins += 1;
                    (mesh.center, mesh.half.length())
                } else {
                    // The node box in entity space: centre and |M| times half extents.
                    let m = record.local;
                    let abs = glam::Mat3::from_cols(
                        m.x_axis.truncate().abs(),
                        m.y_axis.truncate().abs(),
                        m.z_axis.truncate().abs(),
                    );
                    (m.transform_point3(mesh.center), (abs * mesh.half).length())
                };
                // A custom material's vertex shader moves vertices up to its reach.
                let radius = radius + self.models.custom.get(&record.material).unwrap_or(&0.);
                let skin = if record.skin.is_some() { skins } else { 0 };
                let words: [u32; RECORD_WORDS] = [
                    record.transform,
                    skin,
                    // Not this frame's level of detail: drawn in no view.
                    u32::from(self.levels.hidden.get(index).copied().unwrap_or(false)),
                    0,
                    center.x.to_bits(),
                    center.y.to_bits(),
                    center.z.to_bits(),
                    radius.to_bits(),
                ];
                self.cull.words.extend(words);
            }
        }
        let skins_at = self.cull.words.len() as u32;
        if ASSETS && skins > 0 {
            let skinning = self.models.skinning.as_ref().unwrap();
            for (record, &palette) in self.models.records.iter().zip(&skinning.offsets) {
                if let Some(skin) = record.skin {
                    let mut words = [0; SKIN_WORDS];
                    words[..2].copy_from_slice(&[palette & 0x7fff_ffff, skinning.joints(skin)]);
                    words[4..].copy_from_slice(&record.local.to_cols_array().map(f32::to_bits));
                    self.cull.words.extend(words);
                }
            }
        }
        if self.local_culls.iter().any(|l| !l.cull.groups.is_empty()) {
            let words = self.cull.words.clone();
            let items = self.slot_list.len() as u32;
            self.local_setups(&words, items, records, skins_at);
        }
        self.cull.finish_setup(
            &self.device,
            &self.queue,
            self.slot_list.len() as u32,
            records,
            skins_at,
        );
    }
}
