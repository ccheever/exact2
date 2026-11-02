// Caustics (water.rs): the sun's light focused by the waves into a moving net
// on surfaces under the water, a factor on the sun's direct term. Two layers
// of one tiling cell pattern scroll apart; each surface point reads them where
// its ray to the sun crosses the water's height.
const WATER_IOR: f32 = 1.333;
fn caustic_hash(cell: vec2<u32>) -> vec2<f32> {
    var h = cell.x * 1664525u + cell.y * 1013904223u + 0x9e3779b9u;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return vec2(f32(h & 0xffffu), f32(h >> 16u)) / 65535.0;
}
// Distance to the nearest cell edge (the second-nearest feature point's
// distance less the nearest's) in a pattern that repeats every 4 cells.
fn caustic_edge(p: vec2<f32>) -> f32 {
    let base = floor(p);
    let f = p - base;
    var nearest = 8.0;
    var second = 8.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let o = vec2(f32(i), f32(j));
            let c = vec2<i32>(base + o);
            let point = o + 0.15 + 0.7 * caustic_hash(vec2<u32>(((c % 4) + 4) % 4));
            let distance = length(f - point);
            second = min(second, max(nearest, distance));
            nearest = min(nearest, distance);
        }
    }
    return second - nearest;
}
// The pattern's mean over its tile, so the net moves light rather than adding it.
const CAUSTIC_MEAN: f32 = 0.186;
// The sun's direct light at `world` under the water: 1 above it, without
// caustics, and with the sun at or below the horizon. A net finer than the
// pixels fades to its mean rather than shimmer.
@diagnostic(off, derivative_uniformity)
fn water_caustics(world: vec3<f32>) -> f32 {
    let height = frame.water.x;
    let to_sun = -frame.sun_direction_illuminance.xyz;
    let at = world.xz + to_sun.xz * ((height - world.y) / max(to_sun.y, 0.001));
    let p = at / max(frame.water_light.x, 0.01);
    let footprint = length(fwidth(p));
    if (u32(frame.water.y) & 2u) == 0u || world.y >= height || to_sun.y <= 0.0 { return 1.0; }
    // The waves bend the net's lines as they move.
    let warp = 0.12 * sin(p.yx * 2.1 + frame.water_waves.yz);
    let edge = min(caustic_edge(p + warp + frame.water_scroll.xy),
        caustic_edge(p * 1.27 - warp + frame.water_scroll.zw));
    // Lines widen with the footprint and dim to keep their mean, so they do not break up.
    let width = 0.09 + 1.5 * footprint;
    let net = (1.0 - smoothstep(0.0, width, edge)) * (0.09 / width);
    let strength = frame.water_waves.w * (1.0 - smoothstep(0.05, 0.25, footprint));
    return max(1.0 + strength * (net / CAUSTIC_MEAN - 1.0), 0.0);
}
