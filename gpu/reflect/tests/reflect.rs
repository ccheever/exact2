//! What the generator says about a shader is what naga says: offsets and
//! sizes by WGSL's layout rules, visibility from the entry points that use a
//! binding (through helpers), filterability from whether a sampler ever
//! touches a texture, vertex inputs packed in declaration order, and
//! refusals that name their cause.

use exact_gpu_reflect::{generate, reflect};

const SCENE: &str = r#"
struct Light { position: vec3<f32>, intensity: f32 }
struct Scene { time: f32, view: mat3x3<f32>, lights: array<Light, 2>, tint: vec2<f32> }
@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var<storage, read> counts: array<u32>;
@group(0) @binding(2) var<storage, read_write> totals: array<u32>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;
@group(1) @binding(2) var lut: texture_2d<f32>;
struct Vertex { @location(0) position: vec3<f32>, @location(2) uv: vec2<f32> }
@vertex fn vs(v: Vertex, @builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(v.position, 1.0) * scene.time;
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let t = textureLoad(lut, vec2<i32>(0, 0), 0);
    return textureSample(tex, tex_sampler, p.xy) * f32(counts[0]) + t;
}
@compute @workgroup_size(8, 4, 1) fn cs(@builtin(global_invocation_id) id: vec3<u32>) {
    totals[id.x] = counts[id.x];
}
"#;

fn scene() -> String {
    reflect("scene", SCENE).unwrap()
}

const FS: &str = "@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";

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
    // Padding is zeroed before the fields are written.
    assert!(
        g.contains("pub fn write(&self, out: &mut [u8]) {\n            out[..Self::SIZE].fill(0);"),
        "{g}"
    );
}

#[test]
fn bindings_carry_their_stages_and_kinds() {
    let g = scene();
    assert!(g.contains("pub const SCENE: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: ::core::num::NonZeroU64::new(112) }, count: None };"), "{g}");
    // A runtime-sized array's minimum is one element (naga's rule, and wgpu's for `layout: None`).
    assert!(g.contains("pub const COUNTS: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT.union(wgpu::ShaderStages::COMPUTE), ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: ::core::num::NonZeroU64::new(4) }, count: None };"), "{g}");
    assert!(g.contains("pub const TOTALS: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }"), "{g}");
    assert!(g.contains("pub const TEX_SAMPLER: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None };"), "{g}");
    assert!(
        g.contains("label: Some(\"scene group 0\"), entries: &[SCENE, COUNTS, TOTALS] };"),
        "{g}"
    );
    assert!(
        g.contains("label: Some(\"scene group 1\"), entries: &[TEX, TEX_SAMPLER, LUT] };"),
        "{g}"
    );
    assert!(g.contains("/// `@group(0) @binding(1) var<storage, read> counts: array<u32>` — fragment and compute."), "{g}");
}

#[test]
fn a_float_texture_is_filterable_exactly_when_a_sampler_touches_it() {
    let g = scene();
    assert!(g.contains("pub const TEX: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };"), "{g}");
    assert!(
        g.contains("var tex: texture_2d<f32> (sampled with a sampler: filterable)`"),
        "{g}"
    );
    // Read with `textureLoad` only: unfilterable-float, so an `rgba32float` binds.
    assert!(g.contains("pub const LUT: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };"), "{g}");
    assert!(
        g.contains("var lut: texture_2d<f32> (never sampled with a sampler: unfilterable-float)`"),
        "{g}"
    );
}

#[test]
fn a_binding_used_only_through_a_helper_is_visible_to_the_caller() {
    let g = reflect(
        "h",
        "@group(0) @binding(0) var<uniform> k: f32;\nfn helper() -> f32 { return k; }\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(helper()); }",
    )
    .unwrap();
    assert!(g.contains("pub const K: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,"), "{g}");
}

#[test]
fn a_storage_texture_carries_its_access_and_format() {
    let g = reflect(
        "st",
        "@group(0) @binding(0) var img: texture_storage_2d<rgba8unorm, write>;\n@compute @workgroup_size(1) fn cs(@builtin(global_invocation_id) id: vec3<u32>) { textureStore(img, vec2<i32>(id.xy), vec4<f32>(1.0)); }",
    )
    .unwrap();
    assert!(g.contains("pub const IMG: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 }, count: None };"), "{g}");
    assert!(
        g.contains("var img: texture_storage_2d<rgba8unorm, write>`"),
        "{g}"
    );
    assert!(
        g.contains("pub const CS_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];"),
        "{g}"
    );
}

#[test]
fn a_trailing_runtime_sized_array_is_an_offset_and_a_stride_beside_the_fixed_members() {
    let g = reflect(
        "ps",
        "struct Particle { pos: vec2<f32>, vel: vec2<f32> }\nstruct Particles { count: u32, items: array<Particle> }\n@group(0) @binding(0) var<storage, read_write> ps: Particles;\n@compute @workgroup_size(1) fn cs() { ps.items[0].pos = ps.items[0].vel * f32(ps.count); }",
    )
    .unwrap();
    assert!(g.contains("pub struct Particle {"), "{g}");
    assert!(g.contains("pub struct Particles {\n        /// `count: u32` at 0.\n        pub count: u32,\n    }"), "{g}");
    assert!(g.contains("pub const SIZE: usize = 8;"), "{g}");
    assert!(g.contains("pub const ITEMS_OFFSET: usize = 8;"), "{g}");
    assert!(g.contains("pub const ITEMS_STRIDE: usize = 16;"), "{g}");
    assert!(
        g.contains("then `items: array<Particle>` at `ITEMS_OFFSET`"),
        "{g}"
    );
    // The binding's minimum is the fixed members plus one element.
    assert!(
        g.contains("min_binding_size: ::core::num::NonZeroU64::new(24)"),
        "{g}"
    );
}

#[test]
fn nested_arrays_and_keyword_fields_write_correctly() {
    let g = reflect(
        "g",
        "struct Grid { cells: array<array<f32, 2>, 3>, box: f32 }\n@group(0) @binding(0) var<storage, read> grid: Grid;\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(grid.cells[1][0] + grid.box); }",
    )
    .unwrap();
    assert!(g.contains("pub cells: [[f32; 2]; 3],"), "{g}");
    assert!(g.contains("let o0 = i0 * 8;"), "{g}");
    assert!(g.contains("let o1 = o0 + i1 * 4;"), "{g}");
    assert!(
        g.contains("out[o1..o1 + 4].copy_from_slice(&e1.to_le_bytes());"),
        "{g}"
    );
    assert!(g.contains("pub r#box: f32,"), "{g}");
    assert!(g.contains("&self.r#box.to_le_bytes()"), "{g}");
}

#[test]
fn a_struct_with_a_long_array_has_no_default() {
    let g = reflect(
        "big",
        "struct Big { v: array<f32, 33> }\n@group(0) @binding(0) var<storage, read> big: Big;\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(big.v[32]); }",
    )
    .unwrap();
    assert!(
        g.contains("#[derive(Debug, Clone, Copy, PartialEq)]\n    pub struct Big"),
        "{g}"
    );
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
    // What the shader declares, and the buffer this generator packs by convention.
    assert!(g.contains("pub const INPUTS: &[(u32, wgpu::VertexFormat)] = &[(0, wgpu::VertexFormat::Float32x3), (2, wgpu::VertexFormat::Float32x2)];"), "{g}");
    assert!(
        g.contains("this generator's convention, not the shader's"),
        "{g}"
    );
    assert!(g.contains("array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex, attributes: &[wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 }, wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 12, shader_location: 2 }] };"), "{g}");
    assert!(
        g.contains("out[12..16].copy_from_slice(&self.uv[0].to_le_bytes());"),
        "{g}"
    );
    assert!(g.contains("pub fn module() -> wgpu::ShaderModuleDescriptor<'static>"));
    assert!(g.contains("pub const NAME: &str = \"scene\";"));
    assert!(g.contains("pub const INTERFACE_DIGEST: u64 = 0x"), "{g}");
    assert!(
        !g.contains("struct Light { position"),
        "the source text is not in the binary: {g}"
    );
}

#[test]
fn two_vertex_entry_points_sharing_one_struct_get_one_rust_struct() {
    let g = reflect(
        "two",
        "struct V { @location(0) p: vec2<f32> }\n@vertex fn a(v: V) -> @builtin(position) vec4<f32> { return vec4<f32>(v.p, 0.0, 1.0); }\n@vertex fn b(v: V) -> @builtin(position) vec4<f32> { return vec4<f32>(v.p, 1.0, 1.0); }",
    )
    .unwrap();
    assert_eq!(g.matches("pub struct V {").count(), 1, "{g}");
}

#[test]
fn colliding_generated_names_are_refused_by_name() {
    // Two vertex entry points whose derived input structs would share a name.
    let e = reflect(
        "vv",
        "@vertex fn foo_bar(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> { return vec4<f32>(p, 0.0, 1.0); }\n@vertex fn foo__bar(@location(0) q: vec3<f32>) -> @builtin(position) vec4<f32> { return vec4<f32>(q, 1.0); }",
    )
    .unwrap_err();
    assert!(
        e.contains("`FooBarInput`") && e.contains("already generated"),
        "{e}"
    );
    // A struct named like the entry-point module.
    let e = reflect(
        "em",
        "struct entry { a: f32 }\n@group(0) @binding(0) var<uniform> e: entry;\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(e.a); }",
    )
    .unwrap_err();
    assert!(e.contains("`entry`") && e.contains("collides"), "{e}");
    // A binding named like the generated name const.
    let e = reflect(
        "x",
        &format!("@group(0) @binding(0) var<uniform> name: f32;\n{FS}"),
    )
    .unwrap_err();
    assert!(e.contains("`NAME`") && e.contains("collides"), "{e}");
    // Two bindings whose upper-snake names agree.
    let e = reflect(
        "y",
        &format!("@group(0) @binding(0) var<uniform> a_b: f32;\n@group(0) @binding(1) var<uniform> aB: f32;\n{FS}"),
    )
    .unwrap_err();
    assert!(e.contains("`A_B`") && e.contains("collides"), "{e}");
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
fn a_shader_without_bindings_or_inputs_is_just_its_module_and_entries() {
    let g = reflect(
        "flat",
        &format!("@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{ return vec4<f32>(0.0); }}\n{FS}"),
    )
    .unwrap();
    assert!(g.contains("pub mod flat {"));
    assert!(g.contains("pub const FS: &str = \"fs\";"));
    assert!(
        !g.contains("BindGroupLayoutEntry") && !g.contains("pub struct"),
        "{g}"
    );
}

#[test]
fn generate_reads_a_directory_and_refuses_a_stem_that_is_not_an_identifier() {
    let dir = std::env::temp_dir().join(format!("exact-gpu-reflect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ok.wgsl"), FS).unwrap();
    std::fs::write(dir.join("bad-name.wgsl"), FS).unwrap();
    std::fs::write(dir.join("notes.txt"), "not a shader").unwrap();
    let e = generate(&dir).unwrap_err();
    assert!(
        e.contains("bad-name.wgsl") && e.contains("file stem"),
        "{e}"
    );
    std::fs::remove_file(dir.join("bad-name.wgsl")).unwrap();
    let g = generate(&dir).unwrap();
    assert_eq!(g.sources, vec![dir.join("ok.wgsl")]);
    assert!(g.rust.starts_with("// @generated by exact-gpu-reflect"));
    assert!(g.rust.contains("pub mod ok {"), "{}", g.rust);
    assert!(
        g.rust
            .contains("pub const SHADERS: &[(&str, u64)] = &[(\"ok\", 0x"),
        "{}",
        g.rust
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_interface_digest_ignores_the_text_and_follows_the_interface() {
    use exact_gpu_reflect::{interface_digest, reflect_with_digest};
    let (_, generated) = reflect_with_digest("scene", SCENE).unwrap();
    assert_eq!(
        generated,
        interface_digest(SCENE).unwrap(),
        "the runtime digest is the generated one"
    );
    // A body edit — a constant, a comment — is not an interface change.
    let color = SCENE
        .replace("* scene.time", "* scene.time * 0.5")
        .replace("struct Light", "// palette\nstruct Light");
    assert_ne!(color, SCENE);
    assert_eq!(interface_digest(&color).unwrap(), generated);
    // A binding an entry point uses is.
    let bound = SCENE
        .replace(
            "@group(1) @binding(2) var lut: texture_2d<f32>;",
            "@group(1) @binding(2) var lut: texture_2d<f32>;\n@group(0) @binding(9) var<uniform> extra: vec4<f32>;",
        )
        .replace("* f32(counts[0]) + t;", "* f32(counts[0]) + t + extra;");
    assert_ne!(interface_digest(&bound).unwrap(), generated);
    // So is a struct layout, a vertex input, and a fragment output.
    let layout = SCENE.replace(
        "struct Light { position: vec3<f32>, intensity: f32 }",
        "struct Light { intensity: f32, position: vec3<f32> }",
    );
    assert_ne!(interface_digest(&layout).unwrap(), generated);
    let input = SCENE.replace("@location(2) uv: vec2<f32>", "@location(3) uv: vec2<f32>");
    assert_ne!(interface_digest(&input).unwrap(), generated);
    let output = SCENE.replace(
        "@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {",
        "@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(1) vec4<f32> {",
    );
    assert_ne!(interface_digest(&output).unwrap(), generated);
    // A shader that does not validate has no digest, and says where.
    let e = interface_digest("@fragment fn fs() -> @location(0) vec4<f32> {\n    return 1;\n}")
        .unwrap_err();
    assert!(e.contains(":2:"), "{e}");
}
