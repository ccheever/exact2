// A prelude after the engine's material WGSL: the engine places each tree
// (instance_transform); the wind then leans it about its trunk's base on slow gusts
// shifted by where it stands, and branch tips flutter by their distance from the
// trunk. Every part of a tree (bark, crown, leaves) runs the same function, so the
// parts stay joined. The displacement stays under REACH (lib.rs).
struct Wind { time: f32, unused: vec3<f32> }
@group(2) @binding(0) var<uniform> wind: Wind;
fn sway(p: vec3f, n: vec3f, uv: vec2f, i: u32, color: vec4f) -> ModelVarying {
    var v = instance_transform(p, n, uv, i, color);
    let t = draw_instance(i).transform * 10u;
    let base = vec3f(curr[t], curr[t + 1u], curr[t + 2u]);
    let h = max(v.world.y - base.y, 0.0);
    let gust = 0.6 + 0.4 * sin(wind.time * 0.37 + base.x * 0.02);
    let lean = 0.022 * gust * sin(wind.time * 1.3 + base.x * 0.11 + base.z * 0.07);
    let side = 0.012 * gust * sin(wind.time * 0.9 + base.z * 0.13 + 1.7);
    // Small turns about world x (top toward +z) and z (top toward -x).
    var d = vec3f(-side, 0.0, lean) * h;
    let phase = wind.time * 5.1 + dot(v.world, vec3f(1.7, 0.9, 1.3));
    let tip = length(v.world.xz - base.xz);
    d += vec3f(sin(phase), 0.5 * sin(phase * 1.3), cos(phase * 0.8)) * 0.012 * gust * tip;
    v.world += d;
    v.clip = frame.view_proj * vec4f(v.world, 1.0);
    return v;
}
