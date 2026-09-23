// Frustum culling for the camera and each sun cascade, after skinning and before
// the geometry passes. Prepended: frame.wgsl and transform.wgsl (bindings 0-5).
// Three dispatches: `test` masks every item per view and scans its 64-item chunk,
// `scan` turns chunk counts into offsets and indirect arguments per group, and
// `scatter` writes each visible slot at its stable position. Order within a draw
// group is the retained slot order, so depth ties resolve as before culling.
// @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
struct Cull {
    planes: array<vec4<f32>, 24>, // six normalized planes per view: camera, cascades 0-2
    // views, chunks, groups, region stride (words)
    counts: vec4<u32>,
    // groups, chunks, model records, skins: word offsets into `setup`
    sections: vec4<u32>,
    // chunk counts, chunk offsets: word offsets into `scratch`; items start at zero
    scratch_sections: vec4<u32>,
}
@group(0) @binding(6) var<uniform> cull: Cull;
@group(0) @binding(7) var<storage, read> setup: array<u32>;
@group(0) @binding(8) var<storage, read> palette: array<mat4x4<f32>>;
@group(0) @binding(9) var<storage, read_write> scratch: array<u32>;
@group(0) @binding(10) var<storage, read_write> compacted: array<u32>;
@group(0) @binding(11) var<storage, read_write> indirect: array<u32>;

const KEEP_ALL: u32 = 16u;   // group flag: GPU deformation, never culled
const CAPSULE: u32 = 64u;    // group flag: some vertices use the capsule scale
const PLAIN: u32 = 128u;     // group flag: some vertices use per-axis dimensions
const GROUP_WORDS: u32 = 16u;
var<workgroup> lanes: array<u32, 64>;
var<workgroup> sums: array<vec4<u32>, 64>;

fn f(word: u32) -> f32 { return bitcast<f32>(word); }
fn fv(at: u32) -> vec3<f32> { return vec3(f(setup[at]), f(setup[at + 1u]), f(setup[at + 2u])); }
fn group_at(chunk: u32) -> u32 { return cull.sections.x + setup[cull.sections.y + chunk] * GROUP_WORDS; }
fn index(id: vec3<u32>) -> u32 { return id.x + id.y * 65535u; }
fn max3(v: vec3<f32>) -> f32 { return max(v.x, max(v.y, v.z)); }
// Upper bound of the spectral norm of an affine map's linear part.
fn norm(m: mat4x4<f32>) -> f32 {
    return sqrt(dot(m[0].xyz, m[0].xyz) + dot(m[1].xyz, m[1].xyz) + dot(m[2].xyz, m[2].xyz));
}
fn unit(q: vec4<f32>) -> vec4<f32> { return q * inverseSqrt(max(dot(q, q), 1e-12)); }

// Entity-local bounding sphere (centre xyz, radius w) of one item.
fn local_sphere(value: u32, transform: u32, g: u32) -> vec4<f32> {
    let flags = setup[g + 7u];
    if value < 2147483648u {
        // transform.wgsl: select(dims, dims.x, capsule) * position + (0, cap * dims.y, 0).
        let m = transform * 12u;
        let dims = abs(vec3(materials[m + 9u], materials[m + 10u], materials[m + 11u]));
        var scale = vec3(0.0);
        if (flags & PLAIN) != 0u { scale = dims; }
        if (flags & CAPSULE) != 0u { scale = max(scale, vec3(dims.x)); }
        let extent = scale * fv(g + 9u) + vec3(0.0, f(setup[g + 12u]) * dims.y, 0.0);
        return vec4(0.0, 0.0, 0.0, length(extent));
    }
    let record = cull.sections.z + (value - 2147483648u) * 8u;
    let sphere = vec4(fv(record + 4u), f(setup[record + 7u]));
    let skin = setup[record + 1u];
    if skin == 0u { return sphere; }
    // Skinned positions are convex combinations of each joint's transformed bind
    // position, so the union of every joint's transformed mesh sphere contains them.
    let at = cull.sections.w + (skin - 1u) * 20u;
    let first = setup[at];
    var lo = vec3(3.0e38);
    var hi = vec3(-3.0e38);
    for (var j = 0u; j < setup[at + 1u]; j++) {
        let joint = palette[first + j];
        let c = (joint * vec4(sphere.xyz, 1.0)).xyz;
        let r = sphere.w * norm(joint);
        lo = min(lo, c - r);
        hi = max(hi, c + r);
    }
    let local = mat4x4(bitcast<vec4<f32>>(vec4(setup[at + 4u], setup[at + 5u], setup[at + 6u], setup[at + 7u])),
        bitcast<vec4<f32>>(vec4(setup[at + 8u], setup[at + 9u], setup[at + 10u], setup[at + 11u])),
        bitcast<vec4<f32>>(vec4(setup[at + 12u], setup[at + 13u], setup[at + 14u], setup[at + 15u])),
        bitcast<vec4<f32>>(vec4(setup[at + 16u], setup[at + 17u], setup[at + 18u], setup[at + 19u])));
    let centre = (lo + hi) * 0.5;
    return vec4((local * vec4(centre, 1.0)).xyz, length(hi - lo) * 0.5 * norm(local));
}

// World sphere containing every interpolated pose between the two ticks.
fn world_sphere(value: u32, g: u32) -> vec4<f32> {
    var transform = value;
    if value >= 2147483648u { transform = setup[cull.sections.z + (value - 2147483648u) * 8u]; }
    let local = local_sphere(value, transform, g);
    if attached(transform) {
        let m = attachment_matrices[transform];
        return vec4((m * vec4(local.xyz, 1.0)).xyz, local.w * norm(m));
    }
    let i = transform * 10u;
    let p0 = vec3(prev[i], prev[i + 1u], prev[i + 2u]);
    let p1 = vec3(curr[i], curr[i + 1u], curr[i + 2u]);
    let q0 = unit(vec4(prev[i + 3u], prev[i + 4u], prev[i + 5u], prev[i + 6u]));
    let q1 = unit(vec4(curr[i + 3u], curr[i + 4u], curr[i + 5u], curr[i + 6u]));
    var s0 = vec3(prev[i + 7u], prev[i + 8u], prev[i + 9u]);
    var s1 = vec3(curr[i + 7u], curr[i + 8u], curr[i + 9u]);
    if value < 2147483648u { s0 = abs(s0); s1 = abs(s1); }
    // Scale interpolates linearly, so the scaled centre stays on the u0-u1 segment.
    // Rotating a fixed vector along the shortest-path nlerp sweeps an arc of at most
    // pi about one axis, inside the ball on its chord. Translation is a segment.
    let u0 = s0 * local.xyz;
    let u1 = s1 * local.xyz;
    let mid = (u0 + u1) * 0.5;
    let e0 = rotate(q0, mid);
    let e1 = rotate(q1, mid);
    let centre = (p0 + p1 + e0 + e1) * 0.5;
    let radius = distance(p0, p1) * 0.5 + distance(e0, e1) * 0.5 + distance(u0, u1) * 0.5
        + max(max3(abs(s0)), max3(abs(s1))) * local.w;
    return vec4(centre, radius);
}

fn visible(view: u32, sphere: vec4<f32>) -> bool {
    // Rounding margin: vertices are transformed in f32 relative to large coordinates.
    let r = sphere.w * 1.0001 + 1e-4 + 1e-6 * max3(abs(sphere.xyz));
    for (var i = 0u; i < 6u; i++) {
        let plane = cull.planes[view * 6u + i];
        if dot(plane.xyz, sphere.xyz) + plane.w < -r { return false; }
    }
    return true;
}

@compute @workgroup_size(64)
fn test(@builtin(workgroup_id) id: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let chunk = index(id);
    if chunk >= cull.counts.y { return; }
    let g = group_at(chunk);
    let local = (chunk - setup[g + 2u]) * 64u + lane;
    let flags = setup[g + 7u];
    var mask = 0u;
    if local < setup[g + 1u] {
        mask = flags & 15u;
        if (flags & KEEP_ALL) == 0u {
            let sphere = world_sphere(slots[setup[g] + local], g);
            for (var v = 0u; v < cull.counts.x; v++) {
                if (mask & (1u << v)) != 0u && !visible(v, sphere) { mask &= ~(1u << v); }
            }
        }
    }
    // One inclusive scan for four byte-packed views; a chunk counts at most 64.
    let packed = (mask & 1u) | ((mask & 2u) << 7u) | ((mask & 4u) << 14u) | ((mask & 8u) << 21u);
    lanes[lane] = packed;
    for (var step = 1u; step < 64u; step <<= 1u) {
        workgroupBarrier();
        var add = 0u;
        if lane >= step { add = lanes[lane - step]; }
        workgroupBarrier();
        lanes[lane] += add;
    }
    workgroupBarrier();
    let before = lanes[lane] - packed;
    if local < setup[g + 1u] {
        scratch[setup[g] + local] = mask | ((before & 63u) << 4u) | (((before >> 8u) & 63u) << 10u)
            | (((before >> 16u) & 63u) << 16u) | (((before >> 24u) & 63u) << 22u);
    }
    if lane == 63u { scratch[cull.scratch_sections.x + chunk] = lanes[63]; }
}

fn unpack(packed: u32) -> vec4<u32> {
    return vec4(packed & 255u, (packed >> 8u) & 255u, (packed >> 16u) & 255u, packed >> 24u);
}

@compute @workgroup_size(64)
fn scan(@builtin(workgroup_id) id: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let group = index(id);
    if group >= cull.counts.z { return; }
    let g = cull.sections.x + group * GROUP_WORDS;
    let first_chunk = setup[g + 2u];
    let chunks = setup[g + 3u];
    let per = (chunks + 63u) / 64u;
    let first = first_chunk + min(lane * per, chunks);
    let last = first_chunk + min(lane * per + per, chunks);
    var sum = vec4(0u);
    for (var c = first; c < last; c++) { sum += unpack(scratch[cull.scratch_sections.x + c]); }
    sums[lane] = sum;
    for (var step = 1u; step < 64u; step <<= 1u) {
        workgroupBarrier();
        var add = vec4(0u);
        if lane >= step { add = sums[lane - step]; }
        workgroupBarrier();
        sums[lane] += add;
    }
    workgroupBarrier();
    var running = sums[lane] - sum;
    for (var c = first; c < last; c++) {
        for (var v = 0u; v < 4u; v++) { scratch[cull.scratch_sections.y + c * 4u + v] = running[v]; }
        running += unpack(scratch[cull.scratch_sections.x + c]);
    }
    if lane == 63u {
        let total = sums[63];
        for (var v = 0u; v < 4u; v++) {
            let at = (v * cull.counts.z + group) * 5u;
            indirect[at] = setup[g + 4u];
            indirect[at + 1u] = total[v];
            indirect[at + 2u] = setup[g + 5u];
            indirect[at + 3u] = setup[g + 6u];
            indirect[at + 4u] = 0u;
        }
    }
}

@compute @workgroup_size(64)
fn scatter(@builtin(workgroup_id) id: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let chunk = index(id);
    if chunk >= cull.counts.y { return; }
    let g = group_at(chunk);
    let local = (chunk - setup[g + 2u]) * 64u + lane;
    if local >= setup[g + 1u] { return; }
    let item = setup[g] + local;
    let word = scratch[item];
    for (var v = 0u; v < 4u; v++) {
        if (word & (1u << v)) != 0u {
            let at = v * cull.counts.w + setup[g + 8u]
                + scratch[cull.scratch_sections.y + chunk * 4u + v] + ((word >> (4u + 6u * v)) & 63u);
            compacted[at] = slots[item];
        }
    }
}
