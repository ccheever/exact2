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

// The bright level: `bright_reference`'s 13 thresholded taps. At an even
// logical size each tap falls on a texel corner, where the sampler's bilinear
// weights are exact halves, so one filtered fetch stands for the four
// sanitized loads `sample(uv, true)` makes; a fetch outside finite_hdr's range
// (an infinite or NaN texel among the four, or one past 65472) is not that
// average, and takes the loads. Odd sizes take them throughout.
// One hardware tap of the bright pass, or a negative value where the filtered
// average is not what `finite_hdr` keeps (non-finite, or outside [0, 65472]).
// Bits are compared, as `finite_hdr` tests them: a compiler may assume no NaN
// reaches a float comparison.
fn bright_tap(uv: vec2<f32>) -> vec3<f32> {
    let pixel = clamp(uv * sampling.xy, vec2(0.5), sampling.xy - 0.5);
    let c = textureSampleLevel(source, linear_sampler,
        pixel / vec2<f32>(textureDimensions(source)), 0.0).rgb;
    if all(bitcast<vec3<u32>>(finite_hdr(c)) == bitcast<vec3<u32>>(c)) {
        return threshold(c);
    }
    return vec3(-1.0);
}
// The bright pass by hardware filtering at an even logical size, and by the
// reference's direct loads wherever a tap's average is out of its range. One
// branch for the whole kernel: Adreno's compiler flattened a fallback per tap.
// It equals the reference where the scene's texels are finite and within
// [0, 65472], and wherever one is not finite (a NaN or infinity makes its
// taps' averages so). A finite texel out of that range whose tap averages
// back into it (a negative colour beside a brighter one; f16's 65504) is
// filtered as it is, where the reference first clamps it.
@fragment fn bright(input: Quad) -> @location(0) vec4<f32> {
    let uv = input.clip.xy / max(floor(sampling.xy / 2.0), vec2(1.0));
    if all(fract(sampling.xy * 0.5) == vec2(0.0)) {
        let t = 1.0 / sampling.xy;
        let center = bright_tap(uv);
        let corners = array(bright_tap(uv + t * vec2(-2.0, -2.0)), bright_tap(uv + t * vec2(2.0, -2.0)),
            bright_tap(uv + t * vec2(-2.0, 2.0)), bright_tap(uv + t * vec2(2.0, 2.0)));
        let edges = array(bright_tap(uv + t * vec2(-2.0, 0.0)), bright_tap(uv + t * vec2(2.0, 0.0)),
            bright_tap(uv + t * vec2(0.0, -2.0)), bright_tap(uv + t * vec2(0.0, 2.0)));
        let inner = array(bright_tap(uv + t * vec2(-1.0, -1.0)), bright_tap(uv + t * vec2(1.0, -1.0)),
            bright_tap(uv + t * vec2(-1.0, 1.0)), bright_tap(uv + t * vec2(1.0, 1.0)));
        var low = center;
        for (var i = 0; i < 4; i++) {
            low = min(low, min(corners[i], min(edges[i], inner[i])));
        }
        if all(low >= vec3(0.0)) {
            var c = center * 0.125;
            c += (corners[0] + corners[1] + corners[2] + corners[3]) * 0.03125;
            c += (edges[0] + edges[1] + edges[2] + edges[3]) * 0.0625;
            c += (inner[0] + inner[1] + inner[2] + inner[3]) * 0.125;
            return vec4(c, 0.0);
        }
    }
    return filter13(uv, true);
}
