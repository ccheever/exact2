//! `skin_of` reads a vertex whose other joints are u32::MAX (models/merge.rs) as
//! its one joint, whatever its weights, and still blends an unmarked vertex.
use crate::buffers::bytes;
use exact_gpu::wgpu;
use glam::{Mat4, Quat, Vec3};

#[test]
fn a_marked_vertex_takes_its_one_joint_and_others_still_blend() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let skin = include_str!("shaders/model_vertex.wgsl")
        .split("struct ModelVarying")
        .next()
        .unwrap();
    // Each vertex's matrix, then its skinned position and normal, with the
    // draw's palette starting at entry 2.
    let source = format!(
        "{skin}
@group(0) @binding(0) var<storage,read_write> output:array<vec4<f32>>;
@compute @workgroup_size(1) fn test_skin() {{
    let z=mat4x4<f32>();
    let draw=ModelInstance(0u,0u,0u,2u,z,z,vec4(1.0),vec4(0.0));
    for(var v=0u;v<4u;v++) {{
        let m=skin_of(draw,v);
        let s=skinned(draw,v,vec3(1.0,2.0,3.0),normalize(vec3(1.0,1.0,1.0)));
        for(var c=0u;c<4u;c++) {{ output[v*6u+c]=m[c]; }}
        output[v*6u+4u]=vec4(s[0],1.0);
        output[v*6u+5u]=vec4(s[1],0.0);
    }}
}}"
    );
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &shader,
            entry_point: Some("test_skin"),
            compilation_options: Default::default(),
            cache: None,
        });
    let a = Mat4::from_scale_rotation_translation(
        Vec3::new(3., 1., 0.5),
        Quat::from_rotation_z(0.7),
        Vec3::new(1., -2., 0.25),
    );
    let b = Mat4::from_scale_rotation_translation(
        Vec3::new(1., 2., 4.),
        Quat::from_rotation_y(0.3),
        Vec3::new(-0.5, 3., 2.),
    );
    let c = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.75),
        Quat::from_rotation_x(-1.1),
        Vec3::new(4., 0., -1.),
    );
    let buffer = |data: &[u8]| {
        let b = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: data.len() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&b, 0, data);
        b
    };
    // Entries 0 and 1 belong to no joint of this draw.
    let palette = [Mat4::IDENTITY * 7., Mat4::IDENTITY * 9., a, b, c];
    let palette: Vec<f32> = palette.iter().flat_map(|m| m.to_cols_array()).collect();
    let palette = buffer(bytes(&palette));
    let (half, one, mark) = (0.5f32.to_bits(), 1f32.to_bits(), u32::MAX);
    let vertices = [
        // Joint 1 marked, with weights a blend would use: joint 1 alone.
        [1, mark, mark, mark, half, half, 0, 0],
        // Joint 1 with weight one, unmarked.
        [1, 0, 0, 0, one, 0, 0, 0],
        // Joints 0 and 2 blended.
        [0, 2, 0, 0, half, half, 0, 0],
        // Joint 2 as merge.rs writes it.
        [2, mark, mark, mark, one, 0, 0, 0],
    ]
    .concat();
    let vertices = buffer(bytes(&vertices));
    let output = buffer(bytes(&[0f32; 4 * 6 * 4]));
    let bind = |group, entries: &[wgpu::BindGroupEntry<'_>]| {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(group),
            entries,
        })
    };
    let out = bind(
        0,
        &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    );
    let (empty1, empty2) = (bind(1, &[]), bind(2, &[]));
    let input = bind(
        3,
        &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: palette.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: vertices.as_entire_binding(),
            },
        ],
    );
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &out, &[]);
        pass.set_bind_group(1, &empty1, &[]);
        pass.set_bind_group(2, &empty2, &[]);
        pass.set_bind_group(3, &input, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    gpu.queue.submit([encoder.finish()]);
    let words: Vec<u32> = super::tests::read(&gpu, &output, 4 * 6 * 16)
        .chunks_exact(4)
        .map(|v| u32::from_ne_bytes(v.try_into().unwrap()))
        .collect();
    let vertex = |v: usize| &words[v * 24..(v + 1) * 24];
    let matrix = |v: usize| {
        Mat4::from_cols_slice(
            &vertex(v)[..16]
                .iter()
                .map(|w| f32::from_bits(*w))
                .collect::<Vec<_>>(),
        )
    };
    let bits = |m: Mat4| m.to_cols_array().map(f32::to_bits);
    assert_eq!(
        vertex(0),
        vertex(1),
        "a marked joint skins as weight one did"
    );
    assert_eq!(&vertex(0)[..16], bits(b));
    assert_eq!(&vertex(3)[..16], bits(c));
    assert!(
        matrix(2).abs_diff_eq(a * 0.5 + c * 0.5, 1e-5),
        "{:?}",
        matrix(2)
    );
}
