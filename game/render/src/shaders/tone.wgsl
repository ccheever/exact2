// The tone curve: exposure, ACES fitted to the target's headroom, then the sRGB
// transfer unless the target encodes it. The tonemap pass applies it to the
// resolved HDR image; a direct frame's shaders apply it to each sample.
override ENCODE_SRGB: bool = true;
fn tonemap(hdr_color: vec3<f32>) -> vec4<f32> {
    // Glow's optional shoulder is applied to its emissive input, never other materials.
    let x = finite_hdr(finite_hdr(hdr_color) * frame.ground_exposure.w);
    var color = x;
    if frame.logical_size.w <= 0.5 {
        // The curve's white is the target's headroom (LLP 1100 D12b): `h·f(x/h)`
        // keeps the toe's slope and reaches h, and is the SDR curve at h = 1.
        let h = max(frame.logical_size.z, 1.0);
        let y = x / h;
        color = h * clamp((y * (2.51 * y + 0.03)) / (y * (2.43 * y + 0.59) + 0.14), vec3(0.0), vec3(1.0));
    }
    if ENCODE_SRGB {
        color = select(1.055 * pow(color, vec3(1.0 / 2.4)) - 0.055,
                       12.92 * color, color <= vec3(0.0031308));
    }
    return vec4(color, 1.0);
}
