@group(1) @binding(0) var<uniform> light_view_proj: mat4x4<f32>;
@vertex fn vs_shadow(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>,
    @builtin(instance_index) instance: u32) -> @builtin(position) vec4<f32> {
    return light_view_proj * vec4(transform(position, normal, instance).world, 1.0);
}
