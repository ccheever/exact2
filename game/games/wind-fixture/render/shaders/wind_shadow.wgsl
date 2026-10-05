// The paired shadow pass bends the same way, so shadows sway too. Preludes
// (app.json): the engine's material WGSL, then render/wgsl/wind.wgsl.
@group(1) @binding(0) var<uniform> light: mat4x4f;
@vertex fn wind_shadow(@location(0) p: vec3f, @location(1) n: vec3f,
        @builtin(instance_index) i: u32) -> @builtin(position) vec4f {
    return light * vec4f(sway(p, n, i, vec4f(1.0)).world, 1.0);
}
