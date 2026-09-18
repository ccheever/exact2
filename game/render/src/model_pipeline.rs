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
    })
}
pub(crate) fn pipeline(
    device: &wgpu::Device,
    source: &str,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    variant: usize,
    shadow: bool,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("game textured PBR"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("game textured PBR"),
        bind_group_layouts: layouts,
        immediate_size: 0,
    });
    let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2];
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    let blend = variant & 8 != 0;
    let targets = [Some(wgpu::ColorTargetState {
        format: wgpu::TextureFormat::Rgba16Float,
        blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("game textured PBR"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(if shadow {
                "model_shadow_vs"
            } else {
                "model_vs"
            }),
            buffers: &buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(if shadow {
                "model_shadow_fs"
            } else if variant & 1 != 0 {
                "model_fs_shadow"
            } else {
                "model_fs"
            }),
            targets: if shadow { &[] } else { &targets },
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &[("FOG", if variant & 2 != 0 { 1. } else { 0. })],
                ..Default::default()
            },
        }),
        primitive: wgpu::PrimitiveState {
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
