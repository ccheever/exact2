@group(0) @binding(0) var<storage,read> rig:array<u32>;
@group(0) @binding(1) var<storage,read> poses:array<f32>;
@group(0) @binding(2) var<storage,read> jobs:array<vec4<u32>>;
// Four explicit columns preserve the 64-byte matrix ABI. Adreno 620 misaddresses
// storage array<mat4x4<f32>> columns, even without dependent dispatches.
@group(0) @binding(3) var<storage,read_write> palette:array<vec4<f32>>;
fn store_palette(i:u32,m:mat4x4<f32>) {let b=i*4u; palette[b]=m[0]; palette[b+1u]=m[1]; palette[b+2u]=m[2]; palette[b+3u]=m[3];}
struct SkinFrame { view:mat4x4<f32>, camera_alpha:vec4<f32> }
@group(0) @binding(4) var<uniform> frame:SkinFrame;
fn rotate_skin(q:vec4<f32>,v:vec3<f32>)->vec3<f32> { return v+2.0*cross(q.xyz,cross(q.xyz,v)+q.w*v); }
fn rotation(q:vec4<f32>)->vec4<f32> {return q*inverseSqrt(max(dot(q,q),1e-12));}
fn slerp_skin(a:vec4<f32>,v:vec4<f32>,t:f32)->vec4<f32> {
    let b=select(v,-v,dot(a,v)<0.0); let d=clamp(dot(a,b),0.0,1.0);
    if d>0.9995 {return rotation(mix(a,b,t));}
    let angle=acos(d); return rotation((sin((1.0-t)*angle)*a+sin(t*angle)*b)/sin(angle));
}
// Attachments use this same interpolate-local-then-compose rule (skinning.rs).
fn local_pose(p:u32,c:u32)->mat4x4<f32> {
    let a=frame.camera_alpha.w;
    let t=mix(vec3(poses[p],poses[p+1u],poses[p+2u]),vec3(poses[c],poses[c+1u],poses[c+2u]),a);
    let q=slerp_skin(rotation(vec4(poses[p+3u],poses[p+4u],poses[p+5u],poses[p+6u])),rotation(vec4(poses[c+3u],poses[c+4u],poses[c+5u],poses[c+6u])),a);
    let s=mix(vec3(poses[p+7u],poses[p+8u],poses[p+9u]),vec3(poses[c+7u],poses[c+8u],poses[c+9u]),a);
    return mat4x4(vec4(rotate_skin(q,vec3(s.x,0.0,0.0)),0.0),vec4(rotate_skin(q,vec3(0.0,s.y,0.0)),0.0),vec4(rotate_skin(q,vec3(0.0,0.0,s.z)),0.0),vec4(t,1.0));
}
fn column(i:u32)->vec4<f32> {return bitcast<vec4<f32>>(vec4(rig[i],rig[i+1u],rig[i+2u],rig[i+3u]));}
// One lane per joint. A joint's node matrix is composed root first, each parent's
// interpolated local pose computed as it is needed, so a lane reads no other
// lane's work and a job computes only the nodes its joints hang from. The
// products associate as a parent-before-child walk of the hierarchy would.
@compute @workgroup_size(32) fn skin(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
    let job=jobs[group.x]; let count=rig[job.x]; let joints=rig[job.x+1u];
    let base=job.x+4u+count*2u; let parents=base+joints*17u;
    for(var j=lane;j<joints;j+=32u) {
        let node=rig[base+j];
        // Bounded by the node count: a malformed rig's parent cycle ends here.
        var depth=0u;
        for(var n=rig[parents+node];n!=4294967295u&&depth<count;n=rig[parents+n]) {depth++;}
        var global=mat4x4<f32>();
        for(var k=0u;k<=depth;k++) {
            var at=node;
            for(var up=k;up<depth;up++) {at=rig[parents+at];}
            let local=local_pose(job.y+at*10u,job.y+(count+at)*10u);
            if k==0u {global=local;} else {global=global*local;}
        }
        let i=base+joints+j*16u;
        store_palette(job.z+j,global*mat4x4(column(i),column(i+4u),column(i+8u),column(i+12u)));
    }
}
