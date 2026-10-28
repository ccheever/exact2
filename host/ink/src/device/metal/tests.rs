use super::*;
use crate::instance::Encoder;

#[test]
fn readback_scissors_uploads_mips_and_retirement() {
    let mut d = Metal::new().unwrap();
    let target = d.create_target(12, 4).unwrap();
    let image = d
        .create_texture(TextureDesc {
            mipmapped: true,
            ..TextureDesc::rgba(8, 8)
        })
        .unwrap();
    let data: Vec<_> = (0..64)
        .flat_map(|i| {
            if (i % 8 + i / 8) % 2 == 0 {
                [128, 0, 0, 128]
            } else {
                [0, 0, 128, 128]
            }
        })
        .collect();
    d.upload_texture(image, [0, 0], [8, 8], &data).unwrap();
    d.generate_mips(image).unwrap();
    for level in 1..4 {
        for p in d.readback_level(image, level).unwrap().chunks_exact(4) {
            assert!(
                p.iter()
                    .zip([64u8, 0, 64, 128])
                    .all(|(a, b)| a.abs_diff(b) <= 1),
                "{p:?}"
            );
        }
    }
    let mut encoder = Encoder::new(12, 4);
    encoder
        .push(
            Instance::rounded([0., 0., 12., 4.], [0.; 4], [1., 0., 0., 1.]),
            Bindings::default(),
        )
        .unwrap();
    encoder
        .push_clip(crate::instance::Clip::Scissor(crate::instance::Scissor {
            x: 4,
            y: 0,
            width: 4,
            height: 4,
        }))
        .unwrap();
    encoder
        .push(
            Instance::new(Kind::RoundedRect, crate::Brush::Image, [0., 0., 12., 4.]),
            Bindings {
                image,
                ..Default::default()
            },
        )
        .unwrap();
    let frame = encoder.finish();
    let s = d.render(&frame, target, Some([0.; 4])).unwrap();
    d.retire_texture(image).unwrap();
    let pixels = d.readback(target).unwrap();
    assert_eq!(&pixels[..4], &[255, 0, 0, 255]);
    assert_ne!(&pixels[4 * 5..4 * 6], &[255, 0, 0, 255]);
    assert_eq!(&pixels[4 * 9..4 * 10], &[255, 0, 0, 255]);
    assert!(d.completed().unwrap() >= s);
    assert!(d.retired_textures.is_empty());
    assert!(d.render(&frame, target, None).is_err());
    let copy = d.create_target(3, 1).unwrap();
    d.copy_region(target, [3, 1], copy, [0, 0], [3, 1]).unwrap();
    assert_eq!(
        d.readback(copy).unwrap(),
        pixels[(12 + 3) * 4..(12 + 6) * 4]
    );
    assert!(d.upload_texture(copy, [2, 0], [2, 1], &[0; 8]).is_err());
    let resized = d.resize_target(target, 1, 1).unwrap();
    d.render(&Frame::default(), resized, Some([0.25, 0., 0., 0.5]))
        .unwrap();
    assert_eq!(d.readback(resized).unwrap(), [64, 0, 0, 128]);
    d.trim().unwrap();
    assert!(d.buffers.iter().all(|b| b.raw.length() == 192));
}

#[test]
fn odd_mips_include_every_source_pixel() {
    let mut d = Metal::new().unwrap();
    let t = d
        .create_texture(TextureDesc {
            mipmapped: true,
            ..TextureDesc::rgba(3, 1)
        })
        .unwrap();
    d.upload_texture(
        t,
        [0, 0],
        [3, 1],
        &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
    )
    .unwrap();
    d.generate_mips(t).unwrap();
    let p = d.readback_level(t, 1).unwrap();
    assert!(
        p.iter()
            .zip([85u8, 85, 85, 255])
            .all(|(a, b)| a.abs_diff(b) <= 1),
        "{p:?}"
    );
}
