// Light shafts (water.rs): sunlight scattered by the water between the camera
// and what each pixel sees, added to the resolved HDR colour before bloom.
// Each pixel marches across the screen towards the sun as it shines under the
// water, through what the forward pass's depth shows of the surface: where the
// surface is in sight its light passes, broken by the waves; where something
// stands in front of it, a shadow streak falls. Drawn only from below.
@group(0) @binding(1) var depth: texture_depth_multisampled_2d;
const SHAFT_SAMPLES: u32 = 24u;
const SHAFT_DECAY: f32 = 0.96;
// Forward scattering, Henyey-Greenstein with g = 0.6, as 1 looking along the light.
const SHAFT_G: f32 = 0.6;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
fn shaft_ndc(pixel: vec2<f32>) -> vec2<f32> {
    return pixel / frame.logical_size.xy * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
}
fn shaft_point(ndc: vec2<f32>, z: f32) -> vec3<f32> {
    let p = frame.inverse_view_proj * vec4(ndc, z, 1.0);
    return p.xyz / p.w;
}
// The world point a pixel's depth holds; `far` along its ray where nothing drew.
fn shaft_scene(pixel: vec2<f32>, far: f32) -> vec3<f32> {
    let ndc = shaft_ndc(pixel);
    let raw = textureLoad(depth, vec2<i32>(pixel), 0);
    let eye = frame.camera_alpha.xyz;
    if raw >= 1.0 { return eye + normalize(shaft_point(ndc, 0.5) - eye) * far; }
    // World depth fills [split, 1] of the range while a viewmodel draws.
    let split = frame.water.w;
    return shaft_point(ndc, max(raw - split, 0.0) / (1.0 - split));
}
fn shaft_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let c = vec2<u32>(vec2<i32>(i) + 65536);
    let a = caustic_hash(c).x;
    let b = caustic_hash(c + vec2(1u, 0u)).x;
    let d = caustic_hash(c + vec2(0u, 1u)).x;
    let e = caustic_hash(c + vec2(1u, 1u)).x;
    return mix(mix(a, b, u.x), mix(d, e, u.x), u.y);
}
// The light the surface lets through at a screen position: 0 where the ray
// never meets it or something nearer hides it.
fn shaft_source(pixel: vec2<f32>) -> f32 {
    let eye = frame.camera_alpha.xyz;
    let ndc = shaft_ndc(pixel);
    let d = normalize(shaft_point(ndc, 0.5) - eye);
    if d.y <= 0.0 { return 0.0; }
    let t = (frame.water.x - eye.y) / d.y;
    let size = frame.logical_size.xy;
    if all(pixel >= vec2(0.0)) && all(pixel < size) {
        if distance(shaft_scene(pixel, 1.0e4), eye) < t * 0.99 - 0.02 { return 0.0; }
    }
    let hit = (eye + d * t).xz / max(frame.water_light.x, 0.01);
    let n = 0.6 * shaft_noise(hit * 2.3 + frame.water_scroll.xy)
        + 0.4 * shaft_noise(hit * 4.1 + frame.water_scroll.zw);
    return smoothstep(0.35, 0.65, n);
}
@fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = position.xy;
    let eye = frame.camera_alpha.xyz;
    let scene = shaft_scene(pixel, 1.0e4);
    let scattered = 1.0 - fog_transmission(scene);
    let sun = frame.sun_direction_illuminance;
    // The sun's light under a flat surface, and the screen direction towards it
    // through this pixel: towards its vanishing point when that is ahead,
    // away from the opposite one when behind.
    let travel = refract(sun.xyz, vec3(0.0, 1.0, 0.0), 1.0 / WATER_IOR);
    let towards = frame.view_proj * vec4(-travel, 0.0);
    let ndc = shaft_ndc(pixel);
    let screen = (towards.xy - ndc * towards.w) * vec2(1.0, -1.0) * frame.logical_size.xy;
    if scattered <= 0.0 || sun.w <= 0.0 || dot(screen, screen) < 1.0e-12 { return vec4(0.0); }
    let step = normalize(screen) * frame.water_light.z * frame.logical_size.y / f32(SHAFT_SAMPLES);
    var light = 0.0;
    var weight = 1.0;
    var total = 0.0;
    for (var i = 0u; i < SHAFT_SAMPLES; i++) {
        light += weight * shaft_source(pixel + step * (f32(i) + 0.5));
        total += weight;
        weight *= SHAFT_DECAY;
    }
    let view = normalize(scene - eye);
    let g2 = SHAFT_G * SHAFT_G;
    let phase = pow((1.0 + g2 - 2.0 * SHAFT_G) / (1.0 + g2 - 2.0 * SHAFT_G * dot(view, -travel)), 1.5);
    let radiance = frame.sun_color_count.xyz * sun.w * frame.water_light.y * phase * scattered * light / total;
    return vec4(finite_hdr(radiance), 0.0);
}
