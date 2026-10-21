// The shader row's filter (SPEC kind 3): a wave distortion + duotone, as a SwiftUI layer effect.
#include <metal_stdlib>
#include <SwiftUI/SwiftUI_Metal.h>
using namespace metal;

[[ stitchable ]] half4 xheavyWave(float2 pos, SwiftUI::Layer layer, float t, float2 size, float3 a, float3 b) {
    float d = 0.012 * size.x;
    float sx = clamp(pos.x + d * sin(24.0 * pos.y / size.y + 2.0 * t), 0.5, size.x - 0.5);
    float sy = clamp(pos.y + d * cos(18.0 * pos.x / size.x + 1.6 * t), 0.5, size.y - 0.5);
    half4 c = layer.sample(float2(sx, sy));
    float3 rgb = float3(c.rgb);
    float l = dot(rgb, float3(0.2126, 0.7152, 0.0722));
    float3 duo = mix(a, b, l);
    return half4(half3(mix(rgb, duo, 0.7)), 1.0h);
}
