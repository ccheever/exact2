@group(0) @binding(0) var<storage,read> rig:array<u32>;
@group(0) @binding(1) var<storage,read> poses:array<f32>;
@group(0) @binding(2) var<storage,read> jobs:array<vec4<u32>>;
@group(0) @binding(3) var<storage,read_write> palette:array<mat4x4<f32>>;
struct SkinFrame { view:mat4x4<f32>, camera_alpha:vec4<f32> }
@group(0) @binding(4) var<uniform> frame:SkinFrame;
override JOINT_CAPACITY:u32=256;
var<workgroup> globals:array<mat4x4<f32>,JOINT_CAPACITY>;
fn rotate_skin(q:vec4<f32>,v:vec3<f32>)->vec3<f32> { return v+2.0*cross(q.xyz,cross(q.xyz,v)+q.w*v); }
fn rotation(q:vec4<f32>)->vec4<f32> {return q*inverseSqrt(max(dot(q,q),1e-12));}
fn slerp_skin(a:vec4<f32>,v:vec4<f32>,t:f32)->vec4<f32> {
    let b=select(v,-v,dot(a,v)<0.0); let d=clamp(dot(a,b),0.0,1.0);
    if d>0.9995 {return rotation(mix(a,b,t));}
    let angle=acos(d); return rotation((sin((1.0-t)*angle)*a+sin(t*angle)*b)/sin(angle));
}
fn local_pose(p:u32,c:u32)->mat4x4<f32> {
    let a=frame.camera_alpha.w;
    let t=mix(vec3(poses[p],poses[p+1u],poses[p+2u]),vec3(poses[c],poses[c+1u],poses[c+2u]),a);
    let q=slerp_skin(rotation(vec4(poses[p+3u],poses[p+4u],poses[p+5u],poses[p+6u])),rotation(vec4(poses[c+3u],poses[c+4u],poses[c+5u],poses[c+6u])),a);
    let s=mix(vec3(poses[p+7u],poses[p+8u],poses[p+9u]),vec3(poses[c+7u],poses[c+8u],poses[c+9u]),a);
    return mat4x4(vec4(rotate_skin(q,vec3(s.x,0.0,0.0)),0.0),vec4(rotate_skin(q,vec3(0.0,s.y,0.0)),0.0),vec4(rotate_skin(q,vec3(0.0,0.0,s.z)),0.0),vec4(t,1.0));
}
fn column(i:u32)->vec4<f32> {return bitcast<vec4<f32>>(vec4(rig[i],rig[i+1u],rig[i+2u],rig[i+3u]));}
@compute @workgroup_size(32) fn skin(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
    let job=jobs[group.x]; let count=rig[job.x]; let joints=rig[job.x+1u];
    for(var node=lane;node<count;node+=32u) {globals[node]=local_pose(job.y+node*10u,job.y+(count+node)*10u);}
    workgroupBarrier();
    if lane==0u {
        for(var i=0u;i<count;i++) {
            let node=rig[job.x+4u+2u*i]; let parent=rig[job.x+5u+2u*i];
            let local=globals[node];
            if parent==4294967295u {globals[node]=local;} else {globals[node]=globals[parent]*local;}
        }
    }
    workgroupBarrier();
    let base=job.x+4u+count*2u;
    for(var j=lane;j<joints;j+=32u) {
        let node=rig[base+j]; let i=base+joints+j*16u;
        palette[job.z+j]=globals[node]*mat4x4(column(i),column(i+4u),column(i+8u),column(i+12u));
    }
}
