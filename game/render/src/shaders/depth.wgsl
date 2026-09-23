// @ref llp/1046.006.000-render-hooks.rfc.md#d3-optional-services-the-scene-copy
struct DepthView { inverse_projection: mat4x4f, viewport: vec4f }
@group(0) @binding(0) var<uniform> u: DepthView;
@group(0) @binding(1) var depth: texture_depth_multisampled_2d;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) pixel: vec4f) -> @location(0) f32 {
    let p = vec2i(pixel.xy);
    var z = 1.0;
    for (var sample = 0u; sample < textureNumSamples(depth); sample++) {
        z = min(z, textureLoad(depth, p, i32(sample)));
    }
    if z >= 1.0 { return 0.0; }
    let ndc = pixel.xy / u.viewport.xy * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0);
    let position = u.inverse_projection * vec4f(ndc, z, 1.0);
    return max(0.0, -position.z / position.w);
}
