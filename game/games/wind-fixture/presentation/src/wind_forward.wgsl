// After MATERIAL_SHADOWS_WGSL and wind.wgsl: the forward pass, lit by the
// engine's sky light and sun, in the engine's sun shadows.
@vertex fn wind_vs(@location(0) p: vec3f, @location(1) n: vec3f, @location(3) color: vec4f,
        @builtin(instance_index) i: u32) -> Varying {
    return sway(p, n, i, color);
}
@fragment fn wind_fs(v: Varying, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    let n = normalize(select(-v.normal, v.normal, front));
    let l = normalize(-frame.sun_direction_illuminance.xyz);
    let sun = frame.sun_color_count.xyz * frame.sun_direction_illuminance.w;
    let lit = environment(n.y) + sun * max(dot(n, l), 0.0) * sun_shadow(v.world, n) / 3.14159265;
    return vec4f(v.color.rgb * lit, 1.0);
}
