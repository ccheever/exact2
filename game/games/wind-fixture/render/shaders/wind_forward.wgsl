// The reeds' forward pass: the wind's vertex shader, then the engine's own shading
// of the reed material (its texture, MASK cutout, lights, shadows and fog).
// Preludes (app.json): the engine's material and lighting WGSL, then
// render/wgsl/wind.wgsl.
@vertex fn wind_vs(@location(0) p: vec3f, @location(1) n: vec3f, @location(2) uv: vec2f,
        @location(3) color: vec4f, @builtin(instance_index) i: u32) -> ModelVarying {
    return sway(p, n, uv, i, color);
}
@fragment fn wind_fs(v: ModelVarying, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    return material_shade(v, front);
}
