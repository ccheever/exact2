@group(1) @binding(0) var<uniform> light_view_proj: mat4x4<f32>;
struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) slot: u32,
}
@vertex fn vs_shadow(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) cap: vec2<f32>,
    @builtin(instance_index) instance: u32) -> ShadowOut {
    let v = transform(position, normal, cap, instance, vec4(1.0));
    return ShadowOut(light_view_proj * vec4(v.world, 1.0), v.slot);
}
// A primitive faded by Opacity drops the same share of its shadow's texels.
@fragment fn fs_shadow(input: ShadowOut) {
    if faded(input.slot, input.clip.xy) { discard; }
}
