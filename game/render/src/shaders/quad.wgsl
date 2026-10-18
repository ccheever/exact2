struct QuadOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local: vec2<f32>,
    @location(3) @interpolate(flat) mode: f32,
    @location(4) @interpolate(flat) cutoff: f32,
    @location(5) world: vec3<f32>,
}
@vertex fn quad_vs(@builtin(vertex_index) vertex: u32,
    @location(0) center: vec4<f32>, @location(1) right: vec4<f32>,
    @location(2) up: vec4<f32>, @location(3) color: vec4<f32>,
    @location(4) uv: vec4<f32>) -> QuadOut {
    let corners = array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(1.,1.),vec2(0.,0.),vec2(1.,1.),vec2(0.,1.));
    var p = corners[vertex];
    var world = center.xyz + right.xyz*(p.x-0.5) + up.xyz*(p.y-0.5);
    if right.w == 5. {
        // A triangle occupies one quad instance; the second triangle is degenerate.
        let triangle = array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(0.,1.),vec2(0.,0.),vec2(0.,0.),vec2(0.,0.));
        p = triangle[vertex];
        world = center.xyz + right.xyz*p.x + up.xyz*p.y;
    }
    var rgba = color;
    if right.w == 5. {
        let n = normalize(cross(right.xyz, up.xyz));
        let diffuse = environment(n.y)*frame.zenith_ambient.w
            + frame.sun_color_count.xyz*frame.sun_direction_illuminance.w*max(0., dot(n, -frame.sun_direction_illuminance.xyz));
        rgba = vec4(color.rgb*diffuse + uv.xyz, color.a);
    }
    return QuadOut(frame.view_proj*vec4(world,1.),uv.xy+vec2(p.x,1.-p.y)*uv.zw,rgba,p,right.w,center.w,world);
}
// Tonemapped in place for a direct frame, as forward.wgsl's DIRECT; alpha passes
// through to the blend.
override DIRECT: bool = false;
fn quad_out(c: vec4<f32>) -> vec4<f32> {
    if DIRECT { return vec4(tonemap(c.rgb).rgb, c.a); }
    return c;
}
