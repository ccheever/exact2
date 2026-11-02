// Per-slot screen-door fade, 1 - Opacity (lights.rs); zero is opaque.
@group(0) @binding(9) var<storage, read> fades: array<f32>;
fn fade_of(slot: u32) -> f32 {
    if slot >= arrayLength(&fades) { return 0.0; }
    return fades[slot];
}
// Whether an ordered 4 x 4 dither drops this pixel of a fading opaque surface.
fn faded(slot: u32, pixel: vec2<f32>) -> bool {
    let fade = fade_of(slot);
    if fade <= 0.0 { return false; }
    let p = vec2<u32>(pixel) % 4u;
    let bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    return (bayer[p.y * 4u + p.x] + 0.5) / 16.0 < fade;
}
