@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(1) @binding(1) var bloom: texture_2d<f32>;
@group(1) @binding(2) var linear_sampler: sampler;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return tonemap(finite_hdr(textureLoad(hdr, vec2<i32>(p.xy), 0).rgb));
}
@fragment fn fs_bloom(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let logical = max(floor(frame.logical_size.xy / 2.0), vec2(1.0));
    let pixel = clamp(p.xy / frame.logical_size.xy * logical, vec2(0.5), logical - 0.5);
    let uv = pixel / vec2<f32>(textureDimensions(bloom));
    let glow = finite_hdr(textureSampleLevel(bloom, linear_sampler, uv, 0.0).rgb) * frame.height_bloom.z;
    return tonemap(finite_hdr(textureLoad(hdr, vec2<i32>(p.xy), 0).rgb) + glow);
}
