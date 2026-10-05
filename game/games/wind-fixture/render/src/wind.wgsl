// Appended to exact_game_render::hooks::MATERIAL_WGSL: the engine places each
// reed (instance_transform); the wind bends it by height, after placement, so
// every reed sways in the same world-space gust at its own phase.
struct Wind { time: f32, strength: f32, unused: vec2<f32> }
@group(2) @binding(0) var<uniform> wind: Wind;
fn sway(p: vec3f, n: vec3f, i: u32, color: vec4f) -> Varying {
    var v = instance_transform(p, n, i, color);
    let bend = p.y * p.y * 0.08 * wind.strength;
    let phase = wind.time * 1.7 + v.world.x * 0.6 + v.world.z * 0.35;
    v.world += vec3f(sin(phase) + 0.4 * sin(phase * 2.3), 0.0, 0.5 * cos(phase * 0.7)) * bend;
    v.clip = frame.view_proj * vec4f(v.world, 1.0);
    return v;
}
