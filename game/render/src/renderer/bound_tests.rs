//! A merging `Bound` draws what one draw a command does: runs end at a state
//! change, a dynamic offset, another buffer, a gap in the arguments, a single
//! draw and the final flush, each draw keeps its own base vertex and first
//! instance, and none is dropped, repeated or reordered.
use super::Bound;
use exact_gpu::{fixture, wgpu};

const ARGS: u64 = size_of::<wgpu::util::DrawIndexedIndirectArgs>() as u64;
const SHADER: &str = "
struct Tint { colour: vec4<f32> }
@group(0) @binding(0) var<uniform> tint: Tint;
@group(1) @binding(0) var<uniform> scale: vec4<f32>;
// A quad over pixel column i of eight, for a base vertex of 4i or a first
// instance of i.
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = -1.0 + (f32(v / 4u + i) + f32(v & 1u)) * 0.25;
    return vec4(x, select(-1.0, 1.0, (v & 2u) != 0u), 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return tint.colour * scale; }
";

#[test]
fn merged_runs_draw_what_single_draws_do() {
    let Some(gpu) = crate::test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let device = &gpu.device;
    let buffer = |usage, bytes: &[u8]| {
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&b, 0, bytes);
        b
    };
    let floats =
        |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let indices: Vec<u8> = [0u32, 1, 2, 2, 1, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let indices = buffer(wgpu::BufferUsages::INDEX, &indices);
    // Two buffers of arguments where draw i covers column i: by its base
    // vertex in the first, by its first instance in the second where the
    // device allows one.
    let instanced = device
        .features()
        .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
    let args = |by_instance: bool| -> Vec<u8> {
        (0..8)
            .flat_map(|i| {
                wgpu::util::DrawIndexedIndirectArgs {
                    index_count: 6,
                    instance_count: 1,
                    first_index: 0,
                    base_vertex: if by_instance { 0 } else { 4 * i },
                    first_instance: if by_instance { i as u32 } else { 0 },
                }
                .as_bytes()
                .to_vec()
            })
            .collect()
    };
    let (a, b) = (
        buffer(wgpu::BufferUsages::INDIRECT, &args(false)),
        buffer(wgpu::BufferUsages::INDIRECT, &args(instanced)),
    );
    let layout = |dynamic| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: dynamic,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            }],
        })
    };
    let (tint_layout, scale_layout) = (layout(true), layout(false));
    // Red at offset 0, green at 256: the scene group's dynamic offset picks one.
    let mut tints = floats(&[0.25, 0., 0., 0.25]);
    tints.resize(256, 0);
    tints.extend(floats(&[0., 0.25, 0., 0.25]));
    let tints = buffer(wgpu::BufferUsages::UNIFORM, &tints);
    let tint = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &tint_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &tints,
                offset: 0,
                size: wgpu::BufferSize::new(16),
            }),
        }],
    });
    let scale = |s: f32| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &scale_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer(wgpu::BufferUsages::UNIFORM, &floats(&[s; 4])).as_entire_binding(),
            }],
        })
    };
    let (full, half) = (scale(1.), scale(0.5));
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&tint_layout), Some(&scale_layout)],
        immediate_size: 0,
    });
    let pipeline = |blend| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    // Additive draws show a draw dropped or repeated; a replacing one first
    // shows one that ran out of order.
    let add = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    let (replacing, adding) = (
        pipeline(None),
        pipeline(Some(wgpu::BlendState {
            color: add,
            alpha: add,
        })),
    );
    let draw = |merge: bool| {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 8,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        let held = |bound: &Bound| bound.run.map(|(_, first, count)| (first / ARGS, count));
        let held_in = |bound: &Bound, buffer: &wgpu::Buffer| {
            bound.run.is_some_and(|(held, _, _)| held == buffer)
        };
        let mut bound = Bound::merging(merge);
        bound.pipeline(&mut pass, &replacing);
        bound.scene(&mut pass, &tint, 0);
        bound.group(&mut pass, 1, &full);
        bound.indirect(&mut pass, &a, 0);
        bound.pipeline(&mut pass, &adding);
        for i in 0..3 {
            bound.indirect(&mut pass, &a, i * ARGS);
        }
        assert_eq!(held(&bound), merge.then_some((0, 3)));
        // Only the scene group's dynamic offset changes: red to green.
        bound.scene(&mut pass, &tint, 256);
        assert_eq!(held(&bound), None);
        for i in [1, 2] {
            bound.indirect(&mut pass, &a, i * ARGS);
        }
        assert_eq!(held(&bound), merge.then_some((1, 2)));
        // Another buffer starts another run; its draws place their quads by
        // their first instance.
        bound.indirect(&mut pass, &b, 3 * ARGS);
        bound.indirect(&mut pass, &b, 4 * ARGS);
        assert_eq!(held(&bound), merge.then_some((3, 2)));
        assert_eq!(held_in(&bound, &b), merge);
        // A single draw issues the run before it and holds nothing.
        bound.single();
        bound.indirect(&mut pass, &a, 5 * ARGS);
        assert_eq!(held(&bound), None);
        bound.indirect(&mut pass, &a, 6 * ARGS);
        assert_eq!(held(&bound), merge.then_some((6, 1)));
        // A material's group changes under a held draw: it goes out first.
        bound.group(&mut pass, 1, &half);
        assert_eq!(held(&bound), None);
        for i in [7, 6] {
            bound.indirect(&mut pass, &a, i * ARGS);
        }
        // Draw 6 again does not follow draw 7: it waits alone for the flush.
        assert_eq!(held(&bound), merge.then_some((6, 1)));
        bound.flush(&mut pass);
        assert_eq!(held(&bound), None);
        drop(bound);
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
        fixture::read(&gpu, &target).unwrap().data
    };
    let merged = draw(true);
    assert_eq!(merged.len(), 8 * 4);
    assert_eq!(merged, draw(false));
    let expected = [
        [128, 0, 0, 128],
        [64, 64, 0, 128],
        [64, 64, 0, 128],
        [0, 64, 0, 64],
        [0, 64, 0, 64],
        [0, 64, 0, 64],
        [0, 96, 0, 96],
        [0, 32, 0, 32],
    ];
    for (column, (got, want)) in merged.chunks_exact(4).zip(expected).enumerate() {
        assert!(
            got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 1),
            "column {column}: {got:?}, want {want:?}"
        );
    }
}
