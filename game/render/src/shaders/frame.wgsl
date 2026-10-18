override MATTE: bool = false;
// The float of surface colour arithmetic: f32 here, and f16 in the forward
// modules a device with SHADER_F16 compiles (pipeline.rs `half_precision`),
// where a specular peak is capped at SHADING_LIMIT to stay finite in half.
// Positions, depth, directions, texture coordinates and shadows stay f32.
alias shading = f32; const SHADING_LIMIT = 3.0e38;
struct Frame {
    view_proj: mat4x4<f32>,
    camera_alpha: vec4<f32>,
    sun_direction_illuminance: vec4<f32>,
    sun_color_count: vec4<f32>,
    zenith_ambient: vec4<f32>,
    ground_exposure: vec4<f32>,
    // A second, unshadowed directional light (lux-scaled like the sun).
    fill_direction_illuminance: vec4<f32>,
    fill_color: vec4<f32>,
    // Local lights (lights.wgsl): count, camera near, slices per log2 metre, grid word offset.
    lights_info: vec4<f32>,
    inverse_view_proj: mat4x4<f32>,
    view_depth: vec4<f32>,
    horizon_disc: vec4<f32>,
    fog_color_density: vec4<f32>,
    height_bloom: vec4<f32>,
    shadow_matrices: array<mat4x4<f32>, 3>,
    splits_count: vec4<f32>,
    texels_softness: vec4<f32>,
    shadow_near: vec4<f32>,
    logical_size: vec4<f32>,
    // SH9 diffuse irradiance / π, premultiplied (ibl.rs, ibl.wgsl).
    irradiance: array<vec4<f32>, 9>,
    // An environment map drawn as the sky: intensity (0: the procedural sky),
    // yaw about +Y, RGBM range.
    sky: vec4<f32>,
    // Linear fog near/far, metres per CSS pixel at unit depth, orthographic.
    toon: vec4<f32>,
    // Filter, normalized depth bias, normal bias in texels, shadow strength.
    shadow_filter: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
fn environment(y: f32) -> vec3<f32> {
    return mix(frame.horizon_disc.xyz,
        select(frame.ground_exposure.xyz, frame.zenith_ambient.xyz, y >= 0.0), abs(y));
}

// Inspect the bits: fast-math backends may fold floating-point NaN comparisons.
// 65472 is the largest half-float strictly below 65504.
fn finite_hdr(c: vec3<f32>) -> vec3<f32> {
    let nan = (bitcast<vec3<u32>>(c) & vec3(0x7fffffffu)) > vec3(0x7f800000u);
    return clamp(select(c, vec3(0.0), nan), vec3(0.0), vec3(65472.0));
}

// Fog over `color` at `end`: linear in view depth, else exponential with height.
fn height_fog(color: vec3<f32>, end: vec3<f32>) -> vec3<f32> {
    if frame.toon.y > frame.toon.x {
        let depth = dot(frame.view_depth, vec4(end, 1.0));
        return mix(color, frame.fog_color_density.xyz, smoothstep(frame.toon.x, frame.toon.y, depth));
    }
    return mix(frame.fog_color_density.xyz, color, fog_transmission(end));
}
// height_fog in the surface colour's float: the depth and density in f32.
fn shaded_fog(color: vec3<shading>, end: vec3<f32>) -> vec3<shading> {
    let fog = vec3<shading>(frame.fog_color_density.xyz);
    if frame.toon.y > frame.toon.x {
        let depth = dot(frame.view_depth, vec4(end, 1.0));
        return mix(color, fog, shading(smoothstep(frame.toon.x, frame.toon.y, depth)));
    }
    return mix(fog, color, shading(fog_transmission(end)));
}
// Analytic integral of exponential height density along the eye-to-end segment.
// Shared by surfaces and sky; the near-horizontal limit avoids cancellation.
fn fog_transmission(end: vec3<f32>) -> f32 {
    let falloff = frame.height_bloom.x;
    let a = -falloff * frame.camera_alpha.y;
    let difference = -falloff * (end.y - frame.camera_alpha.y);
    let b = -falloff * end.y;
    let start_density = exp(clamp(a, -40.0, 40.0));
    let end_density = exp(clamp(b, -40.0, 40.0));
    var average = start_density;
    if abs(difference) > 0.001 { average = (end_density - start_density) / difference; }
    return exp(-frame.fog_color_density.w * distance(end, frame.camera_alpha.xyz) * average);
}
