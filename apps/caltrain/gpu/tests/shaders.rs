//! The shaders as the build reflected them (`build.rs`, `exact-gpu-reflect`):
//! the Rust the surfaces compile against says what the WGSL says.

use caltrain_gpu::shaders::{aurora, map};
use exact_gpu::wgpu;

#[test]
fn the_aurora_uniforms_are_laid_out_as_the_shader_declares() {
    assert_eq!(aurora::Uniforms::SIZE, 16);
    let u = aurora::Uniforms {
        time: 1.0,
        width: 2.0,
        height: 3.0,
        seed: 4.0,
    };
    let b = u.bytes();
    assert_eq!(&b[0..4], &1f32.to_le_bytes());
    assert_eq!(&b[4..8], &2f32.to_le_bytes());
    assert_eq!(&b[8..12], &3f32.to_le_bytes());
    assert_eq!(&b[12..16], &4f32.to_le_bytes());
}

#[test]
fn the_aurora_bind_group_is_the_shaders_three_bindings() {
    assert_eq!(aurora::GROUP_0.entries.len(), 3);
    assert_eq!(
        (
            aurora::U.binding,
            aurora::CHILDREN.binding,
            aurora::CHILDREN_SAMPLER.binding
        ),
        (0, 1, 2)
    );
    assert_eq!(aurora::U.visibility, wgpu::ShaderStages::FRAGMENT);
    assert!(matches!(
        aurora::U.ty,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            min_binding_size: Some(n),
            ..
        } if n.get() == 16
    ));
    assert!(matches!(
        aurora::CHILDREN.ty,
        wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        }
    ));
    assert!(matches!(
        aurora::CHILDREN_SAMPLER.ty,
        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
    ));
    assert_eq!((aurora::entry::VS, aurora::entry::FS), ("vs", "fs"));
    assert!(aurora::SOURCE.contains("@fragment"));
}

#[test]
fn the_map_vertex_is_one_interleaved_buffer() {
    assert_eq!(map::Vertex::SIZE, 24);
    let l = map::Vertex::LAYOUT;
    assert_eq!(l.array_stride, 24);
    assert_eq!(l.attributes.len(), 2);
    let a = |i: usize| {
        (
            l.attributes[i].format,
            l.attributes[i].offset,
            l.attributes[i].shader_location,
        )
    };
    assert_eq!(a(0), (wgpu::VertexFormat::Float32x2, 0, 0));
    assert_eq!(a(1), (wgpu::VertexFormat::Float32x4, 8, 1));
    let v = map::Vertex {
        position: [1.0, 2.0],
        color: [3.0, 4.0, 5.0, 6.0],
    };
    let expect: Vec<u8> = [1f32, 2.0, 3.0, 4.0, 5.0, 6.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    assert_eq!(v.bytes().to_vec(), expect);
}
