//! Custom shading over renderer-owned model geometry and instance records.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d4-custom-materials-inside-the-engines-batches-phase-2
use crate::{MaterialId, Vertex};
use exact_gpu::wgpu;

/// Reflected engine frame, entity interpolation and unskinned model instance access.
/// Append game entry points; group 2 belongs to the game. `draw_instance(i).data`
/// is the value returned by Hooks::instance_data for that transform slot.
pub const MATERIAL_WGSL: &str = concat!(
    include_str!("../shaders/frame.wgsl"),
    "\n",
    include_str!("../shaders/transform.wgsl"),
    "\n",
    include_str!("../shaders/custom_instance.wgsl")
);

/// Borrowed model lookup and engine layouts. No simulation writes or submission.
pub struct MaterialGpu<'a> {
    pub(crate) device: &'a wgpu::Device,
    pub(crate) models: &'a crate::models::Models,
    pub(crate) pipelines: &'a crate::pipeline::Pipelines,
    pub(crate) instance: &'a wgpu::BindGroupLayout,
    pub(crate) empty: &'a wgpu::BindGroupLayout,
}
impl MaterialGpu<'_> {
    /// Resolve a loaded model's stable material handle.
    pub fn material(&self, model: &str, index: usize) -> Option<MaterialId> {
        self.models.loaded.get(model)?.materials.get(index).copied()
    }
    /// Build a double-sided opaque pipeline over engine vertices and unskinned
    /// instances. Shadow entry points use group 1 binding 0, a light-view matrix.
    /// Forward entry points reserve group 1 empty. Group 2 is game resources;
    /// at most two vertex storage buffers remain under the default limit of eight.
    /// All geometry writes depth; transparent custom materials are not admitted.
    pub fn pipeline(
        &self,
        shader: &wgpu::ShaderModule,
        resources: &wgpu::BindGroupLayout,
        vertex: &str,
        fragment: Option<&str>,
        shadow: bool,
    ) -> wgpu::RenderPipeline {
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
                        self.empty
                    }),
                    Some(resources),
                    Some(self.instance),
                ],
                immediate_size: 0,
            });
        let color = [Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: None,
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
                    depth_write_enabled: Some(true),
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
/// A custom opaque material replaces the shading of an existing model material.
/// The engine still owns its geometry, batches, transforms, pass and draw calls.
/// Both pipelines are required so vertex deformation cannot lose shadow parity.
pub struct CustomMaterial {
    /// Renderer material from MaterialGpu::material.
    pub material: MaterialId,
    /// Double-sided, depth-writing 4× HDR pipeline from MaterialGpu::pipeline.
    pub forward: wgpu::RenderPipeline,
    /// Matching 1× depth-only pipeline; uses the same vertex deformation.
    pub shadow: wgpu::RenderPipeline,
    /// Game-owned group 2 resources, shared by the paired pipelines.
    pub resources: wgpu::BindGroup,
}

pub(crate) struct MaterialBindings {
    pub layout: wgpu::BindGroupLayout,
    pub empty: wgpu::BindGroupLayout,
    pub empty_bind: wgpu::BindGroup,
    pub instances: Option<(wgpu::Buffer, wgpu::BindGroup)>,
}
impl MaterialBindings {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("custom unskinned instances"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let empty = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("custom forward empty"),
            entries: &[],
        });
        let empty_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &empty,
            entries: &[],
        });
        Self {
            layout,
            empty,
            empty_bind,
            instances: None,
        }
    }
    pub fn sync(&mut self, device: &wgpu::Device, models: &crate::models::Models) {
        let Some(buffer) = &models.instances else {
            return;
        };
        if self
            .instances
            .as_ref()
            .is_some_and(|(old, _)| old == &buffer.raw)
        {
            return;
        }
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("custom instances"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.raw.as_entire_binding(),
            }],
        });
        self.instances = Some((buffer.raw.clone(), bind));
    }
}
