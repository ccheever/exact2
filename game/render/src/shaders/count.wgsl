// A look frame's particle coverage (LLP 1127.000 D1, quad_coverage.rs): each
// particle draw drawn again over the frame's depth, every fragment in front of
// it counted once for its draw. No colour is written. The test is the shader's
// own: a fragment's storage writes stand even when a depth test after the
// shader drops it, and on Metal it runs after a shader that writes memory.
@group(1) @binding(0) var<storage, read_write> covered: array<atomic<u32>>;
@group(1) @binding(1) var<uniform> draw: vec4<u32>;
@group(1) @binding(2) var scene_depth: texture_depth_multisampled_2d;
@fragment fn quad_count_fs(v: QuadOut) {
    if v.clip.z <= textureLoad(scene_depth, vec2<i32>(floor(v.clip.xy)), 0) {
        atomicAdd(&covered[draw.x], 1u);
    }
}
