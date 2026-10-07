// Screen-space ambient occlusion (ssao.rs), an optional effect. `ao` estimates,
// at full or half resolution, how much of each pixel's normal hemisphere nearby
// depth covers; `apply` upsamples it with a depth-aware 4 x 4 blur and multiplies
// it into the resolved HDR colour.
struct Ao {
    inverse_projection: mat4x4f,
    projection: mat4x4f,
    // pixels wide, high; radius in metres; intensity
    viewport: vec4f,
    // the viewmodel layer's depth split (zero without one), the occlusion
    // texture's downscale (1 or 2), samples per pixel
    split: vec4f,
}
@group(0) @binding(0) var<uniform> u: Ao;
@group(0) @binding(1) var depth: texture_depth_multisampled_2d;
@group(0) @binding(2) var occlusion: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
// The view-space position at a pixel; w is zero where nothing was drawn and
// where the viewmodel layer drew (it neither receives nor casts occlusion).
// World depth fills [split, 1] of the depth range while a viewmodel draws.
fn view_at(p: vec2i) -> vec4f {
    let size = vec2i(u.viewport.xy);
    let q = clamp(p, vec2i(0), size - 1);
    if any(q != p) { return vec4f(0.0); }
    let z = textureLoad(depth, q, 0);
    let split = u.split.x;
    let ndc = (vec2f(q) + 0.5) / u.viewport.xy * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0);
    let v = u.inverse_projection * vec4f(ndc, (z - split) / (1.0 - split), 1.0);
    return vec4f(v.xyz / v.w, select(1.0, 0.0, z >= 1.0 || z < split));
}
// view_at's z alone, and whether anything world-side was drawn there: two rows
// of the inverse projection instead of the whole point.
fn view_z(p: vec2i) -> vec2f {
    let z = textureLoad(depth, p, 0);
    let split = u.split.x;
    let ndc = vec4f((vec2f(p) + 0.5) / u.viewport.xy * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0),
        (z - split) / (1.0 - split), 1.0);
    let m = u.inverse_projection;
    let zw = vec2f(m[0].z * ndc.x + m[1].z * ndc.y + m[2].z * ndc.z + m[3].z,
        m[0].w * ndc.x + m[1].w * ndc.y + m[2].w * ndc.z + m[3].w);
    return vec2f(zw.x / zw.y, select(1.0, 0.0, z >= 1.0 || z < split));
}
// The shorter of the two one-pixel differences, so silhouettes keep their plane;
// a neighbour off screen, in the sky or in the viewmodel layer does not count.
fn tangent(c: vec3f, a: vec4f, b: vec4f) -> vec3f {
    let da = a.xyz - c;
    let db = c - b.xyz;
    let a_ok = a.w > 0.0 && dot(da, da) > 0.0;
    let b_ok = b.w > 0.0 && dot(db, db) > 0.0;
    return select(db, da, a_ok && (!b_ok || dot(da, da) < dot(db, db)));
}
@fragment fn ao(@builtin(position) pixel: vec4f) -> @location(0) vec4f {
    let p = vec2i(pixel.xy * u.split.y);
    let centre = view_at(p);
    if centre.w == 0.0 { return vec4f(1.0); }
    let dx = tangent(centre.xyz, view_at(p + vec2i(1, 0)), view_at(p - vec2i(1, 0)));
    let dy = tangent(centre.xyz, view_at(p + vec2i(0, 1)), view_at(p - vec2i(0, 1)));
    let crossed = cross(dx, dy);
    if dot(crossed, crossed) < 1e-20 { return vec4f(1.0); }
    var n = normalize(crossed);
    if dot(n, centre.xyz) > 0.0 { n = -n; }
    let up = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(n.x) > 0.9);
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    // Interleaved gradient noise rotates the spiral per pixel; the blur hides it.
    let noise = fract(52.9829189 * fract(dot(pixel.xy, vec2f(0.06711056, 0.00583715))));
    let radius = u.viewport.z;
    var covered = 0.0;
    let samples = u32(u.split.z);
    for (var i = 0u; i < samples; i++) {
        let k = (f32(i) + 0.5) / f32(samples);
        let angle = (f32(i) * 2.3999632 + noise * 6.2831853);
        let r = sqrt(1.0 - k);
        let dir = t * (cos(angle) * r) + b * (sin(angle) * r) + n * sqrt(k);
        let s = centre.xyz + dir * radius * mix(0.2, 1.0, k * k);
        let clip = u.projection * vec4f(s, 1.0);
        let uv = clip.xy / clip.w * vec2f(0.5, -0.5) + 0.5;
        if any(uv < vec2f(0.0)) || any(uv >= vec2f(1.0)) { continue; }
        let scene = view_at(vec2i(uv * u.viewport.xy));
        if scene.w == 0.0 { continue; }
        // View z is negative ahead: a nearer surface than the sample covers it,
        // fading for occluders further away than the radius.
        let range = smoothstep(0.0, 1.0, radius / max(abs(centre.z - scene.z), 1e-4));
        covered += select(0.0, range, scene.z > s.z + 0.02 * radius);
    }
    return vec4f(clamp(1.0 - covered / f32(samples) * u.viewport.w, 0.0, 1.0));
}
@fragment fn apply(@builtin(position) pixel: vec4f) -> @location(0) vec4f {
    let p = vec2i(pixel.xy);
    let centre = view_at(p);
    if centre.w == 0.0 { return vec4f(1.0); }
    let scale = i32(u.split.y);
    let size = vec2i(textureDimensions(occlusion)) - 1;
    // The 3 x 3 occlusion texels around this pixel (6 x 6 pixels at half
    // resolution), each weighted by how near its surface's depth is to this
    // one's: edges do not bleed across.
    let base = vec2i(floor(pixel.xy / f32(scale))) - 1;
    let tolerance = 0.05 * abs(centre.z) + 0.05;
    var sum = 0.0;
    var weight = 0.0;
    for (var y = 0; y < 3; y++) {
        for (var x = 0; x < 3; x++) {
            let q = clamp(base + vec2i(x, y), vec2i(0), size);
            let at = view_z(q * scale + scale / 2);
            if at.y == 0.0 { continue; }
            let w = exp(-abs(at.x - centre.z) / tolerance);
            sum += textureLoad(occlusion, q, 0).r * w;
            weight += w;
        }
    }
    return vec4f(vec3f(select(1.0, sum / weight, weight > 1e-6)), 1.0);
}
