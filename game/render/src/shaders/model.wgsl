struct ModelInstance {
    transform: u32, material: u32, geometry: u32, palette: u32,
    local: mat4x4<f32>, normal: mat4x4<f32>,
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
struct BakedMaterial {
    base: vec4<f32>, surface: vec4<f32>, emission_cutoff: vec4<f32>, flags: vec4<f32>,
    uv: array<vec4<f32>, 10>,
}
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
struct ModelVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(4) color: vec4<f32>,
    @location(2) uv: vec2<f32>, @location(3) @interpolate(flat) slot: u32,
}
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
    let local=(draw.local*vec4(skin[0],1.0)).xyz;
    if attached(slot) {
        let affine=attachment_matrices[slot];
        let world=(affine*vec4(local,1.0)).xyz;
        let n=affine_normal(affine,(draw.normal*vec4(skin[1],0.0)).xyz);
        return ModelVarying(frame.view_proj*vec4(world,1.0),world,n,color,uv,slot);
    }
    let world=p+rotate(q,s*local);
    let safe=select(max(abs(s),vec3(0.000001)),-max(abs(s),vec3(0.000001)),s<vec3(0.0));
    let n=rotate(q,(draw.normal*vec4(skin[1],0.0)).xyz/safe);
    return ModelVarying(frame.view_proj*vec4(world,1.0),world,n,color,uv,slot);
}
@vertex fn model_vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) color:vec4<f32>, @builtin(instance_index) instance:u32, @builtin(vertex_index) vertex:u32) -> ModelVarying {
    return model_transform(position,normal,uv,instance,vertex,color);
}
fn material_uv(uv: vec2<f32>, index:u32) -> vec2<f32> {
    let t=baked.uv[index*2u]; let o=baked.uv[index*2u+1u];
    return mat2x2(t.xy,t.zw)*uv+o.xy;
}
fn model_base(input:ModelVarying) -> vec4<f32> {
    let i=input.slot*12u;
    return textureSample(base_texture,base_sampler,material_uv(input.uv,0u))*baked.base*input.color*vec4(materials[i],materials[i+1u],materials[i+2u],select(materials[i+3u],1.0,materials[i+3u]<0.0));
}
fn mapped_normal(input:ModelVarying, front:bool) -> vec3<f32> {
    let n=normalize(input.normal)*select(-1.0,1.0,front);
    let uv=material_uv(input.uv,1u);
    let dp1=dpdx(input.world); let dp2=dpdy(input.world);
    let duv1=dpdx(uv); let duv2=dpdy(uv);
    // Cotangent frame from screen derivatives: also covers absent baked tangents.
    let p2=cross(dp2,n); let p1=cross(n,dp1);
    let t=p2*duv1.x+p1*duv2.x; let b=p2*duv1.y+p1*duv2.y;
    let length2=max(dot(t,t),dot(b,b));
    // XY only, Z rebuilt: BC5 and two-channel ASTC carry no Z, and RGBA8 reads alike.
    let xy=textureSample(normal_texture,normal_sampler,uv).xy*2.0-1.0;
    let sampled=vec3(xy,sqrt(max(1.0-dot(xy,xy),0.0)));
    if length2 < 1e-16 { return n; }
    return normalize((t*sampled.x+b*sampled.y)*baked.surface.z*inverseSqrt(length2)+n*sampled.z);
}
@diagnostic(off, derivative_uniformity)
fn model_shade(input:ModelVarying, front:bool, visibility:f32) -> vec4<f32> {
    let base=model_base(input);
    let mr=textureSample(mr_texture,mr_sampler,material_uv(input.uv,2u));
    let emission=textureSample(emission_texture,emission_sampler,material_uv(input.uv,3u)).rgb;
    let ao=mix(1.0,textureSample(ao_texture,ao_sampler,material_uv(input.uv,4u)).r,baked.surface.w);
    let n=mapped_normal(input,front);
    if baked.flags.x==1.0 && base.a < baked.emission_cutoff.w { discard; }
    let metallic=clamp(baked.surface.x*mr.b,0.0,1.0);
    let roughness=clamp(baked.surface.y*mr.g,0.045,1.0);
    let v=normalize(frame.camera_alpha.xyz-input.world);
    let i=input.slot*12u;
    let glow=vec3(materials[i+6u],materials[i+7u],materials[i+8u]);
    var color=ambient(n,v,base.rgb,metallic,roughness)*ao+emission*baked.emission_cutoff.rgb*materials[i+9u]+glow;
    if frame.sun_direction_illuminance.w>0.0 {
        color+=brdf(n,v,normalize(-frame.sun_direction_illuminance.xyz),base.rgb,metallic,roughness)*frame.sun_color_count.xyz*frame.sun_direction_illuminance.w*visibility;
    }
    if frame.fill_direction_illuminance.w>0.0 { color+=fill_light(n,v,base.rgb,metallic,roughness); }
    add_local_lights(&color,input.clip.xy,input.world,n,v,base.rgb,metallic,roughness,true);
    if FOG { color=height_fog(color,input.world); }
    return vec4(color,select(1.0,base.a,baked.flags.x==2.0));
}
@fragment fn model_fs(input:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {return model_shade(input,front,1.0);}
@fragment fn model_fs_shadow(input:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {return model_shade(input,front,sun_visibility(input.world,normalize(input.normal)));}
