//! Baked-model instance and material buffers. Primitive draws never bind these.
use crate::{
    buffers::{bytes, Buffer},
    DrawInstance, MaterialId, MeshId, RenderError, Vertex,
};
use exact_game::asset::{AlphaMode, Filter, MaterialData, Model, TextureData, Wrap};
use exact_gpu::wgpu;
use glam::Mat4;
use std::collections::BTreeMap;

pub(crate) struct Material {
    pub bind: wgpu::BindGroup,
    pub alpha: AlphaMode,
    pub double_sided: bool,
    data: MaterialData,
    names: [Option<String>; 5],
}
pub(crate) struct Uploaded {
    pub nodes: Vec<(MeshId, MaterialId, Mat4)>,
}
struct Texture {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}
#[derive(Default)]
pub(crate) struct Models {
    pub loaded: BTreeMap<String, Uploaded>,
    pub revision: u64,
    textures: BTreeMap<String, Texture>,
    samplers: BTreeMap<([Wrap; 2], [Filter; 3]), wgpu::Sampler>,
    pub uploads: u64,
    pub records: Vec<DrawInstance>,
    pub materials: Vec<Material>,
    pub instances: Option<Buffer>,
    pub bind: Option<wgpu::BindGroup>,
    pub no_shadow: Option<wgpu::BindGroup>,
    pub transparent: Vec<(usize, u32, f32)>,
    pub poses: Vec<[Mat4; 2]>,
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
        self.bind = Some(instance_bind(device, &family.instance, &instances));
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
        records: &[DrawInstance],
    ) -> Result<(), RenderError> {
        // Four u32 header words + affine matrix + inverse-transpose normal matrix.
        let mut words = Vec::with_capacity(records.len() * 36);
        for record in records {
            let normal = record.local.inverse().transpose();
            if !normal.is_finite() {
                return Err(RenderError::scene(
                    "model node has singular transform".into(),
                ));
            }
            words.extend([
                record.transform,
                record.material.0 as u32,
                record.geometry.0 as u32,
                0,
            ]);
            words.extend(record.local.to_cols_array().map(f32::to_bits));
            words.extend(normal.to_cols_array().map(f32::to_bits));
        }
        if words.len() as u64 * 4 > device.limits().max_storage_buffer_binding_size {
            return Err(RenderError::scene(
                "model instance buffer exceeds device limit".into(),
            ));
        }
        let instances = self.instances.as_mut().expect("prepared model instances");
        if instances.grow(device, queue, (words.len() * 4) as u64) {
            self.bind = Some(instance_bind(device, layout, instances));
        }
        instances.write(queue, 0, bytes(&words));
        self.records.clear();
        self.records.extend_from_slice(records);
        self.poses.resize(records.len(), [Mat4::IDENTITY; 2]);
        Ok(())
    }
}
fn instance_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("game model instances"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.raw.as_entire_binding(),
        }],
    })
}
impl crate::Renderer {
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
                        texcoord: mesh.uvs[i * 2..i * 2 + 2].try_into().unwrap(),
                    })
                    .collect();
                self.add_mesh(&vertices, &mesh.indices)
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
        if self.models.loaded.contains_key(name) {
            return Ok(());
        }
        let (meshes, materials) = self.add_model(model)?;
        let nodes = model
            .nodes
            .iter()
            .zip(model.offsets().map_err(RenderError::scene)?)
            .filter_map(|(n, local)| {
                n.mesh.map(|m| {
                    (
                        meshes[m as usize],
                        materials[model.meshes[m as usize].material as usize],
                        local,
                    )
                })
            })
            .collect();
        self.models.loaded.insert(name.into(), Uploaded { nodes });
        self.models.revision += 1;
        Ok(())
    }
    /// Upload once by immutable asset name. The caller drops the CPU mip payload.
    pub fn add_texture(&mut self, name: &str, data: &TextureData) -> Result<(), RenderError> {
        if self.models.textures.contains_key(name) {
            return Ok(());
        }
        data.validate().map_err(RenderError::scene)?;
        let texture = upload_texture(&self.device, &self.queue, data, &mut self.models.samplers);
        self.models.textures.insert(name.into(), texture);
        self.models.uploads += 1;
        for material in &mut self.models.materials {
            if material.names.iter().flatten().any(|n| n == name) {
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
    /// Replace additional draw records. Primitive batches retain their compact identity
    /// record: slot = transform = material, geometry in the batch, local = identity.
    pub fn set_draw_instances(&mut self, records: &[DrawInstance]) -> Result<(), RenderError> {
        let Some(family) = &self.pipelines.models else {
            assert!(records.is_empty(), "model instances need prepared assets");
            return Ok(());
        };
        self.models
            .set(&self.device, &self.queue, &family.instance, records)
    }
    /// Feed transparent poses on completed ticks. O(model instances), never primitive entities.
    pub(crate) fn model_poses(
        &mut self,
        world: &exact_game::World,
        entities: &[exact_game::Entity],
        initial: bool,
    ) {
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
            if let Some(pose) = world.global(entity) {
                let pose = Mat4::from(pose);
                history[0] = if initial || world.fresh().contains(&entity) {
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
        view: texture.create_view(&Default::default()),
        sampler: sampler.clone(),
    }
}
