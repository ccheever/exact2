// A dusk sky drawn in the background stage, over the engine's: violet overhead,
// rose, then gold at the horizon. A shader asset, so an edit reloads it live; the
// engine's frame uniform comes from its prelude (gpu.shaderPreludes in app.json).
struct Sky {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}
// One triangle covering the screen at the far plane: only uncovered sky passes depth.
@vertex fn sky_vs(@builtin(vertex_index) i: u32) -> Sky {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return Sky(vec4(p, 1.0, 1.0), p);
}
@fragment fn sky_fs(input: Sky) -> @location(0) vec4<f32> {
    let a = frame.inverse_view_proj * vec4(input.ndc, 0.0, 1.0);
    let b = frame.inverse_view_proj * vec4(input.ndc, 0.5, 1.0);
    let up = clamp(normalize(b.xyz / b.w - a.xyz / a.w).y, 0.0, 1.0);
    var color = mix(vec3(1.0, 0.55, 0.2), vec3(0.7, 0.3, 0.45), smoothstep(0.0, 0.25, up));
    color = mix(color, vec3(0.12, 0.08, 0.3), smoothstep(0.25, 0.8, up));
    // Exposure is applied before the tone curve; undo it so the colour is as written.
    return vec4(color / max(frame.ground_exposure.w, 1e-6), 1.0);
}
