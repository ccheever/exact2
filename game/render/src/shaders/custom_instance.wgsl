// @ref llp/1046.006.000-render-hooks.rfc.md#d4-custom-materials-inside-the-engines-batches-phase-2
// Game custom materials (hooks::MATERIAL_WGSL): the engine's instance records and,
// at group 3, the shaded model material the engine's model shader binds at group 2,
// so model_base.wgsl's `model_base`/`model_discarded` and the textures work here.
struct DrawInstance {
    transform: u32, material: u32, data: u32, palette: u32,
    local: mat4x4f, normal: mat4x4f,
    // The record's tint and added emission (NodeMaterials, MaterialOverrides).
    // glow.w's top bit: the tint replaces the material's base colour factor.
    // Custom shaders draw unmerged, so its low bits (merged part looks) are zero.
    tint: vec4f, glow: vec4f,
    // A MaterialOverride's metallic, roughness and Shimmer (model_base.wgsl).
    surface: vec4u,
}
@group(3) @binding(0) var<storage, read> draw_instances: array<DrawInstance>;
@group(3) @binding(1) var<uniform> baked: BakedMaterial;
@group(3) @binding(2) var base_texture: texture_2d<f32>;
@group(3) @binding(3) var base_sampler: sampler;
@group(3) @binding(4) var normal_texture: texture_2d<f32>;
@group(3) @binding(5) var normal_sampler: sampler;
@group(3) @binding(6) var mr_texture: texture_2d<f32>;
@group(3) @binding(7) var mr_sampler: sampler;
@group(3) @binding(8) var emission_texture: texture_2d<f32>;
@group(3) @binding(9) var emission_sampler: sampler;
@group(3) @binding(10) var ao_texture: texture_2d<f32>;
@group(3) @binding(11) var ao_sampler: sampler;
fn draw_instance(i:u32) -> DrawInstance { return draw_instances[slots[i]-2147483648u]; }
// Same tick interpolation as engine geometry, with full affine node offsets.
fn instance_transform(position:vec3f, normal:vec3f, uv:vec2f, i:u32, color:vec4f) -> ModelVarying {
    let draw=draw_instance(i); let slot=draw.transform; let t=slot*10u;
    let alpha=frame.camera_alpha.w;
    let p=mix(vec3f(prev[t],prev[t+1u],prev[t+2u]),vec3f(curr[t],curr[t+1u],curr[t+2u]),alpha);
    let qa=vec4f(prev[t+3u],prev[t+4u],prev[t+5u],prev[t+6u]);
    let qb=vec4f(curr[t+3u],curr[t+4u],curr[t+5u],curr[t+6u]);
    let q=normalize(mix(qa,select(qb,-qb,dot(qa,qb)<0.0),alpha));
    let s=mix(vec3f(prev[t+7u],prev[t+8u],prev[t+9u]),vec3f(curr[t+7u],curr[t+8u],curr[t+9u]),alpha);
    let local=(draw.local*vec4f(position,1.0)).xyz;
    let n=(draw.normal*vec4f(normal,0.0)).xyz;
    var world=p+rotate(q,s*local); var wn=rotate(q,n/select(max(abs(s),vec3f(1e-6)),-max(abs(s),vec3f(1e-6)),s<vec3f(0.0)));
    if attached(slot) { let m=attachment_matrices[slot]; world=(m*vec4f(local,1.0)).xyz; wn=affine_normal(m,n); }
    var tint=draw.tint; var glow=draw.glow.xyz;
    shimmered(draw.surface,&tint,&glow);
    let look=vec4f(glow,f32(bitcast<u32>(draw.glow.w)>>31u));
    return ModelVarying(frame.view_proj*vec4f(world,1.0),world,wn,color,uv,slot,tint,look,look_surface(draw.surface));
}
