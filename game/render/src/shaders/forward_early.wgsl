// The forward entry points with depth tested before shading, appended where the
// device has SHADER_EARLY_DEPTH_TEST. An Adreno 620's Vulkan driver (2020) tests
// these shaders' depth after shading, so every hidden fragment is shaded (a
// 64,000-cube frame: 170 ms, 15 with this). Only for frames with no fading slot:
// `faded` discards, and a forced early test writes depth before it would.
@fragment @early_depth_test(force) fn fs_early(input: Surface) -> @location(0) vec4<f32> {
    return finish(shade(input, 1.0));
}
@fragment @early_depth_test(force) fn fs_shadow_early(input: Surface) -> @location(0) vec4<f32> {
    return finish(shade(input, sun_visibility(input.world, normalize(input.normal))));
}
