use crate::Vertex;
use exact_gpu::wgpu;
pub(crate) fn material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }];
    for i in 0..5 {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1 + i * 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2 + i * 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("game model material"),
        entries: &entries,
    })
}
pub(crate) fn instance_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("game draw instances"),
        entries: &[0, 1, 2].map(|binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }),
    })
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn pipeline(
    device: &wgpu::Device,
    cache: Option<&wgpu::PipelineCache>,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    variant: usize,
    shadow: bool,
    (samples, format): (u32, wgpu::TextureFormat),
    (early, depth_prepass, camera_depth, matte): (bool, bool, bool, bool),
) -> wgpu::RenderPipeline {
    use crate::pipeline::{MODEL_DIRECT, MODEL_GENERAL};
    let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32x4];
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    let blend = variant & 8 != 0;
    let general = variant & MODEL_GENERAL != 0;
    // A direct frame draws into the target's format, tonemapped in the shader.
    let direct = variant & MODEL_DIRECT != 0;
    let targets = [Some(wgpu::ColorTargetState {
        format: if direct {
            format
        } else {
            wgpu::TextureFormat::Rgba16Float
        },
        blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: if camera_depth {
            wgpu::ColorWrites::empty()
        } else {
            wgpu::ColorWrites::ALL
        },
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("game textured PBR"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(if shadow {
                "model_shadow_vs"
            } else if camera_depth && variant & 32 != 0 {
                // Opaque camera depth: position only.
                "model_depth_vs"
            } else {
                "model_vs"
            }),
            buffers: &buffers,
            compilation_options: Default::default(),
        },
        fragment: (!((shadow || (camera_depth && !direct)) && variant & 32 != 0)).then_some(
            wgpu::FragmentState {
                module: shader,
                entry_point: Some(if camera_depth && variant & 32 != 0 {
                    "camera_depth_empty"
                } else if camera_depth {
                    "model_depth_fs"
                } else if shadow {
                    "model_shadow_fs"
                } else if early && variant & 32 != 0 {
                    if variant & 1 != 0 {
                        "model_fs_shadow_early"
                    } else {
                        "model_fs_early"
                    }
                } else if variant & 1 != 0 {
                    "model_fs_shadow"
                } else {
                    "model_fs"
                }),
                targets: if shadow || (camera_depth && !direct) {
                    &[]
                } else {
                    &targets
                },
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[
                        ("MATTE", if matte { 1. } else { 0. }),
                        ("FOG", if variant & 2 != 0 { 1. } else { 0. }),
                        (
                            "LIGHTS",
                            if general || variant & crate::pipeline::MODEL_LIGHTS != 0 {
                                1.
                            } else {
                                0.
                            },
                        ),
                        ("DEPTH_PREPASS", if depth_prepass { 1. } else { 0. }),
                        ("MODEL_DISCARD", if variant & 32 != 0 { 0. } else { 1. }),
                        ("SINGLE_CASCADE", if variant & 128 != 0 { 1. } else { 0. }),
                        (
                            "SHADOW_FILTER",
                            if general {
                                2.
                            } else if variant & crate::pipeline::MODEL_BILINEAR != 0 {
                                3.
                            } else if variant & 256 != 0 {
                                1.
                            } else {
                                0.
                            },
                        ),
                        (
                            "MODEL_TEXTURES",
                            if !shadow && variant & 64 != 0 { 0. } else { 1. },
                        ),
                        ("DIRECT", if direct { 1. } else { 0. }),
                        ("ENCODE_SRGB", if format.is_srgb() { 0. } else { 1. }),
                    ],
                    ..Default::default()
                },
            },
        ),
        primitive: wgpu::PrimitiveState {
            front_face: if variant & 16 != 0 {
                wgpu::FrontFace::Cw
            } else {
                wgpu::FrontFace::Ccw
            },
            cull_mode: if variant & 4 != 0 {
                None
            } else {
                Some(wgpu::Face::Back)
            },
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(!blend),
            depth_compare: Some(if depth_prepass && !shadow && !blend {
                wgpu::CompareFunction::Equal
            } else {
                wgpu::CompareFunction::Less
            }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: if shadow { 1 } else { samples },
            ..Default::default()
        },
        multiview_mask: None,
        cache,
    })
}

/// A missing normal map uses the geometric normal, without a quantized default map.
pub(crate) fn untextured(m: &exact_game::asset::MaterialData) -> bool {
    [
        m.base_color_texture,
        m.normal_texture,
        m.metallic_roughness_texture,
        m.emissive_texture,
        m.occlusion_texture,
    ]
    .iter()
    .all(Option::is_none)
}
