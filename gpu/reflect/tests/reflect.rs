//! What the generator says about a shader is what naga says: offsets and
//! sizes by WGSL's layout rules, visibility from the entry points that use a
//! binding, vertex inputs packed in declaration order, and refusals that
//! name their cause.

use exact_gpu_reflect::reflect;

const SCENE: &str = r#"
struct Light { position: vec3<f32>, intensity: f32 }
struct Scene { time: f32, view: mat3x3<f32>, lights: array<Light, 2>, tint: vec2<f32> }
@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var<storage, read> counts: array<u32>;
@group(0) @binding(2) var<storage, read_write> totals: array<u32>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;
struct Vertex { @location(0) position: vec3<f32>, @location(2) uv: vec2<f32> }
@vertex fn vs(v: Vertex, @builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(v.position, 1.0) * scene.time;
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, p.xy) * f32(counts[0]);
}
@compute @workgroup_size(8, 4, 1) fn cs(@builtin(global_invocation_id) id: vec3<u32>) {
    totals[id.x] = counts[id.x];
}
"#;

fn scene() -> String {
    reflect("scene", SCENE).unwrap()
}

#[test]
fn structs_are_laid_out_by_wgsls_rules() {
    let g = scene();
    assert!(g.contains("pub struct Light {"), "{g}");
    assert!(g.contains("/// `position: vec3<f32>` at 0."));
    assert!(g.contains("/// `intensity: f32` at 12."));
    assert!(g.contains("pub const SIZE: usize = 16;"));
    assert!(g.contains("pub struct Scene {"));
    assert!(g.contains("/// `view: mat3x3<f32>` at 16."));
    assert!(g.contains("/// `lights: array<Light, 2>` at 64."));
    assert!(g.contains("pub lights: [Light; 2],"));
    assert!(g.contains("/// `tint: vec2<f32>` at 96."));
    assert!(g.contains("pub const SIZE: usize = 112;"));
    // A matrix column is 16 bytes: the second column's first row is at 16 + 16.
    assert!(g.contains("out[32..36].copy_from_slice(&self.view[1][0].to_le_bytes());"));
    // Array elements at their stride; a nested struct by its own `write`.
    assert!(g.contains("let o0 = 64 + i0 * 16;"), "{g}");
    assert!(g.contains("e0.write(&mut out[o0..o0 + 16]);"), "{g}");
    assert!(g.contains("#[derive(Debug, Clone, Copy, PartialEq, Default)]\n    pub struct Scene"));
}

#[test]
fn bindings_carry_their_stages_and_kinds() {
    let g = scene();
    assert!(g.contains("pub const SCENE: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: ::core::num::NonZeroU64::new(112) }, count: None };"), "{g}");
    assert!(g.contains("pub const COUNTS: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT.union(wgpu::ShaderStages::COMPUTE), ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }"), "{g}");
    assert!(g.contains("pub const TOTALS: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }"), "{g}");
    assert!(g.contains("pub const TEX: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };"), "{g}");
    assert!(g.contains("pub const TEX_SAMPLER: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None };"), "{g}");
    assert!(
        g.contains("label: Some(\"scene group 0\"), entries: &[SCENE, COUNTS, TOTALS] };"),
        "{g}"
    );
    assert!(
        g.contains("label: Some(\"scene group 1\"), entries: &[TEX, TEX_SAMPLER] };"),
        "{g}"
    );
    assert!(g.contains("/// `@group(0) @binding(1) var<storage, read> counts: array<u32>` — fragment and compute."), "{g}");
}

#[test]
fn entry_points_and_vertex_inputs_are_named_and_packed() {
    let g = scene();
    assert!(
        g.contains("/// `@vertex fn vs`.\n        pub const VS: &str = \"vs\";"),
        "{g}"
    );
    assert!(
        g.contains("pub const CS_WORKGROUP_SIZE: [u32; 3] = [8, 4, 1];"),
        "{g}"
    );
    assert!(g.contains("pub struct Vertex {"), "{g}");
    assert!(g.contains("pub position: [f32; 3],"));
    assert!(g.contains("pub const SIZE: usize = 20;"));
    assert!(g.contains("array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex, attributes: &[wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 }, wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 12, shader_location: 2 }] };"), "{g}");
    assert!(
        g.contains("out[12..16].copy_from_slice(&self.uv[0].to_le_bytes());"),
        "{g}"
    );
    assert!(g.contains("pub const MODULE: wgpu::ShaderModuleDescriptor<'static>"));
    assert!(g.contains("pub const SOURCE: &str = \"\\nstruct Light"));
}

#[test]
fn a_bad_shader_is_refused_with_its_line() {
    let e = reflect(
        "bad",
        "@fragment fn fs() -> @location(0) vec4<f32> {\n    return 1;\n}",
    )
    .unwrap_err();
    assert!(e.contains(":2:"), "{e}");
}

#[test]
fn a_binding_named_like_a_generated_item_is_refused() {
    let e = reflect(
        "x",
        "@group(0) @binding(0) var<uniform> source: f32;\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(source); }",
    )
    .unwrap_err();
    assert!(e.contains("`SOURCE`"), "{e}");
}

#[test]
fn a_shader_without_bindings_or_inputs_is_just_its_module_and_entries() {
    let g = reflect(
        "flat",
        "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> { return vec4<f32>(0.0); }\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }",
    )
    .unwrap();
    assert!(g.contains("pub mod flat {"));
    assert!(g.contains("pub const FS: &str = \"fs\";"));
    assert!(
        !g.contains("BindGroupLayoutEntry") && !g.contains("pub struct"),
        "{g}"
    );
}
