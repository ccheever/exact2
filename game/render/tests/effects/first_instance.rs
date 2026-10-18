//! First-instance indirect draws (cull.rs): where the device has
//! INDIRECT_FIRST_INSTANCE, each culled draw starts at its slot region and the
//! scene group binds once per pass; elsewhere each draw's dynamic offset
//! selects its region. Both draw the same pixels.
use super::super::support::gpu;
use super::direct::{both, town};
use exact_game::{SceneSamples, ToneMapping};
use exact_gpu::{fixture, wgpu};

#[test]
fn the_town_draws_the_same_pixels_with_and_without_first_instance_draws() {
    let Some(first) = gpu() else {
        return;
    };
    let feature = wgpu::Features::INDIRECT_FIRST_INSTANCE;
    if !first.device.features().contains(feature) {
        eprintln!("this device has no INDIRECT_FIRST_INSTANCE: every draw binds its region");
        return;
    }
    let offset = fixture::device_without(feature).unwrap();
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let w = town(1., ToneMapping::None, true, false);
    for (samples, prepass) in [
        (SceneSamples::One, false),
        (SceneSamples::One, true),
        (SceneSamples::Four, false),
    ] {
        let starts = both(&first, &w, srgb, samples, prepass);
        let offsets = both(&offset, &w, srgb, samples, prepass);
        for (path, (a, b)) in ["hdr", "direct"].iter().zip(starts.iter().zip(&offsets)) {
            assert!(
                a.0.data == b.0.data,
                "{samples:?}, prepass {prepass}, {path}: first-instance draws changed pixels"
            );
        }
    }
}
