// Aurora: domain-warped fractal noise — three noise fields, each warping the
// next — through a cosine palette, lit from the frame clock. One full-screen
// triangle; every pixel is the fragment shader's.
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    seed: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
// The canvas's children (LLP 1014) — laid out by the kernel,
// painted by the host — premultiplied, the canvas's box at its scale.
@group(0) @binding(1) var children: texture_2d<f32>;
@group(0) @binding(2) var children_sampler: sampler;

struct Fragment {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Fragment {
    // A triangle that covers the clip square: (-1,-1), (3,-1), (-1,3).
    var out: Fragment;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, s.x), mix(c, d, s.x), s.y);
}

fn fbm(p0: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var p = p0;
    for (var i = 0; i < 5; i = i + 1) {
        v = v + a * noise(p);
        p = p * 2.03 + vec2<f32>(1.7, 9.2);
        a = a * 0.5;
    }
    return v;
}

fn palette(t: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(6.28318 * (vec3<f32>(t, t, t) + vec3<f32>(0.0, 0.33, 0.67)));
}

@fragment
fn fs(f: Fragment) -> @location(0) vec4<f32> {
    let res = vec2<f32>(u.width, u.height);
    let uv0 = f.position.xy / res;
    var uv = uv0;
    uv.x = uv.x * (res.x / res.y);
    let t = u.time * 0.001;
    let base = uv * 2.4 + vec2<f32>(u.seed * 7.0, u.seed * 3.0);
    let q = vec2<f32>(fbm(base + vec2<f32>(0.0, t * 0.10)), fbm(base + vec2<f32>(5.2, 1.3) + t * 0.07));
    let r = vec2<f32>(fbm(base + 4.0 * q + vec2<f32>(1.7, 9.2) + 0.15 * t), fbm(base + 4.0 * q + vec2<f32>(8.3, 2.8) + 0.126 * t));
    let v = fbm(base + 4.0 * r);
    var col = palette(v * 1.2 + t * 0.03 + u.seed);
    // Dark sky where the field is thin, bright ribbons where it is dense.
    let ribbon = smoothstep(0.35, 0.75, v);
    col = mix(vec3<f32>(0.02, 0.03, 0.08), col, ribbon);
    col = col + vec3<f32>(0.10, 0.25, 0.20) * pow(max(r.x - 0.4, 0.0), 2.0) * 3.0;
    let centre = vec2<f32>(res.x / res.y * 0.5, 0.5);
    let vignette = 1.0 - 0.55 * dot(uv - centre, uv - centre);
    let sky = col * vignette;
    // The children through the aurora: refracted by the warp field (a slow
    // shimmer from the clock), split slightly per channel, glowing where the
    // ribbons are bright, composed premultiplied over the sky.
    let warp = (r - vec2<f32>(0.5, 0.5)) * 0.012;
    let tr = textureSample(children, children_sampler, uv0 + warp * 1.25);
    let tg = textureSample(children, children_sampler, uv0 + warp);
    let tb = textureSample(children, children_sampler, uv0 + warp * 0.75);
    let ink = vec4<f32>(tr.r, tg.g, tb.b, tg.a);
    let glow = ink.rgb * (0.35 + 0.65 * ribbon);
    let out = ink.rgb + glow * 0.35 + sky * (1.0 - ink.a);
    return vec4<f32>(out, 1.0);
}
