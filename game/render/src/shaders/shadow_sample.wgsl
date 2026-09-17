@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
fn cascade_sample(world: vec3<f32>, normal: vec3<f32>, index: u32) -> f32 {
    let l = normalize(-frame.sun_direction_illuminance.xyz);
    let cosine = clamp(dot(normal, l), 0.0, 1.0);
    if cosine <= 0.0 { return 1.0; }
    let sine = sqrt(max(0.0, 1.0 - cosine * cosine));
    let texel = frame.texels_softness[index];
    // Metres, independently scaled for each cascade. No caster displacement:
    // single-sided and thin geometry retain their depth and contact.
    let offset = 0.5 * texel * sine;
    let matrix = frame.shadow_matrices[index];
    let clip = matrix * vec4(world + normal * offset, 1.0);
    let rows = transpose(matrix);
    let depth_scale = length(rows[2].xyz);
    let light_normal = vec2(dot(normal, normalize(rows[0].xyz)),
                            -dot(normal, normalize(rows[1].xyz)));
    // Receiver-plane depth change per shadow texel. Correct each PCF tap for
    // its position on the plane; only the comparison sampler's bilinear footprint
    // needs conservative slope bias, regardless of the authored PCF radius.
    let slope = light_normal / max(cosine, 0.0001) * texel * depth_scale;
    let bias = dot(abs(slope), vec2(1.0)) + 0.000001;
    let uv = clip.xy * vec2(0.5, -0.5) + 0.5;
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) || clip.z < 0.0 || clip.z > 1.0 { return 1.0; }
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let tap = vec2(f32(x), f32(y)) * frame.texels_softness.w;
            let depth = clip.z + dot(slope, tap) - bias;
            sum += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + tap / 2048.0, i32(index), depth);
        }
    }
    return sum / 9.0;
}
fn sun_visibility(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    let depth = dot(frame.view_depth, vec4(world, 1.0));
    let count = u32(frame.splits_count.w);
    let distance = frame.splits_count[count - 1u];
    if depth >= distance { return 1.0; }
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
    return visibility;
}
