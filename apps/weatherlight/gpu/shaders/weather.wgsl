// One full-screen triangle, procedural atmosphere, no textures or per-frame
// geometry. Uniforms are reflected at build; WGSL owns their layout.
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    cloud: f32,
    rain: f32,
    daylight: f32,
    hour: f32,
    wind: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(i32(i & 1u) * 4 - 1), f32(i32(i >> 1u) * 4 - 1), 0.0, 1.0);
}

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(cell), hash(cell + vec2<f32>(1.0, 0.0)), s.x),
        mix(hash(cell + vec2<f32>(0.0, 1.0)), hash(cell + vec2<f32>(1.0, 1.0)), s.x),
        s.y
    );
}

fn fbm(start: vec2<f32>) -> f32 {
    var p = start;
    var value = 0.0;
    var amplitude = 0.52;
    for (var octave = 0; octave < 5; octave = octave + 1) {
        value = value + amplitude * noise(p);
        p = vec2<f32>(p.x * 1.7 - p.y * 1.2, p.x * 1.2 + p.y * 1.7) + 7.3;
        amplitude = amplitude * 0.48;
    }
    return value;
}

fn cloud_field(p: vec2<f32>, t: f32) -> f32 {
    let flow = vec2<f32>(t * (0.008 + u.wind * 0.0003), t * 0.002);
    let warp = noise(p * 0.65 + flow * 0.4);
    return fbm(p + flow + vec2<f32>(warp * 0.8, warp * 0.5));
}

fn rainfall(uv: vec2<f32>, t: f32) -> f32 {
    let slant = 0.1 + min(u.wind, 70.0) * 0.004;
    let column = floor((uv.x + uv.y * slant) * 75.0);
    let phase = hash(vec2<f32>(column, 7.3));
    let p = vec2<f32>((uv.x + uv.y * slant) * 75.0, uv.y * 7.0 - t * 2.8 + phase);
    let cell = floor(p);
    let f = fract(p);
    let seed = hash(cell);
    let centre = 0.2 + 0.6 * hash(vec2<f32>(column, 2.1));
    let line = 1.0 - smoothstep(0.015, 0.11, abs(f.x - centre));
    let tail = smoothstep(0.24, 0.70, f.y) * (1.0 - smoothstep(0.72, 0.80, f.y));
    return line * tail * step(0.68, seed) * 0.55;
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = position.xy / vec2<f32>(u.width, u.height);
    let aspect = u.width / u.height;
    let p = vec2<f32>(uv.x * aspect, uv.y);
    let day = clamp(u.daylight, 0.0, 1.0);
    let dawn = exp(-pow((u.hour - 6.5) / 1.8, 2.0));
    let dusk = exp(-pow((u.hour - 18.5) / 2.0, 2.0));
    let twilight = max(dawn, dusk);
    let storm = clamp(u.rain * 0.22, 0.0, 0.8);

    // Saturated upper sky falls into a warm, luminous horizon. Kept dark
    // enough for the app's white type without tinting the type itself.
    let night_top = vec3<f32>(0.015, 0.030, 0.075);
    let night_low = vec3<f32>(0.065, 0.11, 0.18);
    var upper = mix(night_top, vec3<f32>(0.035, 0.16, 0.31), day);
    var horizon = mix(night_low, vec3<f32>(0.34, 0.57, 0.65), day);
    upper = mix(upper, vec3<f32>(0.11, 0.12, 0.25), twilight * 0.65);
    horizon = mix(horizon, vec3<f32>(0.86, 0.40, 0.26), twilight * 0.88);
    var color = mix(upper, horizon, pow(uv.y, 1.4));
    color = mix(color, vec3<f32>(0.105, 0.16, 0.21), storm * 0.65);

    // Sparse stars have independent brightness and a very quiet twinkle.
    let star_grid = p * 190.0;
    let star_cell = floor(star_grid);
    let star_seed = hash(star_cell);
    let star_offset = vec2<f32>(hash(star_cell + 2.7), hash(star_cell + 8.1));
    let star_distance = length(fract(star_grid) - star_offset);
    let star = (1.0 - smoothstep(0.015, 0.12, star_distance)) * step(0.984, star_seed);
    let twinkle = 0.65 + 0.35 * sin(u.time * 0.7 + star_seed * 100.0);
    color = color + vec3<f32>(0.70, 0.81, 1.0) * star * twinkle * (1.0 - day) * (1.0 - twilight * 0.8);

    // The sun/moon lives to the right of the forecast's large type. The
    // rising/setting arc responds to the hour selected in the hourly strip.
    let arc = sin(clamp((u.hour - 6.0) / 12.0, 0.0, 1.0) * 3.141593);
    let orb_position = vec2<f32>(0.77 * aspect, mix(0.37, 0.70 - arc * 0.44, day));
    let orb_distance = length(p - orb_position);
    let orb_radius = mix(0.026, 0.038, day);
    let halo = exp(-orb_distance * orb_distance * 30.0);
    let corona = exp(-orb_distance * 19.0);
    let orb = 1.0 - smoothstep(orb_radius - 0.0015, orb_radius + 0.0015, orb_distance);
    let orb_color = mix(vec3<f32>(0.72, 0.84, 1.0), vec3<f32>(1.0, 0.85, 0.58), day);
    color = color + orb_color * (halo * 0.14 + corona * 0.18 + orb * 0.65) * (1.0 - storm * 0.6);

    // Two cloud banks at different scales produce parallax. A displaced
    // noise sample lights the rims, suggesting depth without ray marching.
    let cloud_point = vec2<f32>(p.x * 3.0, p.y * 5.5 - 0.7);
    let field = cloud_field(cloud_point, u.time);
    let threshold = mix(0.80, 0.25, u.cloud);
    let density = smoothstep(threshold, threshold + 0.20, field);
    let cloud_mask = density * (1.0 - smoothstep(0.68, 1.0, uv.y));
    let lit_field = cloud_field(cloud_point + vec2<f32>(0.10, -0.16), u.time);
    let rim = clamp((field - lit_field) * 4.0, 0.0, 1.0);
    let cloud_dark = mix(vec3<f32>(0.045, 0.065, 0.12), vec3<f32>(0.26, 0.38, 0.47), day);
    let cloud_light = mix(vec3<f32>(0.22, 0.26, 0.38), vec3<f32>(0.73, 0.79, 0.78), day);
    var cloud_color = mix(cloud_dark, cloud_light, rim * 0.7 + field * 0.25);
    cloud_color = mix(cloud_color, vec3<f32>(0.65, 0.38, 0.35), twilight * rim * 0.8);
    color = mix(color, cloud_color, cloud_mask * 0.92);
    let wisps = smoothstep(0.50, 0.78, fbm(p * vec2<f32>(3.0, 12.0) + vec2<f32>(u.time * 0.012, 2.4)));
    color = mix(color, cloud_light * 0.65, wisps * u.cloud * 0.20);

    // Atmospheric depth, then two low rolling ridges. All terrain is
    // procedural and stationary, anchoring the slowly moving atmosphere.
    let ridge_far = 0.88 + sin(uv.x * 7.0 + 0.4) * 0.025 + noise(vec2<f32>(uv.x * 8.0, 1.0)) * 0.04;
    let ridge_near = 0.94 + sin(uv.x * 8.0 + 2.0) * 0.017 + noise(vec2<f32>(uv.x * 13.0, 6.0)) * 0.025;
    let far_mask = smoothstep(ridge_far - 0.001, ridge_far + 0.001, uv.y);
    let near_mask = smoothstep(ridge_near - 0.001, ridge_near + 0.001, uv.y);
    color = mix(color, mix(vec3<f32>(0.025, 0.050, 0.08), vec3<f32>(0.08, 0.18, 0.22), day), far_mask);
    color = mix(color, vec3<f32>(0.025, 0.060, 0.075), near_mask);

    let rain = rainfall(p, u.time) + rainfall(p * 1.7 + 8.0, u.time * 1.23) * 0.5;
    color = color + vec3<f32>(0.48, 0.66, 0.78) * rain * clamp(u.rain * 0.35, 0.0, 0.8);
    // A gentle left-edge shade leaves room for the headline; subtle grain
    // prevents visible gradient banding on ordinary eight-bit displays.
    color = color * (0.78 + 0.22 * smoothstep(0.0, 0.8, uv.x));
    color = color + (hash(position.xy) - 0.5) / 255.0;
    return vec4<f32>(color, 1.0);
}
