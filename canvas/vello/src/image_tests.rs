use super::*;

unsafe extern "C" fn image(
    ctx: *mut c_void,
    src: *const u8,
    len: usize,
    width: *mut u32,
    height: *mut u32,
    identity: *mut u64,
) -> *const u8 {
    // SAFETY: this test supplies live buffers and writable dimensions.
    let images = unsafe { &*ctx.cast::<Vec<Vec<u8>>>() };
    let source = unsafe { std::slice::from_raw_parts(src, len) };
    let replacement = source == b"replacement";
    let index: usize = if replacement {
        399
    } else {
        std::str::from_utf8(source).unwrap().parse().unwrap()
    };
    unsafe {
        *identity = if replacement { 1000 } else { index as u64 + 1 };
        *width = 256;
        *height = 256;
    }
    images[index].as_ptr()
}

#[test]
fn hundreds_of_decoded_images_have_a_cost_bound_and_live_scenes_keep_their_pixels() {
    ecg_trim();
    let mut pixels: Vec<_> = (0..400)
        .map(|i| vec![(i % 251) as u8; 256 * 256 * 4])
        .collect();
    let callbacks = EcgHost {
        ctx: (&raw mut pixels).cast(),
        text_path: None,
        text_runs: None,
        image: Some(image),
    };
    let mut host = AbiHost {
        host: Some(&callbacks),
    };
    let held = host.image("0").unwrap();
    for i in 1..400 {
        let decoded = host.image(&i.to_string()).unwrap();
        assert_eq!(decoded.data.data()[0], (i % 251) as u8);
    }
    let bytes = ecg_memory();
    eprintln!("D3 decoded images=400 bytes={bytes}");
    assert!(
        bytes <= 32 * 1024 * 1024,
        "decoded cache holds {bytes} bytes"
    );
    assert_eq!(
        held.data.data()[0],
        0,
        "eviction leaves a live scene's image valid"
    );
    let latest = host.image("399").unwrap();
    assert_eq!(latest.data.id(), host.image("399").unwrap().data.id());
    assert_eq!(
        host.image("0").unwrap().data.data()[0],
        0,
        "an evicted image can be decoded again"
    );
    ecg_trim();
    assert_eq!(ecg_memory(), 0);
    assert_eq!(
        held.data.data()[0],
        0,
        "trimming leaves a live scene valid too"
    );
    // Reuse the same allocation for different pixels and a new host identity.
    let previous = host.image("399").unwrap();
    pixels[399].fill(123);
    let replacement = host.image("replacement").unwrap();
    assert_eq!(replacement.data.data()[0], 123);
    assert_ne!(replacement.data.id(), previous.data.id());
    assert_ne!(
        previous.data.data()[0],
        123,
        "live scenes keep the old pixels"
    );
}
