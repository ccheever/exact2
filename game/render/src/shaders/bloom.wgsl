@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
struct Quad { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Quad {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return Quad(vec4(p * 2.0 - 1.0, 0.0, 1.0), vec2(p.x, 1.0 - p.y));
}
fn sample(uv: vec2<f32>, bright_pass: bool) -> vec3<f32> {
    let c = textureSampleLevel(source, linear_sampler, uv, 0.0).rgb;
    if !bright_pass { return c; }
    let peak = max(c.r, max(c.g, c.b));
    let excess = max(peak - frame.height_bloom.y, 0.0);
    // One-sided soft knee: strictly zero below threshold, smooth onset above it.
    let knee = max(frame.height_bloom.y * 0.5, 0.0001);
    return c * (excess * excess / (excess + knee)) / max(peak, 0.0001);
}
fn filter13(uv: vec2<f32>, bright_pass: bool) -> vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(source));
    var c = sample(uv, bright_pass) * 0.125;
    c += (sample(uv + t * vec2(-2.0, -2.0), bright_pass) + sample(uv + t * vec2(2.0, -2.0), bright_pass)
        + sample(uv + t * vec2(-2.0, 2.0), bright_pass) + sample(uv + t * vec2(2.0, 2.0), bright_pass)) * 0.03125;
    c += (sample(uv + t * vec2(-2.0, 0.0), bright_pass) + sample(uv + t * vec2(2.0, 0.0), bright_pass)
        + sample(uv + t * vec2(0.0, -2.0), bright_pass) + sample(uv + t * vec2(0.0, 2.0), bright_pass)) * 0.0625;
    c += (sample(uv + t * vec2(-1.0, -1.0), bright_pass) + sample(uv + t * vec2(1.0, -1.0), bright_pass)
        + sample(uv + t * vec2(-1.0, 1.0), bright_pass) + sample(uv + t * vec2(1.0, 1.0), bright_pass)) * 0.125;
    return vec4(c, 0.0);
}
@fragment fn bright(input: Quad) -> @location(0) vec4<f32> { return filter13(input.uv, true); }
@fragment fn down(input: Quad) -> @location(0) vec4<f32> { return filter13(input.uv, false); }
@fragment fn up(input: Quad) -> @location(0) vec4<f32> {
    let t = frame.height_bloom.w / vec2<f32>(textureDimensions(source));
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
