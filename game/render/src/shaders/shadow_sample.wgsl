@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
// Local light shadows (local_shadows.rs): a spot's layer, or a point light's six.
@group(1) @binding(2) var local_shadow_map: texture_depth_2d_array;
// At steep slopes a single comparison depth cannot represent a bilinear
// footprint without erasing nearby casters. Compare each texel to its own
// receiver-plane depth, then interpolate the four visibility values.
fn plane_sample(uv: vec2<f32>, depth: f32, slope: vec2<f32>, index: u32) -> f32 {
    let pixel = uv * 2048.0 - 0.5;
    let base = floor(pixel);
    let weight = fract(pixel);
    var sum = 0.0;
    for (var y = 0; y <= 1; y++) {
        for (var x = 0; x <= 1; x++) {
            let corner = vec2(f32(x), f32(y));
            let coord = clamp(vec2<i32>(base + corner), vec2(0), vec2(2047));
            let reference = depth + dot(slope, vec2<f32>(coord) - pixel) - 0.000001;
            let stored = textureLoad(shadow_map, coord, i32(index), 0);
            let w = mix(vec2(1.0) - weight, weight, corner);
            sum += select(0.0, 1.0, reference <= stored) * w.x * w.y;
        }
    }
    return sum;
}
fn cascade_sample(world: vec3<f32>, normal: vec3<f32>, index: u32) -> f32 {
    let l = normalize(-frame.sun_direction_illuminance.xyz);
    let cosine = clamp(dot(normal, l), 0.0, 1.0);
    if cosine <= 0.005 { return 0.0; }
    let sine = sqrt(max(0.0, 1.0 - cosine * cosine));
    let texel = frame.texels_softness[index];
    // Metres, independently scaled for each cascade. No caster displacement:
    // single-sided and thin geometry retain their depth and contact.
    // The intersection's displacement along the receiver is offset*tan(theta).
    // Multiplying by cosine bounds that displacement to half a world texel.
    let offset = 0.5 * texel * sine * cosine;
    let matrix = frame.shadow_matrices[index];
    let clip = matrix * vec4(world + normal * offset, 1.0);
    let rows = transpose(matrix);
    let depth_scale = length(rows[2].xyz);
    let light_normal = vec2(dot(normal, normalize(rows[0].xyz)),
                            -dot(normal, normalize(rows[1].xyz)));
    // Receiver-plane depth change per shadow texel. Correct each PCF tap for
    // its position on the plane; only the comparison sampler's bilinear footprint
    // needs conservative slope bias, regardless of the authored PCF radius.
    let slope = light_normal / cosine * texel * depth_scale;
    let footprint = dot(abs(slope), vec2(1.0));
    let budget = 2.0 * texel * depth_scale;
    let bias = min(footprint, budget) + 0.000001;
    // Project a receiver-plane filter into the light map. An isotropic light-map
    // kernel stretches by 1/N.L on the receiver and washes out dusk contacts.
    let axis = light_normal / max(sine, 0.000001);
    let uv = clip.xy * vec2(0.5, -0.5) + 0.5;
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) || clip.z < 0.0 || clip.z > 1.0 { return 1.0; }
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let grid = vec2(f32(x), f32(y)) * frame.texels_softness.w;
            let tap = grid - axis * dot(grid, axis) * (1.0 - cosine);
            let depth = clip.z + dot(slope, tap);
            let tap_uv = uv + tap / 2048.0;
            if footprint > budget {
                sum += plane_sample(tap_uv, depth, slope, index);
            } else {
                sum += textureSampleCompareLevel(shadow_map, shadow_sampler, tap_uv, i32(index), depth - bias);
            }
        }
    }
    return sum / 9.0;
}
fn sun_visibility(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    // A plane is singular at N.L=0. Fade direct illumination to zero through
    // the unsupported interval, including cascade/distance fades; never to lit.
    let cosine = dot(normal, normalize(-frame.sun_direction_illuminance.xyz));
    let support = smoothstep(0.005, 0.01, cosine);
    if support == 0.0 { return 0.0; }
    let depth = dot(frame.view_depth, vec4(world, 1.0));
    let count = u32(frame.splits_count.w);
    let distance = frame.splits_count[count - 1u];
    if depth >= distance { return support; }
    var index = 0u;
    for (var i = 0u; i + 1u < count; i++) {
        if depth > frame.splits_count[i] { index = i + 1u; }
    }
    let end = frame.splits_count[index];
    var start = frame.shadow_near.x;
    if index > 0u { start = frame.splits_count[index - 1u]; }
    let blend = smoothstep(end - (end - start) * 0.1, end, depth);
    var visibility = cascade_sample(world, normal, index);
    if blend > 0.0 {
        var next = 1.0;
        if index + 1u < count { next = cascade_sample(world, normal, index + 1u); }
        visibility = mix(visibility, next, blend);
    }
    return visibility * support;
}

// A local light's shadow at `world` (lights.wgsl record words 13-15: first layer,
// matrix word offset, texel width per metre). A point light picks its cube face
// by the major axis from the light; 3x3 PCF in that face's map.
fn light_visibility(at: u32, world: vec3<f32>, n: vec3<f32>, l: vec3<f32>) -> f32 {
    let layer = light_f(at + 13u);
    if layer < 0.0 { return 1.0; }
    let from_light = -l;
    var face = 0u;
    if light_f(at + 11u) < -1.5 {
        let a = abs(from_light);
        if a.x >= a.y && a.x >= a.z { face = select(1u, 0u, from_light.x > 0.0); }
        else if a.y >= a.z { face = select(3u, 2u, from_light.y > 0.0); }
        else { face = select(5u, 4u, from_light.z > 0.0); }
    }
    let m = u32(light_f(at + 14u)) + face * 16u;
    let view = mat4x4(
        vec4(light_f(m), light_f(m + 1u), light_f(m + 2u), light_f(m + 3u)),
        vec4(light_f(m + 4u), light_f(m + 5u), light_f(m + 6u), light_f(m + 7u)),
        vec4(light_f(m + 8u), light_f(m + 9u), light_f(m + 10u), light_f(m + 11u)),
        vec4(light_f(m + 12u), light_f(m + 13u), light_f(m + 14u), light_f(m + 15u)));
    // Normal offset of one and a half texels at this distance, less towards the light.
    let cosine = clamp(dot(n, l), 0.0, 1.0);
    let texel = distance(world, light_v(at)) * light_f(at + 15u);
    let offset = n * texel * 1.5 * sqrt(max(1.0 - cosine * cosine, 0.0)) + l * texel * 0.5;
    let clip = view * vec4(world + offset, 1.0);
    if clip.w <= 0.0 { return 1.0; }
    let ndc = clip.xyz / clip.w;
    let uv = ndc.xy * vec2(0.5, -0.5) + 0.5;
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) || ndc.z > 1.0 { return 1.0; }
    let size = vec2<f32>(textureDimensions(local_shadow_map));
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            sum += textureSampleCompareLevel(local_shadow_map, shadow_sampler,
                uv + vec2(f32(x), f32(y)) / size, i32(u32(layer) + face), ndc.z - 0.00002);
        }
    }
    return sum / 9.0;
}
