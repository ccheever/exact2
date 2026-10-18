// The soft particle pipelines' probe variant (count.wgsl): appended after
// soft.wgsl and count.wgsl.
@fragment fn quad_soft_count_fs(v: QuadOut) -> @location(0) vec4<f32> {
    let c = soft_color(v);
    count(v, c);
    return c;
}
