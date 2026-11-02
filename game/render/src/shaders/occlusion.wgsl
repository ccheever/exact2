// The camera's occlusion pyramid (occlusion.rs): each level's texel holds the
// farthest depth under it, from the depth of what the camera drew last frame,
// drawn again at this frame's poses. The cull's `occluded` tests against it.
@group(0) @binding(0) var level_in: texture_2d<f32>;
@group(0) @binding(1) var level_out: texture_storage_2d<r32float, write>;

// Level 0 from the occluders' depth (or level k from k - 1): the farthest of each
// 2×2 block. Levels halve rounding down, so the last row and column also take a
// source's odd one.
@compute @workgroup_size(8, 8)
fn reduce(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(level_out);
    if id.x >= size.x || id.y >= size.y { return; }
    let last = textureDimensions(level_in) - 1u;
    let at = id.xy * 2u;
    let end = select(at + 1u, last, id.xy == size - 1u);
    var far = 0.0;
    for (var y = at.y; y <= min(end.y, last.y); y++) {
        for (var x = at.x; x <= min(end.x, last.x); x++) {
            far = max(far, textureLoad(level_in, vec2(x, y), 0).r);
        }
    }
    textureStore(level_out, id.xy, vec4(far, 0.0, 0.0, 0.0));
}
