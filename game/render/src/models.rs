//! Baked-model instance and material buffers. Primitive draws never bind these.
use crate::{
    buffers::{bytes, Buffer},
    DrawInstance, MaterialId, MeshId, RenderError, Vertex,
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
pub(crate) type ModelNode = (MeshId, MaterialId, Mat4, Option<u32>);
pub(crate) struct Uploaded {
    pub nodes: Vec<ModelNode>,
    pub(crate) digest: u64,
    pub active: bool,
    pub meshes: Vec<MeshId>,
    pub materials: Vec<MaterialId>,
    pub skins: Vec<u32>,
}
pub(crate) struct Texture {
    bytes: u64,
    pub(crate) active: bool,
    pub digest: u64,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: [u32; 2],
    pub(crate) sprite_bind: Option<wgpu::BindGroup>,
}
struct PoseHistory {
    entity: exact_game::Entity,
    saved: Option<(u64, u64, [exact_game::Transform; 2])>,
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
    pub no_shadow: Option<wgpu::BindGroup>,
    pub transparent: Vec<(usize, u32, f32)>,
    pub poses: Vec<[exact_game::Transform; 2]>,
    pub pose_indices: Vec<usize>,
    pose_history: Vec<PoseHistory>,
    bind_buffers: Option<(wgpu::BindGroupLayout, [wgpu::Buffer; 3])>,
    words: Vec<u32>,
    normals: Vec<([u32; 16], [u32; 16])>,
}
impl Models {
    pub fn custom_data(&mut self, queue: &wgpu::Queue, data: impl Fn(u32) -> u32) {
        let mut changed = false;
        for (i, record) in self.records.iter_mut().enumerate() {
            let value = data(record.transform);
            if record.data != value {
                record.data = value;
                self.words[i * 36 + 2] = value;
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
    fn reconcile_pose_history(&mut self, entities: &[exact_game::Entity]) {
        if self.pose_history.len() == entities.len()
            && self
                .pose_history
                .iter()
                .zip(entities)
                .all(|(history, entity)| history.entity == *entity)
        {
            return;
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
            let saved = if old.peek().is_some_and(|history| history.entity == entity) {
                old.next().unwrap().saved
            } else {
                if old
                    .peek()
                    .is_some_and(|history| history.entity.index() == entity.index())
                {
                    old.next();
                }
                None
            };
            merged.push(PoseHistory { entity, saved });
        }
        self.pose_history = merged;
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
        self.no_shadow = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game no shadows"),
            layout: &family.empty,
            entries: &[],
        }));
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
        for ((record, cached), &palette) in
            records.iter().zip(&mut self.normals).zip(&skinning.offsets)
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
    /// Upload model geometry and material textures once; return stable renderer handles.
    pub fn add_model(
        &mut self,
        model: &Model,
    ) -> Result<(Vec<MeshId>, Vec<MaterialId>), RenderError> {
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
        let meshes = model
            .meshes
            .iter()
            .map(|mesh| {
                let vertices: Vec<_> = mesh
                    .positions
                    .chunks_exact(3)
                    .enumerate()
                    .map(|(i, p)| Vertex {
                        position: [p[0], p[1], p[2]],
                        normal: mesh.normals[i * 3..i * 3 + 3].try_into().unwrap(),
                        uv: mesh.uvs[i * 2..i * 2 + 2].try_into().unwrap(),
                        color: if mesh.colors.is_empty() {
                            [1.; 4]
                        } else {
                            mesh.colors[i * 4..i * 4 + 4].try_into().unwrap()
                        },
                    })
                    .collect();
                let id = self.add_mesh(&vertices, &mesh.indices);

                if !mesh.joints.is_empty() {
                    let start = self.meshes[id.0].base_vertex as u64 * 32;
                    let mut words = Vec::with_capacity(mesh.joints.len() * 2);
                    for (j, w) in mesh
                        .joints
                        .chunks_exact(4)
                        .zip(mesh.weights.chunks_exact(4))
                    {
                        words.extend(j.iter().map(|j| u32::from(*j)));
                        words.extend(w.iter().map(|w| w.to_bits()));
                    }
                    let weights = &mut self.models.skinning.as_mut().unwrap().weights;
                    self.models.reallocations += u64::from(weights.grow(
                        &self.device,
                        &self.queue,
                        start + (words.len() * 4) as u64,
                    ));
                    weights.write(&self.queue, start, bytes(&words));
                }
                id
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
            return Ok(());
        }
        model.validate().map_err(RenderError::scene)?;
        let replaced = self.models.loaded.remove(name).is_some();
        if replaced {
            self.reclaim_orphan_slots();
        }
        let (meshes, materials) = self.add_model(model)?;
        for id in &meshes {
            self.meshes[id.0].asset = true;
        }
        let skins = self
            .models
            .skinning
            .as_mut()
            .unwrap()
            .add(&self.device, &self.queue, model);
        if replaced {
            self.models.skinning.as_mut().unwrap().mark_fresh(&skins);
        }
        let nodes = model
            .nodes
            .iter()
            .zip(model.offsets().map_err(RenderError::scene)?)
            .filter_map(|(n, local)| {
                n.mesh.map(|m| {
                    (
                        meshes[m as usize],
                        materials[model.meshes[m as usize].material as usize],
                        if n.skin.is_some() {
                            Mat4::IDENTITY
                        } else {
                            local
                        },
                        n.skin.map(|s| skins[s as usize]),
                    )
                })
            })
            .collect();
        self.models.loaded.insert(
            name.into(),
            Uploaded {
                nodes,
                digest,
                active: true,
                meshes,
                materials,
                skins,
            },
        );
        self.reclaim_orphan_slots();
        self.models.revision += 1;
        Ok(())
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
                    .flat_map(|(_, m)| m.nodes.iter().filter(|n| n.3.is_some()).map(|n| n.0 .0))
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
            upload_texture(&self.device, &self.queue, data, &mut self.models.samplers);
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
        self.models.set(
            &self.device,
            &self.queue,
            &family.instance,
            &self.uniform,
            records,
        )
    }
    /// Feed poses for transparency and winding on completed ticks. O(model instances).
    pub(crate) fn model_poses(
        &mut self,
        world: &exact_game::World,
        entities: &[exact_game::Entity],
        initial: bool,
    ) {
        if let Some(skinning) = &mut self.models.skinning {
            skinning.feed(&self.queue, world, entities, initial);
        }
        self.models.reconcile_pose_history(entities);
        let loaded = &self.models.loaded;
        for (active, history) in self
            .models
            .poses
            .iter_mut()
            .zip(&mut self.models.pose_history)
        {
            let entity = history.entity;
            let digest = world
                .get::<exact_game::Mesh>(entity)
                .and_then(|m| match &*m {
                    exact_game::Mesh::Asset(name) => loaded.get(name).map(|m| m.digest),
                    _ => None,
                })
                .unwrap_or(0);
            if let Some(pose) = crate::world::scene::pose(world, entity) {
                let saved = history
                    .saved
                    .get_or_insert((digest, world.tick(), [pose; 2]));
                if initial || saved.0 != digest || crate::world::scene::snap(world, entity, false) {
                    saved.2 = [pose; 2];
                } else {
                    if saved.1 != world.tick() {
                        saved.2[0] = saved.2[1];
                    }
                    saved.2[1] = pose;
                }
                saved.0 = digest;
                saved.1 = world.tick();
                *active = saved.2;
            }
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
fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &TextureData,
    samplers: &mut BTreeMap<([Wrap; 2], [Filter; 3]), wgpu::Sampler>,
) -> Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game baked texture"),
        size: wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: data.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if data.srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    for (level, mip) in data.mips.iter().enumerate() {
        let (w, h) = ((data.width >> level).max(1), (data.height >> level).max(1));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    let sampler = samplers.entry((data.wrap, data.filter)).or_insert_with(|| {
        let wrap = |w| match w {
            Wrap::Repeat => wgpu::AddressMode::Repeat,
            Wrap::Clamp => wgpu::AddressMode::ClampToEdge,
            Wrap::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        let filters = data.filter.map(|f| {
            if f == Filter::Nearest {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            }
        });
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game shared sampler"),
            address_mode_u: wrap(data.wrap[0]),
            address_mode_v: wrap(data.wrap[1]),
            mag_filter: filters[0],
            min_filter: filters[1],
            mipmap_filter: if data.filter[2] == Filter::Nearest {
                wgpu::MipmapFilterMode::Nearest
            } else {
                wgpu::MipmapFilterMode::Linear
            },
            anisotropy_clamp: if data.filter.iter().all(|f| *f == Filter::Linear) {
                4
            } else {
                1
            },
            ..Default::default()
        })
    });
    Texture {
        active: true,
        size: [data.width, data.height],
        bytes: (0..texture.mip_level_count())
            .map(|level| {
                u64::from((texture.width() >> level).max(1))
                    * u64::from((texture.height() >> level).max(1))
                    * 4
            })
            .sum(),
        digest: 0,
        view: texture.create_view(&Default::default()),
        sampler: sampler.clone(),
        sprite_bind: None,
    }
}

#[cfg(test)]
mod arrival_tests {
    use super::*;
    #[test]
    fn content_digest_reuses_equal_bytes_and_replaces_changed_names() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let mut model = Model {
            meshes: vec![exact_game::asset::MeshData {
                positions: vec![0.; 9],
                normals: vec![0.; 9],
                uvs: vec![0.; 6],
                indices: vec![0, 1, 2],
                ..Default::default()
            }],
            nodes: vec![exact_game::asset::Node {
                mesh: Some(0),
                ..Default::default()
            }],
            materials: vec![MaterialData::default()],
            ..Default::default()
        };
        renderer.prepare_model("resident.model", &model).unwrap();
        let before = renderer.residency_work();
        let old_mesh = renderer.models.loaded["resident.model"].nodes[0].0;
        renderer
            .prepare_model("resident.model", &model.clone())
            .unwrap();
        assert_eq!(before.json(), renderer.residency_work().json());
        model.meshes[0].positions[0] = 0.5;
        renderer.prepare_model("resident.model", &model).unwrap();
        assert_eq!(
            old_mesh, renderer.models.loaded["resident.model"].nodes[0].0,
            "replacement reuses the retired slot after accepting its digest"
        );
        let delta = renderer.residency_work().since(before);
        assert_eq!(delta.mesh_uploads, 1);
        assert_eq!(delta.pipeline_creations, 0);
        assert_eq!(delta.texture_uploads, 0);
        let mut texture = TextureData {
            width: 1,
            height: 1,
            mips: vec![vec![255; 4]],
            ..Default::default()
        };
        renderer.add_texture("resident.tex", &texture).unwrap();
        let before = renderer.residency_work();
        renderer
            .add_texture("resident.tex", &texture.clone())
            .unwrap();
        assert_eq!(before.json(), renderer.residency_work().json());
        texture.mips[0][0] = 0;
        renderer.add_texture("resident.tex", &texture).unwrap();
        assert_eq!(renderer.residency_work().since(before).texture_uploads, 1);
        texture.srgb = !texture.srgb;
        renderer.add_texture("resident.tex", &texture).unwrap();
        texture.wrap[0] = Wrap::Clamp;
        renderer.add_texture("resident.tex", &texture).unwrap();
        assert_eq!(renderer.residency_work().since(before).texture_uploads, 3);
    }
    #[test]
    fn normal_cache_and_rebatch_scratch_follow_the_live_records() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer
            .prepare_model("empty.model", &Model::default())
            .unwrap();
        let layout = &renderer.pipelines.models.as_ref().unwrap().instance;
        for x in 0..8 {
            let records = [DrawInstance {
                data: 0,
                transform: 0,
                geometry: MeshId(0),
                material: MaterialId(0),
                local: Mat4::from_translation(glam::Vec3::new(x as f32, 0., 0.)),
                skin: None,
            }];
            renderer
                .models
                .set(&gpu.device, &gpu.queue, layout, &renderer.uniform, &records)
                .unwrap();
            assert_eq!(renderer.models.normals.len(), 1);
            let ptr = renderer.models.words.as_ptr();
            renderer
                .models
                .set(&gpu.device, &gpu.queue, layout, &renderer.uniform, &records)
                .unwrap();
            assert_eq!(ptr, renderer.models.words.as_ptr());
        }
    }
    #[test]
    fn material_waits_for_all_textures_before_rebinding() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let model = Model {
            meshes: vec![exact_game::asset::MeshData {
                positions: vec![0.; 9],
                normals: vec![0.; 9],
                uvs: vec![0.; 6],
                indices: vec![0, 1, 2],
                ..Default::default()
            }],
            nodes: vec![exact_game::asset::Node {
                mesh: Some(0),
                ..Default::default()
            }],
            materials: vec![MaterialData {
                base_color_texture: Some(0),
                normal_texture: Some(1),
                ..Default::default()
            }],
            textures: vec!["color.tex".into(), "normal.tex".into()],
            ..Default::default()
        };
        renderer.prepare_model("two.model", &model).unwrap();
        let initial = renderer.models.materials[0].bind.clone();
        let texture = TextureData {
            width: 1,
            height: 1,
            mips: vec![vec![255; 4]],
            ..Default::default()
        };
        renderer.add_texture("color.tex", &texture).unwrap();
        assert_eq!(renderer.models.materials[0].bind, initial);
        renderer.add_texture("normal.tex", &texture).unwrap();
        let final_bind = renderer.models.materials[0].bind.clone();
        assert_ne!(final_bind, initial);
        renderer.add_texture("normal.tex", &texture).unwrap();
        assert_eq!(renderer.models.materials[0].bind, final_bind);
    }
}

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
        renderer.model_poses(sim.world(), &[hero], false);
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
        };
        renderer
            .set_draw_instances(std::slice::from_ref(&record))
            .unwrap();
        renderer.model_poses(&world, &[missing], false);
        assert_eq!(
            renderer.models.poses[0],
            [exact_game::Transform::default(); 2]
        );

        world.propagate();
        world.load(&world.save()).unwrap();
        let missing = world.resolve("model").unwrap();
        assert!(!world.is_fresh(missing));
        let propagated = crate::world::scene::pose(&world, missing).unwrap();
        renderer.model_poses(&world, &[missing], false);
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
        renderer.model_poses(&world, &[replacement], false);
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
pub struct ModelPresentation<P: crate::Presentation = ()> {
    inner: P,
    definitions: Option<exact_game::animation::Definitions>,
}
impl<P: crate::Presentation> crate::Presentation for ModelPresentation<P> {
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
