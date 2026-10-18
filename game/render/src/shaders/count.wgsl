// A look frame's particle coverage (LLP 1127.000 D1, quad_coverage.rs): the
// probe variant of a particle pipeline, appended after particle.wgsl or
// sprite.wgsl. It draws what the particle pipeline draws, and counts for its
// draw each fragment that contributes to the frame: a contribution above zero
// (alpha, times the brightest channel for an additive draw), in front of the
// scene's depth at one of the samples the fragment covers. The depth test is
// the shader's own: a fragment's storage writes stand even when a depth test
// after the shader drops it, and on Metal that test runs after a shader that
// writes memory.
@group(3) @binding(0) var<storage, read_write> covered: array<atomic<u32>>;
// x: the draw's counter; y: 1 for an additive draw.
@group(3) @binding(1) var<uniform> counted: vec4<u32>;
@group(3) @binding(2) var count_depth: texture_depth_multisampled_2d;
fn count(v: QuadOut, mask: u32, c: vec4<f32>) {
    let contribution = select(c.a, c.a * max(c.r, max(c.g, c.b)), counted.y != 0u);
    if contribution <= 0.0 {
        return;
    }
    let at = vec2<i32>(floor(v.clip.xy));
    var front = false;
    for (var i = 0u; i < textureNumSamples(count_depth); i++) {
        if (mask & (1u << i)) != 0u && v.clip.z <= textureLoad(count_depth, at, i32(i)) {
            front = true;
        }
    }
    if front {
        atomicAdd(&covered[counted.x], 1u);
    }
}
@fragment fn quad_count_fs(v: QuadOut, @builtin(sample_mask) mask: u32) -> @location(0) vec4<f32> {
    let c = quad_out(quad_color(v));
    count(v, mask, c);
    return c;
}
