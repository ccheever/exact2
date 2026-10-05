struct ModelInstance {
    transform: u32, material: u32, geometry: u32, palette: u32,
    local: mat4x4<f32>, normal: mat4x4<f32>,
    // NodeMaterials: base-colour multiplier and added emission; glow.w's low
    // 31 bits are 1 + the first per-part look of a merged draw (0: none), and
    // its top bit makes the tint replace the material's base colour factor
    // (MaterialOverrides). In a part look entry, glow.w's bits are the part's
    // first vertex, and a run's first entry's `transform` is its part count.
    tint: vec4<f32>, glow: vec4<f32>,
}
@group(3) @binding(0) var<storage, read> instances: array<ModelInstance>;
@group(3) @binding(1) var<storage, read> skin_palette: array<mat4x4<f32>>;
struct SkinVertex { joints:vec4<u32>, weights:vec4<f32> }
@group(3) @binding(2) var<storage, read> skin_vertices: array<SkinVertex>;
fn skinned(draw:ModelInstance,vertex:u32,position:vec3<f32>,normal:vec3<f32>)->mat2x3<f32> {
    if draw.palette==4294967295u {return mat2x3(position,normal);}
    var m=mat4x4<f32>();
    if (draw.palette & 2147483648u)!=0u {m=skin_palette[draw.palette & 2147483647u];}
    else {
        let v=skin_vertices[vertex];
        for(var i=0u;i<4u;i++) { m+=skin_palette[draw.palette+v.joints[i]]*v.weights[i]; }
    }
    let p=(m*vec4(position,1.0)).xyz;
    // Inverse transpose of the blended affine map, including hierarchy shear.
    let cof=mat3x3(cross(m[1].xyz,m[2].xyz),cross(m[2].xyz,m[0].xyz),cross(m[0].xyz,m[1].xyz));
    let det=dot(m[0].xyz,cof[0]);
    // A collapsed joint/weight blend has no inverse; retain a finite authored normal.
    if abs(det)<1e-10 {return mat2x3(p,normal);}
    return mat2x3(p,(cof*normal)/det);
}
// The material at group 2; model_base.wgsl and model_shade.wgsl read it (game custom
// materials bind the same at group 3, custom_instance.wgsl).
@group(2) @binding(0) var<uniform> baked: BakedMaterial;
@group(2) @binding(1) var base_texture: texture_2d<f32>;
@group(2) @binding(2) var base_sampler: sampler;
@group(2) @binding(3) var normal_texture: texture_2d<f32>;
@group(2) @binding(4) var normal_sampler: sampler;
@group(2) @binding(5) var mr_texture: texture_2d<f32>;
@group(2) @binding(6) var mr_sampler: sampler;
@group(2) @binding(7) var emission_texture: texture_2d<f32>;
@group(2) @binding(8) var emission_sampler: sampler;
@group(2) @binding(9) var ao_texture: texture_2d<f32>;
@group(2) @binding(10) var ao_sampler: sampler;
fn model_transform(position: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>, instance: u32, vertex:u32, color:vec4<f32>) -> ModelVarying {
    let draw = instances[slots[instance] - 2147483648u];
    let slot = draw.transform;
    let i = slot * 10u;
    let a = frame.camera_alpha.w;
    let p = mix(vec3(prev[i],prev[i+1u],prev[i+2u]), vec3(curr[i],curr[i+1u],curr[i+2u]),a);
    let qp=vec4(prev[i+3u],prev[i+4u],prev[i+5u],prev[i+6u]);
    let qc=vec4(curr[i+3u],curr[i+4u],curr[i+5u],curr[i+6u]);
    let qm=mix(qp,select(qc,-qc,dot(qp,qc)<0.0),a);
    let q=qm*inverseSqrt(max(dot(qm,qm),1e-12));
    let s=mix(vec3(prev[i+7u],prev[i+8u],prev[i+9u]),vec3(curr[i+7u],curr[i+8u],curr[i+9u]),a);
    let skin=skinned(draw,vertex,position,normal);
    var tint=draw.tint;
    var glow=draw.glow.xyz;
    let word=bitcast<u32>(draw.glow.w);
    let looks=word & 2147483647u;
    if looks!=0u {
        // A merged part's look: the last part starting at or before this vertex,
        // found by binary search (the run's first entry holds its part count).
        var at=looks-1u;
        var end=at+instances[at].transform;
        while end-at>1u {
            let mid=(at+end)/2u;
            if bitcast<u32>(instances[mid].glow.w)<=vertex {at=mid;} else {end=mid;}
        }
        let look=instances[at];
        tint*=look.tint;
        glow+=look.glow.xyz;
    }
    let local=(draw.local*vec4(skin[0],1.0)).xyz;
    if attached(slot) {
        let affine=attachment_matrices[slot];
        let world=(affine*vec4(local,1.0)).xyz;
        let n=affine_normal(affine,(draw.normal*vec4(skin[1],0.0)).xyz);
        return ModelVarying(frame.view_proj*vec4(world,1.0),world,n,color,uv,slot,tint,vec4(glow,f32(word>>31u)));
    }
    let world=p+rotate(q,s*local);
    let safe=select(max(abs(s),vec3(0.000001)),-max(abs(s),vec3(0.000001)),s<vec3(0.0));
    let n=rotate(q,(draw.normal*vec4(skin[1],0.0)).xyz/safe);
    return ModelVarying(frame.view_proj*vec4(world,1.0),world,n,color,uv,slot,tint,vec4(glow,f32(word>>31u)));
}
@vertex fn model_vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) color:vec4<f32>, @builtin(instance_index) instance:u32, @builtin(vertex_index) vertex:u32) -> ModelVarying {
    return model_transform(position,normal,uv,instance,vertex,color);
}
@fragment fn model_fs(input:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {return model_lit(input,front,1.0,FOG);}
@fragment fn model_fs_shadow(input:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {return model_lit(input,front,sun_visibility(input.world,normalize(input.normal)),FOG);}
