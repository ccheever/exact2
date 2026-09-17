use crate::Vertex;
use exact_gpu::wgpu;

pub(crate) const FORWARD: &str = include_str!("shaders/forward.wgsl");
pub(crate) const TONEMAP: &str = include_str!("shaders/tonemap.wgsl");

pub(crate) struct Pipelines {
    pub forward: wgpu::RenderPipeline,
    pub tone: wgpu::RenderPipeline,
    pub scene_layout: wgpu::BindGroupLayout,
    pub tone_layout: wgpu::BindGroupLayout,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let buffer = |binding, ty, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = buffer(
            0,
            wgpu::BufferBindingType::Uniform,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        let storage = |binding, visibility| {
            buffer(
                binding,
                wgpu::BufferBindingType::Storage { read_only: true },
                visibility,
            )
        };
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game scene"),
            entries: &[
                uniform,
                storage(1, wgpu::ShaderStages::VERTEX),
                storage(2, wgpu::ShaderStages::VERTEX),
                storage(3, wgpu::ShaderStages::FRAGMENT),
                storage(4, wgpu::ShaderStages::VERTEX),
            ],
        });
        let tone_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game tonemap"),
            entries: &[
                uniform,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let make = |source: &'static str, layout: &wgpu::BindGroupLayout, forward: bool| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(if forward { "game PBR" } else { "game ACES" }),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            let vertex_layout = [Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
            })];
            let tone_constants = [("ENCODE_SRGB", if format.is_srgb() { 0.0 } else { 1.0 })];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if forward {
                    "game forward"
                } else {
                    "game tonemap"
                }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: if forward { &vertex_layout } else { &[] },
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    targets: &[Some(
                        if forward {
                            wgpu::TextureFormat::Rgba16Float
                        } else {
                            format
                        }
                        .into(),
                    )],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: if forward { &[] } else { &tone_constants },
                        ..Default::default()
                    },
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: if forward {
                        Some(wgpu::Face::Back)
                    } else {
                        None
                    },
                    ..Default::default()
                },
                depth_stencil: forward.then_some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: if forward { 4 } else { 1 },
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            forward: make(FORWARD, &scene_layout, true),
            tone: make(TONEMAP, &tone_layout, false),
            scene_layout,
            tone_layout,
        }
    }
}
