const OUTLINE_OFFSET: bool = false;
struct ModelInstance {
    transform: u32, material: u32, geometry: u32, palette: u32,
    local: mat4x4<f32>, normal: mat4x4<f32>,
    // NodeMaterials: base-colour multiplier and added emission; glow.w's low
    // 30 bits are 1 + the first per-part look of a merged draw (0: none), its
    // bit 30 says the draw's parts have several materials, numbered in uv.x,
    // and its top bit makes the tint replace the material's base colour factor
    // (MaterialOverrides). In a part look entry, glow.w's bits are the part's
    // first vertex, and with several materials local's first two columns are
    // its material (models/merge.rs `PartMaterial`).
    tint: vec4<f32>, glow: vec4<f32>,
}
@group(3) @binding(0) var<storage, read> instances: array<ModelInstance>;
// Four explicit columns preserve the 64-byte matrix ABI. Adreno 620 misaddresses
// storage array<mat4x4<f32>> columns, even without dependent dispatches.
@group(3) @binding(1) var<storage, read> skin_palette: array<vec4<f32>>;
fn skin_matrix(i:u32)->mat4x4<f32> {let b=i*4u; return mat4x4(skin_palette[b],skin_palette[b+1u],skin_palette[b+2u],skin_palette[b+3u]);}
struct SkinVertex { joints:vec4<u32>, weights:vec4<f32> }
@group(3) @binding(2) var<storage, read> skin_vertices: array<SkinVertex>;
// A skinned draw's blended joint matrix at `vertex` (the palette is not u32::MAX).
fn skin_of(draw:ModelInstance,vertex:u32)->mat4x4<f32> {
    if (draw.palette & 2147483648u)!=0u {return skin_matrix(draw.palette & 2147483647u);}
    var m=mat4x4<f32>();
    let v=skin_vertices[vertex];
    // Rigid parts merged into a skin retain exactly one joint.
    if all(v.weights == vec4(1.0,0.0,0.0,0.0)) { return skin_matrix(draw.palette+v.joints.x); }
    for(var i=0u;i<4u;i++) { m+=skin_matrix(draw.palette+v.joints[i])*v.weights[i]; }
    return m;
}
fn skinned_position(draw:ModelInstance,vertex:u32,position:vec3<f32>)->vec3<f32> {
    if draw.palette==4294967295u {return position;}
    return (skin_of(draw,vertex)*vec4(position,1.0)).xyz;
}
fn skinned(draw:ModelInstance,vertex:u32,position:vec3<f32>,normal:vec3<f32>)->mat2x3<f32> {
    if draw.palette==4294967295u {return mat2x3(position,normal);}
    let m=skin_of(draw,vertex);
    let p=(m*vec4(position,1.0)).xyz;
    if OUTLINE_OFFSET {return mat2x3(p,(m*vec4(normal,0.0)).xyz);}
    // Inverse transpose of the blended affine map, including hierarchy shear.
    let cof=mat3x3(cross(m[1].xyz,m[2].xyz),cross(m[2].xyz,m[0].xyz),cross(m[0].xyz,m[1].xyz));
    let det=dot(m[0].xyz,cof[0]);
    // A collapsed joint/weight blend has no inverse; retain a finite authored normal.
    if abs(det)<1e-10 {return mat2x3(p,normal);}
    return mat2x3(p,(cof*normal)/det);
}
struct ModelVarying {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>,
    // Tint is constant within a triangle, so its product with colour interpolates alike.
    @location(4) color: vec4<f32>,
    @location(2) uv: vec2<f32>, @location(3) @interpolate(flat) slot: u32,
    @location(6) @interpolate(flat) glow: vec4<f32>,
    // The part's metallic, roughness and rim factors (ones outside a merged
    // material draw), and its emission times the instance's emission strength.
    @location(8) @interpolate(flat) part: vec4<f32>, @location(9) @interpolate(flat) part_emission: vec3<f32>,
}
// A model draw's transform slot between the two ticks, at the frame's alpha;
// its scale keeps its sign.
fn model_pose(slot:u32) -> Pose {
    let i = slot * 10u;
    let a = frame.camera_alpha.w;
    let p = mix(vec3(prev[i],prev[i+1u],prev[i+2u]), vec3(curr[i],curr[i+1u],curr[i+2u]),a);
    let qp=vec4(prev[i+3u],prev[i+4u],prev[i+5u],prev[i+6u]);
    let qc=vec4(curr[i+3u],curr[i+4u],curr[i+5u],curr[i+6u]);
    let qm=mix(qp,select(qc,-qc,dot(qp,qc)<0.0),a);
    let q=qm*inverseSqrt(max(dot(qm,qm),1e-12));
    let s=mix(vec3(prev[i+7u],prev[i+8u],prev[i+9u]),vec3(curr[i+7u],curr[i+8u],curr[i+9u]),a);
    return Pose(p,q,s);
}
// Where a model vertex is in the world. The colour and depth entry points both
// take their invariant position from it, so the prepass's depth is the colour
// pass's to the bit (its Equal test).
fn model_world(draw:ModelInstance,at:Pose,position:vec3<f32>) -> vec3<f32> {
    let local=(draw.local*vec4(position,1.0)).xyz;
    if attached(draw.transform) { return (attachment_matrices[draw.transform]*vec4(local,1.0)).xyz; }
    return at.p+rotate(at.q,at.s*local);
}
// The camera depth prepass of an opaque model without coverage: position only,
// none of the colour pass's normal, material or look work.
@vertex fn model_depth_vs(@location(0) position: vec3<f32>, @builtin(instance_index) instance:u32, @builtin(vertex_index) vertex:u32) -> @builtin(position) @invariant vec4<f32> {
    let draw = instances[slots[instance] - 2147483648u];
    return frame.view_proj*vec4(model_world(draw,model_pose(draw.transform),skinned_position(draw,vertex,position)),1.0);
}
fn model_transform(position: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>, instance: u32, vertex:u32, color:vec4<f32>) -> ModelVarying {
    let draw = instances[slots[instance] - 2147483648u];
    let slot = draw.transform;
    let posed = model_pose(slot);
    let q = posed.q;
    let s = posed.s;
    let skin=skinned(draw,vertex,position,normal);
    var tint=draw.tint;
    var glow=draw.glow.xyz;
    var part=vec4(1.0);
    var part_emission=vec3(1.0);
    let word=bitcast<u32>(draw.glow.w);
    let looks=word & 1073741823u;
    let numbered=(word & 1073741824u)!=0u;
    if looks!=0u {
        // A merged part's look: the vertex's numbered part, or the last part
        // starting at or before this vertex (a run ends at a u32::MAX start).
        var at=looks-1u;
        if numbered { at+=u32(uv.x); }
        else { while bitcast<u32>(instances[at+1u].glow.w)<=vertex { at+=1u; } }
        let look=instances[at];
        tint*=look.tint;
        glow+=look.glow.xyz;
        if numbered { part=look.local[0]; part_emission=look.local[1].xyz; }
    }
    // Read the instance material per vertex, as the primitive path does. Flat
    // varyings avoid storage-buffer reads for every covered fragment.
    let material=slot*12u;
    tint*=vec4(materials[material],materials[material+1u],materials[material+2u],select(materials[material+3u],1.0,materials[material+3u]<0.0));
    glow+=vec3(materials[material+6u],materials[material+7u],materials[material+8u]);
    let strength=materials[material+9u];
    let world=model_world(draw,posed,skin[0]);
    var local_normal=(draw.normal*vec4(skin[1],0.0)).xyz;
    if OUTLINE_OFFSET { local_normal=(draw.local*vec4(skin[1],0.0)).xyz; }
    if attached(slot) {
        let affine=attachment_matrices[slot];
        var n=affine_normal(affine,local_normal);
        if OUTLINE_OFFSET { n=(affine*vec4(local_normal,0.0)).xyz; }
        return ModelVarying(frame.view_proj*vec4(world,1.0),world,n,color*tint,uv,slot,vec4(glow,f32(word>>31u)),part,part_emission*strength);
    }
    let safe=select(max(abs(s),vec3(0.000001)),-max(abs(s),vec3(0.000001)),s<vec3(0.0));
    var n=rotate(q,local_normal/safe);
    if OUTLINE_OFFSET { n=rotate(q,local_normal*s); }
    return ModelVarying(frame.view_proj*vec4(world,1.0),world,n,color*tint,uv,slot,vec4(glow,f32(word>>31u)),part,part_emission*strength);
}
