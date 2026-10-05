// A model material's surface inputs, shared by the engine's model shader (model.wgsl
// binds the material at group 2) and game custom materials (custom_instance.wgsl,
// group 3, in hooks::MATERIAL_WGSL). Each declares `baked` and the five textures.
struct BakedMaterial {
    base: vec4<f32>, surface: vec4<f32>, emission_cutoff: vec4<f32>, flags: vec4<f32>,
    uv: array<vec4<f32>, 10>,
}
// tint: NodeMaterials/MaterialOverrides base colour multiplier; glow: added emission,
// w = 1 when the tint replaces the material's base colour factor; surface: a
// MaterialOverride's metallic and roughness factors (negative keeps the material's).
struct ModelVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(4) color: vec4<f32>,
    @location(2) uv: vec2<f32>, @location(3) @interpolate(flat) slot: u32,
    @location(5) @interpolate(flat) tint: vec4<f32>, @location(6) @interpolate(flat) glow: vec4<f32>,
    @location(7) @interpolate(flat) surface: vec2<f32>,
}
// A record's surface words (models/looks.rs `surface_words`): the override's
// metallic and roughness, each -1 when not set.
fn look_surface(look:vec4<u32>) -> vec2<f32> {
    let flags=(look.xx>>vec2(16u,17u))&vec2(1u);
    let value=vec2<f32>((look.xx>>vec2(0u,8u))&vec2(255u))/255.0;
    return select(vec2(-1.0),value,flags==vec2(1u));
}
fn shimmer_hash(v:u32) -> f32 {
    let s=v*747796405u+2891336453u;
    let w=((s>>((s>>28u)+4u))^s)*277803737u;
    return f32((w>>22u)^w)/4294967295.0;
}
// A Shimmer's hue turn, on the colour's square root (see `Shimmer::Hue`).
fn shimmer_hue(c:vec3<f32>, turns:f32) -> vec3<f32> {
    let s=sqrt(max(c,vec3(0.0)));
    let high=max(s.r,max(s.g,s.b)); let d=high-min(s.r,min(s.g,s.b));
    if d<=0.0 { return c; }
    var h=(s.r-s.g)/d+4.0;
    if high==s.r { h=(s.g-s.b)/d; } else if high==s.g { h=(s.b-s.r)/d+2.0; }
    let k=(vec3(5.0,3.0,1.0)+fract(h/6.0+turns)*6.0)%6.0;
    let rgb=high-d*clamp(min(k,4.0-k),vec3(0.0),vec3(1.0));
    return rgb*rgb;
}
// A record's tint and emission as its Shimmer moves them at the displayed time.
fn shimmered(look:vec4<u32>, tint:ptr<function,vec4<f32>>, glow:ptr<function,vec3<f32>>) {
    let wave=(look.x>>24u)&3u;
    if wave==0u { return; }
    let phase=bitcast<f32>(look.z);
    let at=bitcast<f32>(look.y)*frame.time.x+phase;
    let low=f32(look.w&65535u)/4096.0; let high=f32(look.w>>16u)/4096.0;
    if wave==1u { *glow*=mix(low,high,0.5+0.5*sin(at*6.2831853)); }
    else if wave==2u { *glow*=mix(low,high,shimmer_hash(u32(i32(floor(at)))^(bitcast<u32>(phase)*2654435769u))); }
    else { *tint=vec4(shimmer_hue((*tint).rgb,at),(*tint).a); *glow=shimmer_hue(*glow,at); }
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
