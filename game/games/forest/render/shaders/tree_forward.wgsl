// Trees in the wind: the sway vertex shader, then the engine's own shading of each
// tree material (bark and needle textures, the leaves' MASK cutout, the crowns'
// Opacity fade, lights, shadows and fog). Preludes (app.json): the engine's
// material and lighting WGSL, then render/wgsl/sway.wgsl.
@vertex fn tree_vs(@location(0) p: vec3f, @location(1) n: vec3f, @location(2) uv: vec2f,
        @location(3) color: vec4f, @builtin(instance_index) i: u32) -> ModelVarying {
    return sway(p, n, uv, i, color);
}
@fragment fn tree_fs(v: ModelVarying, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    return material_shade(v, front);
}
