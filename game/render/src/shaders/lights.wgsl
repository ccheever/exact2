// Clustered point and spot lights (lights.rs): records of 16 words, then a
// 16 x 9 x 24 grid of (first index word, count), then the clusters' light lists.
@group(0) @binding(8) var<storage, read> light_words: array<u32>;
fn light_f(at: u32) -> f32 { return bitcast<f32>(light_words[at]); }
fn light_v(at: u32) -> vec3<f32> { return vec3(light_f(at), light_f(at + 1u), light_f(at + 2u)); }
// The fragment's cluster: its screen tile and exponential view-depth slice.
fn light_cluster(frag: vec2<f32>, world: vec3<f32>) -> u32 {
    let tile = min(vec2<u32>(max(frag * vec2(16.0, 9.0) / frame.logical_size.xy, vec2(0.0))), vec2(15u, 8u));
    let depth = max(dot(frame.view_depth, vec4(world, 1.0)), 1e-6);
    let slice = u32(clamp(floor(log2(depth / frame.lights_info.y) * frame.lights_info.z), 0.0, 23.0));
    return (slice * 9u + tile.y) * 16u + tile.x;
}
// A spot's cone: full inside the inner cosine, smoothly zero at the outer.
fn light_cone(at: u32, l: vec3<f32>) -> f32 {
    let outer = light_f(at + 11u);
    if outer < -1.5 { return 1.0; }
    return smoothstep(outer, light_f(at + 12u), dot(-l, light_v(at + 8u)));
}
// Add each of the fragment's cluster lights to `color`, in frame order. The
// model shader's historical evaluation order is kept for its pixels.
fn add_local_lights(color: ptr<function, vec3<f32>>, frag: vec2<f32>, world: vec3<f32>,
        n: vec3<f32>, v: vec3<f32>, base: vec3<f32>, metallic: f32, roughness: f32, model: bool) {
    if frame.lights_info.x == 0.0 { return; }
    let cell = u32(frame.lights_info.w) + light_cluster(frag, world) * 2u;
    let first = light_words[cell];
    let count = light_words[cell + 1u];
    for (var k = 0u; k < count; k++) {
        let at = light_words[first + k] * 16u;
        let delta = light_v(at) - world;
        let distance2 = max(dot(delta, delta), 0.0001);
        let range = max(light_f(at + 3u), 0.0001);
        let ratio2 = distance2 / (range * range);
        let window = max(1.0 - ratio2 * ratio2, 0.0);
        let l = delta * inverseSqrt(distance2);
        let cone = light_cone(at, l) * light_visibility(at, world, n, l);
        let tint = light_v(at + 4u);
        let intensity = light_f(at + 7u);
        if model {
            *color += brdf(n, v, l, base, metallic, roughness) * tint * intensity * window * window / distance2 * cone;
        } else {
            let radiance = tint * intensity * window * window / distance2;
            *color += brdf(n, v, l, base, metallic, roughness) * radiance * cone;
        }
    }
}
// The second directional light, unshadowed.
fn fill_light(n: vec3<f32>, v: vec3<f32>, base: vec3<f32>, metallic: f32, roughness: f32) -> vec3<f32> {
    let l = normalize(-frame.fill_direction_illuminance.xyz);
    return brdf(n, v, l, base, metallic, roughness) * frame.fill_color.xyz * frame.fill_direction_illuminance.w;
}
// Unshadowed until local shadow maps exist.
fn light_visibility(at: u32, world: vec3<f32>, n: vec3<f32>, l: vec3<f32>) -> f32 { return 1.0; }
