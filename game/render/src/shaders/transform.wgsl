@group(0) @binding(1) var<storage, read> prev: array<f32>;
@group(0) @binding(2) var<storage, read> curr: array<f32>;
@group(0) @binding(3) var<storage, read> materials: array<f32>;
@group(0) @binding(4) var<storage, read> slots: array<u32>;

@group(0) @binding(5) var<storage, read> attachment_matrices: array<mat4x4<f32>>;
fn attached(slot:u32) -> bool {
    if slot >= arrayLength(&attachment_matrices) { return false; }
    return attachment_matrices[slot][3].w == 1.0;
}
fn affine_normal(m:mat4x4<f32>, normal:vec3<f32>) -> vec3<f32> {
    let cof=mat3x3(cross(m[1].xyz,m[2].xyz),cross(m[2].xyz,m[0].xyz),cross(m[0].xyz,m[1].xyz));
    let det=dot(m[0].xyz,cof[0]);
    if abs(det)<1e-10 { return normal; }
    return (cof*normal)/det;
}
struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) slot: u32,
    @location(3) color: vec4<f32>,
}
fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
// A slot's pose between the two ticks, at the frame's alpha.
struct Pose {
    p: vec3<f32>,
    q: vec4<f32>,
    s: vec3<f32>,
}
fn pose(slot: u32) -> Pose {
    let i = slot * 10u;
    let a = frame.camera_alpha.w;
    let p = mix(vec3(prev[i], prev[i+1u], prev[i+2u]), vec3(curr[i], curr[i+1u], curr[i+2u]), a);
    let qp = vec4(prev[i+3u], prev[i+4u], prev[i+5u], prev[i+6u]);
    let qc = vec4(curr[i+3u], curr[i+4u], curr[i+5u], curr[i+6u]);
    let mixed_q = mix(qp, select(qc, -qc, dot(qp, qc) < 0.0), a);
    let q = mixed_q * inverseSqrt(max(dot(mixed_q, mixed_q), 1e-12));
    let s = mix(abs(vec3(prev[i+7u], prev[i+8u], prev[i+9u])), abs(vec3(curr[i+7u], curr[i+8u], curr[i+9u])), a);
    return Pose(p, q, s);
}
// A primitive's dimensions scale its unit mesh; a capsule's caps keep their
// radius and move by its height instead.
fn primitive_scale(slot: u32, cap: vec2<f32>) -> vec3<f32> {
    let m = slot * 12u;
    let dimensions = vec3(materials[m+9u], materials[m+10u], materials[m+11u]);
    return select(dimensions, vec3(dimensions.x), cap.y == 1.0);
}
// Where a primitive vertex is in the world. The colour entry and the depth
// prepass's both take their invariant position from it (its Equal test).
fn primitive_world(slot: u32, at: Pose, position: vec3<f32>, cap: vec2<f32>) -> vec3<f32> {
    let local = primitive_scale(slot, cap) * position + vec3(0.0, cap.x * materials[slot * 12u + 10u], 0.0);
    if attached(slot) { return (attachment_matrices[slot] * vec4(local, 1.0)).xyz; }
    return at.p + rotate(at.q, at.s * local);
}
fn transform(position: vec3<f32>, normal: vec3<f32>, cap: vec2<f32>,
             instance: u32, color: vec4<f32>) -> Varying {
    let slot = slots[instance];
    let at = pose(slot);
    let q = at.q;
    let s = at.s;
    let shape_scale = primitive_scale(slot, cap);
    let world = primitive_world(slot, at, position, cap);
    if attached(slot) {
        let affine=attachment_matrices[slot];
        return Varying(frame.view_proj*vec4(world,1.0),world,affine_normal(affine,normal/max(shape_scale,vec3(0.000001))),slot,color);
    }
    // A collapsed axis has a finite limiting normal instead of a NaN.
    let safe_scale = max(s * shape_scale, vec3(0.000001));
    return Varying(frame.view_proj * vec4(world, 1.0), world, rotate(q, normal / safe_scale), slot, color);
}

