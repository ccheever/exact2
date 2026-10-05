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
    let p = corners[vertex];
    let world = center.xyz + right.xyz*(p.x-0.5) + up.xyz*(p.y-0.5);
    return QuadOut(frame.view_proj*vec4(world,1.),uv.xy+vec2(p.x,1.-p.y)*uv.zw,color,p,right.w,center.w,world);
}
