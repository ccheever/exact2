// Ambient light from the environment: SH9 diffuse irradiance (frame.irradiance)
// and split-sum specular from the prefiltered radiance cube (ibl.rs).
// @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
@group(0) @binding(6) var environment_radiance: texture_cube<f32>;
@group(0) @binding(7) var environment_sampler: sampler;
const ENVIRONMENT_LODS: f32 = 5.0; // ibl.rs MIPS - 1: lod = roughness × this
// A uniform sky (one colour, no map): its irradiance is the constant band alone.
override UNIFORM_AMBIENT: bool = false;

// Outgoing radiance per unit albedo of a Lambertian surface facing n.
// Indexes the uniform directly: a local copy of the array can stay in memory per fragment.
fn irradiance(n: vec3<f32>) -> vec3<shading> {
    let h = vec3<shading>(n);
    let e = sh(0u) + sh(1u) * h.y + sh(2u) * h.z
        + sh(3u) * h.x + sh(4u) * (h.x * h.y)
        + sh(5u) * (h.y * h.z) + sh(6u) * (3.0 * h.z * h.z - 1.0)
        + sh(7u) * (h.x * h.z) + sh(8u) * (h.x * h.x - h.y * h.y);
    return max(e, vec3<shading>(0.0));
}
fn sh(k: u32) -> vec3<shading> { return vec3<shading>(frame.irradiance[k].xyz); }
// Karis's analytic fit of the preintegrated environment BRDF: F0 scale and bias.
fn environment_brdf(roughness: shading, nv: shading) -> vec2<shading> {
    let r = roughness * vec4<shading>(-1.0, -0.0275, -0.572, 0.022) + vec4<shading>(1.0, 0.0425, 1.04, -0.04);
    let a004 = min(r.x * r.x, exp2(-9.28 * nv)) * r.x + r.y;
    return vec2<shading>(-1.04, 1.04) * a004 + r.zw;
}
// Diffuse plus specular ambient light, before occlusion and exposure.
fn ambient(n: vec3<f32>, v: vec3<f32>, base: vec3<shading>, metallic: f32, roughness: f32) -> vec3<shading> {
    let strength = shading(frame.zenith_ambient.w);
    if MATTE { return irradiance(n) * base * strength; }
    let ab = environment_brdf(shading(roughness), shading(max(dot(n, v), 0.0001)));
    let specular = mix(vec3<shading>(0.04), base, shading(metallic)) * ab.x + ab.y;
    // A uniform sky's prefiltered cube is one colour: any direction reads it.
    let direction = select(reflect(-v, n), vec3(0.0, 1.0, 0.0), UNIFORM_AMBIENT);
    let radiance = vec3<shading>(textureSampleLevel(environment_radiance, environment_sampler,
        direction, roughness * ENVIRONMENT_LODS).rgb);
    let e = select(irradiance(n), max(sh(0u), vec3<shading>(0.0)), UNIFORM_AMBIENT);
    let diffuse = e * base * (1.0 - shading(metallic)) * (vec3<shading>(1.0) - specular);
    return (diffuse + radiance * specular) * strength;
}
