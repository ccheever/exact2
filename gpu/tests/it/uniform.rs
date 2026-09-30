//! A frame uniform: each frame's bytes reach that frame's draw, across the
//! ring's slots, as the queue's write would.

use exact_gpu::{fixture, wgpu, FrameUniform};

const TEXT: &str = "struct U { c: vec4<f32> }\n@group(0) @binding(0) var<uniform> u: U;\n@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {\n  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));\n  return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);\n}\n@fragment fn fs() -> @location(0) vec4<f32> { return u.c; }\n";

#[test]
fn each_frame_draws_its_own_bytes() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let device = &gpu.device;
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(TEXT.into()),
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut uniform = FrameUniform::new(device, 16, "t");
    #[cfg(target_vendor = "apple")]
    assert!(uniform.slots() > 1, "Metal writes a ring in place");
    let groups: Vec<_> = (0..uniform.slots())
        .map(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.buffer(i).as_entire_binding(),
                }],
            })
        })
        .collect();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    for frame in 0..9u8 {
        let red = f32::from(frame) / 8.0;
        let bytes: Vec<u8> = [red, 0.5, 0.25, 1.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let slot = uniform.write(&gpu.queue, &bytes);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        let px = fixture::read(&gpu, &texture).unwrap();
        assert_eq!(
            px.at(0, 0),
            [(red * 255.0).round() as u8, 128, 64, 255],
            "frame {frame}"
        );
    }
}
