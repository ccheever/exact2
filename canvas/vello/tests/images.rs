#![cfg(target_vendor = "apple")]
use exact_canvas::{images_in, Context2d};
use exact_canvas_vello::surface::make;
use exact_canvas_vello::*;
use std::ffi::c_void;

struct Input {
    pixels: Vec<Vec<u8>>,
    calls: usize,
}

unsafe extern "C" fn image(
    ctx: *mut c_void,
    src: *const u8,
    len: usize,
    width: *mut u32,
    height: *mut u32,
    identity: *mut u64,
) -> *const u8 {
    // SAFETY: the replay owns this host and each of its pixel buffers.
    let input = unsafe { &mut *ctx.cast::<Input>() };
    input.calls += 1;
    let source = unsafe { std::slice::from_raw_parts(src, len) };
    let index: usize = std::str::from_utf8(source).unwrap().parse().unwrap();
    unsafe {
        *identity = index as u64 + 1;
        *width = 512;
        *height = 512;
    }
    input.pixels[index].as_ptr()
}

#[test]
fn hundreds_of_images_replay_past_the_atlas_capacity_without_growing_at_rest() {
    let canvas = ecg_canvas_new(64, 64, 1.0);
    if canvas.is_null() {
        let why = "missing gpu: the image-cache replay test requires Metal";
        assert_ne!(std::env::var("EXACT_REQUIRED").as_deref(), Ok("1"), "{why}");
        eprintln!("SKIP {why}");
        return;
    }
    let surface = make::surface(64, 64);
    let pixels: Vec<_> = (0..400)
        .map(|i| {
            let mut bytes = vec![(i % 251) as u8; 512 * 512 * 4];
            for pixel in bytes.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            bytes
        })
        .collect();
    let mut input = Input { pixels, calls: 0 };
    let host = EcgHost {
        ctx: (&raw mut input).cast(),
        text_path: None,
        text_runs: None,
        image: Some(image),
    };
    let context = Context2d::new();
    let mut after_300 = 0;
    for i in 0..400 {
        images_in(&context.env().images).answer(&i.to_string(), Ok((512, 512)), &[]);
        context.clear_rect(0.0, 0.0, 64.0, 64.0);
        context
            .draw_image_with_image_handle_and_dw_and_dh(&i.to_string(), 0.0, 0.0, 64.0, 64.0)
            .unwrap();
        let lists = context.take_lists();
        let pointers: Vec<_> = lists.iter().map(|l| l.as_ptr()).collect();
        let lengths: Vec<_> = lists.iter().map(|l| l.len()).collect();
        // SAFETY: live canvas, surface, lists, callbacks and source pixels.
        let result = unsafe {
            ecg_canvas_replay(
                canvas,
                surface.0,
                std::ptr::null_mut(),
                pointers.as_ptr(),
                lengths.as_ptr(),
                lists.len(),
                &host,
            )
        };
        assert_eq!(result, 0, "image {i} failed to replay");
        if i == 299 {
            after_300 = ecg_memory();
        }
    }
    assert_eq!(input.calls, 400, "every source image reached the renderer");
    let after_400 = ecg_memory();
    eprintln!("D3 GPU images=400 bytes_at_300={after_300} bytes_at_400={after_400}");
    assert!(
        after_400 <= 320 * 1024 * 1024,
        "decoded pixels plus the 8192² atlas and fixed render buffers are bounded: {after_400}"
    );
    assert!(
        after_400 <= after_300 + 1024 * 1024,
        "another hundred images do not keep growing the module"
    );
    // SAFETY: no replay uses the canvas or surface after this point.
    unsafe { ecg_canvas_free(canvas) };
    make::release(surface);
    ecg_trim();
}
