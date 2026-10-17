// The occlusion pass's occluders (occlusion.rs), drawn so that each depth texel
// holds no nearer a depth than the real geometry anywhere in the texel: a convex
// mesh with a ball of radius r about its origin, shrunk about it by k, keeps a
// ball of radius (1 - k) r about every point of the shrunk shape inside the real
// one. With (1 - k) r the world size of a texel's half diagonal at the shape's
// farthest depth, a texel whose centre the shrunk shape covers is covered by the
// real one throughout; pushed back along each ray by that size, its depth is
// never nearer than the real surface's there.
struct Occluder {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // 1 / (fx · width) and 1 / (fy · height) of the depth texture, the mesh's
    // inscribed radius, and 1 for a box (every face square to an axis).
    texel: vec4<f32>,
    // The mesh's largest |position| per axis.
    reach: vec4<f32>,
}
@group(1) @binding(0) var<uniform> occluder: Occluder;

@vertex fn vs_occluder(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(2) cap: vec2<f32>, @builtin(instance_index) instance: u32) -> @builtin(position) vec4<f32> {
    let slot = slots[instance];
    if attached(slot) { return vec4(0.0); }
    let at = pose(slot);
    let m = slot * 12u;
    let scale = abs(at.s * vec3(materials[m + 9u], materials[m + 10u], materials[m + 11u]));
    let inner = occluder.texel.z * min(scale.x, min(scale.y, scale.z));
    let m3 = occluder.view_proj;
    let row3 = vec4(m3[0].w, m3[1].w, m3[2].w, m3[3].w);
    let w = dot(row3, vec4(at.p, 1.0));
    // Only a shape wholly in front of the eye (its box's nearest corner): each
    // vertex then moves back along its own ray by exactly `reach` in depth, and
    // every point between, as 1/w interpolates, by at least that.
    let half = at.s * vec3(materials[m + 9u], materials[m + 10u], materials[m + 11u]) * occluder.reach.xyz;
    let depths = abs(vec3(dot(row3.xyz, rotate(at.q, vec3(half.x, 0.0, 0.0))),
        dot(row3.xyz, rotate(at.q, vec3(0.0, half.y, 0.0))), dot(row3.xyz, rotate(at.q, vec3(0.0, 0.0, half.z)))));
    let spread = depths.x + depths.y + depths.z;
    if w - spread <= 1e-4 { return vec4(0.0); }
    let reach = (w + spread) * length(occluder.texel.xy);
    // A box's faces each move in by `reach`; another shape shrinks by one factor
    // that moves its inscribed ball's surface in by `reach`.
    var k = vec3(1.0 - reach / inner);
    if occluder.texel.w != 0.0 { k = 1.0 - reach / max(abs(half), vec3(1e-30)); }
    if any(k <= vec3(0.0)) { return vec4(0.0); }
    let world = at.p + rotate(at.q, at.s * vec3(materials[m + 9u], materials[m + 10u], materials[m + 11u]) * position * k);
    let ray = world - occluder.eye.xyz;
    let pushed = world + ray * (reach / max(dot(row3.xyz, ray), 1e-6));
    return occluder.view_proj * vec4(pushed, 1.0);
}
