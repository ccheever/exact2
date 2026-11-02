// Custom materials' lighting (hooks::MATERIAL_SHADOWS_WGSL): the engine's sun and
// local light shadows, `brdf` and `add_local_lights`. Forward custom pipelines
// receive group 1 (shadow maps) and the scene's light buffer; bind nothing more.
fn sun_shadow(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    if frame.splits_count.w == 0.0 { return 1.0; }
    return sun_visibility(world, normal);
}
