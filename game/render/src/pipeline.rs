use crate::Vertex;
use exact_gpu::wgpu;

const FRAME: &str = include_str!("shaders/frame.wgsl");
const TRANSFORM: &str = include_str!("shaders/transform.wgsl");

pub(crate) fn shader_sources() -> [String; 5] {
    [
        [
            FRAME,
            TRANSFORM,
            include_str!("shaders/shadow_sample.wgsl"),
            include_str!("shaders/forward.wgsl"),
        ]
        .concat(),
        [FRAME, TRANSFORM, include_str!("shaders/shadow.wgsl")].concat(),
        [FRAME, include_str!("shaders/sky.wgsl")].concat(),
        [FRAME, include_str!("shaders/tonemap.wgsl")].concat(),
        [FRAME, include_str!("shaders/bloom.wgsl")].concat(),
    ]
}

pub(crate) struct Pipelines {
    pub forward: [wgpu::RenderPipeline; 4],
    pub shadow: wgpu::RenderPipeline,
    pub sky: wgpu::RenderPipeline,
    pub tone: [wgpu::RenderPipeline; 2],
    pub bloom: [wgpu::RenderPipeline; 3],
    pub scene_layout: wgpu::BindGroupLayout,
    pub tone_layout: wgpu::BindGroupLayout,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub camera_layout: wgpu::BindGroupLayout,
    pub bloom_layout: wgpu::BindGroupLayout,
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
        let texture = |binding, sample_type, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let sampler = |binding, kind| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(kind),
            count: None,
        };
        let layout = |label, entries: &[_]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let scene_layout = layout(
            "game scene",
            &[
                uniform,
                storage(1, wgpu::ShaderStages::VERTEX),
                storage(2, wgpu::ShaderStages::VERTEX),
                storage(3, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage(4, wgpu::ShaderStages::VERTEX),
            ],
        );
        let tone_layout = layout(
            "game tonemap",
            &[
                uniform,
                texture(
                    1,
                    wgpu::TextureSampleType::Float { filterable: false },
                    wgpu::TextureViewDimension::D2,
                ),
            ],
        );
        let shadow_layout = layout(
            "game sun sample",
            &[
                texture(
                    0,
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2Array,
                ),
                sampler(1, wgpu::SamplerBindingType::Comparison),
            ],
        );
        let camera_layout = layout("game cascade camera", &[uniform]);
        let bloom_layout = layout(
            "game bloom",
            &[
                uniform,
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                texture(
                    1,
                    wgpu::TextureSampleType::Float { filterable: true },
                    wgpu::TextureViewDimension::D2,
                ),
                sampler(2, wgpu::SamplerBindingType::Filtering),
            ],
        );
        let sources = shader_sources();
        let shaders: [_; 5] = std::array::from_fn(|i| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(
                    [
                        "game PBR",
                        "game shadow",
                        "game sky",
                        "game ACES",
                        "game bloom",
                    ][i],
                ),
                source: wgpu::ShaderSource::Wgsl(sources[i].as_str().into()),
            })
        });
        let vertex_attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let vertex_layout = [Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        })];
        let depth = |write, compare, bias| {
            Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(write),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias,
            })
        };
        let forward = std::array::from_fn(|i| {
            let shadow = i & 1 != 0;
            let layouts = [Some(&scene_layout), Some(&shadow_layout)];
            make_pipeline(
                device,
                &shaders[0],
                "game forward",
                "vs",
                Some(if shadow { "fs_shadow" } else { "fs" }),
                &layouts[..if shadow { 2 } else { 1 }],
                &vertex_layout,
                depth(true, wgpu::CompareFunction::Less, Default::default()),
                4,
                wgpu::TextureFormat::Rgba16Float,
                None,
                &[("FOG", if i & 2 != 0 { 1.0 } else { 0.0 })],
            )
        });
        let shadow = make_pipeline(
            device,
            &shaders[1],
            "game shadow",
            "vs_shadow",
            None,
            &[Some(&scene_layout), Some(&camera_layout)],
            &vertex_layout,
            depth(true, wgpu::CompareFunction::Less, Default::default()),
            1,
            wgpu::TextureFormat::Rgba16Float,
            None,
            &[],
        );
        let sky = make_pipeline(
            device,
            &shaders[2],
            "game sky",
            "sky_vs",
            Some("sky_fs"),
            &[Some(&scene_layout)],
            &[],
            depth(false, wgpu::CompareFunction::LessEqual, Default::default()),
            4,
            wgpu::TextureFormat::Rgba16Float,
            None,
            &[],
        );
        let tone = std::array::from_fn(|i| {
            let layouts = [Some(&tone_layout), Some(&bloom_layout)];
            make_pipeline(
                device,
                &shaders[3],
                "game ACES",
                "vs",
                Some(if i == 0 { "fs" } else { "fs_bloom" }),
                &layouts[..i + 1],
                &[],
                None,
                1,
                format,
                None,
                &[("ENCODE_SRGB", if format.is_srgb() { 0.0 } else { 1.0 })],
            )
        });
        let additive = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let bloom = std::array::from_fn(|i| {
            make_pipeline(
                device,
                &shaders[4],
                "game bloom",
                "vs",
                Some(["bright", "down", "up"][i]),
                &[Some(&bloom_layout)],
                &[],
                None,
                1,
                wgpu::TextureFormat::Rgba16Float,
                (i == 2).then_some(wgpu::BlendState {
                    color: additive,
                    alpha: additive,
                }),
                &[],
            )
        });
        Self {
            forward,
            shadow,
            sky,
            tone,
            bloom,
            scene_layout,
            tone_layout,
            shadow_layout,
            camera_layout,
            bloom_layout,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    label: &str,
    vs: &str,
    fs: Option<&str>,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    depth_stencil: Option<wgpu::DepthStencilState>,
    samples: u32,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    constants: &[(&str, f64)],
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: layouts,
        immediate_size: 0,
    });
    let targets = [Some(wgpu::ColorTargetState {
        format,
        blend,
        write_mask: wgpu::ColorWrites::ALL,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vs),
            buffers,
            compilation_options: Default::default(),
        },
        fragment: fs.map(|entry| wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            targets: &targets,
            compilation_options: wgpu::PipelineCompilationOptions {
                constants,
                ..Default::default()
            },
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: if buffers.is_empty() {
                None
            } else {
                Some(wgpu::Face::Back)
            },
            ..Default::default()
        },
        depth_stencil,
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}
