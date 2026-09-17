struct Point { position_range: vec4<f32>, color_intensity: vec4<f32> }
struct Frame {
    view_proj: mat4x4<f32>,
    camera_alpha: vec4<f32>,
    sun_direction_illuminance: vec4<f32>,
    sun_color_count: vec4<f32>,
    sky_ambient: vec4<f32>,
    ground_exposure: vec4<f32>,
    points: array<Point, 16>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> prev: array<f32>;
@group(0) @binding(2) var<storage, read> curr: array<f32>;
@group(0) @binding(3) var<storage, read> materials: array<f32>;
@group(0) @binding(4) var<storage, read> slots: array<u32>;

struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) slot: u32,
}
fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>,
             @location(2) uv: vec2<f32>, @builtin(instance_index) instance: u32) -> Varying {
    let slot = slots[instance];
    let i = slot * 10u;
    let a = frame.camera_alpha.w;
    let p = mix(vec3(prev[i], prev[i+1u], prev[i+2u]), vec3(curr[i], curr[i+1u], curr[i+2u]), a);
    let qp = vec4(prev[i+3u], prev[i+4u], prev[i+5u], prev[i+6u]);
    let qc = vec4(curr[i+3u], curr[i+4u], curr[i+5u], curr[i+6u]);
    let q = normalize(mix(qp, select(qc, -qc, dot(qp, qc) < 0.0), a));
    let s = mix(vec3(prev[i+7u], prev[i+8u], prev[i+9u]), vec3(curr[i+7u], curr[i+8u], curr[i+9u]), a);
    let world = p + rotate(q, s * position);
    // A collapsed axis has a finite limiting normal instead of a NaN.
    let safe_scale = select(vec3(1.0), vec3(-1.0), s < vec3(0.0)) * max(abs(s), vec3(0.000001));
    return Varying(frame.view_proj * vec4(world, 1.0), world, rotate(q, normal / safe_scale), slot);
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
@fragment fn fs(input: Varying) -> @location(0) vec4<f32> {
    let i = input.slot * 12u;
    let base = vec3(materials[i], materials[i+1u], materials[i+2u]);
    let metallic = clamp(materials[i+4u], 0.0, 1.0);
    let roughness = clamp(materials[i+5u], 0.045, 1.0);
    let emissive = vec3(materials[i+6u], materials[i+7u], materials[i+8u]);
    let n = normalize(input.normal);
    let v = normalize(frame.camera_alpha.xyz - input.world);
    let hemi = mix(frame.ground_exposure.xyz, frame.sky_ambient.xyz, n.y * 0.5 + 0.5);
    var color = hemi * frame.sky_ambient.w * base * (1.0 - metallic) + emissive;
    if frame.sun_direction_illuminance.w > 0.0 {
        let l = normalize(-frame.sun_direction_illuminance.xyz);
        color += brdf(n, v, l, base, metallic, roughness) * frame.sun_color_count.xyz * frame.sun_direction_illuminance.w;
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
    return vec4(color, 1.0);
}
