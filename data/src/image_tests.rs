use super::*;

/// A deadline no test reaches.
fn later() -> Instant {
    Instant::now() + std::time::Duration::from_secs(60)
}

/// The size a recorded case's `model` gives a `w × h` trial at quality `q`
/// (`scripts/fixtures/picker/compress-trials.json`).
fn model(name: &str, w: u32, h: u32, q: u32) -> usize {
    let (w, h, q) = (u64::from(w), u64::from(h), u64::from(q));
    (match name {
        "q*1000" => q * 1000,
        "floor(w*h/4)+q*1000" => w * h / 4 + q * 1000,
        "w*h" => w * h,
        "floor(w*h*q/1000)" => w * h * q / 1000,
        "floor(w*h*q/250)" => w * h * q / 250,
        other => panic!("no model {other}"),
    }) as usize
}

#[test]
fn the_search_makes_blueskys_trials() {
    let recorded: serde_json::Value = serde_json::from_str(include_str!(
        "../../scripts/fixtures/picker/compress-trials.json"
    ))
    .unwrap();
    for case in recorded["cases"].as_array().unwrap() {
        let n = |k: &str| case[k].as_u64().unwrap();
        let name = case["model"].as_str().unwrap();
        let mut trials = Vec::new();
        let found = search(
            n("width") as u32,
            n("height") as u32,
            n("maxDimension") as u32,
            n("maxBytes"),
            |w, h, q| {
                trials.push(serde_json::json!([w, h, q]));
                Ok(vec![0; model(name, w, h, q)])
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::Value::Array(trials),
            case["trials"],
            "{}",
            case["name"]
        );
        let found = found.map(
            |c| serde_json::json!({"width": c.width, "height": c.height, "size": c.bytes.len()}),
        );
        assert_eq!(
            found.unwrap_or(serde_json::Value::Null),
            case["result"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn contain_floors_never_upscales_and_keeps_a_pixel() {
    assert_eq!(contain(6000, 4000, 4000), (4000, 2666));
    assert_eq!(contain(3024, 4032, 2000), (1500, 2000));
    assert_eq!(contain(800, 601, 4000), (800, 601));
    assert_eq!(contain(4000, 4000, 3200), (3200, 3200));
    // Bluesky would floor to 0; a side is at least one pixel.
    assert_eq!(contain(9000, 2, 2048), (2048, 1));
}

/// A1.2's declared deviation: a side is never 0, so a one-pixel limit or a
/// sliver of a source still makes trials (Bluesky's would ask for 0).
#[test]
fn the_smallest_sizes_keep_a_pixel_per_side() {
    let mut sizes = Vec::new();
    let found = search(64, 48, 1, 1, |w, h, _| {
        sizes.push((w, h));
        Ok(vec![0; 2])
    })
    .unwrap();
    assert!(found.is_none());
    assert!(sizes.iter().all(|&(w, h)| (w, h) == (1, 1)), "{sizes:?}");
    assert_eq!(sizes.len(), 8);
    let mut sizes = Vec::new();
    search(8000, 1, 4000, 10, |w, h, _| {
        sizes.push((w, h));
        Ok(vec![0; 1])
    })
    .unwrap()
    .unwrap();
    assert!(sizes.iter().all(|&s| s == (4000, 1)), "{sizes:?}");
}

#[test]
fn an_encoder_error_ends_the_search_with_it() {
    let mut calls = 0;
    let err = search(100, 100, 100, 10, |_, _, _| {
        calls += 1;
        Err(failure("timeout", "late"))
    })
    .unwrap_err();
    assert_eq!((calls, err.as_str()), (1, "compressImage: timeout: late"));
}

#[test]
fn limits_are_integers_in_range() {
    assert_eq!(check_limits(4000.0, 2e6), Ok((4000, 2_000_000)));
    for (d, b) in [
        (0.0, 1.0),
        (8193.0, 1.0),
        (1.5, 1.0),
        (1.0, 0.0),
        (1.0, 67108865.0),
        (f64::NAN, 1.0),
    ] {
        assert!(
            check_limits(d, b)
                .unwrap_err()
                .starts_with("compressImage: invalid: "),
            "{d} {b}"
        );
    }
}

#[cfg(not(target_vendor = "apple"))]
#[test]
fn without_a_codec_the_call_is_unsupported() {
    let fixture = include_bytes!("../../scripts/fixtures/picker/oriented-gps.jpg");
    assert_eq!(
        compress(fixture, 4000, 2_000_000, later()).unwrap_err(),
        "compressImage: unsupported: no JPEG encoder on this host"
    );
}

#[cfg(target_vendor = "apple")]
mod apple_codec {
    use super::super::apple::{first_pixel, inspect, png};
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../scripts/fixtures/picker/oriented-gps.jpg");

    #[test]
    fn the_fixture_is_turned_upright_and_loses_its_metadata() {
        let source = inspect(FIXTURE).unwrap();
        assert_eq!(
            (source.width, source.height, source.orientation),
            (64, 48, Some(6))
        );
        assert!(source.gps && source.exif_date);
        let out = compress(FIXTURE, 4000, 2_000_000, later()).unwrap();
        let read = inspect(&out.bytes).unwrap();
        assert!(read.jpeg, "{read:?}");
        assert_eq!((out.width, out.height), (48, 64));
        assert_eq!((read.width, read.height), (48, 64));
        assert!(matches!(read.orientation, None | Some(1)), "{read:?}");
        assert!(!read.gps && !read.exif_date && !read.tiff, "{read:?}");
        // Stored red over blue, turned a quarter clockwise: blue on the left.
        let [r, g, b] = first_pixel(&out.bytes);
        assert!(b > 200 && r < 60 && g < 60, "top-left {r},{g},{b}");
    }

    #[test]
    fn a_smaller_dimension_scales_the_upright_image() {
        let out = compress(FIXTURE, 32, 2_000_000, later()).unwrap();
        assert_eq!((out.width, out.height), (24, 32));
        let read = inspect(&out.bytes).unwrap();
        assert_eq!((read.width, read.height), (24, 32));
    }

    /// Noise, which JPEG cannot compress: deterministic bytes from an LCG.
    fn noise(w: u32, h: u32) -> Vec<u8> {
        let mut seed: u32 = 0x1234_5678;
        (0..w * h * 4)
            .map(|i| {
                if i % 4 == 3 {
                    return 255;
                }
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn noise_shrinks_until_it_fits() {
        let source = png(1000, 800, &noise(1000, 800));
        // 1000×800 noise is far over 120 kB at quality 26; a smaller size fits.
        let out = compress(&source, 1000, 120_000, later()).unwrap();
        assert!(out.bytes.len() <= 120_000);
        assert!(out.width < 1000 && out.width >= 512, "{}", out.width);
        assert_eq!(out.height, (f64::from(out.width) * 0.8).floor() as u32);
        let read = inspect(&out.bytes).unwrap();
        assert_eq!((read.width, read.height), (out.width, out.height));
    }

    #[test]
    fn nothing_fitting_is_unfit() {
        let source = png(200, 200, &noise(200, 200));
        let err = compress(&source, 200, 100, later()).unwrap_err();
        assert!(err.starts_with("compressImage: unfit: "), "{err}");
    }

    #[test]
    fn transparency_becomes_white() {
        let clear = vec![0u8; 8 * 8 * 4];
        let out = compress(&png(8, 8, &clear), 8, 100_000, later()).unwrap();
        let [r, g, b] = first_pixel(&out.bytes);
        assert!(r > 245 && g > 245 && b > 245, "{r},{g},{b}");
    }

    #[test]
    fn bytes_that_are_no_image_are_undecodable() {
        for bytes in [&b"not an image at all"[..], &[], &FIXTURE[..200]] {
            let err = compress(bytes, 100, 100_000, later()).unwrap_err();
            assert!(err.starts_with("compressImage: undecodable: "), "{err}");
        }
    }

    #[test]
    fn a_source_over_the_limits_is_too_large() {
        let err = compress(&vec![0; MAX_BYTES as usize + 1], 100, 100, later()).unwrap_err();
        assert!(err.starts_with("compressImage: too-large: "), "{err}");
        // The fixture's frame header rewritten to claim 9000 × 9000 (JPEG
        // has no checksum): refused from the header, before decoding.
        let mut header = FIXTURE.to_vec();
        let sof = header.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        header[sof + 5..sof + 7].copy_from_slice(&9000u16.to_be_bytes());
        header[sof + 7..sof + 9].copy_from_slice(&9000u16.to_be_bytes());
        let err = compress(&header, 100, 100, later()).unwrap_err();
        assert!(err.starts_with("compressImage: too-large: "), "{err}");
    }

    #[test]
    fn a_passed_deadline_starts_nothing() {
        let err = compress(FIXTURE, 4000, 2_000_000, Instant::now()).unwrap_err();
        assert_eq!(err, "compressImage: timeout: the search ran past its time");
    }

    #[test]
    fn a_one_pixel_limit_makes_a_one_pixel_jpeg() {
        let out = compress(FIXTURE, 1, 2_000_000, later()).unwrap();
        assert_eq!((out.width, out.height), (1, 1));
        let read = inspect(&out.bytes).unwrap();
        assert_eq!((read.width, read.height), (1, 1));
    }

    #[test]
    fn the_photo_fixtures_decode() {
        for bytes in [
            &include_bytes!("../../scripts/fixtures/picker/photo.heic")[..],
            &include_bytes!("../../scripts/fixtures/picker/photo.png")[..],
        ] {
            let out = compress(bytes, 100, 2_000_000, later()).unwrap();
            assert!(out.width.max(out.height) <= 100);
            assert!(inspect(&out.bytes).unwrap().jpeg);
        }
    }
}
