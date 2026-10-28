// @ref LLP 1149 D1.1, D2, D6.1. All branch selectors are flat per primitive.

struct Viewport { size: vec2<f32>, pad: vec2<f32> }
@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(0) @binding(1) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(2) var color_atlas: texture_2d<f32>;
@group(0) @binding(3) var path_atlas: texture_2d<f32>;
@group(0) @binding(4) var ramp: texture_2d<f32>;
@group(0) @binding(5) var picture: texture_2d<f32>;
@group(0) @binding(6) var filtering: sampler;
@group(0) @binding(7) var clip_atlas: texture_2d<f32>;
@group(0) @binding(8) var destination: texture_2d<f32>;

struct Attributes {
    @location(0) rect: vec4<f32>,
    @location(1) affine: vec4<f32>,
    // Translation's f32 bits, packed kind/brush, flags.
    @location(2) data: vec4<u32>,
    @location(3) radii: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) clip0: vec4<f32>,
    @location(6) clip1: vec4<f32>,
    @location(7) clip_radii: vec4<u32>,
    @location(8) mask_rect: vec4<f32>,
    @location(9) mask_uv: vec4<f32>,
    @location(10) uv: vec4<f32>,
    @location(11) param: vec4<f32>,
}

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
    @location(2) @interpolate(flat) data: vec2<u32>,
    @location(3) @interpolate(flat) rect: vec4<f32>,
    @location(4) @interpolate(flat) radii: vec4<f32>,
    @location(5) @interpolate(flat) color: vec4<f32>,
    @location(6) @interpolate(flat) clip0: vec4<f32>,
    @location(7) @interpolate(flat) clip1: vec4<f32>,
    @location(8) @interpolate(flat) clip_radii: vec4<u32>,
    @location(9) @interpolate(flat) mask_rect: vec4<f32>,
    @location(10) @interpolate(flat) mask_uv: vec4<f32>,
    @location(11) @interpolate(flat) uv: vec4<f32>,
    @location(12) @interpolate(flat) param: vec4<f32>,
}

@vertex
fn vs_main(a: Attributes, @builtin(vertex_index) vertex: u32,
           @builtin(instance_index) index: u32) -> Vertex {
    let corner = vec2<f32>(f32(vertex & 1u), f32(vertex >> 1u));
    let det = abs(a.affine.x * a.affine.w - a.affine.y * a.affine.z);
    var fringe = vec2<f32>(0.0);
    if (a.data.z & 65535u) <= 1u {
        fringe = vec2<f32>(length(a.affine.zw), length(a.affine.xy)) / max(det, 0.000001);
    }
    var local = a.rect.xy - fringe + corner * (a.rect.zw + 2.0 * fringe);
    var device = vec2<f32>(a.affine.x * local.x + a.affine.z * local.y,
                           a.affine.y * local.x + a.affine.w * local.y) + bitcast<vec2<f32>>(a.data.xy);
    if (a.data.z & 65535u) == 4u {
        device = a.rect.xy + corner * a.rect.zw;
        let p = device - bitcast<vec2<f32>>(a.data.xy);
        let determinant = a.affine.x * a.affine.w - a.affine.y * a.affine.z;
        local = vec2<f32>(a.affine.w * p.x - a.affine.z * p.y,
                          a.affine.x * p.y - a.affine.y * p.x) / determinant;
    }
    var out: Vertex;
    out.position = vec4<f32>(device / viewport.size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.local = local;
    out.index = index;
    out.data = a.data.zw;
    out.rect = a.rect;
    out.radii = a.radii;
    out.color = a.color;
    out.clip0 = a.clip0;
    out.clip1 = a.clip1;
    out.clip_radii = a.clip_radii;
    out.mask_rect = a.mask_rect;
    out.mask_uv = a.mask_uv;
    out.uv = a.uv;
    out.param = a.param;
    return out;
}

fn unpack_radii(packed: vec2<u32>) -> vec4<f32> {
    return vec4<f32>(f32(packed.x & 65535u), f32(packed.x >> 16u),
                     f32(packed.y & 65535u), f32(packed.y >> 16u)) / 16.0;
}

fn rounded_distance(p: vec2<f32>, rect: vec4<f32>, r: vec4<f32>) -> f32 {
    let lo = rect.xy;
    let hi = lo + rect.zw;
    if p.x < lo.x + r.x && p.y < lo.y + r.x {
        return length(p - (lo + vec2<f32>(r.x))) - r.x;
    }
    if p.x > hi.x - r.y && p.y < lo.y + r.y {
        return length(p - vec2<f32>(hi.x - r.y, lo.y + r.y)) - r.y;
    }
    if p.x > hi.x - r.z && p.y > hi.y - r.z {
        return length(p - (hi - vec2<f32>(r.z))) - r.z;
    }
    if p.x < lo.x + r.w && p.y > hi.y - r.w {
        return length(p - vec2<f32>(lo.x + r.w, hi.y - r.w)) - r.w;
    }
    let q = abs(p - (lo + hi) * 0.5) - rect.zw * 0.5;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

fn shape_coverage(p: vec2<f32>, rect: vec4<f32>, radii: vec4<f32>, pixel: vec2<f32>) -> f32 {
    let distance = rounded_distance(p, rect, radii);
    let normal = vec2<f32>(rounded_distance(p + vec2<f32>(0.01, 0.0), rect, radii),
                          rounded_distance(p + vec2<f32>(0.0, 0.01), rect, radii)) - vec2<f32>(distance);
    let width = max(dot(abs(normal) * 100.0, pixel), 0.00001);
    let alpha = clamp(0.5 - distance / width, 0.0, 1.0);
    return select(0.0, alpha, all(rect.zw > vec2<f32>(0.0)));
}

fn normal_cdf(x: f32) -> f32 {
    let a = abs(x) * 0.707106781;
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let poly = t * (0.2548296 + t * (-0.2844967 + t * (1.4214137 + t * (-1.4531521 + t * 1.0614054))));
    let tail = 0.5 * poly * exp(-a * a);
    return select(tail, 1.0 - tail, x >= 0.0);
}

// Evan Wallace's CC0 separable Gaussian integral, extended to four radii.
// https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/
fn shadow(p: vec2<f32>, rect: vec4<f32>, radii: vec4<f32>, sigma: f32, pixel: vec2<f32>) -> f32 {
    if sigma <= 0.0 { return shape_coverage(p, rect, radii, pixel); }
    let low = p.y - rect.y - rect.w;
    let high = p.y - rect.y;
    let start = clamp(-3.0 * sigma, low, high);
    let end = clamp(3.0 * sigma, low, high);
    let step = (end - start) / 8.0;
    var alpha = 0.0;
    for (var i = 0u; i < 8u; i++) {
        let y = start + (f32(i) + 0.5) * step;
        let line = p.y - y;
        let top = line - rect.y;
        let bottom = rect.y + rect.w - line;
        let left_r = select(radii.w, radii.x, top < radii.x);
        let right_r = select(radii.z, radii.y, top < radii.y);
        let dl = max(left_r - select(bottom, top, top < radii.x), 0.0);
        let dr = max(right_r - select(bottom, top, top < radii.y), 0.0);
        let left = rect.x + left_r - sqrt(max(left_r * left_r - dl * dl, 0.0));
        let right = rect.x + rect.z - right_r + sqrt(max(right_r * right_r - dr * dr, 0.0));
        let integral = normal_cdf((right - p.x) / sigma) - normal_cdf((left - p.x) / sigma);
        alpha += integral * exp(-0.5 * (y / sigma) * (y / sigma)) * (0.39894228 / sigma) * step;
    }
    return clamp(alpha, 0.0, 1.0);
}

fn mapped(p: vec2<f32>, rect: vec4<f32>, uv: vec4<f32>) -> vec2<f32> {
    return uv.xy + (p - rect.xy) / max(rect.zw, vec2<f32>(0.00001)) * uv.zw;
}

fn inside(p: vec2<f32>, rect: vec4<f32>) -> f32 {
    return select(0.0, 1.0, all(p >= rect.xy) && all(p < rect.xy + rect.zw));
}

fn ramp_word(x: i32, row: i32) -> u32 {
    let b = vec4<u32>(round(textureLoad(ramp, vec2<i32>(x, row), 0) * 255.0));
    return b.x | (b.y << 8u) | (b.z << 16u) | (b.w << 24u);
}

fn stop_ramp(t: f32, row: i32) -> vec4<f32> {
    let count = i32(ramp_word(0, row));
    var at = bitcast<f32>(ramp_word(1, row));
    var c = textureLoad(ramp, vec2<i32>(2, row), 0);
    for (var n = 1; n < count; n++) {
        let next_at = bitcast<f32>(ramp_word(1 + 2 * n, row));
        let next = textureLoad(ramp, vec2<i32>(2 + 2 * n, row), 0);
        if t < next_at {
            c = mix(c, next, clamp((t - at) / max(next_at - at, 0.0000001), 0.0, 1.0));
            break;
        }
        at = next_at;
        c = next;
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn brush(v: Vertex, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let kind = v.data.x & 65535u;
    let flags = v.data.y;
    switch v.data.x >> 16u {
        case 1u: {
            var t = 0.0;
            switch (flags >> 9u) & 3u {
                case 1u: { t = length((v.local - v.param.xy) / max(v.param.zw, vec2<f32>(0.00001))); }
                case 2u: {
                    let d = v.local - v.param.xy;
                    t = fract((atan2(d.x, -d.y) - v.param.z) / 6.283185307 + 1.0);
                }
                default: {
                    let d = v.param.zw - v.param.xy;
                    t = dot(v.local - v.param.xy, d) / max(dot(d, d), 0.00001);
                }
            }
            if (flags & 134217728u) != 0u {
                let dc = v.param.xy - v.radii.xy;
                let p = v.local - v.radii.xy;
                let dr = v.param.z - v.param.w;
                let a = dot(dc, dc) - dr * dr;
                let b = -2.0 * (dot(p, dc) + v.param.w * dr);
                let c = dot(p, p) - v.param.w * v.param.w;
                if abs(a) < 0.00001 { t = -c / b; }
                else {
                    let d = b * b - 4.0 * a * c;
                    if d < 0.0 { return vec4<f32>(0.0); }
                    let t0 = (-b - sqrt(d)) / (2.0 * a);
                    let t1 = (-b + sqrt(d)) / (2.0 * a);
                    t = max(t0, t1);
                    if v.param.w + t * dr < 0.0 { t = min(t0, t1); }
                    if v.param.w + t * dr < 0.0 { return vec4<f32>(0.0); }
                }
            }
            if (flags & 8u) != 0u { t = fract(t); }
            if (flags & 268435456u) != 0u { t = 1.0 - abs(fract(t * 0.5) * 2.0 - 1.0); }
            if (flags & 4u) != 0u { return stop_ramp(t, i32((flags >> 16u) & 1023u)); }
            let size = vec2<f32>(textureDimensions(ramp));
            let uv = vec2<f32>((0.5 + clamp(t, 0.0, 1.0) * (size.x - 1.0)) / size.x,
                               (f32((flags >> 16u) & 1023u) + 0.5) / size.y);
            return textureSample(ramp, filtering, uv);
        }
        case 2u, 3u: {
            var uv = mapped(v.local, v.rect, v.uv);
            // Coverage uses uv for its path mask; its image occupies the full slot.
            if kind == 4u { uv = (v.local - v.rect.xy) / v.rect.zw; }
            if (v.data.x >> 16u) == 3u {
                uv = (v.local - v.param.xy) / max(v.param.zw, vec2<f32>(0.00001));
            }
            if (flags & 1073741824u) != 0u {
                uv = v.position.xy / vec2<f32>(textureDimensions(picture));
            }
            var c = textureSample(picture, filtering, uv);
            if (v.data.x >> 16u) == 3u {
                c = textureSampleGrad(picture, filtering, uv, dx / v.param.zw, dy / v.param.zw);
            }
            if (flags & 16u) != 0u {
                let size = vec2<i32>(textureDimensions(picture));
                var at = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
                if (v.data.x >> 16u) == 3u { at = vec2<i32>(floor(fract(uv) * vec2<f32>(size))); }
                c = textureLoad(picture, at, 0);
            }
            if (flags & 1u) != 0u { c = v.color * c.a; }
            if (v.data.x >> 16u) == 3u { c *= v.color.a; }
            return c;
        }
        default: { return v.color; }
    }
}

fn mirror_index(value: i32, size: i32) -> i32 {
    let period = size * 2;
    let index = ((value % period) + period) % period;
    return select(period - 1 - index, index, index < size);
}

fn blur_pixel(v: Vertex) -> vec4<f32> {
    let lo = vec2<i32>(v.rect.xy);
    let size = vec2<i32>(v.rect.zw);
    let at = vec2<i32>(v.position.xy) - lo;
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var k = -i32(v.param.x); k <= i32(v.param.y); k++) {
        var p = at;
        if v.param.z == 0.0 { p.x += k; } else { p.y += k; }
        p = lo + vec2<i32>(mirror_index(p.x, size.x), mirror_index(p.y, size.y));
        var weight = 1.0;
        if v.param.w > 0.0 { weight = exp(-0.5 * f32(k * k) / (v.param.w * v.param.w)); }
        sum += textureLoad(picture, p, 0) * weight;
        total += weight;
    }
    return sum / total;
}

// Each mip dimension falls by at most three, including odd source dimensions.
fn area_pixel(v: Vertex) -> vec4<f32> {
    let source_size = vec2<f32>(textureDimensions(picture));
    let ratio = source_size * v.uv.zw / v.rect.zw;
    let lo = (floor(v.position.xy) - v.rect.xy) * ratio + v.uv.xy * source_size;
    let hi = lo + ratio;
    var sum = vec4<f32>(0.0);
    for (var y = i32(floor(lo.y)); y < i32(ceil(hi.y)); y++) {
        for (var x = i32(floor(lo.x)); x < i32(ceil(hi.x)); x++) {
            let p = vec2<f32>(f32(x), f32(y));
            let weight = max(vec2<f32>(0.0), min(hi, p + 1.0) - max(lo, p));
            sum += textureLoad(picture, vec2<i32>(x, y), 0) * weight.x * weight.y;
        }
    }
    return sum / (ratio.x * ratio.y);
}

fn luminosity(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.3, 0.59, 0.11)); }
fn saturation(c: vec3<f32>) -> f32 { return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b)); }
fn set_luminosity(c: vec3<f32>, lum: f32) -> vec3<f32> {
    var v = c + vec3<f32>(lum - luminosity(c));
    let low = min(v.r, min(v.g, v.b));
    let high = max(v.r, max(v.g, v.b));
    if low < 0.0 { v = vec3<f32>(lum) + (v - vec3<f32>(lum)) * lum / max(lum - low, 0.000001); }
    if high > 1.0 { v = vec3<f32>(lum) + (v - vec3<f32>(lum)) * (1.0 - lum) / max(high - lum, 0.000001); }
    return v;
}
fn set_saturation(c: vec3<f32>, sat: f32) -> vec3<f32> {
    return (c - vec3<f32>(min(c.r, min(c.g, c.b)))) * sat / max(saturation(c), 0.000001);
}
fn blend_channel(b: f32, s: f32, mode: u32) -> f32 {
    switch mode {
        case 1u: { return b * s; }
        case 2u: { return b + s - b * s; }
        case 3u: { return select(1.0 - 2.0 * (1.0 - b) * (1.0 - s), 2.0 * b * s, b <= 0.5); }
        case 4u: { return min(b, s); }
        case 5u: { return max(b, s); }
        case 6u: {
            if b <= 0.0 { return 0.0; }
            if s >= 1.0 { return 1.0; }
            return min(1.0, b / (1.0 - s));
        }
        case 7u: {
            if b >= 1.0 { return 1.0; }
            if s <= 0.0 { return 0.0; }
            return 1.0 - min(1.0, (1.0 - b) / s);
        }
        case 8u: { return select(1.0 - 2.0 * (1.0 - b) * (1.0 - s), 2.0 * b * s, s <= 0.5); }
        case 9u: {
            if s <= 0.5 { return b - (1.0 - 2.0 * s) * b * (1.0 - b); }
            let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= 0.25);
            return b + (2.0 * s - 1.0) * (d - b);
        }
        case 10u: { return abs(b - s); }
        case 11u: { return b + s - 2.0 * b * s; }
        default: { return s; }
    }
}

// CSS Compositing 1: the blend operates on straight colours, then source-over.
fn blended(back: vec4<f32>, source: vec4<f32>, mode: u32) -> vec4<f32> {
    let b = back.rgb / max(back.a, 0.000001);
    let s = source.rgb / max(source.a, 0.000001);
    var mix_color = vec3<f32>(blend_channel(b.r, s.r, mode), blend_channel(b.g, s.g, mode), blend_channel(b.b, s.b, mode));
    switch mode {
        case 12u: { mix_color = set_luminosity(set_saturation(s, saturation(b)), luminosity(b)); }
        case 13u: { mix_color = set_luminosity(set_saturation(b, saturation(s)), luminosity(b)); }
        case 14u: { mix_color = set_luminosity(s, luminosity(b)); }
        case 15u: { mix_color = set_luminosity(b, luminosity(s)); }
        default: {}
    }
    return vec4<f32>((1.0 - source.a) * back.rgb + (1.0 - back.a) * source.rgb + back.a * source.a * mix_color,
                     source.a + back.a * (1.0 - source.a));
}

@fragment
fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let pixel = fwidth(v.local);
    let dx = dpdx(v.local);
    let dy = dpdy(v.local);
    let kind = v.data.x & 65535u;
    let flags = v.data.y;
    var alpha = 1.0;
    var color = brush(v, dx, dy);
    switch kind {
        case 0u: { alpha = shape_coverage(v.local, v.rect, v.radii, pixel); }
        case 1u: {
            alpha = max(0.0, shape_coverage(v.local, v.rect, v.radii, pixel) - shape_coverage(v.local, v.uv, v.param, pixel));
        }
        case 2u: {
            alpha = shadow(v.local, v.uv, v.radii, v.param.x, pixel);
        }
        case 3u: {
            let uv = mapped(v.local, v.rect, v.uv);
            if (flags & 32u) != 0u { color = textureSample(color_atlas, filtering, uv) * v.color.a; }
            else { alpha = textureSample(glyph_atlas, filtering, uv).r; }
        }
        case 4u: { alpha = textureSample(path_atlas, filtering, mapped(v.position.xy, v.rect, v.uv)).r; }
        case 5u: {
            color = textureSample(picture, filtering, mapped(v.local, v.rect, v.uv));
            alpha = v.param.x;
        }
        case 6u: { color = blur_pixel(v); }
        case 7u: {
            let c = textureLoad(picture, vec2<i32>(v.position.xy), 0);
            let gray = dot(c.rgb, vec3<f32>(0.213, 0.715, 0.072));
            color = vec4<f32>(clamp(vec3<f32>(gray) + v.param.x * (c.rgb - vec3<f32>(gray)), vec3<f32>(0.0), vec3<f32>(c.a)), c.a);
        }
        case 8u: { color = area_pixel(v); }
        default: { alpha = 0.0; }
    }
    if (flags & 536870912u) != 0u {
        alpha *= textureLoad(color_atlas, vec2<i32>(v.position.xy), 0).a;
    }
    if (flags & 64u) != 0u {
        alpha *= shape_coverage(v.position.xy, v.clip0, unpack_radii(v.clip_radii.xy), vec2<f32>(1.0));
    }
    if (flags & 128u) != 0u {
        var clip = shape_coverage(v.position.xy, v.clip1, unpack_radii(v.clip_radii.zw), vec2<f32>(1.0));
        // Shadow's second slot is its border box, excluded for an outer shadow.
        if kind == 2u { clip = 1.0 - clip; }
        alpha *= clip;
    }
    if (flags & 256u) != 0u {
        alpha *= textureSample(clip_atlas, filtering, mapped(v.position.xy, v.mask_rect, v.mask_uv)).r
                 * inside(v.position.xy, v.mask_rect);
    }
    alpha = clamp(alpha, 0.0, 1.0);
    if (flags & 1073741824u) != 0u {
        return mix(textureLoad(destination, vec2<i32>(v.position.xy), 0), color, alpha);
    }
    let mode = (flags >> 11u) & 15u;
    if mode != 0u {
        return blended(textureLoad(destination, vec2<i32>(v.position.xy), 0), color * alpha, mode);
    }
    return color * alpha;
}
