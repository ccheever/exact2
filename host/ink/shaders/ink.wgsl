// @ref LLP 1149 D1.1, D2, D6.1. All branch selectors are flat per primitive.
diagnostic(off, derivative_uniformity);

struct Viewport { size: vec2<f32>, pad: vec2<f32> }
@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(0) @binding(1) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(2) var color_atlas: texture_2d<f32>;
@group(0) @binding(3) var path_atlas: texture_2d<f32>;
@group(0) @binding(4) var ramp: texture_2d<f32>;
@group(0) @binding(5) var picture: texture_2d<f32>;
@group(0) @binding(6) var filtering: sampler;

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
    let local = a.rect.xy - fringe + corner * (a.rect.zw + 2.0 * fringe);
    let device = vec2<f32>(a.affine.x * local.x + a.affine.z * local.y,
                           a.affine.y * local.x + a.affine.w * local.y) + bitcast<vec2<f32>>(a.data.xy);
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

fn covered(distance: f32) -> f32 {
    return clamp(0.5 - distance / max(fwidth(distance), 0.00001), 0.0, 1.0);
}

fn shape_coverage(p: vec2<f32>, rect: vec4<f32>, radii: vec4<f32>) -> f32 {
    let alpha = covered(rounded_distance(p, rect, radii));
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
fn shadow(p: vec2<f32>, rect: vec4<f32>, radii: vec4<f32>, sigma: f32) -> f32 {
    if sigma <= 0.0 { return shape_coverage(p, rect, radii); }
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

fn brush(v: Vertex) -> vec4<f32> {
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
            if (flags & 8u) != 0u { t = fract(t); }
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
                uv = fract((v.local - v.param.xy) / max(v.param.zw, vec2<f32>(0.00001)));
            }
            var c = textureSample(picture, filtering, uv);
            if (flags & 16u) != 0u {
                let size = vec2<i32>(textureDimensions(picture));
                c = textureLoad(picture, clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1)), 0);
            }
            if (flags & 1u) != 0u { c = v.color * c.a; }
            return c;
        }
        default: { return v.color; }
    }
}

@fragment
fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let kind = v.data.x & 65535u;
    let flags = v.data.y;
    var alpha = 1.0;
    var color = brush(v);
    switch kind {
        case 0u: { alpha = shape_coverage(v.local, v.rect, v.radii); }
        case 1u: {
            alpha = max(0.0, shape_coverage(v.local, v.rect, v.radii) - shape_coverage(v.local, v.uv, v.param));
        }
        case 2u: {
            alpha = shadow(v.local, v.uv, v.radii, v.param.x);
            if (flags & 2u) != 0u { alpha = 1.0 - alpha; }
        }
        case 3u: {
            let uv = mapped(v.local, v.rect, v.uv);
            if (flags & 32u) != 0u { color = textureSample(color_atlas, filtering, uv); }
            else { alpha = textureSample(glyph_atlas, filtering, uv).r; }
        }
        case 4u: { alpha = textureSample(path_atlas, filtering, mapped(v.local, v.rect, v.uv)).r; }
        case 5u: {
            color = textureSample(picture, filtering, mapped(v.local, v.rect, v.uv));
            alpha = v.param.x;
        }
        default: { alpha = 0.0; }
    }
    if (flags & 64u) != 0u {
        alpha *= shape_coverage(v.position.xy, v.clip0, unpack_radii(v.clip_radii.xy));
    }
    if (flags & 128u) != 0u {
        var clip = shape_coverage(v.position.xy, v.clip1, unpack_radii(v.clip_radii.zw));
        // Shadow's second slot is its border box, excluded for an outer shadow.
        if kind == 2u && (flags & 2u) == 0u { clip = 1.0 - clip; }
        alpha *= clip;
    }
    if (flags & 256u) != 0u {
        alpha *= textureSample(path_atlas, filtering, mapped(v.position.xy, v.mask_rect, v.mask_uv)).r
                 * inside(v.position.xy, v.mask_rect);
    }
    return color * clamp(alpha, 0.0, 1.0);
}
