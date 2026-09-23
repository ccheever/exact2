// Ambient light from the environment: SH9 diffuse irradiance (frame.irradiance)
// and split-sum specular from the prefiltered radiance cube (ibl.rs).
// @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
@group(0) @binding(6) var environment_radiance: texture_cube<f32>;
@group(0) @binding(7) var environment_sampler: sampler;
const ENVIRONMENT_LODS: f32 = 5.0; // ibl.rs MIPS - 1: lod = roughness × this

// Outgoing radiance per unit albedo of a Lambertian surface facing n.
// Indexes the uniform directly: a local copy of the array can stay in memory per fragment.
fn irradiance(n: vec3<f32>) -> vec3<f32> {
    let e = frame.irradiance[0].xyz + frame.irradiance[1].xyz * n.y + frame.irradiance[2].xyz * n.z
        + frame.irradiance[3].xyz * n.x + frame.irradiance[4].xyz * (n.x * n.y)
        + frame.irradiance[5].xyz * (n.y * n.z) + frame.irradiance[6].xyz * (3.0 * n.z * n.z - 1.0)
        + frame.irradiance[7].xyz * (n.x * n.z) + frame.irradiance[8].xyz * (n.x * n.x - n.y * n.y);
    return max(e, vec3(0.0));
}
// Karis's analytic fit of the preintegrated environment BRDF: F0 scale and bias.
fn environment_brdf(roughness: f32, nv: f32) -> vec2<f32> {
    let r = roughness * vec4(-1.0, -0.0275, -0.572, 0.022) + vec4(1.0, 0.0425, 1.04, -0.04);
    let a004 = min(r.x * r.x, exp2(-9.28 * nv)) * r.x + r.y;
    return vec2(-1.04, 1.04) * a004 + r.zw;
}
// Diffuse plus specular ambient light, before occlusion and exposure.
fn ambient(n: vec3<f32>, v: vec3<f32>, base: vec3<f32>, metallic: f32, roughness: f32) -> vec3<f32> {
    let ab = environment_brdf(roughness, max(dot(n, v), 0.0001));
    let specular = mix(vec3(0.04), base, metallic) * ab.x + ab.y;
    let radiance = textureSampleLevel(environment_radiance, environment_sampler,
        reflect(-v, n), roughness * ENVIRONMENT_LODS).rgb;
    let diffuse = irradiance(n) * base * (1.0 - metallic) * (vec3(1.0) - specular);
    return (diffuse + radiance * specular) * frame.zenith_ambient.w;
}
