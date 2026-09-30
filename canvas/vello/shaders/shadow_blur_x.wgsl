// Canvas 2D shadows, pass 1 of 2 (LLP 1056 §1): a horizontal Gaussian of the
// shape's coverage (its paint's alpha), sigma in device pixels.

struct Params {
    color: vec4<f32>,
    sigma: f32,
    radius: i32,
    pad0: u32,
    pad1: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1)
var src: texture_2d<f32>;

@group(0) @binding(2)
var dst: texture_storage_2d<r32float, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(textureDimensions(src));
    let p = vec2<i32>(id.xy);
    if p.x >= size.x || p.y >= size.y {
        return;
    }
    var sum = 0.0;
    var weight = 0.0;
    let k = -0.5 / max(params.sigma * params.sigma, 1e-6);
    for (var d = -params.radius; d <= params.radius; d += 1) {
        let w = exp(f32(d * d) * k);
        weight += w;
        let x = p.x + d;
        if x >= 0 && x < size.x {
            sum += w * textureLoad(src, vec2(x, p.y), 0).a;
        }
    }
    textureStore(dst, p, vec4(sum / weight, 0.0, 0.0, 0.0));
}
