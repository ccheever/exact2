//! Baked-model instance and material buffers. Primitive draws never bind these.
use crate::{
    buffers::{bytes, Buffer},
    DrawInstance, MaterialId, MeshId, RenderError, Vertex,
};
use exact_game::asset::{AlphaMode, MaterialData, Model, TextureData, Wrap};
use exact_gpu::wgpu;
use glam::Mat4;

pub(crate) struct Material {
    pub bind: wgpu::BindGroup,
    pub alpha: AlphaMode,
    pub double_sided: bool,
}
pub(crate) struct Models {
    pub records: Vec<DrawInstance>,
    pub materials: Vec<Material>,
    pub instances: Buffer,
    pub bind: wgpu::BindGroup,
    pub no_shadow: wgpu::BindGroup,
    pub transparent: Vec<(usize, u32, f32)>,
    pub poses: Vec<[Mat4; 2]>,
}
impl Models {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        empty: &wgpu::BindGroupLayout,
    ) -> Self {
        let instances = Buffer::new(
            device,
            128,
            wgpu::BufferUsages::STORAGE,
            "game model instances",
        );
        let bind = instance_bind(device, layout, &instances);
        let no_shadow = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game no shadows"),
            layout: empty,
            entries: &[],
        });
        Self {
            no_shadow,
            records: Vec::new(),
            materials: Vec::new(),
            instances,
            bind,
            transparent: Vec::new(),
            poses: Vec::new(),
        }
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
        if self.instances.grow(device, queue, (words.len() * 4) as u64) {
            self.bind = instance_bind(device, layout, &self.instances);
        }
        self.instances.write(queue, 0, bytes(&words));
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
                self.add_mesh(&vertices, &mesh.indices)
            })
            .collect();
        let mut materials = Vec::new();
        for material in &model.materials {
            materials.push(MaterialId(self.models.materials.len()));
            self.models.materials.push(material_bind(
                &self.device,
                &self.queue,
                &self.pipelines.model_material_layout,
                material,
                &model.textures,
            ));
        }
        Ok((meshes, materials))
    }
    /// Replace additional draw records. Primitive batches retain their compact identity
    /// record: slot = transform = material, geometry in the batch, local = identity.
    pub fn set_draw_instances(&mut self, records: &[DrawInstance]) -> Result<(), RenderError> {
        self.models.set(
            &self.device,
            &self.queue,
            &self.pipelines.model_instance_layout,
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
    textures: &[TextureData],
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
    let indices = [
        m.base_color_texture,
        m.normal_texture,
        m.metallic_roughness_texture,
        m.emissive_texture,
        m.occlusion_texture,
    ];
    let defaults = [
        [255, 255, 255, 255],
        [128, 128, 255, 255],
        [255, 255, 255, 255],
        [255, 255, 255, 255],
        [255, 255, 255, 255],
    ];
    let mut views = Vec::new();
    let mut samplers = Vec::new();
    for (i, index) in indices.into_iter().enumerate() {
        let default = TextureData {
            width: 1,
            height: 1,
            mips: vec![defaults[i].to_vec()],
            srgb: i == 0 || i == 3,
            ..Default::default()
        };
        let data = index.map_or(&default, |i| &textures[i as usize]);
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
        views.push(texture.create_view(&Default::default()));
        let wrap = |w| match w {
            Wrap::Repeat => wgpu::AddressMode::Repeat,
            Wrap::Clamp => wgpu::AddressMode::ClampToEdge,
            Wrap::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        samplers.push(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game material trilinear 4x"),
            address_mode_u: wrap(data.wrap[0]),
            address_mode_v: wrap(data.wrap[1]),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 4,
            ..Default::default()
        }));
    }
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: uniform.as_entire_binding(),
    }];
    for i in 0..5 {
        entries.push(wgpu::BindGroupEntry {
            binding: 1 + i as u32 * 2,
            resource: wgpu::BindingResource::TextureView(&views[i]),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2 + i as u32 * 2,
            resource: wgpu::BindingResource::Sampler(&samplers[i]),
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
    }
}
