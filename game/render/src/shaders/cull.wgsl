// Frustum culling for the camera and each sun cascade, after skinning and before
// the geometry passes. Prepended: frame.wgsl and transform.wgsl (bindings 0-5).
// Three dispatches: `test` masks every item per view and scans its 64-item chunk,
// `scan` sorts each group's chunks nearest first for the camera and turns chunk
// counts into offsets and indirect arguments per group, and `scatter`
// writes each visible slot at its stable position. The camera draws a group's
// chunks nearest first, so depth tested before shading rejects what they hide (a
// GPU without order-independent hidden-surface removal shades it otherwise).
// Within a chunk, and in the shadow views, order is the retained slot order.
// @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
struct Cull {
    planes: array<vec4<f32>, 24>, // six normalized planes per view: camera, cascades 0-2
    // views, chunks, groups, region stride (words)
    counts: vec4<u32>,
    // groups, chunks, model records, skins: word offsets into `setup`
    sections: vec4<u32>,
    // chunk counts, chunk offsets, chunk depths, then the camera's chunk order
    // followed by `sort_chunks`'s histograms: word offsets into `scratch`;
    // items start at zero
    scratch_sections: vec4<u32>,
    // x nonzero: the camera's occlusion pyramid is this frame's (`occluded`);
    // y nonzero: a draw's first instance is its region (INDIRECT_FIRST_INSTANCE)
    options: vec4<u32>,
}
@group(0) @binding(6) var<uniform> cull: Cull;
@group(0) @binding(7) var<storage, read> setup: array<u32>;
// Four explicit columns preserve the 64-byte matrix ABI. Adreno 620 misaddresses
// storage array<mat4x4<f32>> columns, even without dependent dispatches.
@group(0) @binding(8) var<storage, read> palette: array<vec4<f32>>;
@group(0) @binding(9) var<storage, read_write> scratch: array<u32>;
@group(0) @binding(12) var pyramid: texture_2d<f32>;
@group(0) @binding(10) var<storage, read_write> compacted: array<u32>;
@group(0) @binding(11) var<storage, read_write> indirect: array<u32>;

const KEEP_ALL: u32 = 16u;   // group flag: GPU deformation, never culled
const CAPSULE: u32 = 64u;    // group flag: some vertices use the capsule scale
const PLAIN: u32 = 128u;     // group flag: some vertices use per-axis dimensions
const EXEMPT: u32 = 32u;     // group flag: never occluded (a view model's own depth range)
const OUTLINE: u32 = 256u; // camera-only hull expansion
const GROUP_WORDS: u32 = 16u;
var<workgroup> lanes: array<u32, 64>;
var<workgroup> sums: array<vec4<u32>, 64>;
var<workgroup> nearest: array<f32, 64>;
// The camera's chunks sort by depth in BUCKETS steps (`test`).
const BUCKETS: u32 = 256u;

fn palette_matrix(i:u32)->mat4x4<f32> {let b=i*4u; return mat4x4(palette[b],palette[b+1u],palette[b+2u],palette[b+3u]);}
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
        let joint = palette_matrix(first + j);
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
    var s0 = vec3(prev[i + 7u], prev[i + 8u], prev[i + 9u]);
    var s1 = vec3(curr[i + 7u], curr[i + 8u], curr[i + 9u]);
    if all(local.xyz == vec3(0.0)) {
        // A sphere about the entity's own origin: rotation moves nothing, and the
        // terms below for it are zero (the same sphere, without the quaternions).
        return vec4((p0 + p1) * 0.5, distance(p0, p1) * 0.5
            + max(max3(abs(s0)), max3(abs(s1))) * local.w);
    }
    let q0 = unit(vec4(prev[i + 3u], prev[i + 4u], prev[i + 5u], prev[i + 6u]));
    let q1 = unit(vec4(curr[i + 3u], curr[i + 4u], curr[i + 5u], curr[i + 6u]));
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

// Reach is only a cap. World width and the CSS floor bound the requested push.
// Use the far end of the surface's depth interval: metres per pixel grow with
// depth, so the nearest depth would under-bound vertices on the far side.
fn hull_sphere(value: u32, g: u32, sphere: vec4<f32>) -> vec4<f32> {
    if (setup[g + 7u] & OUTLINE) == 0u { return sphere; }
    let record = cull.sections.z + (value - 2147483648u) * 8u;
    var reach = f(setup[record + 3u]);
    let skin = setup[record + 1u];
    if skin != 0u {
        let at = cull.sections.w + (skin - 1u) * 20u;
        var stretch = 0.0;
        for (var j = 0u; j < setup[at + 1u]; j++) { stretch = max(stretch, norm(palette_matrix(setup[at] + j))); }
        let local = mat4x4(bitcast<vec4<f32>>(vec4(setup[at + 4u], setup[at + 5u], setup[at + 6u], setup[at + 7u])),
            bitcast<vec4<f32>>(vec4(setup[at + 8u], setup[at + 9u], setup[at + 10u], setup[at + 11u])),
            bitcast<vec4<f32>>(vec4(setup[at + 12u], setup[at + 13u], setup[at + 14u], setup[at + 15u])),
            bitcast<vec4<f32>>(vec4(setup[at + 16u], setup[at + 17u], setup[at + 18u], setup[at + 19u])));
        reach *= stretch * norm(local);
    }
    let transform = setup[record];
    if attached(transform) { reach *= norm(attachment_matrices[transform]); }
    else {
        let i = transform * 10u;
        reach *= max(max3(abs(vec3(prev[i+7u], prev[i+8u], prev[i+9u]))),
            max3(abs(vec3(curr[i+7u], curr[i+8u], curr[i+9u]))));
    }
    let depth = max(dot(frame.view_depth, vec4(sphere.xyz, 1.0)) + sphere.w, 0.0);
    let pixel = frame.toon.z * select(depth, 1.0, frame.toon.w > 0.5);
    let width = min(max(f(setup[g + 9u]), f(setup[g + 10u]) * pixel), reach);
    return vec4(sphere.xyz, sphere.w + width);
}

// Whether `sphere` lies behind the occlusion pyramid wherever it can cover the
// screen: its nearest depth beyond the farthest depth in every texel under its
// bounds, read at the level where those bounds span at most two texels a side.
fn occluded(sphere: vec4<f32>) -> bool {
    let m = frame.view_proj;
    let row0 = vec4(m[0].x, m[1].x, m[2].x, m[3].x);
    let row1 = vec4(m[0].y, m[1].y, m[2].y, m[3].y);
    let row2 = vec4(m[0].z, m[1].z, m[2].z, m[3].z);
    let row3 = vec4(m[0].w, m[1].w, m[2].w, m[3].w);
    let c = vec4(sphere.xyz, 1.0);
    let r = sphere.w * 1.0001 + 1e-4;
    // Each clip coordinate ranges over its value at the centre plus or minus the
    // radius times its row's length; w stays positive or nothing is claimed.
    let x = dot(row0, c);
    let y = dot(row1, c);
    let w = dot(row3, c);
    let ex = r * length(row0.xyz);
    let ey = r * length(row1.xyz);
    let ew = r * length(row3.xyz);
    let w0 = w - ew;
    if w0 <= 1e-5 { return false; }
    let w1 = w + ew;
    let lo = vec2(min((x - ex) / w0, (x - ex) / w1), min((y - ey) / w0, (y - ey) / w1));
    let hi = vec2(max((x + ex) / w0, (x + ex) / w1), max((y + ey) / w0, (y + ey) / w1));
    // Depth follows w alone: the sphere's nearest is at w0.
    let near = dot(row2, c - vec4(row3.xyz / max(length(row3.xyz), 1e-30) * r, 0.0)) / w0;
    let size = vec2<f32>(textureDimensions(pyramid, 0));
    let a = clamp(vec2(lo.x * 0.5 + 0.5, 0.5 - hi.y * 0.5), vec2(0.0), vec2(1.0)) * size;
    let b = clamp(vec2(hi.x * 0.5 + 0.5, 0.5 - lo.y * 0.5), vec2(0.0), vec2(1.0)) * size;
    var t0 = min(vec2<u32>(a), vec2<u32>(size) - 1u);
    var t1 = min(vec2<u32>(b), vec2<u32>(size) - 1u);
    let levels = textureNumLevels(pyramid);
    var level = 0u;
    while level + 1u < levels && any((t1 >> vec2(level)) - (t0 >> vec2(level)) > vec2(1u)) {
        level++;
    }
    if any((t1 >> vec2(level)) - (t0 >> vec2(level)) > vec2(1u)) { return false; }
    // A level's last texel also holds its source's odd row and column.
    let top = textureDimensions(pyramid, level) - 1u;
    t0 = min(t0 >> vec2(level), top);
    t1 = min(t1 >> vec2(level), top);
    var far = 0.0;
    for (var ty = t0.y; ty <= t1.y; ty++) {
        for (var tx = t0.x; tx <= t1.x; tx++) {
            far = max(far, textureLoad(pyramid, vec2(tx, ty), level).r);
        }
    }
    return near > far;
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
    // How far the item's nearest point lies from the camera's near plane towards
    // its far plane, 0 to 1 (cull.rs `planes`: an infinite far plane is 1 away).
    var depth = 3.0e38;
    if local < setup[g + 1u] {
        mask = flags & 15u;
        if (flags & KEEP_ALL) == 0u {
            let value = slots[setup[g] + local];
            let sphere = world_sphere(value, g);
            let camera = hull_sphere(value, g, sphere);
            for (var v = 0u; v < cull.counts.x; v++) {
                if (mask & (1u << v)) != 0u && !visible(v, select(sphere, camera, v == 0u)) { mask &= ~(1u << v); }
            }
            if (mask & 1u) != 0u && cull.options.x != 0u && (flags & EXEMPT) == 0u && occluded(camera) {
                mask &= ~1u;
            }
            let near = max(dot(cull.planes[4].xyz, sphere.xyz) + cull.planes[4].w - sphere.w, 0.0);
            let far = max(dot(cull.planes[5].xyz, sphere.xyz) + cull.planes[5].w + sphere.w, 0.0);
            depth = near / max(near + far, 1e-30);
        }
        // A level of detail not chosen for its entity this frame draws in no view,
        // kept-all groups included.
        let value = slots[setup[g] + local];
        if value >= 2147483648u && setup[cull.sections.z + (value - 2147483648u) * 8u + 2u] != 0u {
            mask = 0u;
        }
    }
    // The chunk's nearest item the camera draws.
    nearest[lane] = select(3.0e38, depth, (mask & 1u) != 0u);
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
    for (var step = 32u; step > 0u; step >>= 1u) {
        if lane < step { nearest[lane] = min(nearest[lane], nearest[lane + step]); }
        workgroupBarrier();
    }
    // The chunk's depth bucket, finer near the camera; a chunk the camera does not
    // draw sorts last.
    if lane == 0u {
        scratch[cull.scratch_sections.z + chunk] =
            u32(sqrt(clamp(nearest[0], 0.0, 1.0)) * f32(BUCKETS - 1u));
    }
}

// A look frame's account of the cull (LLP 1127.000 D4), dispatched after the
// frame's passes with an output buffer bound as `scratch`: for each item, at
// (chunk * 64 + lane) * 6, its world sphere (the interpolation-swept, skinned
// one `test` reads), the views whose frustum holds it, and whether the camera's
// occlusion pyramid hides it. `test`'s own functions decide each.
@compute @workgroup_size(64)
fn probe(@builtin(workgroup_id) id: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let chunk = index(id);
    if chunk >= cull.counts.y { return; }
    let g = group_at(chunk);
    let local = (chunk - setup[g + 2u]) * 64u + lane;
    if local >= setup[g + 1u] { return; }
    let flags = setup[g + 7u];
    var sphere = vec4(0.0);
    var seen = flags & 15u;
    var hidden = 0u;
    if (flags & KEEP_ALL) == 0u {
        let value = slots[setup[g] + local];
        sphere = world_sphere(value, g);
        let camera = hull_sphere(value, g, sphere);
        for (var v = 0u; v < cull.counts.x; v++) {
            if (seen & (1u << v)) != 0u && !visible(v, select(sphere, camera, v == 0u)) { seen &= ~(1u << v); }
        }
        if (seen & 1u) != 0u && cull.options.x != 0u && (flags & EXEMPT) == 0u && occluded(camera) {
            hidden = 1u;
        }
    }
    let at = (chunk * 64u + lane) * 6u;
    scratch[at] = bitcast<u32>(sphere.x);
    scratch[at + 1u] = bitcast<u32>(sphere.y);
    scratch[at + 2u] = bitcast<u32>(sphere.z);
    scratch[at + 3u] = bitcast<u32>(sphere.w);
    scratch[at + 4u] = seen;
    scratch[at + 5u] = hidden;
}

fn unpack(packed: u32) -> vec4<u32> {
    return vec4(packed & 255u, (packed >> 8u) & 255u, (packed >> 16u) & 255u, packed >> 24u);
}
// Position `c`'s counts: the camera's from its chunk in `camera` order, the rest from `c`.
fn counts_at(c: u32, camera: u32) -> vec4<u32> {
    let near = unpack(scratch[cull.scratch_sections.x + camera]);
    let rest = unpack(scratch[cull.scratch_sections.x + c]);
    return vec4(near.x, rest.yzw);
}

// The chunk the camera takes at position `c`.
fn camera_chunk(c: u32) -> u32 {
    return scratch[cull.scratch_sections.w + c];
}

// The camera's chunks of a group by depth bucket, ties by chunk, into `scratch`'s
// order section: a stable counting sort over the workgroup's 64 lanes, each with
// a contiguous run of chunks and a histogram row in the group's region (setup
// word 13; none for a group of few chunks, which keeps its order). Workgroup
// arrays this size took an Adreno 620's Vulkan driver seconds to compile.
const UNSORTED: u32 = 0xffffffffu;
fn bucket(chunk: u32) -> u32 { return min(scratch[cull.scratch_sections.z + chunk], BUCKETS - 1u); }
fn sort_chunks(g: u32, first: u32, chunks: u32, lane: u32) {
    let out = cull.scratch_sections.w + first;
    let region = setup[g + 13u];
    let sorted = region != UNSORTED;
    let per = (chunks + 63u) / 64u;
    let lo = min(lane * per, chunks);
    let hi = min(lo + per, chunks);
    let hist = cull.scratch_sections.w + cull.counts.y + region * 64u * BUCKETS;
    let row = hist + lane * BUCKETS;
    // Lane `lane` owns buckets [lane * OWN, lane * OWN + OWN) in the prefix.
    let own = BUCKETS / 64u;
    if sorted {
        for (var b = 0u; b < BUCKETS; b++) { scratch[row + b] = 0u; }
        for (var c = lo; c < hi; c++) { scratch[row + bucket(first + c)] += 1u; }
    } else {
        for (var c = lo; c < hi; c++) { scratch[out + c] = first + c; }
    }
    storageBarrier();
    workgroupBarrier();
    var total = 0u;
    if sorted {
        for (var k = 0u; k < own; k++) {
            let b = lane * own + k;
            for (var l = 0u; l < 64u; l++) {
                let at = hist + l * BUCKETS + b;
                let count = scratch[at];
                scratch[at] = total;
                total += count;
            }
        }
    }
    lanes[lane] = total;
    workgroupBarrier();
    if sorted {
        var base = 0u;
        for (var l = 0u; l < lane; l++) { base += lanes[l]; }
        for (var k = 0u; k < own; k++) {
            let b = lane * own + k;
            for (var l = 0u; l < 64u; l++) { scratch[hist + l * BUCKETS + b] += base; }
        }
    }
    storageBarrier();
    workgroupBarrier();
    if sorted {
        for (var c = lo; c < hi; c++) {
            let at = row + bucket(first + c);
            scratch[out + scratch[at]] = first + c;
            scratch[at] += 1u;
        }
    }
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
    // In this dispatch: across dispatches, an Adreno 620's Vulkan driver sometimes
    // left `scan` without the order another wrote.
    sort_chunks(g, first_chunk, chunks, lane);
    storageBarrier();
    workgroupBarrier();
    var sum = vec4(0u);
    for (var c = first; c < last; c++) { sum += counts_at(c, camera_chunk(c)); }
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
        let camera = camera_chunk(c);
        scratch[cull.scratch_sections.y + camera * 4u] = running.x;
        for (var v = 1u; v < 4u; v++) { scratch[cull.scratch_sections.y + c * 4u + v] = running[v]; }
        running += counts_at(c, camera);
    }
    if lane == 63u {
        let total = sums[63];
        for (var v = 0u; v < 4u; v++) {
            let at = (v * cull.counts.z + group) * 5u;
            indirect[at] = setup[g + 4u];
            indirect[at + 1u] = total[v];
            indirect[at + 2u] = setup[g + 5u];
            indirect[at + 3u] = setup[g + 6u];
            indirect[at + 4u] = select(0u, v * cull.counts.w + setup[g + 8u], cull.options.y != 0u);
        }
        // An outlined group's hull draws the camera's instances again from a
        // stream of its own, which starts at vertex zero.
        if (setup[g + 7u] & OUTLINE) != 0u {
            let at = (4u * cull.counts.z + group) * 5u;
            indirect[at] = setup[g + 4u];
            indirect[at + 1u] = total[0];
            indirect[at + 2u] = setup[g + 5u];
            indirect[at + 3u] = 0u;
            indirect[at + 4u] = select(0u, setup[g + 8u], cull.options.y != 0u);
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
