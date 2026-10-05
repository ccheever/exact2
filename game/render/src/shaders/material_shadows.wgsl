// Custom materials' lighting (hooks::MATERIAL_SHADOWS_WGSL): the engine's sun and
// local light shadows, `brdf`, `add_local_lights`, environment light and its model
// shading. Forward custom pipelines receive group 1 (shadow maps) and the scene's
// light buffer; bind nothing more.
fn sun_shadow(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    if frame.splits_count.w == 0.0 { return 1.0; }
    return sun_visibility(world, normal);
}
// The engine's own shading of a model fragment, as its model pipeline draws the
// material (textures, normal map, lights, shadows, fog; Opacity and MASK discard;
// a BLEND material's alpha): a custom vertex shader keeps the stock look.
fn material_shade(input: ModelVarying, front: bool) -> vec4<f32> {
    let shadow = sun_shadow(input.world, normalize(input.normal));
    return model_lit(input, front, shadow, frame.fog_color_density.w > 0.0);
}
