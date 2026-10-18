// A look frame's account of the sun's shadow maps (LLP 1127.000 D1): for each
// cascade, the texels its pass wrote nearer than the clear depth of 1. What
// the shadow passes drew, read from what they drew into.
@group(0) @binding(0) var cascades: texture_depth_2d_array;
@group(0) @binding(1) var<storage, read_write> written: array<atomic<u32>>;
var<workgroup> found: atomic<u32>;
@compute @workgroup_size(8, 8)
fn count(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let size = textureDimensions(cascades);
    if id.x < size.x && id.y < size.y && textureLoad(cascades, id.xy, id.z, 0) < 1.0 {
        atomicAdd(&found, 1u);
    }
    workgroupBarrier();
    if lane == 0u {
        let n = atomicLoad(&found);
        if n > 0u {
            atomicAdd(&written[id.z], n);
        }
    }
}
