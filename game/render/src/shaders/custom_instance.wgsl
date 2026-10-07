// @ref llp/1046.006.000-render-hooks.rfc.md#d4-custom-materials-inside-the-engines-batches-phase-2
struct DrawInstance {
    transform: u32, material: u32, data: u32, palette: u32,
    local: mat4x4f, normal: mat4x4f,
    // The record's tint and added emission (NodeMaterials, MaterialOverrides).
    // glow.w's bits point at a merged draw's per-part looks; custom shaders
    // draw unmerged, so it is zero here.
    tint: vec4f, glow: vec4f,
}
@group(3) @binding(0) var<storage, read> draw_instances: array<DrawInstance>;
fn draw_instance(i:u32) -> DrawInstance { return draw_instances[slots[i]-2147483648u]; }
// Same tick interpolation as engine geometry, with full affine node offsets.
fn instance_transform(position:vec3f, normal:vec3f, i:u32, color:vec4f) -> Varying {
    let draw=draw_instance(i); let slot=draw.transform; let t=slot*10u;
    let alpha=frame.camera_alpha.w;
    let p=mix(vec3f(prev[t],prev[t+1u],prev[t+2u]),vec3f(curr[t],curr[t+1u],curr[t+2u]),alpha);
    let qa=vec4f(prev[t+3u],prev[t+4u],prev[t+5u],prev[t+6u]);
    let qb=vec4f(curr[t+3u],curr[t+4u],curr[t+5u],curr[t+6u]);
    let q=normalize(mix(qa,select(qb,-qb,dot(qa,qb)<0.0),alpha));
    let s=mix(vec3f(prev[t+7u],prev[t+8u],prev[t+9u]),vec3f(curr[t+7u],curr[t+8u],curr[t+9u]),alpha);
    let local=(draw.local*vec4f(position,1.0)).xyz;
    let n=(draw.normal*vec4f(normal,0.0)).xyz;
    var world=p+rotate(q,s*local); var wn=rotate(q,n/select(max(abs(s),vec3f(1e-6)),-max(abs(s),vec3f(1e-6)),s<vec3f(0.0)));
    if attached(slot) { let m=attachment_matrices[slot]; world=(m*vec4f(local,1.0)).xyz; wn=affine_normal(m,n); }
    return Varying(frame.view_proj*vec4f(world,1.0),world,wn,slot,color);
}
