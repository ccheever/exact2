@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var<uniform> sampling: vec4<f32>;
struct Quad { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Quad {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return Quad(vec4(p * 2.0 - 1.0, 0.0, 1.0), vec2(p.x, 1.0 - p.y));
}
fn sample(uv: vec2<f32>, bright_pass: bool) -> vec3<f32> {
    let logical = sampling.xy;
    let pixel = clamp(uv * logical, vec2(0.5), logical - 0.5);
    if !bright_pass {
        return finite_hdr(textureSampleLevel(source, linear_sampler,
            pixel / vec2<f32>(textureDimensions(source)), 0.0).rgb);
    }
    // Sanitize BEFORE interpolation: Inf mixed with zero can already yield NaN.
    let lo = vec2<i32>(floor(pixel - 0.5));
    let hi = min(lo + 1, vec2<i32>(logical) - 1);
    let t = fract(pixel - 0.5);
    let c = mix(mix(finite_hdr(textureLoad(source, lo, 0).rgb),
                    finite_hdr(textureLoad(source, vec2(hi.x, lo.y), 0).rgb), t.x),
                mix(finite_hdr(textureLoad(source, vec2(lo.x, hi.y), 0).rgb),
                    finite_hdr(textureLoad(source, hi, 0).rgb), t.x), t.y);
    return threshold(c);
}
fn threshold(c: vec3<f32>) -> vec3<f32> {
    let peak = max(c.r, max(c.g, c.b));
    let excess = max(peak - frame.height_bloom.y, 0.0);
    // One-sided soft knee: strictly zero below threshold, smooth onset above it.
    let knee = max(frame.height_bloom.y * 0.5, 0.0001);
    return c * (excess * excess / (excess + knee)) / max(peak, 0.0001);
}
fn filter13(uv: vec2<f32>, bright_pass: bool) -> vec4<f32> {
    let t = 1.0 / sampling.xy;
    var c = sample(uv, bright_pass) * 0.125;
    c += (sample(uv + t * vec2(-2.0, -2.0), bright_pass) + sample(uv + t * vec2(2.0, -2.0), bright_pass)
        + sample(uv + t * vec2(-2.0, 2.0), bright_pass) + sample(uv + t * vec2(2.0, 2.0), bright_pass)) * 0.03125;
    c += (sample(uv + t * vec2(-2.0, 0.0), bright_pass) + sample(uv + t * vec2(2.0, 0.0), bright_pass)
        + sample(uv + t * vec2(0.0, -2.0), bright_pass) + sample(uv + t * vec2(0.0, 2.0), bright_pass)) * 0.0625;
    c += (sample(uv + t * vec2(-1.0, -1.0), bright_pass) + sample(uv + t * vec2(1.0, -1.0), bright_pass)
        + sample(uv + t * vec2(-1.0, 1.0), bright_pass) + sample(uv + t * vec2(1.0, 1.0), bright_pass)) * 0.125;
    return vec4(c, 0.0);
}
// Independent direct-load oracle for the tiled bright pass. Pixel centers use
// the same exact UV division: raster-interpolated UV roundoff is amplified by
// 65k HDR values beside zero and would obscure errors in the shared tile.
@fragment fn bright_reference(input: Quad) -> @location(0) vec4<f32> {
    return filter13(input.clip.xy / max(floor(sampling.xy / 2.0), vec2(1.0)), true);
}
@fragment fn down(input: Quad) -> @location(0) vec4<f32> { return filter13(input.uv, false); }
@fragment fn up(input: Quad) -> @location(0) vec4<f32> {
    let t = frame.height_bloom.w / sampling.xy;
    var c = vec3(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let weight = f32((2 - abs(x)) * (2 - abs(y)));
            c += sample(input.uv + vec2(f32(x), f32(y)) * t, false) * weight;
        }
    }
    // Each coarser octave contributes half as much; energy stays bounded by 2.
    return vec4(c / 32.0, 0.0);
}

// The 13 bilinear taps overlap both within one pixel and across neighbors.
// An 8x8 output tile needs at most 21x21 source texels, including odd source
// extents; 22 leaves room for roundoff at a texel boundary. Sanitize each
// source texel once, before any interpolation (including Inf and NaN inputs).
@group(1) @binding(0) var bright_target: texture_storage_2d<rgba16float, write>;
var<workgroup> tile: array<vec3<f32>, 484>;
fn tile_at(p: vec2<i32>, origin: vec2<i32>) -> vec3<f32> {
    let at = p - origin;
    return tile[u32(at.y * 22 + at.x)];
}
fn tile_sample(uv: vec2<f32>, origin: vec2<i32>) -> vec3<f32> {
    let pixel = clamp(uv * sampling.xy, vec2(0.5), sampling.xy - 0.5);
    let lo = vec2<i32>(floor(pixel - 0.5));
    let hi = min(lo + 1, vec2<i32>(sampling.xy) - 1);
    let t = fract(pixel - 0.5);
    return threshold(mix(mix(tile_at(lo, origin), tile_at(vec2(hi.x, lo.y), origin), t.x),
                         mix(tile_at(vec2(lo.x, hi.y), origin), tile_at(hi, origin), t.x), t.y));
}
@compute @workgroup_size(8, 8)
fn bright_tile(@builtin(global_invocation_id) id: vec3<u32>,
               @builtin(workgroup_id) group: vec3<u32>,
               @builtin(local_invocation_index) lane: u32) {
    let dest = max(vec2<u32>(sampling.xy) / 2u, vec2(1u));
    let first_uv = (vec2<f32>(group.xy * 8u) + 0.5) / vec2<f32>(dest);
    let origin = vec2<i32>(floor(first_uv * sampling.xy - 0.5)) - 2;
    for (var i = lane; i < 484u; i += 64u) {
        let at = origin + vec2<i32>(i32(i % 22u), i32(i / 22u));
        let pixel = clamp(at, vec2(0), vec2<i32>(sampling.xy) - 1);
        tile[i] = finite_hdr(textureLoad(source, pixel, 0).rgb);
    }
    workgroupBarrier();
    if any(id.xy >= dest) { return; }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(dest);
    let t = 1.0 / sampling.xy;
    var c = tile_sample(uv, origin) * 0.125;
    c += (tile_sample(uv + t * vec2(-2.0, -2.0), origin) + tile_sample(uv + t * vec2(2.0, -2.0), origin)
        + tile_sample(uv + t * vec2(-2.0, 2.0), origin) + tile_sample(uv + t * vec2(2.0, 2.0), origin)) * 0.03125;
    c += (tile_sample(uv + t * vec2(-2.0, 0.0), origin) + tile_sample(uv + t * vec2(2.0, 0.0), origin)
        + tile_sample(uv + t * vec2(0.0, -2.0), origin) + tile_sample(uv + t * vec2(0.0, 2.0), origin)) * 0.0625;
    c += (tile_sample(uv + t * vec2(-1.0, -1.0), origin) + tile_sample(uv + t * vec2(1.0, -1.0), origin)
        + tile_sample(uv + t * vec2(-1.0, 1.0), origin) + tile_sample(uv + t * vec2(1.0, 1.0), origin)) * 0.125;
    textureStore(bright_target, id.xy, vec4(c, 0.0));
}
