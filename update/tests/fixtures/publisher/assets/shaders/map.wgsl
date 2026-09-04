// The line map: colored triangles in normalized device coordinates.
struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs(v: Vertex) -> Fragment {
    var out: Fragment;
    out.position = vec4<f32>(v.position, 0.0, 1.0);
    out.color = v.color;
    return out;
}

@fragment
fn fs(f: Fragment) -> @location(0) vec4<f32> {
    return f.color;
}
