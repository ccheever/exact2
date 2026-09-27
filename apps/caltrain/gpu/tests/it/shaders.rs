//! The shaders as the build reflected them (`build.rs`, `exact-gpu-reflect`):
//! the Rust the surfaces compile against says what the WGSL says — and,
//! since the text travels as an asset (LLP 1030 D8), the interface digest
//! tells a color edit from a bindings edit, and the registry refuses the
//! latter by name.

use caltrain_gpu::shaders::{aurora, SHADERS};
use exact_gpu::shaders::interface_digest;
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
    assert_eq!(aurora::NAME, "aurora");
    assert!(
        SHADERS.contains(&("aurora", aurora::INTERFACE_DIGEST)),
        "the registry's table names every shader at its digest: {SHADERS:?}"
    );
}

#[test]
fn a_color_edit_keeps_the_interface_and_a_bindings_edit_is_refused_by_name() {
    let source = std::fs::read_to_string(caltrain_gpu::shader_dir().join("aurora.wgsl")).unwrap();
    assert_eq!(
        interface_digest(&source).unwrap(),
        aurora::INTERFACE_DIGEST,
        "the file on disk is the interface the build reflected"
    );
    // A color edit — the palette's phase — is an asset change.
    let color = source.replace("vec3<f32>(0.02, 0.03, 0.08)", "vec3<f32>(0.08, 0.02, 0.03)");
    assert_ne!(color, source, "the edit found its line");
    assert_eq!(interface_digest(&color).unwrap(), aurora::INTERFACE_DIGEST);
    exact_gpu::shaders::set_shader("aurora", color.clone(), Some(aurora::INTERFACE_DIGEST))
        .unwrap();
    assert_eq!(
        exact_gpu::shaders::shader_source("aurora").as_deref(),
        Some(color.as_str())
    );
    // A binding an entry point uses is a binary change: refused, naming the
    // shader and both digests, and the registry keeps what it had.
    let bound = source
        .replace(
            "@group(0) @binding(2) var children_sampler: sampler;",
            "@group(0) @binding(2) var children_sampler: sampler;\n@group(0) @binding(9) var<uniform> extra: vec4<f32>;",
        )
        .replace("return vec4<f32>(out, 1.0);", "return vec4<f32>(out, 1.0) + extra;");
    assert_ne!(bound, source, "the edit found its lines");
    let moved = interface_digest(&bound).unwrap();
    assert_ne!(moved, aurora::INTERFACE_DIGEST);
    let e = exact_gpu::shaders::set_shader("aurora", bound, Some(aurora::INTERFACE_DIGEST))
        .unwrap_err();
    assert!(e.contains("`aurora`"), "{e}");
    assert!(
        e.contains(&format!("{moved:#018x}"))
            && e.contains(&format!("{:#018x}", aurora::INTERFACE_DIGEST)),
        "{e}"
    );
    assert_eq!(
        exact_gpu::shaders::shader_source("aurora").as_deref(),
        Some(color.as_str()),
        "the refusal left the registry as it was"
    );
    // Put the file's own text back for any fixture that follows.
    exact_gpu::shaders::set_shader("aurora", source, Some(aurora::INTERFACE_DIGEST)).unwrap();
}
