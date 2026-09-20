// Deterministic finite-radius cosine AO against a regular-simplified source proxy.
#include "meshoptimizer.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <thread>
#include <vector>
namespace {
struct V { float x, y, z; float operator[](int i) const { return i==0?x:i==1?y:z; } };
V operator+(V a,V b) { return {a.x+b.x,a.y+b.y,a.z+b.z}; }
V operator-(V a,V b) { return {a.x-b.x,a.y-b.y,a.z-b.z}; }
V operator*(V a,float s) { return {a.x*s,a.y*s,a.z*s}; }
float dot(V a,V b) { return a.x*b.x+a.y*b.y+a.z*b.z; }
V cross(V a,V b) { return {a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x}; }
V unit(V a) { return a*(1.f/std::sqrt(std::max(dot(a,a),1e-30f))); }
V low(V a,V b) { return {std::min(a.x,b.x),std::min(a.y,b.y),std::min(a.z,b.z)}; }
V high(V a,V b) { return {std::max(a.x,b.x),std::max(a.y,b.y),std::max(a.z,b.z)}; }
struct Tri { V a,b,c,center; };
struct Node { V lo,hi; uint32_t start,count,left,right; };
struct Bvh {
    std::vector<Tri> tri;
    std::vector<uint32_t> order;
    std::vector<Node> nodes;
    uint32_t build(uint32_t start,uint32_t count) {
        V lo={INFINITY,INFINITY,INFINITY},hi={-INFINITY,-INFINITY,-INFINITY};
        for(uint32_t i=start;i<start+count;i++) { const auto& t=tri[order[i]]; lo=low(lo,low(t.a,low(t.b,t.c))); hi=high(hi,high(t.a,high(t.b,t.c))); }
        uint32_t id=uint32_t(nodes.size()); nodes.push_back({lo,hi,start,count,0,0});
        if(count>8) {
            V extent=hi-lo; int axis=extent.x>extent.y?0:1; if(extent.z>extent[axis]) axis=2;
            std::nth_element(order.begin()+start,order.begin()+start+count/2,order.begin()+start+count,[&](uint32_t a,uint32_t b) { float x=tri[a].center[axis],y=tri[b].center[axis]; return x==y?a<b:x<y; });
            uint32_t left=build(start,count/2),right=build(start+count/2,count-count/2);
            nodes[id].left=left; nodes[id].right=right;
        }
        return id;
    }
    bool hit(uint32_t id,V o,V d,V inv,float radius) const {
        const auto& n=nodes[id]; float near=0,far=radius;
        for(int i=0;i<3;i++) {
            if(std::abs(d[i])<1e-15f) { if(o[i]<n.lo[i]||o[i]>n.hi[i]) return false; }
            else { float a=(n.lo[i]-o[i])*inv[i],b=(n.hi[i]-o[i])*inv[i]; near=std::max(near,std::min(a,b)); far=std::min(far,std::max(a,b)); }
        }
        if(near>far) return false;
        if(n.left) return hit(n.left,o,d,inv,radius)||hit(n.right,o,d,inv,radius);
        for(uint32_t i=n.start;i<n.start+n.count;i++) {
            const auto& t=tri[order[i]]; V e=t.b-t.a,f=t.c-t.a,h=cross(d,f); float det=dot(e,h);
            // Ignore exits from the proxy: source vertices may lie inside a simplified face.
            if(det<1e-20f) continue;
            float inverse=1.f/det; V s=o-t.a; float u=dot(s,h)*inverse; if(u<0||u>1) continue;
            V q=cross(s,e); float v=dot(d,q)*inverse; if(v<0||u+v>1) continue;
            float distance=dot(f,q)*inverse; if(distance>0&&distance<radius) return true;
        }
        return false;
    }
};
}
extern "C" bool exact_clod_ao(const float* positions,const float* normals,size_t vertex_count,const uint32_t* indices,size_t index_count,uint8_t* output,uint32_t* proxy_count) {
    try {
        std::vector<uint32_t> proxy(index_count);
        size_t count=index_count;
        float error=0;
        if(index_count>750000) count=meshopt_simplify(proxy.data(),indices,index_count,positions,vertex_count,12,750000,0.002f,0,&error);
        else std::copy(indices,indices+index_count,proxy.begin());
        *proxy_count=uint32_t(count/3);
        auto position=[&](uint32_t i) { return V{positions[i*3],positions[i*3+1],positions[i*3+2]}; };
        Bvh bvh; bvh.tri.reserve(count/3); bvh.order.reserve(count/3);
        for(size_t i=0;i<count;i+=3) { V a=position(proxy[i]),b=position(proxy[i+1]),c=position(proxy[i+2]); bvh.order.push_back(uint32_t(bvh.tri.size())); bvh.tri.push_back({a,b,c,(a+b+c)*(1.f/3.f)}); }
        bvh.build(0,uint32_t(bvh.tri.size()));
        V ext=bvh.nodes[0].hi-bvh.nodes[0].lo; float extent=std::max(ext.x,std::max(ext.y,ext.z));
        const float bias=extent*std::max(0.00008f,error*1.2f), radius=extent*0.035f;
        std::array<V,16> samples;
        for(unsigned i=0;i<samples.size();i++) { float r=std::sqrt((i+0.5f)/samples.size()),phi=i*2.39996323f; samples[i]={r*std::cos(phi),r*std::sin(phi),std::sqrt(1-r*r)}; }
        unsigned workers=std::max(1u,std::min(16u,std::thread::hardware_concurrency()));
        std::vector<std::thread> threads;
        for(unsigned worker=0;worker<workers;worker++) threads.emplace_back([&,worker] {
            for(size_t i=vertex_count*worker/workers;i<vertex_count*(worker+1)/workers;i++) {
                V n=unit({normals[i*3],normals[i*3+1],normals[i*3+2]});
                V tangent=unit(cross(std::abs(n.z)<0.9f?V{0,0,1}:V{0,1,0},n)),bitangent=cross(n,tangent);
                V o=position(uint32_t(i))+n*bias; unsigned blocked=0;
                for(V sample:samples) { V d=tangent*sample.x+bitangent*sample.y+n*sample.z; V inv={1.f/d.x,1.f/d.y,1.f/d.z}; blocked+=bvh.hit(0,o,d,inv,radius); }
                output[i]=uint8_t(255-(blocked*255+8)/16);
            }
        });
        for(auto& thread:threads) thread.join();
        // Two integer neighbour averages remove sampling bands without schedule-dependent reductions.
        std::vector<uint32_t> sums(vertex_count),weights(vertex_count);
        std::vector<uint8_t> smooth(vertex_count);
        for(unsigned pass=0;pass<2;pass++) {
            for(size_t i=0;i<vertex_count;i++) { sums[i]=output[i]*2; weights[i]=2; }
            for(size_t i=0;i<index_count;i+=3) for(unsigned a=0;a<3;a++) for(unsigned b=0;b<3;b++) if(a!=b) { sums[indices[i+a]]+=output[indices[i+b]]; weights[indices[i+a]]++; }
            for(size_t i=0;i<vertex_count;i++) smooth[i]=uint8_t((sums[i]+weights[i]/2)/weights[i]);
            std::copy(smooth.begin(),smooth.end(),output);
        }
        return true;
    } catch(...) { return false; }
}
