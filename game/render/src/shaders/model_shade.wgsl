// The engine's model shading, shared by model.wgsl and game custom materials
// (hooks::MATERIAL_SHADOWS_WGSL's `material_shade`). After model_base.wgsl.
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
fn model_lit(input:ModelVarying, front:bool, visibility:f32, fog:bool) -> vec4<f32> {
    // Opaque materials fade by dither; blended ones multiply alpha below.
    if baked.flags.x!=2.0 && faded(input.slot,input.clip.xy) { discard; }
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
    var color=ambient(n,v,base.rgb,metallic,roughness)*ao+emission*baked.emission_cutoff.rgb*materials[i+9u]+glow+input.glow.xyz;
    if frame.sun_direction_illuminance.w>0.0 {
        color+=brdf(n,v,normalize(-frame.sun_direction_illuminance.xyz),base.rgb,metallic,roughness)*frame.sun_color_count.xyz*frame.sun_direction_illuminance.w*visibility;
    }
    if frame.fill_direction_illuminance.w>0.0 { color+=fill_light(n,v,base.rgb,metallic,roughness); }
    add_local_lights(&color,input.clip.xy,input.world,n,v,base.rgb,metallic,roughness,true);
    if fog { color=height_fog(color,input.world); }
    return vec4(color,select(1.0,base.a*(1.0-fade_of(input.slot)),baked.flags.x==2.0));
}
