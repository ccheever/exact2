// The card stack (LLP 1014 D5): each direct child of the canvas — a card
// laid out by the kernel and captured by the host on its own — drawn as a
// textured quad through its own placement, in perspective, back to front.
// The placement's columns come from the surface, which also hands the same
// mapping to the host as a homography for hit-testing.
struct Card {
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    c3: vec4<f32>,
    tint: vec4<f32>,
    size: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> card: Card;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Fragment {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    let c = corners[i];
    let m = mat4x4<f32>(card.c0, card.c1, card.c2, card.c3);
    var out: Fragment;
    out.position = m * vec4<f32>(c * card.size, 0.0, 1.0);
    out.uv = c;
    return out;
}

@fragment
fn fs(f: Fragment) -> @location(0) vec4<f32> {
    let t = textureSample(picture, smp, f.uv);
    // Premultiplied in, premultiplied out; the tint dims a card that is not
    // the focus, the alpha fades one that is off the deck.
    return vec4<f32>(t.rgb * card.tint.rgb, t.a) * card.tint.a;
}
