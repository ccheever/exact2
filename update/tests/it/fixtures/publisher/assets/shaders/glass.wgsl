// Glass: the whole app inside the aurora (LLP 1014). Two passes — the sky
// at a quarter of the resolution (fs_sky: the aurora's field, so it costs a
// sixteenth of a full-resolution sky and comes back soft), then the
// children composed over it at full resolution (fs_compose) in one of three
// materials: glass (frosted panels, the sky refracted at the ink's edges, a
// slow sheen), ink (paper, the interface as ink), crt (phosphor, scanlines,
// a gentle barrel). The children now and before crossfade when they change.
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    seed: f32,
    material: f32,
    fade: f32,
    sky_width: f32,
    sky_height: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var sky: texture_2d<f32>;
@group(0) @binding(2) var children: texture_2d<f32>;
@group(0) @binding(3) var previous: texture_2d<f32>;
@group(0) @binding(4) var smp: sampler;

struct Fragment {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Fragment {
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

// The sky, at the sky texture's resolution: colour in rgb, the ribbons'
// brightness in a.
@fragment
fn fs_sky(f: Fragment) -> @location(0) vec4<f32> {
    let res = vec2<f32>(u.sky_width, u.sky_height);
    var uv = f.position.xy / res;
    uv.x = uv.x * (u.width / u.height);
    let t = u.time * 0.001;
    let base = uv * 2.4 + vec2<f32>(u.seed * 7.0, u.seed * 3.0);
    let q = vec2<f32>(fbm(base + vec2<f32>(0.0, t * 0.10)), fbm(base + vec2<f32>(5.2, 1.3) + t * 0.07));
    let r = vec2<f32>(fbm(base + 4.0 * q + vec2<f32>(1.7, 9.2) + 0.15 * t), fbm(base + 4.0 * q + vec2<f32>(8.3, 2.8) + 0.126 * t));
    let v = fbm(base + 4.0 * r);
    var col = palette(v * 1.2 + t * 0.03 + u.seed);
    let ribbon = smoothstep(0.35, 0.75, v);
    col = mix(vec3<f32>(0.02, 0.03, 0.08), col, ribbon);
    col = col + vec3<f32>(0.10, 0.25, 0.20) * pow(max(r.x - 0.4, 0.0), 2.0) * 3.0;
    let centre = vec2<f32>(u.width / u.height * 0.5, 0.5);
    let vignette = 1.0 - 0.45 * dot(uv - centre, uv - centre);
    return vec4<f32>(col * vignette, ribbon);
}

@fragment
fn fs_compose(f: Fragment) -> @location(0) vec4<f32> {
    let res = vec2<f32>(u.width, u.height);
    let uv0 = f.position.xy / res;
    let glass = step(u.material, 0.5);
    let paper = step(0.5, u.material) * step(u.material, 1.5);
    let crt = step(1.5, u.material);
    // The crt bows the picture a little (two or three pixels at the edges;
    // hit-testing tolerates that; nothing else moves a box).
    let c = uv0 - vec2<f32>(0.5, 0.5);
    let uv = uv0 + crt * c * 0.012 * dot(c, c) * 4.0;
    let px = vec2<f32>(1.0, 1.0) / res;

    // The sky, soft from its quarter resolution, and a wider blur of it.
    let sky_px = textureSample(sky, smp, uv);
    let d = vec2<f32>(1.5, 1.5) / vec2<f32>(u.sky_width, u.sky_height);
    let blur = (sky_px
        + textureSample(sky, smp, uv + vec2<f32>(d.x, 0.0))
        + textureSample(sky, smp, uv - vec2<f32>(d.x, 0.0))
        + textureSample(sky, smp, uv + vec2<f32>(0.0, d.y))
        + textureSample(sky, smp, uv - vec2<f32>(0.0, d.y))) / 5.0;

    // The children now and before; the sky's own field refracts them a
    // little (glass), the way the aurora did — uniformly, so a glyph moves
    // whole and stays crisp.
    let warp = (blur.rg - vec2<f32>(0.5, 0.5)) * 0.004 * glass;
    let cur = textureSample(children, smp, uv + warp);
    let prev = textureSample(previous, smp, uv + warp);
    // Neighbours, for the ink's bleed and the crt's bloom.
    let n1 = textureSample(children, smp, uv + vec2<f32>(px.x * 2.0, 0.0));
    let n2 = textureSample(children, smp, uv - vec2<f32>(px.x * 2.0, 0.0));
    let n3 = textureSample(children, smp, uv + vec2<f32>(0.0, px.y * 2.0));
    let n4 = textureSample(children, smp, uv - vec2<f32>(0.0, px.y * 2.0));

    // Glass crossfades; paper dissolves grain by grain; the crt cuts.
    let grain = hash(floor(f.position.xy / 3.0));
    let mixed = mix(prev, cur, u.fade);
    let dissolved = mix(prev, cur, step(grain, u.fade));
    let ink = mixed * glass + dissolved * paper + cur * crt;
    let a = ink.a;
    let lum = dot(ink.rgb, vec3<f32>(0.299, 0.587, 0.114));
    // How dark the interface is here: white panels are light, text is dark
    // (premultiplied: a white pixel's luminance equals its alpha).
    let dark = clamp(a - lum, 0.0, 1.0);

    // Glass: frosted panels over the blurred sky, a slow sheen across them,
    // crisp ink on top.
    let sheen = 0.10 * smoothstep(0.2, 1.0, sin((uv.x - uv.y) * 4.0 + u.time * 0.0005)) * a * (1.0 - step(0.98, a));
    let under = mix(sky_px.rgb, blur.rgb, min(1.0, a * 2.0));
    let out_glass = ink.rgb + under * (1.0 - a) + vec3<f32>(sheen, sheen, sheen);

    // Paper: warm stock with fibres, the interface as sepia ink that bleeds
    // a hair into the paper.
    let fibre = 0.97 + 0.03 * hash(floor(f.position.xy / 2.0));
    let stock = vec3<f32>(0.96, 0.93, 0.86) * fibre * (1.0 - 0.25 * dot(c, c));
    let bleed = max(max(n1.a - dot(n1.rgb, vec3<f32>(0.333, 0.333, 0.334)), n2.a - dot(n2.rgb, vec3<f32>(0.333, 0.333, 0.334))),
                    max(n3.a - dot(n3.rgb, vec3<f32>(0.333, 0.333, 0.334)), n4.a - dot(n4.rgb, vec3<f32>(0.333, 0.333, 0.334))));
    let ink_amount = clamp(dark + 0.35 * clamp(bleed, 0.0, 1.0), 0.0, 1.0);
    let out_paper = mix(stock, vec3<f32>(0.10, 0.07, 0.05), ink_amount) - vec3<f32>(0.03, 0.03, 0.02) * a * (1.0 - dark);

    // Crt: dark ink becomes bright phosphor, panels a faint glow, scanlines,
    // bloom from the neighbours, a flicker from the clock.
    let glow = (clamp(n1.a - dot(n1.rgb, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0) + clamp(n2.a - dot(n2.rgb, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0)
              + clamp(n3.a - dot(n3.rgb, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0) + clamp(n4.a - dot(n4.rgb, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0)) * 0.25;
    let phosphor = vec3<f32>(1.0, 0.72, 0.25);
    let scan = 0.80 + 0.20 * sin(f.position.y * 1.5707963);
    let flicker = 0.965 + 0.035 * sin(u.time * 0.05) * sin(u.time * 0.013);
    let lit = dark * 1.15 + a * 0.06 + glow * 0.55;
    let out_crt = (phosphor * lit * scan + vec3<f32>(0.015, 0.012, 0.02)) * flicker * (1.0 - 0.6 * dot(c, c));

    let out = out_glass * glass + out_paper * paper + out_crt * crt;
    return vec4<f32>(out, 1.0);
}
