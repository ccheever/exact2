// The soft particle pipelines' probe variant (count.wgsl): appended after
// soft.wgsl and count.wgsl.
@fragment fn quad_soft_count_fs(v: QuadOut, @builtin(sample_mask) mask: u32) -> @location(0) vec4<f32> {
    let c = soft_color(v);
    count(v, mask, c);
    return c;
}
