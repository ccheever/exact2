@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;
fn quad_color(v: QuadOut) -> vec4<f32> {
    var color = textureSample(sprite_texture,sprite_sampler,v.uv) * v.color;
    if v.mode == 1. && color.a < v.cutoff { discard; }
    if v.mode < 2. { color.a = 1.; }
    return color;
}
@fragment fn quad_fs(v: QuadOut) -> @location(0) vec4<f32> { return quad_color(v); }
