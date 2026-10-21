@group(1) @binding(0) var<uniform> light_view_proj:mat4x4<f32>;
@vertex fn model_shadow_vs(@location(0) position:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) color:vec4<f32>,@builtin(instance_index) instance:u32,@builtin(vertex_index) vertex:u32)->ModelVarying {
    var out=model_transform(position,normal,uv,instance,vertex,color);
    out.clip=light_view_proj*vec4(out.world,1.0); return out;
}
@fragment fn model_shadow_fs(input:ModelVarying) {
    if faded(input.slot,input.clip.xy) { discard; }
    if baked.flags.x==1.0 && model_base(input).a < baked.emission_cutoff.w { discard; }
}
