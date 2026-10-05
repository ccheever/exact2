// The paired shadow pass sways the same way and drops what the engine's shadow pass
// drops (the leaves' cutout, a faded crown's dither). Preludes (app.json): the
// engine's material WGSL, then render/wgsl/sway.wgsl.
@group(1) @binding(0) var<uniform> light: mat4x4f;
@vertex fn tree_shadow(@location(0) p: vec3f, @location(1) n: vec3f, @location(2) uv: vec2f,
        @location(3) color: vec4f, @builtin(instance_index) i: u32) -> ModelVarying {
    var v = sway(p, n, uv, i, color);
    v.clip = light * vec4f(v.world, 1.0);
    return v;
}
@fragment fn tree_cutout(v: ModelVarying) {
    if model_discarded(v) { discard; }
}
