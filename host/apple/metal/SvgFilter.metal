// @ref LLP 1055.000 D14 — a live filter picture's chain in Metal (iOS):
// `SvgFilterMetal`'s kernels. Compiled at build (`host/apple/build.mjs`,
// `xcrun metal`) into `ExactSvgFilter.metallib` beside the app's
// executable: compiled from source at run time, the library cost the first
// launch after an install 113–264 ms inside the commit that shows F3.
#include <metal_stdlib>
using namespace metal;
kernel void colorMatrix(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]],
                        constant float *m [[buffer(0)]], uint2 g [[thread_position_in_grid]]) {
    if (g.x >= dst.get_width() || g.y >= dst.get_height()) return;
    float4 p = src.read(g);
    float4 c = p.a > 0 ? float4(p.rgb / p.a, p.a) : float4(0);
    float4 o;
    for (int r = 0; r < 4; r++) o[r] = m[r*5] * c.r + m[r*5+1] * c.g + m[r*5+2] * c.b + m[r*5+3] * c.a + m[r*5+4];
    o = clamp(o, 0.0, 1.0);
    dst.write(float4(o.rgb * o.a, o.a), g);
}
kernel void offset(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]],
                   constant int2 &d [[buffer(0)]], uint2 g [[thread_position_in_grid]]) {
    if (g.x >= dst.get_width() || g.y >= dst.get_height()) return;
    int2 s = int2(g) - d;
    bool in = s.x >= 0 && s.y >= 0 && s.x < int(src.get_width()) && s.y < int(src.get_height());
    dst.write(in ? src.read(uint2(s)) : float4(0), g);
}
kernel void dropShadow(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::read> blurred [[texture(1)]],
                       texture2d<float, access::write> dst [[texture(2)]], constant float4 &color [[buffer(0)]],
                       constant int2 &d [[buffer(1)]], uint2 g [[thread_position_in_grid]]) {
    if (g.x >= dst.get_width() || g.y >= dst.get_height()) return;
    int2 s = int2(g) - d;
    bool in = s.x >= 0 && s.y >= 0 && s.x < int(blurred.get_width()) && s.y < int(blurred.get_height());
    float a = in ? blurred.read(uint2(s)).a : 0;
    float4 shadow = float4(color.rgb * color.a, color.a) * a;
    float4 p = src.read(g);
    dst.write(p + shadow * (1 - p.a), g);
}
