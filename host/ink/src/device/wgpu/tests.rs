use super::*;
use crate::instance::{Clip, CoverageClip, Encoder, RoundedClip};

fn open() -> Option<Wgpu> {
    match Wgpu::new() {
        Ok(device) => Some(device),
        Err(Error::NoAdapter(e)) => {
            eprintln!("no GPU here ({e}); ink's pixels are not checked");
            None
        }
        Err(e) => panic!("ink device: {e}"),
    }
}

fn pixel(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    bytes[(y * width + x) * 4..][..4].try_into().unwrap()
}

fn near(actual: [u8; 4], expected: [u8; 4], tolerance: u8) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= tolerance),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn rounded_rectangles_ring_and_gaussian_shadow_at_two_times_scale() {
    let Some(mut device) = open() else {
        return;
    };
    let target = device.create_target(200, 200).unwrap();
    let mut encoder = Encoder::new(200, 200);
    let red = Instance::rounded([10.25, 10., 45., 40.], [8., 4., 12., 0.], [1., 0., 0., 1.]);
    let blue = Instance::rounded([35., 30., 40., 30.], [6.; 4], [0., 0., 0.5, 0.5]);
    let mut ring = Instance::new(Kind::Ring, Brush::Solid, [10., 65., 30., 20.]);
    ring.radii = [6.; 4];
    ring.uv = [13., 68., 24., 14.];
    ring.param = [3.; 4];
    ring.color = [0., 1., 0., 1.];
    let mut shadow = Instance::new(Kind::Shadow, Brush::Solid, [56., 56., 38., 33.]);
    shadow.uv = [65., 65., 20., 15.];
    shadow.radii = [4.; 4];
    shadow.param = [3., 0., 0., 0.];
    shadow.color = [0., 0., 0., 1.];
    shadow
        .set_clip(
            1,
            RoundedClip {
                rect: [130., 130., 40., 30.],
                radii: [8.; 4],
            },
        )
        .unwrap();
    for mut instance in [red, blue, ring, shadow] {
        instance.xform = [2., 0., 0., 2., 0., 0.];
        encoder.push(instance, Bindings::default()).unwrap();
    }
    let frame = encoder.finish();
    assert_eq!(frame.batches.len(), 1);
    let first = Instant::now();
    device.render(&frame, target, Some([0.; 4])).unwrap();
    let bytes = device.readback(target).unwrap();
    let first_ms = first.elapsed().as_secs_f64() * 1000.;
    assert_eq!(bytes.len(), 200 * 200 * 4);
    near(pixel(&bytes, 200, 40, 40), [255, 0, 0, 255], 0);
    near(pixel(&bytes, 200, 90, 80), [128, 0, 128, 255], 1);
    near(pixel(&bytes, 200, 140, 95), [0, 0, 128, 128], 1);
    near(pixel(&bytes, 200, 5, 5), [0; 4], 0);
    near(pixel(&bytes, 200, 21, 21), [0; 4], 0);
    near(pixel(&bytes, 200, 20, 55), [128, 0, 0, 128], 2);
    near(pixel(&bytes, 200, 23, 150), [0, 255, 0, 255], 0);
    near(pixel(&bytes, 200, 50, 150), [0; 4], 0);
    near(pixel(&bytes, 200, 150, 145), [0; 4], 0);
    let shadow_alpha = pixel(&bytes, 200, 126, 145)[3];
    assert!(
        (40..90).contains(&shadow_alpha),
        "Gaussian falloff: {shadow_alpha}"
    );
    assert_eq!(pixel(&bytes, 200, 109, 145), [0; 4]);
    assert!(bytes
        .chunks_exact(4)
        .all(|p| p[..3].iter().all(|c| *c <= p[3])));

    let mut times = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        device.render(&frame, target, Some([0.; 4])).unwrap();
        let encoded = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(device.readback(target).unwrap(), bytes);
        times.push((start.elapsed().as_secs_f64() * 1000., encoded));
    }
    times.sort_by(|a, b| a.0.total_cmp(&b.0));
    let info = device.info();
    eprintln!("ink {} ({}): device {:.3} ms, shaders {:.3} ms, cached={}; first frame {:.3} ms; warm frame including readback p50 {:.3} ms, range {:.3}–{:.3} ms, encode+submit at p50 {:.3} ms (7 frames, 200x200, 2x)",
        info.adapter, info.api, info.device_ms, info.shaders_ms, info.cached, first_ms,
        times[3].0, times[0].0, times[6].0, times[3].1);
}

#[test]
fn textures_brushes_clips_mips_and_retirement_reach_the_pixels() {
    let Some(mut device) = open() else {
        return;
    };
    let alpha = device
        .create_texture(TextureDesc {
            width: 3,
            height: 1,
            format: Format::Alpha,
            mipmapped: false,
        })
        .unwrap();
    device
        .upload_texture(alpha, [0, 0], [3, 1], &[0, 128, 255])
        .unwrap();
    assert_eq!(device.readback(alpha).unwrap(), [0, 128, 255]);
    let image = device
        .create_texture(TextureDesc {
            mipmapped: true,
            ..TextureDesc::rgba(8, 8)
        })
        .unwrap();
    let checker: Vec<u8> = (0..64)
        .flat_map(|i| {
            if (i % 8 + i / 8) % 2 == 0 {
                [128, 0, 0, 128]
            } else {
                [0, 0, 128, 128]
            }
        })
        .collect();
    device
        .upload_texture(image, [0, 0], [8, 8], &checker)
        .unwrap();
    device.generate_mips(image).unwrap();
    for level in 1..4 {
        let bytes = block_on(device.readback_async(image, level)).unwrap();
        for p in bytes.chunks_exact(4) {
            near(p.try_into().unwrap(), [64, 0, 64, 128], 1);
        }
    }
    let ramp = device.create_texture(TextureDesc::rgba(2, 1)).unwrap();
    device
        .upload_texture(ramp, [0, 0], [2, 1], &[255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap();
    let white = device
        .create_texture(TextureDesc {
            width: 1,
            height: 1,
            format: Format::Alpha,
            mipmapped: false,
        })
        .unwrap();
    device
        .upload_texture(white, [0, 0], [1, 1], &[128])
        .unwrap();
    let color = device.create_texture(TextureDesc::rgba(1, 1)).unwrap();
    device
        .upload_texture(color, [0, 0], [1, 1], &[0, 128, 0, 128])
        .unwrap();
    let bindings = Bindings {
        glyph: alpha,
        color,
        path: white,
        ramp,
        image,
        ..Default::default()
    };
    let target = device.create_target(64, 32).unwrap();
    let mut e = Encoder::new(64, 32);
    let mut glyph = Instance::new(Kind::Glyph, Brush::Solid, [0., 0., 3., 1.]);
    glyph.color = [1., 0., 0., 1.];
    e.push(glyph, bindings).unwrap();
    glyph.rect = [4., 0., 1., 1.];
    glyph.flags = flags::COLOR_GLYPH;
    e.push(glyph, bindings).unwrap();
    let mut gradient = Instance::new(Kind::RoundedRect, Brush::Gradient, [8., 0., 16., 16.]);
    gradient.param = [8., 0., 24., 0.];
    e.push(gradient, bindings).unwrap();
    let mut coverage = Instance::new(Kind::Coverage, Brush::Gradient, [24., 0., 16., 16.]);
    coverage.param = [24., 0., 40., 0.];
    e.push(coverage, bindings).unwrap();
    let mut layer = Instance::new(Kind::Layer, Brush::Solid, [40., 0., 2., 2.]);
    layer.param[0] = 0.5;
    e.push(layer, bindings).unwrap();
    let mut tinted = Instance::new(Kind::RoundedRect, Brush::Image, [44., 0., 2., 2.]);
    tinted.flags = flags::TINT;
    tinted.color = [0., 0.5, 0., 0.5];
    e.push(tinted, bindings).unwrap();
    let mut radial = Instance::new(Kind::RoundedRect, Brush::Gradient, [24., 16., 8., 16.]);
    radial.flags = flags::RADIAL;
    radial.param = [28., 24., 4., 8.];
    e.push(radial, bindings).unwrap();
    let mut conic = Instance::new(Kind::RoundedRect, Brush::Gradient, [32., 16., 16., 16.]);
    conic.flags = flags::CONIC;
    conic.param = [40.5, 24.5, 0., 0.];
    e.push(conic, bindings).unwrap();
    let mut pattern = Instance::new(Kind::RoundedRect, Brush::Pattern, [48., 16., 16., 16.]);
    pattern.flags = flags::NEAREST;
    pattern.param = [48., 16., 8., 8.];
    e.push(pattern, bindings).unwrap();
    e.push_clip(Clip::Rounded(RoundedClip {
        rect: [0., 16., 16., 16.],
        radii: [4.; 4],
    }))
    .unwrap();
    e.push_clip(Clip::Rounded(RoundedClip {
        rect: [4., 16., 16., 16.],
        radii: [4.; 4],
    }))
    .unwrap();
    e.push_clip(Clip::Coverage(CoverageClip {
        texture: white,
        rect: [4., 20., 16., 12.],
        uv: [0., 0., 1., 1.],
    }))
    .unwrap();
    e.push(
        Instance::rounded([0., 16., 24., 16.], [0.; 4], [1.; 4]),
        bindings,
    )
    .unwrap();
    let frame = e.finish();
    let submitted = device.render(&frame, target, Some([0.; 4])).unwrap();
    // The encoded command retains the sampled resources after logical eviction.
    device.retire_texture(image).unwrap();
    let pixels = device.readback(target).unwrap();
    near(pixel(&pixels, 64, 0, 0), [0; 4], 0);
    near(pixel(&pixels, 64, 1, 0), [128, 0, 0, 128], 1);
    near(pixel(&pixels, 64, 2, 0), [255, 0, 0, 255], 0);
    near(pixel(&pixels, 64, 4, 0), [0, 128, 0, 128], 0);
    near(pixel(&pixels, 64, 8, 8), [247, 0, 8, 255], 1);
    near(pixel(&pixels, 64, 24, 8), [124, 0, 4, 128], 1);
    near(pixel(&pixels, 64, 40, 0), [32, 0, 32, 64], 1);
    near(pixel(&pixels, 64, 44, 0), [0, 64, 0, 64], 1);
    near(pixel(&pixels, 64, 28, 24), [219, 0, 36, 255], 1);
    near(pixel(&pixels, 64, 40, 18), [255, 0, 0, 255], 1);
    near(pixel(&pixels, 64, 46, 24), [191, 0, 64, 255], 1);
    near(pixel(&pixels, 64, 48, 16), [128, 0, 0, 128], 1);
    near(pixel(&pixels, 64, 49, 16), [0, 0, 128, 128], 1);
    assert_eq!(pixel(&pixels, 64, 49, 16), pixel(&pixels, 64, 57, 16));
    near(pixel(&pixels, 64, 10, 24), [128; 4], 1);
    near(pixel(&pixels, 64, 2, 24), [0; 4], 0);
    near(pixel(&pixels, 64, 10, 18), [0; 4], 0);
    assert!(device.completed().unwrap() >= submitted);
    assert!(device.retired_textures.is_empty());
    let copy = device.create_target(3, 1).unwrap();
    device
        .copy_region(target, [0, 0], copy, [0, 0], [3, 1])
        .unwrap();
    assert_eq!(device.readback(copy).unwrap(), pixels[..12]);
    assert!(
        device.render(&frame, target, None).is_err(),
        "retired bindings must refuse"
    );
    let resized = device.resize_target(target, 13, 7).unwrap();
    device
        .render(&Frame::default(), resized, Some([0.25, 0., 0., 0.5]))
        .unwrap();
    assert_eq!(device.readback(resized).unwrap().len(), 13 * 7 * 4);
    device.trim().unwrap();
    assert!(device.buffers.iter().all(|s| s.capacity == 192));
}
