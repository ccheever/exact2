// A look frame's particle coverage (LLP 1127.000 D1, quad_coverage.rs): the
// probe variant of a particle pipeline, appended after particle.wgsl or
// sprite.wgsl. It draws what the particle pipeline draws and counts, for its
// draw, each fragment in front of the scene that moves an 8-bit pixel. The
// depth test is the shader's own: a fragment's storage writes stand even when
// a depth test after the shader drops it, and on Metal that test runs after a
// shader that writes memory.
@group(3) @binding(0) var<storage, read_write> covered: array<atomic<u32>>;
@group(3) @binding(1) var<uniform> counted: vec4<u32>;
@group(3) @binding(2) var count_depth: texture_depth_multisampled_2d;
fn count(v: QuadOut, c: vec4<f32>) {
    let front = v.clip.z <= textureLoad(count_depth, vec2<i32>(floor(v.clip.xy)), 0);
    if front && c.a * 255.0 >= 1.0 {
        atomicAdd(&covered[counted.x], 1u);
    }
}
@fragment fn quad_count_fs(v: QuadOut) -> @location(0) vec4<f32> {
    let c = quad_out(quad_color(v));
    count(v, c);
    return c;
}
