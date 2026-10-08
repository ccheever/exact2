// One sparkline canvas: the trimmed polyline (round joins and caps), the pulse
// ring and the dot, anti-aliased by distance, over the row's white background.
// Everything is in points in canvas space; `size.z` is the pixel scale.

struct Uniforms {
  // x, y, cumulative length along the line, unused — per point.
  pts: array<vec4<f32>, 48>,
  // The line colour (rgb, a unused).
  color: vec4<f32>,
  // canvas width, height (points), scale, half the stroke width.
  size: vec4<f32>,
  // visible length, ring radius, ring alpha, dot radius (0 hides the dot).
  anim: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
  let ab = b - a;
  let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
  return length(p - a - ab * t);
}

fn cover(d: f32, scale: f32) -> f32 {
  return clamp(d * scale + 0.5, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
  let scale = u.size.z;
  let hw = u.size.w;
  let p = frag.xy / scale;
  let visible = u.anim.x;
  let x0 = u.pts[0].x;
  let dx = u.pts[1].x - u.pts[0].x;
  let reach = hw + 1.0;
  let lo = i32(clamp(floor((p.x - x0 - reach) / dx), 0.0, 46.0));
  let hi = i32(clamp(floor((p.x - x0 + reach) / dx), 0.0, 46.0));
  var d = 1e6;
  if (visible > 0.0) {
    for (var i = lo; i <= hi; i = i + 1) {
      let a = u.pts[i];
      let b = u.pts[i + 1];
      if (a.z >= visible) { break; }
      let f = clamp((visible - a.z) / max(b.z - a.z, 1e-6), 0.0, 1.0);
      d = min(d, segment(p, a.xy, mix(a.xy, b.xy, f)));
    }
  }
  let last = u.pts[47].xy;
  let r = length(p - last);
  let line = cover(hw - d, scale);
  let ring = cover(u.anim.y - r, scale) * u.anim.z;
  let dot_ = select(0.0, cover(u.anim.w - r, scale), u.anim.w > 0.0);
  var c = vec3<f32>(1.0, 1.0, 1.0);
  c = mix(c, u.color.rgb, ring);
  c = mix(c, u.color.rgb, line);
  c = mix(c, u.color.rgb, dot_);
  return vec4<f32>(c, 1.0);
}
