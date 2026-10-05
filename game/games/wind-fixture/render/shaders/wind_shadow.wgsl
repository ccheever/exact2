// The paired shadow pass bends the same way, so shadows sway too, and drops what the
// engine's shadow pass drops (the blades' MASK cutout). Preludes (app.json): the
// engine's material WGSL, then render/wgsl/wind.wgsl.
@group(1) @binding(0) var<uniform> light: mat4x4f;
@vertex fn wind_shadow(@location(0) p: vec3f, @location(1) n: vec3f, @location(2) uv: vec2f,
        @builtin(instance_index) i: u32) -> ModelVarying {
    var v = sway(p, n, uv, i, vec4f(1.0));
    v.clip = light * vec4f(v.world, 1.0);
    return v;
}
@fragment fn wind_cutout(v: ModelVarying) {
    if model_discarded(v) { discard; }
}
