// A model material's surface inputs, shared by the engine's model shader (model.wgsl
// binds the material at group 2) and game custom materials (custom_instance.wgsl,
// group 3, in hooks::MATERIAL_WGSL). Each declares `baked` and the five textures.
struct BakedMaterial {
    base: vec4<f32>, surface: vec4<f32>, emission_cutoff: vec4<f32>, flags: vec4<f32>,
    uv: array<vec4<f32>, 10>,
}
// tint: NodeMaterials/MaterialOverrides base colour multiplier; glow: added emission,
// w = 1 when the tint replaces the material's base colour factor.
struct ModelVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(4) color: vec4<f32>,
    @location(2) uv: vec2<f32>, @location(3) @interpolate(flat) slot: u32,
    @location(5) @interpolate(flat) tint: vec4<f32>, @location(6) @interpolate(flat) glow: vec4<f32>,
}
fn material_uv(uv: vec2<f32>, index:u32) -> vec2<f32> {
    let t=baked.uv[index*2u]; let o=baked.uv[index*2u+1u];
    return mat2x2(t.xy,t.zw)*uv+o.xy;
}
// The base colour the engine shades: the base texture through its UV transform,
// times the material's factor, vertex colour, entity colour and instance tint.
fn model_base(input:ModelVarying) -> vec4<f32> {
    let i=input.slot*12u;
    return textureSample(base_texture,base_sampler,material_uv(input.uv,0u))*select(baked.base,vec4(1.0),input.glow.w>0.5)*input.color*vec4(materials[i],materials[i+1u],materials[i+2u],select(materials[i+3u],1.0,materials[i+3u]<0.0))*input.tint;
}
// Whether the engine's model shader drops this fragment: an Opacity dither (opaque
// and masked materials; blended ones multiply alpha instead), or base alpha below
// a MASK material's cutoff. The texture is sampled before the per-pixel dither
// test, in uniform control flow (Tint refuses it after a varying branch).
fn model_discarded(input:ModelVarying) -> bool {
    let cut=baked.flags.x==1.0 && model_base(input).a < baked.emission_cutoff.w;
    return cut || (baked.flags.x!=2.0 && faded(input.slot,input.clip.xy));
}
