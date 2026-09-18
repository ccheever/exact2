override FOG: bool = false;
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) cap: vec2<f32>,
    @builtin(instance_index) instance: u32) -> Varying {
    return transform(position, normal, cap, instance);
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
    let base = vec3(materials[i], materials[i+1u], materials[i+2u]);
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
