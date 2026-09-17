struct SkyVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}
@vertex fn sky_vs(@builtin(vertex_index) i: u32) -> SkyVarying {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return SkyVarying(vec4(p, 1.0, 1.0), p);
}
@fragment fn sky_fs(input: SkyVarying) -> @location(0) vec4<f32> {
    let a = frame.inverse_view_proj * vec4(input.ndc, 0.0, 1.0);
    let b = frame.inverse_view_proj * vec4(input.ndc, 0.5, 1.0);
    let direction = normalize(b.xyz / b.w - a.xyz / a.w);
    var color = environment(direction.y);
    if frame.horizon_disc.w > 0.0 && frame.sun_direction_illuminance.w > 0.0 {
        let cosine = dot(direction, normalize(-frame.sun_direction_illuminance.xyz));
        let angle = acos(clamp(cosine, -1.0, 1.0));
        let radius = frame.horizon_disc.w;
        let disc = 1.0 - smoothstep(radius * 0.85, radius * 1.15, angle);
        let glow = exp(-angle / (radius * 6.0)) * 0.12;
        color += frame.sun_color_count.xyz * frame.sun_direction_illuminance.w * (disc + glow);
    }
    return vec4(color, 1.0);
}
