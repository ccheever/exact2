@group(0) @binding(10) var sky_map: texture_2d<f32>;
@group(0) @binding(11) var sky_sampler: sampler;
// environment.wgsl's equirectangular lookup: +Y the top row, -Z the centre
// column, turned by the map's yaw.
fn sky_texel(d: vec3<f32>) -> vec3<f32> {
    let c = cos(frame.sky.y);
    let s = sin(frame.sky.y);
    let r = vec3(c * d.x + s * d.z, d.y, c * d.z - s * d.x);
    let uv = vec2(0.5 + atan2(r.x, -r.z) / (2.0 * 3.141592653589793),
        acos(clamp(r.y, -1.0, 1.0)) / 3.141592653589793);
    let texel = textureSampleLevel(sky_map, sky_sampler, uv, 0.0);
    let rgb = select(texel.rgb, texel.rgb * texel.a * frame.sky.z, frame.sky.z > 0.0);
    return rgb * frame.sky.x;
}
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
    if frame.sky.x > 0.0 { color = sky_texel(direction); }
    if frame.horizon_disc.w > 0.0 && frame.sun_direction_illuminance.w > 0.0 {
        let cosine = dot(direction, normalize(-frame.sun_direction_illuminance.xyz));
        let angle = acos(clamp(cosine, -1.0, 1.0));
        let radius = frame.horizon_disc.w;
        let disc = 1.0 - smoothstep(radius * 0.85, radius * 1.15, angle);
        let glow = exp(-angle / (radius * 6.0)) * 0.12;
        color += frame.sun_color_count.xyz * frame.sun_direction_illuminance.w * (disc + glow);
    }
    if frame.fog_color_density.w > 0.0 {
        color = height_fog(color, frame.camera_alpha.xyz + direction * 10000.0);
    }
    return vec4(color, 1.0);
}
