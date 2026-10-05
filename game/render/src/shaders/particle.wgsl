fn quad_color(v: QuadOut) -> vec4<f32> {
    let radius = length(v.local*2.-1.);
    let alpha = v.color.a * (1.-smoothstep(0.35,1.,radius));
    return vec4(v.color.rgb,alpha);
}
@fragment fn quad_fs(v: QuadOut) -> @location(0) vec4<f32> { return quad_color(v); }
