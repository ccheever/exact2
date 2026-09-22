override FOG: bool = false;
@group(1) @binding(0) var base_texture: texture_2d<f32>;
@group(1) @binding(1) var base_sampler: sampler;
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) cap: vec2<f32>, @location(3) uv: vec2<f32>,
    @builtin(instance_index) instance: u32) -> Varying {
    return transform(position, normal, cap, uv, instance);
}
@fragment fn fs(input: Varying) -> @location(0) vec4<f32> { return shade(input, 1.0); }
@fragment fn fs_shadow(input: Varying) -> @location(0) vec4<f32> {
    return shade(input, sun_visibility(input.world, normalize(input.normal)));
}
const PI: f32 = 3.141592653589793;
fn brdf(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, base: vec3<f32>, metallic: f32,
        roughness: f32) -> vec3<f32> {
    let nl = max(dot(n, l), 0.0);
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
    let f0 = mix(vec3(0.04), base, metallic);
    let fresnel = f0 + (vec3(1.0) - f0) * pow(1.0 - vh, 5.0);
    let diffuse = (vec3(1.0) - fresnel) * (1.0 - metallic) * base / PI;
    return (diffuse + distribution * visibility * fresnel) * nl;
}
fn shade(input: Varying, visibility: f32) -> vec4<f32> {
    let i = input.slot * 12u;
    var base = vec3(materials[i], materials[i+1u], materials[i+2u]) * textureSample(base_texture, base_sampler, input.uv).rgb;
    // Opaque materials reuse alpha as a negative grid-spacing flag: no wider uploads.
    let spacing = -materials[i+3u];
    // Derivatives must run before material-dependent control flow (including on Safari).
    let world_footprint = fwidth(input.world);
    if spacing > 0.0 {
        let cell = input.world / spacing;
        let footprint = max(world_footprint / spacing, vec3(0.0001));
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
    let hemi = environment(n.y);
    var color = hemi * frame.zenith_ambient.w * base * (1.0 - metallic) + emissive;
    if frame.sun_direction_illuminance.w > 0.0 {
        let l = normalize(-frame.sun_direction_illuminance.xyz);
        color += brdf(n, v, l, base, metallic, roughness) * frame.sun_color_count.xyz * frame.sun_direction_illuminance.w * visibility;
    }
    for (var j = 0u; j < u32(frame.sun_color_count.w); j++) {
        let light = frame.points[j];
        let delta = light.position_range.xyz - input.world;
        let distance2 = max(dot(delta, delta), 0.0001);
        let range = max(light.position_range.w, 0.0001);
        let ratio2 = distance2 / (range * range);
        let window = max(1.0 - ratio2 * ratio2, 0.0);
        let radiance = light.color_intensity.xyz * light.color_intensity.w * window * window / distance2;
        color += brdf(n, v, delta * inverseSqrt(distance2), base, metallic, roughness) * radiance;
    }
    if FOG {
        color = height_fog(color, input.world);
    }
    return vec4(color, 1.0);
}
