// Clustered point and spot lights (lights.rs): records of 16 words, then a
// 16 x 9 x 24 grid of (first index word, count), then the clusters' light lists.
@group(0) @binding(8) var<storage, read> light_words: array<u32>;
const PI = 3.141592653589793;
// Directions and the lobes' scalars in f32; colour in `shading` (frame.wgsl).
fn brdf(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, base: vec3<shading>, metallic: f32,
        roughness: f32) -> vec3<shading> {
    let nl = max(dot(n, l), 0.0);
    if MATTE { return base * shading(nl) / PI; }
    let nv = max(dot(n, v), 0.0001);
    let sum = v + l;
    let h = sum * inverseSqrt(max(dot(sum, sum), 0.000001));
    let nh = max(dot(n, h), 0.0);
    let vh = max(dot(v, h), 0.0);
    let a = roughness * roughness;
    let a2 = a * a;
    let d0 = nh * nh * (a2 - 1.0) + 1.0;
    let distribution = a2 / (PI * d0 * d0);
    // Height-correlated Smith-GGX visibility, including 1/(4 NoL NoV).
    let gv = nl * sqrt(nv * nv * (1.0 - a2) + a2);
    let gl = nv * sqrt(nl * nl * (1.0 - a2) + a2);
    let visibility = 0.5 / max(gv + gl, 0.00001);
    let f0 = mix(vec3<shading>(0.04), base, shading(metallic));
    // (1 - vh)^5 by multiplication: pow is two transcendental operations.
    let f = 1.0 - vh;
    let f2 = f * f;
    let fresnel = f0 + (vec3<shading>(1.0) - f0) * shading(f2 * f2 * f);
    let diffuse = (vec3<shading>(1.0) - fresnel) * shading(1.0 - metallic) * base / PI;
    let peak = shading(min(distribution * visibility, SHADING_LIMIT));
    return (diffuse + peak * fresnel) * shading(nl);
}
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
fn add_local_lights(color: ptr<function, vec3<shading>>, frag: vec2<f32>, world: vec3<f32>,
        n: vec3<f32>, v: vec3<f32>, base: vec3<shading>, metallic: f32, roughness: f32, model: bool) {
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
        var cone = light_cone(at, l);
        // Shadow maps are read only where the light reaches.
        if window > 0.0 && cone > 0.0 && dot(n, l) > 0.0 {
            cone *= light_visibility(at, world, n, l);
        }
        // Nothing to add where the window or cone is zero (in f16 an
        // overflowed product times zero would not be zero).
        if window <= 0.0 || cone <= 0.0 { continue; }
        let tint = vec3<shading>(light_v(at + 4u));
        let intensity = shading(light_f(at + 7u));
        let w = shading(window);
        let d2 = shading(distance2);
        let c = shading(cone);
        if model {
            *color += brdf(n, v, l, base, metallic, roughness) * tint * intensity * w * w / d2 * c;
        } else {
            let radiance = tint * intensity * w * w / d2;
            *color += brdf(n, v, l, base, metallic, roughness) * radiance * c;
        }
    }
}
// The second directional light, unshadowed.
fn fill_light(n: vec3<f32>, v: vec3<f32>, base: vec3<shading>, metallic: f32, roughness: f32) -> vec3<shading> {
    let l = (-frame.fill_direction_illuminance.xyz);
    return brdf(n, v, l, base, metallic, roughness) * vec3<shading>(frame.fill_color.xyz)
        * shading(frame.fill_direction_illuminance.w);
}
