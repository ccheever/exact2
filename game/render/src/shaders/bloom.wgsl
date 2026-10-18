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
// Independent direct-load oracle for the bright pass. Pixel centers use the
// same exact UV division: raster-interpolated UV roundoff is amplified by 65k
// HDR values beside zero and would obscure errors in the filtered taps.
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

// The bright pass in two at an even logical size (an odd one takes
// `bright_reference`). Its 13 taps sit on texel corners: nine on corners
// (2i - 1, 2j - 1), which neighbouring outputs share, and four on (2i, 2j).
// `bright_split` takes each corner's tap once as the reference does, over the
// half-size grid and one texel round it; `bright_join` weighs the nine by
// their tent, (1, 2, 1) x (1, 2, 1) / 32, with four filtered fetches on
// edges, and the four with one. On a Note 9 at 1080 x 2220, 2.4 ms where
// hardware-filtered taps in one pass took 9.8.
@group(0) @binding(4) var even_taps: texture_2d<f32>;
struct Split { @location(0) odd: vec4<f32>, @location(1) even: vec4<f32> }
fn split_tap(corner: vec2<f32>) -> vec3<f32> {
    return sample(corner / sampling.xy, true);
}
@fragment fn bright_split(input: Quad) -> Split {
    let at = floor(input.clip.xy) * 2.0;
    return Split(vec4(split_tap(at - 1.0), 0.0), vec4(split_tap(at), 0.0));
}
@fragment fn bright_join(input: Quad) -> @location(0) vec4<f32> {
    let x = floor(input.clip.xy);
    let odd = 1.0 / vec2<f32>(textureDimensions(source));
    let tent = textureSampleLevel(source, linear_sampler, (x + vec2(1.0, 1.0)) * odd, 0.0).rgb
        + textureSampleLevel(source, linear_sampler, (x + vec2(2.0, 1.0)) * odd, 0.0).rgb
        + textureSampleLevel(source, linear_sampler, (x + vec2(1.0, 2.0)) * odd, 0.0).rgb
        + textureSampleLevel(source, linear_sampler, (x + vec2(2.0, 2.0)) * odd, 0.0).rgb;
    let inner = textureSampleLevel(even_taps, linear_sampler,
        (x + 1.0) / vec2<f32>(textureDimensions(even_taps)), 0.0).rgb;
    return vec4(tent * 0.125 + inner * 0.5, 0.0);
}
