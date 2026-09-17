// Same prefix layout as the frame block; only exposure is needed here.
struct Frame {
    view_proj: mat4x4<f32>, camera_alpha: vec4<f32>,
    sun_direction_illuminance: vec4<f32>, sun_color_count: vec4<f32>,
    sky_ambient: vec4<f32>, ground_exposure: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var hdr: texture_2d<f32>;
override ENCODE_SRGB: bool = true;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let x = max(textureLoad(hdr, vec2<i32>(p.xy), 0).rgb * frame.ground_exposure.w, vec3(0.0));
    // Narkowicz ACES-fitted curve, applied to linear scene RGB.
    var color = clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3(0.0), vec3(1.0));
    if ENCODE_SRGB {
        color = select(1.055 * pow(color, vec3(1.0 / 2.4)) - 0.055,
                       12.92 * color, color <= vec3(0.0031308));
    }
    return vec4(color, 1.0);
}
