@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(1) @binding(1) var bloom: texture_2d<f32>;
@group(1) @binding(2) var linear_sampler: sampler;
override ENCODE_SRGB: bool = true;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
fn tonemap(hdr_color: vec3<f32>) -> vec4<f32> {
    let exposed = finite_hdr(finite_hdr(hdr_color) * frame.ground_exposure.w);
    // A soft HDR shoulder stays below display white even at full bloom.
    // Values <= 2 retain the existing diffuse/exposure response.
    let excess = max(exposed - 2.0, vec3(0.0));
    let x = min(exposed, vec3(2.0)) + excess / (1.0 + excess * 0.5);
    var color = clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3(0.0), vec3(1.0));
    if ENCODE_SRGB {
        color = select(1.055 * pow(color, vec3(1.0 / 2.4)) - 0.055,
                       12.92 * color, color <= vec3(0.0031308));
    }
    return vec4(color, 1.0);
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
