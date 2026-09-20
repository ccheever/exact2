struct Globals {
    vp: mat4x4<f32>, light_vp: mat4x4<f32>, eye: vec4<f32>,
    ground: vec4<f32>, params: vec4<u32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var<storage, read> geometry: array<u32>;
@group(1) @binding(1) var<storage, read> clusters: array<u32>;
@group(1) @binding(2) var<storage, read> visible: array<vec2<u32>>;
@group(1) @binding(3) var<storage, read> transforms: array<mat4x4<f32>>;
@group(1) @binding(4) var<uniform> page: vec4<u32>;
@group(1) @binding(5) var<storage, read> draws: array<u32>;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
struct VertexOut {
    @invariant @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) ids: vec4<u32>,
};
fn normal_decode(bits: u32) -> vec3<f32> {
    let x = f32(bitcast<i32>(bits << 16u) >> 16) / 32767.0;
    let y = f32(bitcast<i32>(bits) >> 16) / 32767.0;
    var n = vec3(x,y,1.0-abs(x)-abs(y));
    if n.z < 0.0 {
        // Rust signum preserves positive zero as positive, unlike WGSL sign(0).
        n.x = (1.0-abs(y))*select(-1.0,1.0,x>=0.0);
        n.y = (1.0-abs(x))*select(-1.0,1.0,y>=0.0);
    }
    return normalize(n);
}
fn vertex(p:vec3<f32>, n:u32, color:u32, instance:u32, ids:vec4<u32>) -> VertexOut {
    let model=transforms[instance];
    let world=model*vec4(p,1.0);
    return VertexOut(g.vp*world,world.xyz,normalize((model*vec4(normal_decode(n),0.0)).xyz),unpack4x8unorm(color),ids);
}
fn visible_offset(slot:u32)->u32 {
    if g.params.z!=0u { return slot+draws[page.y*8u+4u]; }
    return slot;
}
fn pulled(v:u32, slot:u32) -> VertexOut {
    let item=visible[visible_offset(slot)]; let base=item.x*32u;
    let count=clusters[base+27u];
    if v>=count*3u {
        // All three padding vertices coincide beyond the clip volume: no fragments.
        return VertexOut(vec4(2.0,2.0,2.0,1.0),vec3(0.0),vec3(0.0,0.0,1.0),vec4(1.0),vec4(0u));
    }
    let byte=page.x+clusters[base+26u]+v;
    let local=(geometry[byte/4u]>>((byte%4u)*8u))&255u;
    let address=(clusters[base+24u]+local)*5u;
    let p=vec3(bitcast<f32>(geometry[address]),bitcast<f32>(geometry[address+1u]),bitcast<f32>(geometry[address+2u]));
    return vertex(p,geometry[address+3u],geometry[address+4u],item.y,vec4(item.x,clusters[base+28u],v/3u,item.y));
}
@vertex fn pull(@builtin(vertex_index) v:u32,@builtin(instance_index) slot:u32)->VertexOut { return pulled(v,slot); }
@vertex fn shadow_pull(@builtin(vertex_index) v:u32,@builtin(instance_index) slot:u32)->@invariant @builtin(position) vec4<f32> {
    let p=pulled(v,slot);
    if v>=clusters[visible[visible_offset(slot)].x*32u+27u]*3u { return vec4(2.0,2.0,2.0,1.0); }
    return g.light_vp*vec4(p.world,1.0);
}
struct Input { @location(0) position:vec3<f32>, @location(1) normal:u32, @location(2) color:u32 };
@vertex fn indexed(v:Input,@builtin(vertex_index) id:u32,@builtin(instance_index) instance:u32)->VertexOut {
    return vertex(v.position,v.normal,v.color,instance,vec4(0u,0u,id,instance));
}
@vertex fn shadow_indexed(v:Input,@builtin(instance_index) instance:u32)->@invariant @builtin(position) vec4<f32> {
    let world=transforms[instance]*vec4(v.position,1.0);
    return g.light_vp*world;
}
@vertex fn ground(@builtin(vertex_index) id:u32)->VertexOut {
    let corners=array(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(-1.0,1.0),vec2(-1.0,1.0),vec2(1.0,-1.0),vec2(1.0,1.0));
    let p=vec3(g.ground.xy+corners[id]*g.ground.w,g.ground.z);
    return VertexOut(g.vp*vec4(p,1.0),p,vec3(0.0,0.0,1.0),vec4(0.23,0.27,0.30,1.0),vec4(0xffffffffu));
}
fn hash_color(value:u32)->vec3<f32> {
    var x=value+1u; x=(x^(x>>16u))*0x7feb352du; x=(x^(x>>15u))*0x846ca68bu; x=x^(x>>16u);
    return vec3(f32(x&255u),f32((x>>8u)&255u),f32((x>>16u)&255u))/255.0*0.8+0.15;
}
fn shadow(world:vec3<f32>, n:vec3<f32>)->f32 {
    let p=g.light_vp*vec4(world+n*0.001,1.0); let q=p.xyz/p.w;
    let uv=q.xy*vec2(0.5,-0.5)+0.5;
    if any(uv<vec2(0.0)) || any(uv>vec2(1.0)) || q.z<0.0 || q.z>1.0 { return 1.0; }
    var sum=0.0;
    for(var y=-1;y<=1;y++) { for(var x=-1;x<=1;x++) {
        sum+=textureSampleCompareLevel(shadow_map,shadow_sampler,uv+vec2(f32(x),f32(y))/2048.0,q.z-0.00025);
    }}
    return sum/9.0;
}
fn filmic(x:vec3<f32>)->vec3<f32> { return clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14),vec3(0.0),vec3(1.0)); }
@fragment fn shade(v:VertexOut)->@location(0) vec4<f32> {
    let n=normalize(v.normal); let l=normalize(vec3(-0.5,-0.6,1.0));
    let is_ground=v.ids.x==0xffffffffu;
    if !is_ground {
        switch g.params.x {
            case 1u: {return vec4(hash_color(v.ids.x),1.0);}
            case 2u: {let d=f32(v.ids.y)/max(f32(g.params.y),1.0);return vec4(d,0.25+0.6*(1.0-d),1.0-d,1.0);}
            case 3u: {return vec4(hash_color(v.ids.x*131u+v.ids.z),1.0);}
            case 4u: {return vec4(hash_color(v.ids.w),1.0);}
            default: {}
        }
    }
    let eye=normalize(g.eye.xyz-v.world); let h=normalize(l+eye);
    let nl=max(dot(n,l),0.0); let nv=max(dot(n,eye),0.001); let nh=max(dot(n,h),0.0); let vh=max(dot(eye,h),0.0);
    let rough=select(0.32,0.8,is_ground); let a=rough*rough; let a2=a*a;
    let d=a2/(3.14159265*pow(nh*nh*(a2-1.0)+1.0,2.0)); let k=pow(rough+1.0,2.0)/8.0;
    let geometry_term=(nl/(nl*(1.0-k)+k))*(nv/(nv*(1.0-k)+k));
    let fresnel=0.04+0.96*pow(1.0-vh,5.0);
    let spec=d*geometry_term*fresnel/max(4.0*nl*nv,0.001);
    let base=select(v.color.rgb*vec3(0.78,0.72,0.62),v.color.rgb,is_ground);
    let sky=mix(vec3(0.13,0.12,0.11),vec3(0.34,0.42,0.53),n.z*0.5+0.5);
    let illumination=shadow(v.world,n);
    let color=base*sky+(base/3.14159265+vec3(spec))*nl*vec3(3.4,3.2,2.9)*illumination;
    return vec4(filmic(color),1.0);
}
@fragment fn overdraw()->@location(0) vec4<f32> { return vec4(0.04,0.013,0.002,1.0); }
