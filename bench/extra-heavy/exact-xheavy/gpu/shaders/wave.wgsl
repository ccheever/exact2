// The shader row (SPEC 3): a wave displacement and a duotone over the photo, aspect-filled into
// the canvas box. Every quantity is in points of the box; `size.z` is the pixel scale. Colours
// are sRGB-encoded values throughout (the photo is uploaded as Rgba8Unorm); `cover.w` > 0.5
// means the target is an sRGB format, so the result is linearised before it is written.

struct Uniforms {
  // box width, height (points), scale, t (seconds)
  size: vec4<f32>,
  duo_a: vec4<f32>,
  duo_b: vec4<f32>,
  // visible fraction of the image (x, y), then the offset of that window (x, y)
  cover: vec4<f32>,
  // 1 when the target format is sRGB
  flags: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var photo: texture_2d<f32>;
@group(0) @binding(2) var photo_sampler: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
  let lo = c / 12.92;
  let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
  return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
  let W = u.size.x;
  let H = u.size.y;
  let t = u.size.w;
  let p = frag.xy / u.size.z;
  let d = 0.012 * W;
  let sx = clamp(p.x + d * sin(24.0 * p.y / H + 2.0 * t), 0.5, W - 0.5);
  let sy = clamp(p.y + d * cos(18.0 * p.x / W + 1.6 * t), 0.5, H - 0.5);
  let uv = u.cover.zw + vec2<f32>(sx / W, sy / H) * u.cover.xy;
  let c = textureSampleLevel(photo, photo_sampler, uv, 0.0).rgb;
  let l = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
  var out = mix(c, mix(u.duo_a.rgb, u.duo_b.rgb, l), 0.7);
  if (u.flags.x > 0.5) {
    out = to_linear(out);
  }
  return vec4<f32>(out, 1.0);
}
