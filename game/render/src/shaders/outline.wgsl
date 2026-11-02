struct Hull { width:f32, pixels:f32, base:u32, pad:u32, color:vec4<f32> }
// Tonemapped in place for a direct frame, as forward.wgsl's DIRECT.
override DIRECT: bool = false;
// Some slot of the batch may be fading; false where none is, so no fragment
// can be discarded and depth is tested before shading.
override FADES: bool = true;
@group(2) @binding(0) var<uniform> hull: Hull;
@vertex fn outline_vs(@location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) color:vec4<f32>, @location(4) outline_normal:vec3<f32>, @location(5) outline_reach:vec3<f32>, @builtin(instance_index) instance:u32, @builtin(vertex_index) vertex:u32) -> ModelVarying {
    var v=model_transform(position,outline_reach,uv,instance,vertex+hull.base,color);
    let offset=v.normal;
    let reach=length(offset);
    let depth=dot(frame.view_depth,vec4(v.world,1.0));
    let pixel=frame.toon.z*select(max(depth,0.0),1.0,frame.toon.w>0.5);
    let width=min(max(hull.width,hull.pixels*pixel),reach);
    v.world+=offset*(width/max(reach,1e-8));
    v.clip=frame.view_proj*vec4(v.world,1.0);
    return v;
}
@fragment fn outline_fs(v:ModelVarying) -> @location(0) vec4<f32> {
    if FADES && faded(v.slot,v.clip.xy) { discard; }
    let color=height_fog(hull.color.rgb,v.world);
    if DIRECT { return tonemap(color); }
    return vec4(color,1.0);
}
