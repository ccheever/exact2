// Soft particles: fade where a quad nears the opaque scene behind it, over
// `cutoff` metres along the view ray. Appended to particle.wgsl or sprite.wgsl.
struct Soft {
    inverse_view_proj: mat4x4<f32>,
    // The world layer's depth range starts at x (after the viewmodel layer).
    split: vec4<f32>,
}
@group(2) @binding(0) var scene_depth: texture_depth_multisampled_2d;
@group(2) @binding(1) var<uniform> soft: Soft;
@fragment fn quad_soft_fs(v: QuadOut) -> @location(0) vec4<f32> {
    var c = quad_color(v);
    let d = textureLoad(scene_depth, vec2<i32>(floor(v.clip.xy)), 0);
    let clip = frame.view_proj * vec4(v.world, 1.0);
    let ndc = vec3(clip.xy / clip.w, (d - soft.split.x) / (1.0 - soft.split.x));
    let h = soft.inverse_view_proj * vec4(ndc, 1.0);
    let eye = frame.camera_alpha.xyz;
    let gap = length(h.xyz / h.w - eye) - length(v.world - eye);
    c.a *= clamp(gap / max(v.cutoff, 1e-4), 0.0, 1.0);
    return c;
}
