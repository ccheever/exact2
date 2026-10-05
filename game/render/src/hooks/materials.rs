//! Custom shading over renderer-owned model geometry and instance records.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d4-custom-materials-inside-the-engines-batches-phase-2
use crate::{MaterialId, Vertex};
use exact_gpu::wgpu;

/// Reflected engine frame, entity interpolation, unskinned model instance access and
/// the shaded material. Append game entry points; group 2 belongs to the game.
/// `instance_transform(position, normal, uv, i, color)` places a vertex as the
/// engine does and returns its `ModelVarying`; `draw_instance(i).data` is the value
/// Hooks::instance_data returned for that transform slot. Group 3 also holds the
/// material the engine would shade (`baked` factors, `base_texture`, `normal_texture`,
/// `mr_texture`, `emission_texture`, `ao_texture` and their samplers):
/// `model_base(v)` is its base colour and `model_discarded(v)` whether the engine
/// drops the fragment (Opacity's dither, a MASK cutout), for forward and shadow.
pub const MATERIAL_WGSL: &str = concat!(
    include_str!("../shaders/frame.wgsl"),
    "\n",
    include_str!("../shaders/transform.wgsl"),
    "\n",
    include_str!("../shaders/fade.wgsl"),
    "\n",
    include_str!("../shaders/custom_instance.wgsl"),
    "\n",
    include_str!("../shaders/model_base.wgsl")
);

/// Appended after [`MATERIAL_WGSL`] in a forward module: `sun_shadow(world, normal)`
/// (1 without sun shadows), `light_visibility`, `brdf`, `ambient` and
/// `add_local_lights`, over the shadow maps and lights forward custom pipelines
/// receive in groups 0 and 1, and `material_shade(v, front)`, the engine's own
/// shading of the material (a custom vertex shader keeping the stock look).
pub const MATERIAL_SHADOWS_WGSL: &str = concat!(
    include_str!("../shaders/shadow_sample.wgsl"),
    "\n",
    include_str!("../shaders/ibl.wgsl"),
    "\n",
    include_str!("../shaders/lights.wgsl"),
    "\n",
    include_str!("../shaders/model_shade.wgsl"),
    "\n",
    include_str!("../shaders/material_shadows.wgsl")
);

/// Borrowed model lookup and engine layouts. No simulation writes or submission.
pub struct MaterialGpu<'a> {
    pub(crate) device: &'a wgpu::Device,
    pub(crate) models: &'a crate::models::Models,
    pub(crate) pipelines: &'a crate::pipeline::Pipelines,
    pub(crate) instance: &'a wgpu::BindGroupLayout,
}
impl MaterialGpu<'_> {
    /// Resolve a loaded model's stable material handle.
    pub fn material(&self, model: &str, index: usize) -> Option<MaterialId> {
        self.models.loaded.get(model)?.materials.get(index).copied()
    }
    /// Build a double-sided pipeline for `material` over engine vertices and
    /// unskinned instances. Shadow entry points use group 1 binding 0, a light-view
    /// matrix; a shadow fragment entry may discard (`model_discarded`). Forward
    /// entry points receive the engine's shadow maps in group 1
    /// ([`MATERIAL_SHADOWS_WGSL`] samples them). Group 2 is game resources; group 3
    /// the engine's instances and the material. At most two vertex storage buffers
    /// remain under the default limit of eight. Opaque and MASK materials write
    /// depth; a BLEND material's forward pipeline alpha-blends without writing
    /// depth, drawn sorted with the engine's translucency, and casts no shadow.
    pub fn pipeline(
        &self,
        material: MaterialId,
        shader: &wgpu::ShaderModule,
        resources: &wgpu::BindGroupLayout,
        vertex: &str,
        fragment: Option<&str>,
        shadow: bool,
    ) -> wgpu::RenderPipeline {
        let blend = !shadow
            && self
                .models
                .materials
                .get(material.0)
                .is_some_and(|m| m.alpha == exact_game::asset::AlphaMode::Blend);
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32x4];
        let vertices = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        };
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("game custom material"),
                bind_group_layouts: &[
                    Some(&self.pipelines.scene_layout),
                    Some(if shadow {
                        &self.pipelines.camera_layout
                    } else {
                        &self.pipelines.shadow_layout
                    }),
                    Some(resources),
                    Some(self.instance),
                ],
                immediate_size: 0,
            });
        let color = [Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("game custom material"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some(vertex),
                    buffers: &[Some(vertices)],
                    compilation_options: Default::default(),
                },
                fragment: fragment.map(|entry| wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(entry),
                    targets: if shadow { &[] } else { &color },
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: if shadow { 1 } else { 4 },
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
    }
}
/// A custom material replaces the shading of an existing model material, opaque,
/// MASK or BLEND. The engine still owns its geometry, batches, transforms, culling,
/// level of detail, pass and draw calls. Both pipelines are required so vertex
/// deformation cannot lose shadow parity.
pub struct CustomMaterial {
    /// Renderer material from MaterialGpu::material.
    pub material: MaterialId,
    /// Double-sided 4× HDR pipeline from MaterialGpu::pipeline.
    pub forward: wgpu::RenderPipeline,
    /// Matching 1× depth-only pipeline; uses the same vertex deformation.
    pub shadow: wgpu::RenderPipeline,
    /// Game-owned group 2 resources, shared by the paired pipelines.
    pub resources: wgpu::BindGroup,
    /// How far the vertex shaders move a vertex from where `instance_transform`
    /// puts it, in the model's units (before the entity's scale). The GPU cull
    /// grows each part's bounds by it; `f32::INFINITY` never culls.
    pub reach: f32,
}

/// Group 3 of custom pipelines: the engine's instance records and, per custom
/// material, the uniform and textures of that material's own binding.
pub(crate) struct MaterialBindings {
    pub layout: wgpu::BindGroupLayout,
    groups: Vec<MaterialGroup>,
}
struct MaterialGroup {
    material: MaterialId,
    /// What `bind` was made from: the instance buffer and the material's binding.
    instances: wgpu::Buffer,
    source: wgpu::BindGroup,
    bind: wgpu::BindGroup,
}
impl MaterialBindings {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        // The model material's own layout (model_pipeline.rs), one binding up.
        entries.extend(
            crate::model_pipeline::material_entries()
                .into_iter()
                .map(|e| wgpu::BindGroupLayoutEntry {
                    binding: e.binding + 1,
                    ..e
                }),
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("custom instances and material"),
            entries: &entries,
        });
        Self {
            layout,
            groups: Vec::new(),
        }
    }
    /// One group per custom material, rebuilt when the instance buffer grows or
    /// the material's binding changes (its textures arrived).
    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        models: &crate::models::Models,
        custom: &[CustomMaterial],
    ) {
        let Some(buffer) = &models.instances else {
            self.groups.clear();
            return;
        };
        let source = |m: MaterialId| models.materials.get(m.0).and_then(|m| m.bind.as_ref());
        self.groups.retain(|g| {
            g.instances == buffer.raw
                && custom.iter().any(|c| c.material == g.material)
                && source(g.material) == Some(&g.source)
        });
        for c in custom {
            if self.groups.iter().any(|g| g.material == c.material) {
                continue;
            }
            let (Some(material), Some(bind)) =
                (models.materials.get(c.material.0), source(c.material))
            else {
                continue;
            };
            let (uniform, views) = &material.parts;
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.raw.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ];
            for (i, (view, sampler)) in views.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: 2 + i as u32 * 2,
                    resource: wgpu::BindingResource::TextureView(view),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3 + i as u32 * 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                });
            }
            self.groups.push(MaterialGroup {
                material: c.material,
                instances: buffer.raw.clone(),
                source: bind.clone(),
                bind: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("custom instances and material"),
                    layout: &self.layout,
                    entries: &entries,
                }),
            });
        }
    }
    /// The group 3 binding of a custom material, after `sync`.
    pub fn group(&self, material: MaterialId) -> &wgpu::BindGroup {
        let group = self.groups.iter().find(|g| g.material == material);
        &group.expect("custom material bound before drawing").bind
    }
}
