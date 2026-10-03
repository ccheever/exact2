override FOG: bool = false;
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) cap: vec2<f32>, @location(3) color: vec4<f32>,
    @builtin(instance_index) instance: u32) -> Varying {
    return transform(position, normal, cap, instance, color);
}
@fragment fn fs(input: Varying) -> @location(0) vec4<f32> { return shade(input, 1.0); }
@fragment fn fs_shadow(input: Varying) -> @location(0) vec4<f32> {
    return shade(input, sun_visibility(input.world, normalize(input.normal)));
}
@diagnostic(off, derivative_uniformity)
fn shade(input: Varying, visibility: f32) -> vec4<f32> {
    let i = input.slot * 12u;
    var base = vec3(materials[i], materials[i+1u], materials[i+2u]) * input.color.rgb;
    // Opaque materials reuse alpha as a negative grid-spacing flag: no wider uploads.
    let spacing = -materials[i+3u];
    if spacing > 0.0 {
        let cell = input.world / spacing;
        let footprint = max(fwidth(cell), vec3(0.0001));
        let distance = abs(fract(cell - 0.5) - 0.5);
        let lines = 1.0 - smoothstep(vec3(0.0), footprint * 1.25, distance);
        // Triplanar projection: omit each face's normal axis, so walls work too.
        let weight = pow(abs(normalize(input.normal)), vec3(8.0));
        let planes = vec3(max(lines.y, lines.z), max(lines.x, lines.z), max(lines.x, lines.y));
        let fade = 1.0 - smoothstep(0.2, 1.0, max(footprint.x, max(footprint.y, footprint.z)));
        // Integrated line coverage, including the union of both projected axes.
        // Minification converges to this lined mean, never the unlined base.
        let width = footprint * 1.25;
        let t = min(0.5 / width, vec3(1.0));
        let mean = 2.0 * width * (t - t*t*t + 0.5*t*t*t*t);
        let plane_mean = vec3(1.0) - vec3((1.0-mean.y)*(1.0-mean.z), (1.0-mean.x)*(1.0-mean.z), (1.0-mean.x)*(1.0-mean.y));
        let line = dot(mix(plane_mean, planes, fade), weight) / max(dot(weight, vec3(1.0)), 0.0001);
        base *= 1.0 + 0.35 * line;
    }
    let metallic = clamp(materials[i+4u], 0.0, 1.0);
    let roughness = clamp(materials[i+5u], 0.045, 1.0);
    let emissive = vec3(materials[i+6u], materials[i+7u], materials[i+8u]);
    let n = normalize(input.normal);
    let v = normalize(frame.camera_alpha.xyz - input.world);
    var color = ambient(n, v, base, metallic, roughness) + emissive;
    if frame.sun_direction_illuminance.w > 0.0 {
        let l = normalize(-frame.sun_direction_illuminance.xyz);
        color += brdf(n, v, l, base, metallic, roughness) * frame.sun_color_count.xyz * frame.sun_direction_illuminance.w * visibility;
    }
    if frame.fill_direction_illuminance.w > 0.0 {
        color += fill_light(n, v, base, metallic, roughness);
    }
    add_local_lights(&color, input.clip.xy, input.world, n, v, base, metallic, roughness, false);
    if FOG {
        color = height_fog(color, input.world);
    }
    return vec4(color, 1.0);
}
