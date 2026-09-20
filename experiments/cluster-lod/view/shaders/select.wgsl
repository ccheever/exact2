struct Config {
    planes: array<vec4<f32>,6>, eye: vec4<f32>, direction: vec4<f32>, sphere: vec4<f32>,
    projection: vec4<f32>, // height, cot, near, ortho span (zero means perspective)
    sizes: vec4<u32>, // clusters, instances, pages, quota
    options: vec4<u32>, // culling, brute force, max triangles, unused
    threshold: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cfg: Config;
@group(0) @binding(1) var<storage,read> clusters: array<u32>;
@group(0) @binding(2) var<storage,read> transforms: array<mat4x4<f32>>;
@group(0) @binding(3) var<storage,read> envelopes: array<vec2<f32>>;
@group(0) @binding(4) var<storage,read_write> scratch: array<u32>;
// Each instance/page: count, destination offset, source offset. Then four stats per instance.
@group(0) @binding(5) var<storage,read_write> counts: array<atomic<u32>>;
// Each page: four indirect words, visible base, three unused. Then four totals.
@group(0) @binding(6) var<storage,read_write> draws: array<u32>;
@group(0) @binding(7) var<storage,read_write> visible: array<vec2<u32>>;
fn f(base:u32)->f32 { return bitcast<f32>(clusters[base]); }
fn v3(base:u32)->vec3<f32> { return vec3(f(base),f(base+1u),f(base+2u)); }
fn projected(base:u32,model:mat4x4<f32>,scale:f32)->f32 {
    let error=f(base+4u);
    if error==bitcast<f32>(0x7f7fffffu) { return error; }
    if cfg.projection.w>0.0 { return error*scale*cfg.projection.x/cfg.projection.w; }
    let center=(model*vec4(v3(base),1.0)).xyz;
    let distance=max(length(center-cfg.eye.xyz)-f(base+3u)*scale,cfg.projection.z);
    return error*scale/distance*(cfg.projection.y*0.5*cfg.projection.x);
}
fn sphere_visible(sphere:vec4<f32>,model:mat4x4<f32>,scale:f32)->bool {
    let center=(model*vec4(sphere.xyz,1.0)).xyz;
    let radius=sphere.w*scale;
    for(var i=0u;i<6u;i++) {
        let p=cfg.planes[i];
        if dot(p.xyz,center)+p.w < -radius-1e-5 { return false; }
    }
    return true;
}
fn cone_visible(base:u32,model:mat4x4<f32>,scale:f32)->bool {
    let cutoff=f(base+17u);
    if cutoff>=1.0 { return true; }
    let axis=(model*vec4(v3(base+18u),0.0)).xyz/scale;
    var view=cfg.direction.xyz;
    if cfg.projection.w==0.0 {
        let delta=(model*vec4(v3(base+14u),1.0)).xyz-cfg.eye.xyz;
        if dot(delta,delta)<1e-20 { return true; }
        view=normalize(delta);
    }
    return dot(view,axis)<cutoff+1e-5;
}
fn selected(id:u32,model:mat4x4<f32>,scale:f32)->bool {
    let b=id*32u;
    if !(projected(b+4u,model,scale)>cfg.threshold.x) { return false; }
    if clusters[b+22u]!=0xffffffffu && projected(b+9u,model,scale)>cfg.threshold.x { return false; }
    if cfg.options.x!=0u && (!sphere_visible(vec4(v3(b),f(b+3u)),model,scale) || !cone_visible(b,model,scale)) { return false; }
    return true;
}
var<workgroup> first:u32;
var<workgroup> end:u32;
var<workgroup> total:u32;
var<workgroup> scan:array<u32,256>;
@compute @workgroup_size(256)
fn choose(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
    let instance=group.x;
    let model=transforms[instance];
    let scale=length(model[0].xyz);
    let stat=cfg.sizes.y*cfg.sizes.z*3u+instance*4u;
    if lane==0u {
        first=0u; end=cfg.sizes.x; total=0u;
        if cfg.options.x!=0u && !sphere_visible(cfg.sphere,model,scale) { end=0u; }
        if end>0u && cfg.options.y==0u {
            let center=(model*vec4(cfg.sphere.xyz,1.0)).xyz;
            let radius=cfg.sphere.w*scale;
            let distance=length(center-cfg.eye.xyz);
            var low=cfg.threshold.x*max(distance-radius,cfg.projection.z)/(scale*(cfg.projection.y*0.5*cfg.projection.x));
            var high=cfg.threshold.x*max(distance+radius,cfg.projection.z)/(scale*(cfg.projection.y*0.5*cfg.projection.x));
            if cfg.projection.w>0.0 {
                low=cfg.threshold.x*cfg.projection.w/(scale*cfg.projection.x); high=low;
            }
            low=low*(1.0-1e-5); high=high*(1.0+1e-5);
            var a=0u; var b=cfg.sizes.x;
            while a<b { let m=(a+b)/2u; if envelopes[m].x>low { a=m+1u; } else { b=m; } }
            end=a; a=0u; b=end;
            while a<b { let m=(a+b)/2u; if envelopes[m].y>high { a=m+1u; } else { b=m; } }
            first=a;
        }
        atomicStore(&counts[stat],end-first);
    }
    workgroupBarrier();
    for(var block=first;block<end;block+=256u) {
        let id=block+lane;
        var keep=false;
        if id<end { keep=selected(id,model,scale); }
        scan[lane]=select(0u,1u,keep);
        workgroupBarrier();
        for(var step=1u;step<256u;step*=2u) {
            var previous=0u;
            if lane>=step { previous=scan[lane-step]; }
            workgroupBarrier();
            scan[lane]+=previous;
            workgroupBarrier();
        }
        let offset=total+scan[lane]-1u;
        if keep && offset<cfg.sizes.w {
            scratch[instance*cfg.sizes.w+offset]=id;
            atomicAdd(&counts[(instance*cfg.sizes.z+clusters[id*32u+23u])*3u],1u);
            atomicAdd(&counts[stat+3u],clusters[id*32u+27u]);
        }
        workgroupBarrier();
        if lane==0u { total+=scan[255]; }
        workgroupBarrier();
    }
    if lane==0u {
        atomicStore(&counts[stat+1u],min(total,cfg.sizes.w));
        atomicStore(&counts[stat+2u],total-min(total,cfg.sizes.w));
    }
}
// One invocation/page. Page-local prefix across instances preserves CPU draw order.
@compute @workgroup_size(1)
fn page_prefix(@builtin(global_invocation_id) id:vec3<u32>) {
    let page=id.x;
    var total=0u;
    for(var instance=0u;instance<cfg.sizes.y;instance++) {
        let cell=(instance*cfg.sizes.z+page)*3u;
        atomicStore(&counts[cell+1u],total);
        total+=atomicLoad(&counts[cell]);
    }
    draws[page*8u]=cfg.options.z*3u;
    draws[page*8u+1u]=total;
    draws[page*8u+2u]=0u; draws[page*8u+3u]=0u;
}
@compute @workgroup_size(1)
fn finish() {
    var offset=0u;
    for(var page=0u;page<cfg.sizes.z;page++) { draws[page*8u+4u]=offset; offset+=draws[page*8u+1u]; }
    let stat=cfg.sizes.z*8u;
    draws[stat]=offset; draws[stat+1u]=0u; draws[stat+2u]=0u; draws[stat+3u]=0u;
    for(var instance=0u;instance<cfg.sizes.y;instance++) {
        let src=cfg.sizes.y*cfg.sizes.z*3u+instance*4u;
        draws[stat+1u]+=atomicLoad(&counts[src+3u]);
        draws[stat+2u]+=atomicLoad(&counts[src]);
        draws[stat+3u]+=atomicLoad(&counts[src+2u]);
    }
}
@compute @workgroup_size(256)
fn scatter(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
    let instance=group.x;
    if lane==0u {
        var start=0u;
        for(var page=0u;page<cfg.sizes.z;page++) {
            let cell=(instance*cfg.sizes.z+page)*3u;
            atomicStore(&counts[cell+2u],start); start+=atomicLoad(&counts[cell]);
        }
    }
    storageBarrier();
    let total=atomicLoad(&counts[cfg.sizes.y*cfg.sizes.z*3u+instance*4u+1u]);
    for(var i=lane;i<total;i+=256u) {
        let id=scratch[instance*cfg.sizes.w+i]; let page=clusters[id*32u+23u];
        let cell=(instance*cfg.sizes.z+page)*3u;
        let dest=draws[page*8u+4u]+atomicLoad(&counts[cell+1u])+i-atomicLoad(&counts[cell+2u]);
        visible[dest]=vec2(id,instance);
    }
}
