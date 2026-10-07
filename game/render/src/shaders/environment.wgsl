// Prefiltered radiance for split-sum specular: one fullscreen triangle per cube
// face and mip, each mip a GGX roughness. The source is the procedural sky of
// frame.wgsl's environment(), or an authored equirectangular map (ibl.rs); the
// `sh` entry projects the same source onto SH9 irradiance for authored maps.
// @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
struct Prefilter {
    // .w: an authored map's intensity; zero selects the procedural sky.
    zenith: vec4<f32>,
    // .w: the map's RGBM range; zero reads RGB as radiance.
    horizon: vec4<f32>,
    ground: vec4<f32>,
    // face (0-5: +X -X +Y -Y +Z -Z), GGX roughness, face size in texels,
    // and an authored map's yaw about +Y
    face: vec4<f32>,
}
@group(0) @binding(0) var<uniform> p: Prefilter;
@group(0) @binding(1) var map: texture_2d<f32>;
@group(0) @binding(2) var map_sampler: sampler;
// Tests replace the sky with the direction itself to check cube orientation.
override DIRECTIONS: bool = false;
const SAMPLES: u32 = 256u;
const PI: f32 = 3.141592653589793;

// An equirectangular texel's level of detail matching a cube face of `size`.
fn map_lod(size: f32) -> f32 {
    return max(log2(f32(textureDimensions(map).x) / (4.0 * size)), 0.0);
}
fn source_at(d: vec3<f32>, lod: f32) -> vec3<f32> {
    if DIRECTIONS { return d * 0.5 + 0.5; }
    if p.zenith.w > 0.0 {
        // +Y is the top row and -Z the centre column, turned by the yaw.
        let c = cos(p.face.w);
        let s = sin(p.face.w);
        let r = vec3(c * d.x + s * d.z, d.y, c * d.z - s * d.x);
        let uv = vec2(0.5 + atan2(r.x, -r.z) / (2.0 * PI), acos(clamp(r.y, -1.0, 1.0)) / PI);
        let texel = textureSampleLevel(map, map_sampler, uv, lod);
        let rgb = select(texel.rgb, texel.rgb * texel.a * p.horizon.w, p.horizon.w > 0.0);
        return rgb * p.zenith.w;
    }
    return mix(p.horizon.xyz, select(p.ground.xyz, p.zenith.xyz, d.y >= 0.0), abs(d.y));
}
fn source(d: vec3<f32>) -> vec3<f32> { return source_at(d, map_lod(p.face.z)); }
// WebGPU cube faces with texel rows running down, as sampled by texture_cube.
fn direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    switch face {
        case 0u: { return vec3(1.0, -uv.y, -uv.x); }
        case 1u: { return vec3(-1.0, -uv.y, uv.x); }
        case 2u: { return vec3(uv.x, 1.0, uv.y); }
        case 3u: { return vec3(uv.x, -1.0, -uv.y); }
        case 4u: { return vec3(uv.x, -uv.y, 1.0); }
        default: { return vec3(-uv.x, -uv.y, -1.0); }
    }
}
fn hammersley(i: u32) -> vec2<f32> {
    return vec2(f32(i) / f32(SAMPLES), f32(reverseBits(i)) * 2.3283064365386963e-10);
}
// A GGX-distributed half vector around n (roughness a = perceptual²).
fn half_vector(xi: vec2<f32>, n: vec3<f32>, a: f32) -> vec3<f32> {
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    let up = select(vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let x = normalize(cross(up, n));
    let y = cross(n, x);
    return normalize(x * (cos(phi) * sin_theta) + y * (sin(phi) * sin_theta) + n * cos_theta);
}

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(q * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let n = normalize(direction(u32(p.face.x), pixel.xy / p.face.z * 2.0 - 1.0));
    let roughness = p.face.y;
    if roughness == 0.0 { return vec4(source(n), 1.0); }
    // Split-sum's usual assumption: view = normal = reflection.
    var sum = vec3(0.0);
    var weight = 0.0;
    for (var i = 0u; i < SAMPLES; i++) {
        let h = half_vector(hammersley(i), n, roughness * roughness);
        let l = 2.0 * dot(n, h) * h - n;
        let nl = dot(n, l);
        if nl > 0.0 {
            sum += source(l) * nl;
            weight += nl;
        }
    }
    return vec4(sum / max(weight, 1e-6), 1.0);
}

// SH9 irradiance / π of the source, as ibl.rs's Sky::irradiance projects the
// procedural sky: every texel direction of a 32² cube, weighted by solid angle,
// with the basis constants and Lambert's band factors folded in.
@group(0) @binding(3) var<storage, read_write> sh_out: array<vec4<f32>, 9>;
var<workgroup> partial: array<vec4<f32>, 256>;
@compute @workgroup_size(256)
fn sh(@builtin(local_invocation_index) lane: u32) {
    var sums: array<vec3<f32>, 9>;
    var total = 0.0;
    for (var t = lane; t < 6144u; t += 256u) {
        let face = t / 1024u;
        let xy = vec2(f32(t % 32u), f32((t / 32u) % 32u));
        let raw = direction(face, (xy + 0.5) / 32.0 * 2.0 - 1.0);
        let r2 = dot(raw, raw);
        let weight = 1.0 / (r2 * sqrt(r2));
        let d = raw * inverseSqrt(r2);
        let l = source_at(d, map_lod(32.0)) * weight;
        total += weight;
        let basis = array<f32, 9>(0.282095, 0.488603 * d.y, 0.488603 * d.z, 0.488603 * d.x,
            1.092548 * d.x * d.y, 1.092548 * d.y * d.z, 0.315392 * (3.0 * d.z * d.z - 1.0),
            1.092548 * d.x * d.z, 0.546274 * (d.x * d.x - d.y * d.y));
        for (var k = 0u; k < 9u; k++) { sums[k] += l * basis[k]; }
    }
    let band = array<f32, 9>(1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25);
    let constant = array<f32, 9>(0.282095, 0.488603, 0.488603, 0.488603, 1.092548, 1.092548,
        0.315392, 1.092548, 0.546274);
    partial[lane] = vec4(0.0, 0.0, 0.0, total);
    workgroupBarrier();
    for (var step = 128u; step > 0u; step >>= 1u) {
        if lane < step { partial[lane] += partial[lane + step]; }
        workgroupBarrier();
    }
    let solid = 4.0 * PI / partial[0].w;
    for (var k = 0u; k < 9u; k++) {
        workgroupBarrier();
        partial[lane] = vec4(sums[k], 0.0);
        workgroupBarrier();
        for (var step = 128u; step > 0u; step >>= 1u) {
            if lane < step { partial[lane] += partial[lane + step]; }
            workgroupBarrier();
        }
        if lane == 0u { sh_out[k] = vec4(partial[0].xyz * (solid * band[k] * constant[k]), 0.0); }
    }
}
