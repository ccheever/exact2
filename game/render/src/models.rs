//! Baked-model instance and material buffers. Primitive draws never bind these.
use crate::{
    buffers::{bytes, Buffer},
    DrawInstance, MaterialId, MeshId, RenderError,
};
use exact_game::asset::{AlphaMode, Filter, MaterialData, Model, TextureData, Wrap};
use exact_gpu::wgpu;
use glam::Mat4;
use std::collections::BTreeMap;

#[cfg(test)]
thread_local! { static MODEL_HASHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn model_hash_count() -> usize {
    MODEL_HASHES.with(|n| n.get())
}
#[cfg(test)]
thread_local! { static POSE_STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub(crate) fn model_digest(model: &Model) -> u64 {
    #[cfg(test)]
    MODEL_HASHES.with(|n| n.set(n.get() + 1));
    exact_game::hash::of(model)
}

pub(crate) struct Material {
    pub bind: Option<wgpu::BindGroup>,
    bytes: u64,
    pub alpha: AlphaMode,
    pub double_sided: bool,
    data: MaterialData,
    names: [Option<String>; 5],
}
/// u32 words per model instance record: header, local, normal, tint, glow.
pub(crate) const INSTANCE_WORDS: usize = 44;
/// A record's look word flag: its tint replaces the material's base colour factor.
pub(crate) const REPLACE: u32 = 1 << 31;
pub(crate) type ModelNode = (MeshId, MaterialId, Mat4, Option<u32>);
/// What the feed batches one loaded model from.
pub(crate) struct Draws<'a> {
    pub nodes: &'a [ModelNode],
    /// Each node's name, for `NodeMaterials`.
    pub names: &'a [String],
    /// The merged draw list (empty when nothing merges) and each draw's part names.
    pub merged: &'a [ModelNode],
    pub members: &'a [Vec<String>],
    pub starts: &'a [Vec<u32>],
    /// Renderer materials, by model material index.
    pub materials: &'a [MaterialId],
    /// Materials a game's `CustomMaterial` shades: a model with any of them in
    /// a merged draw draws its parts unmerged.
    pub custom: &'a std::collections::BTreeSet<MaterialId>,
}
pub(crate) struct Uploaded {
    /// The unmerged draw list; empty while merged-away parts are not resident.
    pub nodes: Vec<ModelNode>,
    /// Each drawn node's name, for `NodeMaterials`.
    pub names: Vec<String>,
    /// `nodes` with rigid parts sharing a material merged; empty when none merge.
    pub merged: Vec<ModelNode>,
    /// Each `merged` draw's node names, in vertex part order, and where each
    /// part's vertices start in its mesh.
    pub members: Vec<Vec<String>>,
    pub starts: Vec<Vec<u32>>,
    pub(crate) digest: u64,
    pub active: bool,
    pub meshes: Vec<MeshId>,
    /// Each model mesh's own mesh; none for a part drawn only merged until a
    /// `CustomMaterial` needs the parts.
    pub parts: Vec<Option<MeshId>>,
    pub materials: Vec<MaterialId>,
    pub skins: Vec<u32>,
}
impl Uploaded {
    /// Meshes with per-vertex joint words: skinned and rigid-palette nodes, and
    /// animated merged meshes.
    pub(crate) fn weighted_meshes(&self) -> impl Iterator<Item = usize> + '_ {
        (self.nodes.iter().chain(&self.merged))
            .filter(|n| n.3.is_some())
            .map(|n| n.0 .0)
    }
}
mod looks;
mod merge;
mod textures;
use textures::upload_texture;
pub(crate) use textures::Texture;
struct PoseHistory {
    entity: exact_game::Entity,
    saved: Option<(u64, u64, [exact_game::Transform; 2])>,
    /// A negative owner scale axis at either endpoint (counted in `mirrored`).
    mirrored: bool,
}
#[derive(Default)]
pub(crate) struct Models {
    pub loaded: BTreeMap<String, Uploaded>,
    prior_work: crate::world::assets::Work,
    pub revision: u64,
    pub(crate) textures: BTreeMap<String, Texture>,
    samplers: BTreeMap<([Wrap; 2], [Filter; 3]), wgpu::Sampler>,
    pub uploads: u64,
    pub reallocations: u64,
    pub records: Vec<DrawInstance>,
    pub materials: Vec<Material>,
    pub instances: Option<Buffer>,
    pub skinning: Option<crate::skinning::Skinning>,
    pub bind: Option<wgpu::BindGroup>,
    pub transparent: Vec<(usize, u32, f32)>,
    pub poses: Vec<[exact_game::Transform; 2]>,
    pub pose_indices: Vec<usize>,
    pose_history: Vec<PoseHistory>,
    /// Histories still interpolating, which the next tick must collapse.
    moving: Vec<usize>,
    touched: Vec<usize>,
    mirrored: usize,
    bind_buffers: Option<(wgpu::BindGroupLayout, [wgpu::Buffer; 3])>,
    words: Vec<u32>,
    normals: Vec<([u32; 16], [u32; 16])>,
    /// Per record, 1 + the first of its merged parts' looks (0: none), and
    /// those looks (tint, glow, then the part's first vertex as bits; a
    /// `u32::MAX` start ends each record's run), appended after the records.
    pub(crate) part_looks: (Vec<u32>, Vec<[f32; 8]>),
    /// Materials a game's `CustomMaterial` shades (never merged).
    pub(crate) custom: std::collections::BTreeSet<MaterialId>,
    /// The feed's looks for the next `set_draw_instances`, part starts relative.
    pub(crate) pending_looks: (Vec<u32>, Vec<[f32; 8]>),
}
impl Models {
    pub fn custom_data(&mut self, queue: &wgpu::Queue, data: impl Fn(u32) -> u32) {
        let mut changed = false;
        for (i, record) in self.records.iter_mut().enumerate() {
            let value = data(record.transform);
            if record.data != value {
                record.data = value;
                self.words[i * INSTANCE_WORDS + 2] = value;
                changed = true;
            }
        }
        if changed {
            self.instances
                .as_mut()
                .unwrap()
                .write(queue, 0, bytes(&self.words));
        }
    }
    // Whether the instance list changed; histories follow their entities.
    fn reconcile_pose_history(&mut self, entities: &[exact_game::Entity]) -> bool {
        if self.pose_history.len() == entities.len()
            && self
                .pose_history
                .iter()
                .zip(entities)
                .all(|(history, entity)| history.entity == *entity)
        {
            return false;
        }
        let mut old = std::mem::take(&mut self.pose_history)
            .into_iter()
            .peekable();
        let mut merged = Vec::with_capacity(entities.len());
        for &entity in entities {
            while old
                .peek()
                .is_some_and(|history| history.entity.index() < entity.index())
            {
                old.next();
            }
            let (saved, mirrored) = if old.peek().is_some_and(|history| history.entity == entity) {
                let history = old.next().unwrap();
                (history.saved, history.mirrored)
            } else {
                if old
                    .peek()
                    .is_some_and(|history| history.entity.index() == entity.index())
                {
                    old.next();
                }
                (None, false)
            };
            merged.push(PoseHistory {
                entity,
                saved,
                mirrored,
            });
        }
        self.pose_history = merged;
        self.mirrored = self.pose_history.iter().filter(|h| h.mirrored).count();
        true
    }
    fn prepare(&mut self, device: &wgpu::Device, family: &crate::pipeline::ModelPipelines) {
        if self.instances.is_some() {
            return;
        }
        let instances = Buffer::new(
            device,
            128,
            wgpu::BufferUsages::STORAGE,
            "game model instances",
        );
        let skinning = crate::skinning::Skinning::new(device);
        self.bind = Some(instance_bind(
            device,
            &family.instance,
            &instances,
            &skinning,
        ));
        self.skinning = Some(skinning);
        self.instances = Some(instances);
    }
    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        records: &[DrawInstance],
    ) -> Result<(), RenderError> {
        // Four u32 header words + affine matrix + inverse-transpose normal matrix.
        let skinning = self.skinning.as_mut().unwrap();
        skinning.set(device, queue, uniform, records)?;
        let words = &mut self.words;
        words.clear();
        self.normals.resize(records.len(), ([0; 16], [0; 16]));
        for (index, ((record, cached), &palette)) in records
            .iter()
            .zip(&mut self.normals)
            .zip(&skinning.offsets)
            .enumerate()
        {
            let key = record.local.to_cols_array().map(f32::to_bits);
            if cached.0 != key {
                let normal = record.local.inverse().transpose();
                if !normal.is_finite() {
                    return Err(RenderError::scene(
                        "model node has singular transform".into(),
                    ));
                }
                *cached = (key, normal.to_cols_array().map(f32::to_bits));
            }
            let normal = cached.1;
            words.extend([
                record.transform,
                record.material.0 as u32,
                record.data,
                palette,
            ]);
            words.extend(record.local.to_cols_array().map(f32::to_bits));
            words.extend(normal);
            words.extend(record.tint.map(f32::to_bits));
            words.extend(record.glow.map(f32::to_bits));
            // Low 31 bits: 1 + the record's first part look; top bit: replace.
            let look = self.part_looks.0.get(index).copied().unwrap_or(0);
            let base = look & !REPLACE;
            let at = if base == 0 {
                0
            } else {
                records.len() as u32 + base
            };
            words.push(at | (look & REPLACE));
        }
        // A merged part's look is one record-sized entry: its tint and glow. A
        // run's first entry also holds the run's part count, for the shader's
        // binary search over the starts.
        let mut run = words.len();
        for look in &self.part_looks.1 {
            words.extend([0; 36]);
            words.extend(look.map(f32::to_bits));
            if look[7].to_bits() == u32::MAX {
                words[run] = ((words.len() - run) / INSTANCE_WORDS - 1) as u32;
                run = words.len();
            }
        }
        if words.len() as u64 * 4 > device.limits().max_storage_buffer_binding_size {
            return Err(RenderError::scene(
                "model instance buffer exceeds device limit".into(),
            ));
        }
        let instances = self.instances.as_mut().expect("prepared model instances");
        self.reallocations += u64::from(instances.grow(device, queue, (words.len() * 4) as u64));
        let buffers = (
            layout.clone(),
            [
                instances.raw.clone(),
                skinning.weights.raw.clone(),
                skinning.palette.raw.clone(),
            ],
        );
        if self.bind_buffers.as_ref() != Some(&buffers) {
            self.bind = Some(instance_bind(device, layout, instances, skinning));
            self.bind_buffers = Some(buffers);
        }
        instances.write(queue, 0, bytes(words));
        self.records.clear();
        self.records.extend_from_slice(records);
        let mut entities: Vec<_> = records.iter().map(|r| r.transform).collect();
        entities.sort_unstable();
        entities.dedup();
        self.pose_indices.clear();
        self.pose_indices.extend(
            records
                .iter()
                .map(|r| entities.binary_search(&r.transform).unwrap()),
        );
        self.poses
            .resize(entities.len(), [exact_game::Transform::default(); 2]);
        Ok(())
    }
}
fn instance_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &Buffer,
    skinning: &crate::skinning::Skinning,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("game model instances"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.raw.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: skinning.palette.raw.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: skinning.weights.raw.as_entire_binding(),
            },
        ],
    })
}
impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// Upload model geometry and material textures once; return stable renderer
    /// handles. Meshes `skip` marks (parts only drawn merged) are not uploaded.
    pub(crate) fn add_model(
        &mut self,
        model: &Model,
        skip: &[bool],
    ) -> Result<(Vec<Option<MeshId>>, Vec<MaterialId>), RenderError> {
        model.validate().map_err(RenderError::scene)?;
        self.pipelines.prepare_model(&self.device, model);
        self.models
            .prepare(&self.device, self.pipelines.models.as_ref().unwrap());
        for (name, pixel, srgb) in [
            ("\0white-srgb", [255; 4], true),
            ("\0white", [255; 4], false),
            ("\0normal", [128, 128, 255, 255], false),
        ] {
            self.add_texture(
                name,
                &TextureData {
                    width: 1,
                    height: 1,
                    mips: vec![pixel.to_vec()],
                    srgb,
                    ..Default::default()
                },
            )?;
        }
        let meshes = (model.meshes.iter().enumerate())
            .map(|(m, mesh)| {
                (!skip.get(m).copied().unwrap_or(false)).then(|| self.add_model_mesh(mesh))
            })
            .collect();
        let mut materials = Vec::new();
        for material in &model.materials {
            let slot = self
                .models
                .materials
                .iter()
                .position(|m| m.bind.is_none())
                .unwrap_or(self.models.materials.len());
            materials.push(MaterialId(slot));
            let names = texture_names(material, &model.textures);
            let material = material_bind(
                &self.device,
                &self.queue,
                &self.pipelines.models.as_ref().unwrap().material,
                material,
                &names,
                &self.models.textures,
            );
            if slot == self.models.materials.len() {
                self.models.materials.push(material);
            } else {
                self.models.materials[slot] = material;
            }
        }
        Ok((meshes, materials))
    }
    /// Prepare a named model inside its Pending window. Later feeds only read handles.
    pub fn prepare_model(&mut self, name: &str, model: &Model) -> Result<(), RenderError> {
        self.prepare_model_digest(name, model, model_digest(model))
    }
    pub(crate) fn prepare_model_digest(
        &mut self,
        name: &str,
        model: &Model,
        digest: u64,
    ) -> Result<(), RenderError> {
        if let Some(resident) = self
            .models
            .loaded
            .get_mut(name)
            .filter(|m| m.digest == digest)
        {
            if !resident.active {
                resident.active = true;
                self.models.revision += 1;
            }
            return self.upload_parts(name, model);
        }
        model.validate().map_err(RenderError::scene)?;
        let replaced = self.models.loaded.remove(name).is_some();
        if replaced {
            self.reclaim_orphan_slots();
        }
        // Parts only drawn merged are uploaded only when something needs them.
        let away = merge::merged_away(model);
        let (parts, materials) = self.add_model(model, &away)?;
        let mut meshes: Vec<MeshId> = parts.iter().flatten().copied().collect();
        for id in &meshes {
            self.meshes[id.0].asset = true;
        }
        let animated = merge::animated_groups(model);
        let skins = self.models.skinning.as_mut().unwrap().add_merged(
            &self.device,
            &self.queue,
            model,
            &animated.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>(),
        );
        if replaced {
            self.models.skinning.as_mut().unwrap().mark_fresh(&skins);
        }
        let names = model
            .nodes
            .iter()
            .filter(|n| n.mesh.is_some())
            .map(|n| n.name.clone())
            .collect();
        // Merged-away parts have no mesh: the merge reads only their offsets.
        let nodes = merge::draw_nodes(model, &parts, &materials, &skins)?;
        let drawn: Vec<u32> = model.nodes.iter().filter_map(|n| n.mesh).collect();
        let first = skins.len() - animated.len();
        let (merged, members, starts, merged_meshes) =
            self.merge_static(model, &nodes, &drawn, &animated, &skins[first..]);
        meshes.extend(merged_meshes);
        let nodes = if parts.iter().all(Option::is_some) {
            nodes
        } else {
            Vec::new()
        };
        self.models.loaded.insert(
            name.into(),
            Uploaded {
                nodes,
                names,
                merged,
                members,
                starts,
                digest,
                active: true,
                meshes,
                parts,
                materials,
                skins,
            },
        );
        self.reclaim_orphan_slots();
        self.models.revision += 1;
        self.upload_parts(name, model)
    }
    pub(crate) fn retired_bytes(&self, live: &std::collections::BTreeSet<String>) -> u64 {
        let retained: std::collections::BTreeSet<_> = self
            .models
            .loaded
            .iter()
            .filter(|(n, m)| m.active && live.contains(*n))
            .flat_map(|(_, m)| m.meshes.iter().map(|m| m.0))
            .collect();
        let live_mesh_bytes: u64 = self
            .meshes
            .iter()
            .enumerate()
            .filter(|(i, m)| !m.asset || retained.contains(i))
            .map(|(_, m)| m.vertex_bytes + u64::from(m.indices.end - m.indices.start) * 4)
            .sum();
        // Shared arenas are charged at their actual buffer capacity. Unused tails
        // and orphan spans remain retired until a GPU-to-GPU packing pass.
        self.mesh_buffer_bytes().saturating_sub(live_mesh_bytes)
            + self
                .models
                .loaded
                .iter()
                .filter(|(n, m)| !m.active || !live.contains(*n))
                .flat_map(|(_, m)| m.materials.iter())
                .map(|id| self.models.materials[id.0].bytes)
                .sum::<u64>()
            + self
                .models
                .textures
                .iter()
                .filter(|(n, t)| !n.starts_with('\0') && (!t.active || !live.contains(*n)))
                .map(|(_, t)| t.bytes)
                .sum::<u64>()
            + self.models.skinning.as_ref().map_or(0, |s| {
                let live_weights: u64 = self
                    .models
                    .loaded
                    .iter()
                    .filter(|(n, m)| m.active && live.contains(*n))
                    .flat_map(|(_, m)| m.weighted_meshes())
                    .collect::<std::collections::BTreeSet<_>>()
                    .iter()
                    .map(|&i| self.meshes[i].vertex_bytes)
                    .sum();
                s.retired_bytes(&self.models.loaded, live)
                    + s.weights.raw.size().saturating_sub(live_weights)
            })
    }
    /// Reclaim retired asset slots, preserving every live handle and pose history.
    pub(crate) fn compact_assets(
        &mut self,
        live: &std::collections::BTreeSet<String>,
        prepared: &std::collections::BTreeSet<String>,
    ) {
        self.models
            .loaded
            .retain(|n, m| live.contains(n) && (m.active || prepared.contains(n)));
        self.models
            .textures
            .retain(|n, t| n.starts_with('\0') || (t.active && live.contains(n)));
        self.reclaim_orphan_slots();
        if let Some(skin) = &mut self.models.skinning {
            skin.compact_metadata(&self.device, &self.queue, false);
        }
        if self.retired_bytes(live) > crate::renderer::RETIRED_BUDGET {
            if let Some(skin) = &mut self.models.skinning {
                skin.compact_metadata(&self.device, &self.queue, true);
            }
            self.pack_mesh_buffers();
            if let (Some(family), Some(instances), Some(skin)) = (
                &self.pipelines.models,
                &self.models.instances,
                &self.models.skinning,
            ) {
                self.models.bind = Some(instance_bind(
                    &self.device,
                    &family.instance,
                    instances,
                    skin,
                ));
            }
        }
        self.models.revision += 1;
    }
    fn reclaim_orphan_slots(&mut self) {
        let meshes: std::collections::BTreeSet<_> = self
            .models
            .loaded
            .values()
            .flat_map(|m| m.meshes.iter().map(|id| id.0))
            .collect();
        let materials: std::collections::BTreeSet<_> = self
            .models
            .loaded
            .values()
            .flat_map(|m| m.materials.iter().map(|id| id.0))
            .collect();
        for (i, m) in self.meshes.iter_mut().enumerate() {
            if m.asset && !meshes.contains(&i) {
                m.indices = 0..0;
                m.vertex_bytes = 0;
            }
        }
        for (i, m) in self.models.materials.iter_mut().enumerate() {
            if !materials.contains(&i) {
                m.bind = None;
                m.bytes = 0;
                m.names = Default::default();
            }
        }
        while self
            .models
            .materials
            .last()
            .is_some_and(|m| m.bind.is_none())
        {
            self.models.materials.pop();
        }
        if let Some(s) = &mut self.models.skinning {
            s.reclaim(&self.models.loaded);
        }
    }
    pub(crate) fn retire_texture(&mut self, name: &str) {
        if let Some(t) = self.models.textures.get_mut(name) {
            t.active = false;
            t.sprite_bind = None;
        }
    }
    pub(crate) fn sprite_texture(&mut self, name: &str) {
        if let Some(texture) = self.models.textures.get_mut(name).filter(|t| t.active) {
            if texture.sprite_bind.is_none() {
                texture.sprite_bind = Some(self.quads.sprite_bind(&self.device, texture));
            }
        }
    }
    /// Reuse a name only when its content digest matches. CPU mips may be dropped.
    pub fn add_texture(&mut self, name: &str, data: &TextureData) -> Result<(), RenderError> {
        let digest = exact_game::hash::of(data);
        if let Some(texture) = self
            .models
            .textures
            .get_mut(name)
            .filter(|t| t.digest == digest)
        {
            texture.active = true;
            return Ok(());
        }
        data.validate().map_err(RenderError::scene)?;
        let sprite = self
            .models
            .textures
            .get(name)
            .is_some_and(|texture| texture.sprite_bind.is_some());
        let mut texture =
            upload_texture(&self.device, &self.queue, data, &mut self.models.samplers)?;
        texture.digest = digest;
        if sprite {
            texture.sprite_bind = Some(self.quads.sprite_bind(&self.device, &texture));
        }
        self.models.textures.insert(name.into(), texture);
        self.models.uploads += 1;
        for material in &mut self.models.materials {
            if material.bind.is_some()
                && material.names.iter().flatten().any(|n| n == name)
                && material
                    .names
                    .iter()
                    .flatten()
                    .all(|n| self.models.textures.contains_key(n))
            {
                *material = material_bind(
                    &self.device,
                    &self.queue,
                    &self.pipelines.models.as_ref().unwrap().material,
                    &material.data,
                    &material.names,
                    &self.models.textures,
                );
            }
        }
        Ok(())
    }
    /// Cumulative model pipeline creations and asset texture uploads, for diagnostics.
    pub fn asset_work(&self) -> (usize, u64) {
        (
            self.pipelines.models.as_ref().map_or(0, |m| {
                m.forward.iter().filter(|p| p.is_some()).count()
                    + m.shadow.iter().filter(|p| p.is_some()).count()
            }),
            self.models.uploads,
        )
    }
    /// Resident texture bytes and formats, as JSON for the world state.
    pub(crate) fn texture_summary(&self) -> String {
        textures::summary(&self.models.textures)
    }
    pub(crate) fn residency_work(&self) -> crate::world::assets::Work {
        let (pipelines, textures) = self.asset_work();
        let skin = self.models.skinning.as_ref();
        crate::world::assets::Work {
            texture_uploads: textures,
            mesh_uploads: self.mesh_uploads,
            pipeline_creations: crate::pipeline::STARTUP_PIPELINES
                + pipelines as u64
                + skin.map_or(0, |s| s.pipeline_creations)
                + self.quads.work_pipelines(),
            buffer_reallocations: self.models.reallocations
                + skin.map_or(0, |s| s.reallocations)
                + self.quads.reallocations(),
        }
        .plus(self.models.prior_work)
    }
    /// Replace additional draw records. Primitive batches retain their compact identity
    /// record: slot = transform = material, geometry in the batch, local = identity.
    pub fn set_draw_instances(&mut self, records: &[DrawInstance]) -> Result<(), RenderError> {
        self.cull.epoch += 1;
        let Some(family) = &self.pipelines.models else {
            assert!(records.is_empty(), "model instances need prepared assets");
            return Ok(());
        };
        // Part starts become absolute vertices; direct callers bring no looks.
        let (bases, mut looks) = std::mem::take(&mut self.models.pending_looks);
        for (record, &base) in records.iter().zip(&bases) {
            let base = base & !REPLACE;
            if base == 0 {
                continue;
            }
            let first = self.meshes[record.geometry.0].base_vertex as u32;
            for look in &mut looks[base as usize - 1..] {
                let start = look[7].to_bits();
                if start == u32::MAX {
                    break;
                }
                look[7] = f32::from_bits(first + start);
            }
        }
        self.models.part_looks = (bases, looks);
        self.models.set(
            &self.device,
            &self.queue,
            &family.instance,
            &self.uniform,
            records,
        )
    }
    /// Feed poses for transparency and winding on completed ticks. A full pass
    /// walks every model instance; otherwise only instances in blocks whose
    /// local or propagated poses changed and those still interpolating.
    pub(crate) fn model_poses(
        &mut self,
        world: &exact_game::World,
        entities: &[exact_game::Entity],
        initial: bool,
        moved: Moved<'_>,
    ) {
        if let Some(skinning) = &mut self.models.skinning {
            skinning.feed(&self.queue, world, entities, initial);
        }
        let models = &mut self.models;
        // The instance list changes only with the feed's batches, which pass
        // `Moved::All`; an incremental step skips the O(instances) comparison.
        let rebuilt = matches!(moved, Moved::All) && models.reconcile_pose_history(entities);
        let mut touched = std::mem::take(&mut models.touched);
        touched.clear();
        match moved {
            Moved::Pages { pages } if !initial && !rebuilt => {
                touched.append(&mut models.moving);
                let history = &models.pose_history;
                let at =
                    |index: usize| history.partition_point(|h| (h.entity.index() as usize) < index);
                for &page in pages {
                    touched.extend(at(page * exact_game::PAGE)..at((page + 1) * exact_game::PAGE));
                }
                touched.sort_unstable();
                touched.dedup();
                for &i in &touched {
                    models.update_pose(world, i, None);
                }
            }
            _ => {
                models.moving.clear();
                for i in 0..models.pose_history.len() {
                    let digest =
                        crate::world::shown(world, models.pose_history[i].entity, |m| match m {
                            exact_game::Mesh::Asset(name) => {
                                models.loaded.get(name).map(|m| m.digest)
                            }
                            _ => None,
                        })
                        .flatten()
                        .unwrap_or(0);
                    models.update_pose(world, i, Some((digest, initial)));
                }
            }
        }
        models.touched = touched;
    }
}
/// Which model instances a completed tick may have moved.
pub(crate) enum Moved<'a> {
    /// Every instance: structure, parents or assets changed.
    All,
    /// Instances on these pages (`PAGE` entity blocks), in index order, whose
    /// local or propagated poses may have changed. The instance list is the
    /// previous step's.
    Pages { pages: &'a [usize] },
}
impl Models {
    /// Whether any instance's owner pose has a negative scale axis at either
    /// endpoint, so its winding can differ from its batch's.
    pub(crate) fn any_mirrored_owner(&self) -> bool {
        self.mirrored != 0
    }
    // One instance's history step. A full pass supplies its asset digest and
    // whether the feed is initial; an incremental one keeps the saved digest.
    fn update_pose(&mut self, world: &exact_game::World, i: usize, full: Option<(u64, bool)>) {
        #[cfg(test)]
        POSE_STEPS.with(|n| n.set(n.get() + 1));
        let history = &mut self.pose_history[i];
        let Some(pose) = crate::world::scene::pose(world, history.entity) else {
            return;
        };
        let digest = full.map_or_else(|| history.saved.map_or(0, |s| s.0), |(d, _)| d);
        let saved = history
            .saved
            .get_or_insert((digest, world.tick(), [pose; 2]));
        // Conservative: any negative axis at either endpoint may flip parity
        // somewhere between them.
        let mirrored = |pair: &[exact_game::Transform; 2]| {
            pair.iter().any(|t| t.scale.cmplt(glam::Vec3::ZERO).any())
        };
        if full.is_some_and(|(_, initial)| initial)
            || saved.0 != digest
            || crate::world::scene::snap(world, history.entity, false)
        {
            saved.2 = [pose; 2];
        } else {
            if saved.1 != world.tick() {
                saved.2[0] = saved.2[1];
            }
            saved.2[1] = pose;
        }
        saved.0 = digest;
        saved.1 = world.tick();
        let now = mirrored(&saved.2);
        if saved.2[0] != saved.2[1] {
            self.moving.push(i);
        }
        if history.mirrored != now {
            history.mirrored = now;
            if now {
                self.mirrored += 1;
            } else {
                self.mirrored -= 1;
            }
        }
        if let Some(active) = self.poses.get_mut(i) {
            *active = saved.2;
        }
    }
}
fn material_bind(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    m: &MaterialData,
    names: &[Option<String>; 5],
    textures: &BTreeMap<String, Texture>,
) -> Material {
    let mut values = Vec::new();
    values.extend(m.base_color);
    values.extend([
        m.metallic,
        m.roughness,
        m.normal_scale,
        m.occlusion_strength,
    ]);
    values.extend(m.emissive);
    values.push(m.alpha_cutoff);
    values.extend([
        match m.alpha_mode {
            AlphaMode::Opaque => 0.,
            AlphaMode::Mask => 1.,
            AlphaMode::Blend => 2.,
        },
        0.,
        0.,
        0.,
    ]);
    for uv in m.uv_transforms {
        values.extend([uv[0], uv[1], uv[2], uv[3], uv[4], uv[5], 0., 0.]);
    }
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("game baked material"),
        size: (values.len() * 4) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform, 0, bytes(&values));
    let selected: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            name.as_ref()
                .and_then(|n| textures.get(n))
                .unwrap_or_else(|| {
                    &textures[match i {
                        0 | 3 => "\0white-srgb",
                        1 => "\0normal",
                        _ => "\0white",
                    }]
                })
        })
        .collect();
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: uniform.as_entire_binding(),
    }];
    for (i, texture) in selected.iter().enumerate() {
        entries.push(wgpu::BindGroupEntry {
            binding: 1 + i as u32 * 2,
            resource: wgpu::BindingResource::TextureView(&texture.view),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2 + i as u32 * 2,
            resource: wgpu::BindingResource::Sampler(&texture.sampler),
        });
    }
    Material {
        bytes: uniform.size(),
        bind: Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game material textures"),
            layout,
            entries: &entries,
        })),
        alpha: m.alpha_mode,
        double_sided: m.double_sided,
        data: m.clone(),
        names: names.clone(),
    }
}

fn texture_names(m: &MaterialData, names: &[String]) -> [Option<String>; 5] {
    [
        m.base_color_texture,
        m.normal_texture,
        m.metallic_roughness_texture,
        m.emissive_texture,
        m.occlusion_texture,
    ]
    .map(|i| i.map(|i| names[i as usize].clone()))
}
#[cfg(test)]
mod arrival_tests;

#[cfg(test)]
mod retirement_regressions {
    use super::*;
    #[test]
    fn multipart_models_share_entity_history_and_keep_unchanged_bindings() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let mut model = crate::test_model::skinned_model();
        let part = model
            .nodes
            .iter()
            .find(|n| n.mesh.is_some())
            .unwrap()
            .clone();
        model.nodes.push(part);
        renderer.prepare_model("parts.model", &model).unwrap();
        let mut w = exact_game::World::new(60, 0);
        w.spawn((
            exact_game::Transform::default(),
            exact_game::Mesh::asset("parts.model"),
        ));
        w.propagate();
        let mut feed = crate::Feed::default();
        feed.feed(&w, &mut renderer).unwrap();
        assert!(renderer.models.records.len() >= 2);
        assert_eq!(renderer.models.poses.len(), 1);
        assert!(renderer.models.pose_indices.iter().all(|&i| i == 0));
        let bind = renderer.models.bind.clone();
        let records = renderer.models.records.clone();
        renderer.set_draw_instances(&records).unwrap();
        assert_eq!(renderer.models.bind, bind);
        // Direct uploads need the same ascending entity ranks as Feed, even when
        // records arrive out of order or several model parts share one entity.
        for (transforms, indices) in [
            (&[9, 2, 9, u32::MAX, 0, 2][..], &[2, 1, 2, 3, 0, 1][..]),
            (&[2, 9][..], &[0, 1][..]),
            (&[9, 9, 9][..], &[0, 0, 0][..]),
            (&[][..], &[][..]),
            (&[0, u32::MAX][..], &[0, 1][..]),
        ] {
            let upload: Vec<_> = transforms
                .iter()
                .map(|&transform| DrawInstance {
                    data: 0,
                    transform,
                    skin: None,
                    tint: [1.; 4],
                    glow: [0.; 3],
                    ..records[0]
                })
                .collect();
            renderer.set_draw_instances(&upload).unwrap();
            assert_eq!(renderer.models.pose_indices, indices);
            assert_eq!(
                renderer.models.poses.len(),
                indices.iter().max().map_or(0, |i| i + 1)
            );
            assert!(renderer
                .models
                .records
                .iter()
                .map(|r| r.transform)
                .eq(transforms.iter().copied()));
        }
    }

    #[test]
    fn static_instances_cost_no_pose_steps_and_moved_ones_still_interpolate() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let model: Model =
            exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
                .unwrap();
        renderer.prepare_model("crate.model", &model).unwrap();
        struct Grove;
        impl exact_game::Game for Grove {
            type Args = ();
            const ID: &'static str = "static-model-poses";
            fn setup(w: &mut exact_game::World, _: &()) {
                for i in 0..3 * exact_game::PAGE {
                    w.spawn((
                        exact_game::Transform::at(i as f32, 0., 0.),
                        exact_game::Mesh::asset("crate.model"),
                    ));
                }
                w.spawn_named("walker", exact_game::Transform::default());
            }
            fn tick(w: &mut exact_game::World, _: &exact_game::Input, _: &()) {
                w.require_mut::<exact_game::Transform>("walker").position.z += 1.;
            }
        }
        let steps = || POSE_STEPS.with(|n| n.get());
        let mut sim = exact_game::Sim::<Grove>::new(()).unwrap();
        let mut feed = crate::Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let tick = 1000. / 60.;
        for _ in 0..2 {
            sim.run(tick);
            feed.feed(sim.world(), &mut renderer).unwrap();
        }
        // The walker shares no page with any instance: a tick steps none.
        let before = steps();
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_eq!(steps(), before);
        let poses = |r: &crate::Renderer| r.models.poses[exact_game::PAGE + 7];
        let start = poses(&renderer)[1];
        let moved = sim
            .world()
            .query::<&exact_game::Transform>()
            .iter()
            .nth(exact_game::PAGE + 7)
            .unwrap()
            .0;
        sim.world_mut()
            .get_mut::<exact_game::Transform>(moved)
            .unwrap()
            .position
            .y = 5.;
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        // Only the written page steps, and the moved instance interpolates.
        assert_eq!(steps() - before, exact_game::PAGE);
        assert_eq!(poses(&renderer)[0], start);
        assert_eq!(poses(&renderer)[1].position.y, 5.);
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        // The next tick collapses the moved history without revisiting the page.
        assert_eq!(steps() - before, exact_game::PAGE + 1);
        assert_eq!(poses(&renderer)[0], poses(&renderer)[1]);
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_eq!(steps() - before, exact_game::PAGE + 1);
    }

    #[test]
    fn static_parented_instances_cost_no_pose_steps_and_follow_a_moved_parent() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let model: Model =
            exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
                .unwrap();
        renderer.prepare_model("crate.model", &model).unwrap();
        struct Orchard;
        impl exact_game::Game for Orchard {
            type Args = ();
            const ID: &'static str = "static-parented-poses";
            fn setup(w: &mut exact_game::World, _: &()) {
                for i in 0..2 * exact_game::PAGE {
                    let plant = w.spawn((
                        exact_game::Transform::at(i as f32, 0., 0.),
                        exact_game::Mesh::asset("crate.model"),
                    ));
                    w.spawn((
                        exact_game::Transform::at(0., 1., 0.),
                        exact_game::Mesh::asset("crate.model"),
                        exact_game::Parent(plant),
                    ));
                }
                w.spawn_named("walker", exact_game::Transform::default());
            }
            fn tick(w: &mut exact_game::World, _: &exact_game::Input, _: &()) {
                w.require_mut::<exact_game::Transform>("walker").position.z += 1.;
            }
        }
        let steps = || POSE_STEPS.with(|n| n.get());
        let mut sim = exact_game::Sim::<Orchard>::new(()).unwrap();
        let mut feed = crate::Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let tick = 1000. / 60.;
        for _ in 0..2 {
            sim.run(tick);
            feed.feed(sim.world(), &mut renderer).unwrap();
        }
        let before = steps();
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_eq!(steps(), before, "a static hierarchy steps nothing");
        // Move a plant in the second block: its fruit (a parented instance) follows.
        let plant = sim
            .world()
            .query::<&exact_game::Transform>()
            .without::<exact_game::Parent>()
            .iter()
            .nth(exact_game::PAGE + 3)
            .unwrap()
            .0;
        let fruit = sim
            .world()
            .query::<&exact_game::Parent>()
            .iter()
            .find(|(_, p)| p.0 == plant)
            .unwrap()
            .0;
        let fruit_at = renderer
            .models
            .pose_history
            .iter()
            .position(|h| h.entity.index() == fruit.index())
            .unwrap();
        sim.world_mut()
            .get_mut::<exact_game::Transform>(plant)
            .unwrap()
            .position
            .y = 5.;
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        let pair = renderer.models.poses[fruit_at];
        assert_eq!((pair[0].position.y, pair[1].position.y), (1., 6.));
        assert!(
            steps() - before <= 2 * exact_game::PAGE,
            "{}",
            steps() - before
        );
    }

    #[test]
    fn direct_pose_shrink_and_regrow_restores_retained_history() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer
            .prepare_model("moving.model", &crate::test_model::skinned_model())
            .unwrap();
        struct Moving;
        impl exact_game::Game for Moving {
            type Args = ();
            const ID: &'static str = "direct-pose-resize";
            fn setup(w: &mut exact_game::World, _: &()) {
                w.spawn_named(
                    "hero",
                    (
                        exact_game::Transform::default(),
                        exact_game::Mesh::asset("moving.model"),
                    ),
                );
            }
            fn tick(w: &mut exact_game::World, _: &exact_game::Input, _: &()) {
                w.get_mut::<exact_game::Transform>("hero")
                    .unwrap()
                    .position
                    .x += 1.;
            }
        }
        let mut sim = exact_game::Sim::<Moving>::new(()).unwrap();
        sim.world_mut().propagate();
        let mut feed = crate::Feed::default();
        feed.feed(sim.world_mut(), &mut renderer).unwrap();
        sim.run(1000. / 60.);
        feed.feed(sim.world_mut(), &mut renderer).unwrap();
        let hero = sim.world().resolve("hero").unwrap();
        let records = renderer.models.records.clone();
        let expected = renderer.models.poses.clone();
        assert_ne!(expected[0][0], expected[0][1]);

        renderer.set_draw_instances(&[]).unwrap();
        assert!(renderer.models.poses.is_empty());
        renderer.set_draw_instances(&records).unwrap();
        assert_eq!(
            renderer.models.poses,
            vec![[exact_game::Transform::default(); 2]]
        );
        renderer.model_poses(sim.world(), &[hero], false, Moved::All);
        assert_eq!(renderer.models.poses, expected);
    }

    #[test]
    fn missing_global_and_recycled_generation_get_fresh_active_poses() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer
            .prepare_model("generation.model", &crate::test_model::skinned_model())
            .unwrap();
        let mut world = exact_game::World::new(60, 0);
        let parent = world.spawn(exact_game::Transform::default());
        let mut local = exact_game::Transform::default();
        local.position.x = 7.;
        let missing = world.spawn_named(
            "model",
            (
                local,
                exact_game::Mesh::asset("generation.model"),
                exact_game::Parent(parent),
            ),
        );
        let node = renderer.models.loaded["generation.model"].nodes[0];
        let mut record = DrawInstance {
            data: 0,
            transform: missing.index(),
            geometry: node.0,
            material: node.1,
            local: node.2,
            skin: None,
            tint: [1.; 4],
            glow: [0.; 3],
        };
        renderer
            .set_draw_instances(std::slice::from_ref(&record))
            .unwrap();
        renderer.model_poses(&world, &[missing], false, Moved::All);
        assert_eq!(
            renderer.models.poses[0],
            [exact_game::Transform::default(); 2]
        );

        world.propagate();
        world.load(&world.save()).unwrap();
        let missing = world.resolve("model").unwrap();
        assert!(!world.is_fresh(missing));
        let propagated = crate::world::scene::pose(&world, missing).unwrap();
        renderer.model_poses(&world, &[missing], false, Moved::All);
        assert_eq!(renderer.models.poses[0], [propagated; 2]);

        assert!(world.despawn(missing));
        let mut replacement_pose = exact_game::Transform::default();
        replacement_pose.position.x = 19.;
        let replacement = world.spawn_named(
            "model",
            (
                replacement_pose,
                exact_game::Mesh::asset("generation.model"),
            ),
        );
        assert_eq!(replacement.index(), missing.index());
        assert_ne!(replacement.generation(), missing.generation());
        world.load(&world.save()).unwrap();
        let replacement = world.resolve("model").unwrap();
        assert!(!world.is_fresh(replacement));
        record.transform = replacement.index();
        renderer
            .set_draw_instances(std::slice::from_ref(&record))
            .unwrap();
        assert_eq!(renderer.models.poses[0], [propagated; 2]);
        renderer.model_poses(&world, &[replacement], false, Moved::All);
        assert_eq!(renderer.models.poses[0], [replacement_pose; 2]);
    }

    #[test]
    fn pending_names_count_retired_bytes_and_compaction_keeps_hero_handles() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut r = crate::renderer::RendererWithAssets::<true>::new(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let mut model: Model =
            exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
                .unwrap();
        model.materials[0].alpha_mode = AlphaMode::Blend;
        r.prepare_model("hero.model", &model).unwrap();
        let handles = r.models.loaded["hero.model"].nodes.clone();
        r.models.loaded.get_mut("hero.model").unwrap().active = false;
        let work = r.residency_work();
        r.prepare_model("hero.model", &model).unwrap();
        assert!(
            r.models.loaded["hero.model"].active,
            "equal digest reactivates resident"
        );
        assert_eq!(r.models.loaded["hero.model"].nodes, handles);
        assert_eq!(r.residency_work().since(work).mesh_uploads, 0);
        assert_eq!(r.residency_work().since(work).texture_uploads, 0);
        struct Moving;
        impl exact_game::Game for Moving {
            type Args = ();
            const ID: &'static str = "retirement-motion";
            fn setup(w: &mut exact_game::World, _: &()) {
                w.spawn_named(
                    "hero",
                    (
                        exact_game::Transform::default(),
                        exact_game::Mesh::asset("hero.model"),
                    ),
                );
            }
            fn tick(w: &mut exact_game::World, _: &exact_game::Input, _: &()) {
                w.get_mut::<exact_game::Transform>("hero")
                    .unwrap()
                    .position
                    .x += 1.;
            }
        }
        let mut sim = exact_game::Sim::<Moving>::new(()).unwrap();
        let w = sim.world_mut();
        w.propagate();
        let mut feed = crate::Feed::default();
        feed.feed(w, &mut r).unwrap();
        sim.run(1000. / 60.);
        let w = sim.world_mut();
        feed.feed(w, &mut r).unwrap();
        let history = r.models.poses.clone();
        assert_ne!(history[0][0], history[0][1]);
        let hero = r.models.loaded["hero.model"].nodes.clone();
        let bind = r.models.materials[hero[0].1 .0].bind.clone();
        let mut live = std::collections::BTreeSet::from(["hero.model".to_owned()]);
        let texture = TextureData {
            width: 1024,
            height: 1024,
            mips: (0..11)
                .map(|level| vec![255; (1024usize >> level).pow(2) * 4])
                .collect(),
            ..Default::default()
        };
        for i in 0..17 {
            let name = format!("cosmetic-{i}.model");
            let tex = format!("cosmetic-{i}.tex");
            r.prepare_model(&name, &model).unwrap();
            r.add_texture(&tex, &texture).unwrap();
            assert!(
                r.models.textures[&tex].sprite_bind.is_none(),
                "model textures do not become sprite consumers"
            );
            r.models.loaded.get_mut(&name).unwrap().active = false;
            r.retire_texture(&tex);
            live.insert(name);
            live.insert(tex); // immediately re-requested, still Pending
        }
        assert!(r.retired_bytes(&live) > crate::renderer::RETIRED_BUDGET);
        let before = r.residency_work();
        r.compact_assets(&live, &Default::default());
        assert!(r.retired_bytes(&live) <= crate::renderer::RETIRED_BUDGET);
        assert_eq!(r.models.loaded["hero.model"].nodes, hero);
        assert_eq!(r.models.materials[hero[0].1 .0].bind, bind);
        assert_eq!(
            r.residency_work().since(before).json(),
            crate::world::assets::Work::default().json()
        );
        assert_eq!(r.models.loaded.len(), 1);
        feed.feed(w, &mut r).unwrap();
        assert_eq!(
            r.models.poses, history,
            "compaction preserves the moving hero history"
        );
    }
}

/// Model-capable presentation executor. Primitive modules never instantiate it.
#[derive(Default)]
pub struct ModelExecutor<P: crate::Executor = ()> {
    inner: P,
    definitions: Option<exact_game::animation::Definitions>,
}
impl<P: crate::Executor> crate::Executor for ModelExecutor<P> {
    fn wants_audio(&self) -> bool {
        self.inner.wants_audio()
    }
    fn clock(&mut self, seekable: bool) {
        self.inner.clock(seekable);
    }
    fn suspend(&mut self, suspended: bool) {
        self.inner.suspend(suspended);
    }
    fn sync(&mut self, world: &exact_game::World, generation: u64, playing: bool, seekable: bool) {
        self.inner.sync(world, generation, playing, seekable);
    }
    fn before_restore(&mut self, world: &exact_game::World, mode: exact_gpu::Restore) {
        self.definitions = (mode == exact_gpu::Restore::Carry)
            .then(|| exact_game::animation::Definitions::capture(world));
        self.inner.before_restore(world, mode);
    }
    fn after_restore(&mut self, world: &mut exact_game::World, mode: exact_gpu::Restore) {
        if let Some(definitions) = self.definitions.take() {
            definitions.apply(world);
        }
        self.inner.after_restore(world, mode);
    }
    fn inspect(
        world: &exact_game::World,
        entity: exact_game::Entity,
        pose: bool,
    ) -> Result<String, String> {
        exact_game::animation::inspect(world, entity, pose)
    }
    fn unlock(&mut self) {
        self.inner.unlock();
    }
}
