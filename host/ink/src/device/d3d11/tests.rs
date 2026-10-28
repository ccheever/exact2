use super::*;
use crate::instance::Encoder;

#[test]
fn readback_scissors_uploads_mips_and_retirement() {
    let mut d = D3d11::new().unwrap();
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
    assert!(d.retired.is_empty());
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
    assert_eq!(d.buffer.size, 192);
}

#[test]
fn odd_mips_include_every_source_pixel() {
    let mut d = D3d11::new().unwrap();
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

#[test]
#[ignore = "native Windows composition and RichEdit; requires fxc and a desktop session"]
fn composition_and_windowless_text() {
    use windows::{
        core::w,
        Win32::{Foundation::HWND, System::Com::*, UI::WindowsAndMessaging::*},
    };
    struct Owner(HWND);
    impl Drop for Owner {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            w!("STATIC"),
            w!("Ink C6"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            80,
            80,
            400,
            240,
            None,
            None,
            None,
            None,
        )
        .unwrap()
    };
    let owner = Rc::new(Owner(hwnd));
    let window = unsafe { Window::new(hwnd.0 as isize, owner) };
    let mut device = D3d11::new().unwrap();
    device.attach_window(window.clone(), [384, 200]).unwrap();
    let editor = rich_edit::RichEdit::new(&mut device, window, [320, 48]).unwrap();
    editor.type_text("Shelf on Direct3D 11").unwrap();
    assert_eq!(editor.text().unwrap(), "Shelf on Direct3D 11");
    let image = editor.draw(&mut device).unwrap();
    let pixels = device.readback(image).unwrap();
    assert!(
        pixels.chunks_exact(4).any(|p| p[3] > 128),
        "RichEdit drew no ink"
    );
    assert!(
        pixels.chunks_exact(4).any(|p| p[3] == 0),
        "RichEdit ground is not transparent"
    );
    let target = device.create_target(384, 200).unwrap();
    let mut encoder = Encoder::new(384, 200);
    let mut quad = Instance::new(
        Kind::RoundedRect,
        crate::Brush::Image,
        [16., 24., 320., 48.],
    );
    quad.uv = [0., 0., 1., 1.];
    encoder
        .push(
            quad,
            Bindings {
                image,
                ..Default::default()
            },
        )
        .unwrap();
    device
        .render(&encoder.finish(), target, Some([0.7, 0.7, 0.7, 0.8]))
        .unwrap();
    device.present(target).unwrap();
    let pixels = device.readback(target).unwrap();
    tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(384, 200).unwrap())
        .unwrap()
        .save_png(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/ink-c6-richedit.png"),
        )
        .unwrap();
    drop(editor);
    drop(device);
    unsafe {
        CoUninitialize();
    }
}
