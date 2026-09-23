// Prefiltered radiance for split-sum specular: one fullscreen triangle per cube
// face and mip, each mip a GGX roughness. The source is the procedural sky of
// frame.wgsl's environment(); an authored environment map would be sampled here
// instead, leaving everything downstream unchanged.
// @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
struct Prefilter {
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    ground: vec4<f32>,
    // face (0-5: +X -X +Y -Y +Z -Z), GGX roughness, face size in texels
    face: vec4<f32>,
}
@group(0) @binding(0) var<uniform> p: Prefilter;
// Tests replace the sky with the direction itself to check cube orientation.
override DIRECTIONS: bool = false;
const SAMPLES: u32 = 256u;
const PI: f32 = 3.141592653589793;

fn source(d: vec3<f32>) -> vec3<f32> {
    if DIRECTIONS { return d * 0.5 + 0.5; }
    return mix(p.horizon.xyz, select(p.ground.xyz, p.zenith.xyz, d.y >= 0.0), abs(d.y));
}
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
