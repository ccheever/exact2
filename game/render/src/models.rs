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
    pub bind: wgpu::BindGroup,
    pub alpha: AlphaMode,
    pub double_sided: bool,
    data: MaterialData,
    names: [Option<String>; 5],
}
pub(crate) type ModelNode = (MeshId, MaterialId, Mat4, Option<u32>);
pub(crate) struct Uploaded {
    pub nodes: Vec<ModelNode>,
    digest: u64,
    pub active: bool,
    pub bytes: u64,
}
pub(crate) struct Texture {
    bytes: u64,
    pub digest: u64,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: [u32; 2],
}
#[derive(Default)]
pub(crate) struct Models {
    pub loaded: BTreeMap<String, Uploaded>,
    prior_work: crate::world::assets::Work,
    pub revision: u64,
    textures: BTreeMap<String, Texture>,
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
    words: Vec<u32>,
    normals: Vec<([u32; 16], [u32; 16])>,
}
impl Models {
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
                record.geometry.0 as u32,
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
        self.bind = Some(instance_bind(device, layout, instances, skinning));
        instances.write(queue, 0, bytes(words));
        self.records.clear();
        self.records.extend_from_slice(records);
        self.poses
            .resize(records.len(), [exact_game::Transform::default(); 2]);
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
            materials.push(MaterialId(self.models.materials.len()));
            let names = texture_names(material, &model.textures);
            self.models.materials.push(material_bind(
                &self.device,
                &self.queue,
                &self.pipelines.models.as_ref().unwrap().material,
                material,
                &names,
                &self.models.textures,
            ));
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
        if self
            .models
            .loaded
            .get(name)
            .is_some_and(|m| m.digest == digest)
        {
            return Ok(());
        }
        let (meshes, materials) = self.add_model(model)?;
        let skins = self
            .models
            .skinning
            .as_mut()
            .unwrap()
            .add(&self.device, &self.queue, model);
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
                // Conservative arena charge: capacity growth plus per-entry metadata.
                bytes: (exact_game::bin::to_vec(model).len() as u64 * 4)
                    // Each skin expands the full node hierarchy and a retained rest pose.
                    + model.nodes.len() as u64 * model.skins.len() as u64 * 96
                    + (model.meshes.len() + model.materials.len() + model.skins.len() + 1) as u64
                        * 1024,
            },
        );
        self.models.revision += 1;
        Ok(())
    }
    pub(crate) fn model_changed(&self, name: &str, digest: u64) -> bool {
        self.models
            .loaded
            .get(name)
            .is_some_and(|m| m.digest != digest)
    }
    pub(crate) fn retired_bytes(&self, live: &std::collections::BTreeSet<String>) -> u64 {
        self.models
            .loaded
            .iter()
            .filter(|(n, _)| !live.contains(*n))
            .map(|(_, m)| m.bytes)
            .sum::<u64>()
            + self
                .models
                .textures
                .iter()
                .filter(|(n, _)| !n.starts_with('\0') && !live.contains(*n))
                .map(|(_, t)| t.bytes)
                .sum::<u64>()
    }
    /// Rebuild arenas from the live CPU models after this call. Texture views survive,
    /// but geometry, materials and skin templates (including replacements) are reclaimed.
    pub(crate) fn compact_assets(
        &mut self,
        format: wgpu::TextureFormat,
        live: &std::collections::BTreeSet<String>,
    ) {
        let before = self.residency_work();
        let mut fresh = Self::new(&self.device, &self.queue, format);
        fresh.models.textures = std::mem::take(&mut self.models.textures);
        fresh
            .models
            .textures
            .retain(|n, _| n.starts_with('\0') || live.contains(n));
        if ASSETS {
            for (name, texture) in &fresh.models.textures {
                if !name.starts_with('\0') && self.quads.has_texture(name) {
                    fresh.quads.texture(&fresh.device, name, texture);
                }
            }
        }
        fresh.models.samplers = std::mem::take(&mut self.models.samplers);
        fresh.models.prior_work = before;
        fresh.models.revision = self.models.revision + 1;
        *self = fresh;
    }
    /// Reuse a name only when its content digest matches. CPU mips may be dropped.
    pub fn add_texture(&mut self, name: &str, data: &TextureData) -> Result<(), RenderError> {
        let digest = exact_game::hash::of(data);
        if let Some(texture) = self
            .models
            .textures
            .get(name)
            .filter(|t| t.digest == digest)
        {
            if ASSETS {
                self.quads.texture(&self.device, name, texture);
            }
            return Ok(());
        }
        data.validate().map_err(RenderError::scene)?;
        let mut texture =
            upload_texture(&self.device, &self.queue, data, &mut self.models.samplers);
        texture.digest = digest;
        if ASSETS {
            self.quads.texture(&self.device, name, &texture);
        }
        self.models.textures.insert(name.into(), texture);
        self.models.uploads += 1;
        for material in &mut self.models.materials {
            if material.names.iter().flatten().any(|n| n == name)
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
            mesh_uploads: self.meshes.len() as u64,
            pipeline_creations: 11 + pipelines as u64 + skin.map_or(0, |s| s.pipeline_creations),
            buffer_reallocations: self.models.reallocations + skin.map_or(0, |s| s.reallocations),
        }
        .plus(self.models.prior_work)
    }
    /// Replace additional draw records. Primitive batches retain their compact identity
    /// record: slot = transform = material, geometry in the batch, local = identity.
    pub fn set_draw_instances(&mut self, records: &[DrawInstance]) -> Result<(), RenderError> {
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
    /// Feed transparent poses on completed ticks. O(model instances), never primitive entities.
    pub(crate) fn model_poses(
        &mut self,
        world: &exact_game::World,
        entities: &[exact_game::Entity],
        initial: bool,
    ) {
        if let Some(skinning) = &mut self.models.skinning {
            skinning.feed(&self.queue, world, entities, initial);
        }
        for ((record, history), &entity) in self
            .models
            .records
            .iter()
            .zip(&mut self.models.poses)
            .zip(entities)
        {
            if self.models.materials[record.material.0].alpha != AlphaMode::Blend {
                continue;
            }
            if let Some(pose) = crate::world::scene::pose(world, entity) {
                history[0] = if initial || crate::world::scene::snap(world, entity, false) {
                    pose
                } else {
                    history[1]
                };
                history[1] = pose;
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
        bind: device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game material textures"),
            layout,
            entries: &entries,
        }),
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
        size: [data.width, data.height],
        bytes: data
            .mips
            .iter()
            .map(|m| m.len() as u64)
            .sum::<u64>()
            .next_power_of_two(),
        digest: 0,
        view: texture.create_view(&Default::default()),
        sampler: sampler.clone(),
    }
}

#[cfg(test)]
mod arrival_tests {
    use super::*;
    #[test]
    fn content_digest_reuses_equal_bytes_and_replaces_changed_names() {
        let Ok(gpu) = exact_gpu::fixture::device() else {
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
        assert_ne!(
            old_mesh,
            renderer.models.loaded["resident.model"].nodes[0].0
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
        let Ok(gpu) = exact_gpu::fixture::device() else {
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
        let Ok(gpu) = exact_gpu::fixture::device() else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let model = Model {
            meshes: vec![exact_game::asset::MeshData {
                positions: vec![0.; 9],
                normals: vec![0.; 9],
                uvs: vec![0.; 6],
                tangents: vec![0.; 12],
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
