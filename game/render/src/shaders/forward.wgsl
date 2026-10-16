override FOG: bool = false;
// The fill and local lights, and the grid. A frame with none compiles without
// them: their registers, unused, still cost a mobile GPU much of its occupancy.
override LIGHTS: bool = true;
override GRID: bool = true;
// Tonemapped in place for an LDR target: a direct frame (`Renderer::direct`).
override DIRECT: bool = false;
// Some slot may be fading (`faded`); false where none is, as early depth requires.
override FADES: bool = true;
// Transform's varyings and the slot's material, read once per vertex: a storage
// read per fragment is the costlier on a mobile GPU.
struct Surface {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) slot: u32,
    // The vertex colour times the material's (flat across the triangle, so the
    // product interpolates as the colour would).
    @location(3) albedo: vec3<f32>,
    // Metallic, roughness, and the alpha opaque materials use as a negative grid spacing.
    @location(4) @interpolate(flat) material: vec3<f32>,
    @location(5) @interpolate(flat) emissive: vec3<f32>,
}
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) cap: vec2<f32>, @location(3) color: vec4<f32>,
    @builtin(instance_index) instance: u32) -> Surface {
    let v = transform(position, normal, cap, instance, color);
    let i = v.slot * 12u;
    return Surface(v.clip, v.world, v.normal, v.slot,
        vec3(materials[i], materials[i+1u], materials[i+2u]) * v.color.rgb,
        vec3(materials[i+4u], materials[i+5u], materials[i+3u]),
        vec3(materials[i+6u], materials[i+7u], materials[i+8u]));
}
@fragment fn fs(input: Surface) -> @location(0) vec4<f32> { return finish(shade(input, 1.0)); }
@fragment fn fs_shadow(input: Surface) -> @location(0) vec4<f32> {
    return finish(shade(input, sun_visibility(input.world, normalize(input.normal))));
}
fn finish(color: vec4<f32>) -> vec4<f32> {
    if DIRECT { return tonemap(color.rgb); }
    return color;
}
@diagnostic(off, derivative_uniformity)
fn shade(input: Surface, visibility: f32) -> vec4<f32> {
    if FADES && faded(input.slot, input.clip.xy) { discard; }
    var base = input.albedo;
    // Opaque materials reuse alpha as a negative grid-spacing flag: no wider uploads.
    let spacing = -input.material.z;
    if GRID && spacing > 0.0 {
        let cell = input.world / spacing;
        let footprint = max(fwidth(cell), vec3(0.0001));
        let distance = abs(fract(cell - 0.5) - 0.5);
        let lines = 1.0 - smoothstep(vec3(0.0), footprint * 1.25, distance);
        // Triplanar projection: omit each face's normal axis, so walls work too.
        let weight = pow(abs(normalize(input.normal)), vec3(8.0));
        let planes = vec3(max(lines.y, lines.z), max(lines.x, lines.z), max(lines.x, lines.y));
        let fade = 1.0 - smoothstep(0.2, 1.0, max(footprint.x, max(footprint.y, footprint.z)));
        // Integrated line coverage, including the union of both projected axes.
        // Minification converges to this lined mean, never the unlined base.
        let width = footprint * 1.25;
        let t = min(0.5 / width, vec3(1.0));
        let mean = 2.0 * width * (t - t*t*t + 0.5*t*t*t*t);
        let plane_mean = vec3(1.0) - vec3((1.0-mean.y)*(1.0-mean.z), (1.0-mean.x)*(1.0-mean.z), (1.0-mean.x)*(1.0-mean.y));
        let line = dot(mix(plane_mean, planes, fade), weight) / max(dot(weight, vec3(1.0)), 0.0001);
        base *= 1.0 + 0.35 * line;
    }
    let metallic = clamp(input.material.x, 0.0, 1.0);
    let roughness = clamp(input.material.y, 0.045, 1.0);
    let emissive = input.emissive;
    let n = normalize(input.normal);
    let v = normalize(frame.camera_alpha.xyz - input.world);
    var color = ambient(n, v, base, metallic, roughness) + emissive;
    if frame.sun_direction_illuminance.w > 0.0 {
        let l = -frame.sun_direction_illuminance.xyz;
        color += brdf(n, v, l, base, metallic, roughness) * frame.sun_color_count.xyz * frame.sun_direction_illuminance.w * visibility;
    }
    if LIGHTS {
        if frame.fill_direction_illuminance.w > 0.0 {
            color += fill_light(n, v, base, metallic, roughness);
        }
        add_local_lights(&color, input.clip.xy, input.world, n, v, base, metallic, roughness, false);
    }
    if FOG {
        color = height_fog(color, input.world);
    }
    return vec4(color, 1.0);
}
