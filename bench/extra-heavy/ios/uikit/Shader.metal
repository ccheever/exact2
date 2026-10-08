// The shader row's filter (SPEC kind 3) for the UIKit app: the SwiftUI app's xheavyWave, as a full-box
// fragment shader over the photo texture (aspect-fill crop by uv scale/offset). Positions in points.
#include <metal_stdlib>
using namespace metal;

struct VOut { float4 pos [[position]]; float2 uv; };

vertex VOut xhVertex(uint vid [[vertex_id]]) {
    float2 p = float2((vid << 1) & 2, vid & 2);
    VOut o;
    o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = float2(p.x, 1.0 - p.y);
    return o;
}

struct Uniforms { float t; float pad; float2 size; float4 a; float4 b; float2 uvScale; float2 uvOffset; };

fragment float4 xhFragment(VOut in [[stage_in]], constant Uniforms &u [[buffer(0)]],
                           texture2d<float> tex [[texture(0)]]) {
    constexpr sampler s(filter::linear, address::clamp_to_edge);
    float2 pos = in.uv * u.size;
    float d = 0.012 * u.size.x;
    float sx = clamp(pos.x + d * sin(24.0 * pos.y / u.size.y + 2.0 * u.t), 0.5, u.size.x - 0.5);
    float sy = clamp(pos.y + d * cos(18.0 * pos.x / u.size.x + 1.6 * u.t), 0.5, u.size.y - 0.5);
    float3 c = tex.sample(s, u.uvOffset + float2(sx, sy) / u.size * u.uvScale).rgb;
    float l = dot(c, float3(0.2126, 0.7152, 0.0722));
    float3 duo = mix(u.a.rgb, u.b.rgb, l);
    return float4(mix(c, duo, 0.7), 1.0);
}
