struct Point { position_range: vec4<f32>, color_intensity: vec4<f32> }
struct Frame {
    view_proj: mat4x4<f32>,
    camera_alpha: vec4<f32>,
    sun_direction_illuminance: vec4<f32>,
    sun_color_count: vec4<f32>,
    zenith_ambient: vec4<f32>,
    ground_exposure: vec4<f32>,
    points: array<Point, 16>,
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

// Analytic integral of exponential height density along the eye-to-end segment.
// Shared by surfaces and sky; the near-horizontal limit avoids cancellation.
fn height_fog(color: vec3<f32>, end: vec3<f32>) -> vec3<f32> {
    let falloff = frame.height_bloom.x;
    let a = -falloff * frame.camera_alpha.y;
    let difference = -falloff * (end.y - frame.camera_alpha.y);
    let b = -falloff * end.y;
    let start_density = exp(clamp(a, -40.0, 40.0));
    let end_density = exp(clamp(b, -40.0, 40.0));
    var average = start_density;
    if abs(difference) > 0.001 { average = (end_density - start_density) / difference; }
    let transmission = exp(-frame.fog_color_density.w * distance(end, frame.camera_alpha.xyz) * average);
    return mix(frame.fog_color_density.xyz, color, transmission);
}
