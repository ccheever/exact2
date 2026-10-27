use super::png_decode::{decode_rows, inspect, DecodePlan};
use std::io::Cursor;

#[cfg(windows)]
#[test]
fn windows_applies_jpeg_exif_orientation_to_dimensions_pixels_and_crops() {
    let source = include_bytes!("../../../../apps/shared-elements/assets/alpine-water.jpg");
    let header = inspect(&mut Cursor::new(source), source.len() as u64).unwrap();
    let natural = header.metadata.natural;
    let base_plan = DecodePlan::new(header, (natural.width, natural.height)).unwrap();
    let base = decode_rows(Cursor::new(source), &base_plan, || false).unwrap();
    for orientation in [6u8, 1, 2, 3, 4, 5, 7, 8] {
        // One little-endian SHORT in IFD0, inserted before the JPEG's existing segments.
        let mut bytes =
            b"\xff\xd8\xff\xe1\0\x22Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0"
                .to_vec();
        bytes.extend([orientation, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend_from_slice(&source[2..]);
        let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let (w, h) = if orientation >= 5 {
            (natural.height, natural.width)
        } else {
            (natural.width, natural.height)
        };
        assert_eq!(
            (
                header.metadata.natural.width,
                header.metadata.natural.height
            ),
            (w, h),
            "orientation {orientation}"
        );
        let plan = DecodePlan::new(header, (w, h)).unwrap();
        let image = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        for (x, y) in [
            (0, 0),
            (w - 1, 0),
            (0, h - 1),
            (w - 1, h - 1),
            (w / 3, h / 4),
        ] {
            let (sx, sy) = match orientation {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, w - 1 - x),
                7 => (h - 1 - y, w - 1 - x),
                8 => (h - 1 - y, x),
                _ => (x, y),
            };
            assert_eq!(
                image.pixel(x, y),
                base.pixel(sx, sy),
                "orientation {orientation} at {x},{y}"
            );
        }
        let crop = plan.part(w / 3, h / 4, (5, 7)).unwrap();
        let cropped = decode_rows(Cursor::new(&bytes), &crop, || false).unwrap();
        for y in 0..7 {
            for x in 0..5 {
                assert_eq!(
                    cropped.pixel(x, y),
                    image.pixel(w / 3 + x, h / 4 + y),
                    "orientation {orientation}"
                );
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_decodes_photos_and_animation_first_frames_at_the_requested_size() {
    for path in [
        "apps/shared-elements/assets/alpine-water.jpg",
        "apps/motion-gallery/assets/spin.gif",
        "apps/motion-gallery/assets/pulse.webp",
    ] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bytes = std::fs::read(root.join(path)).unwrap();
        let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let size = (
            header.metadata.natural.width.min(64),
            header.metadata.natural.height.min(64),
        );
        let plan = DecodePlan::new(header, size).unwrap();
        let decoded = decode_rows(Cursor::new(&bytes), &plan, || false)
            .unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!((decoded.width(), decoded.height()), size, "{path}");
        assert!(decoded.data().chunks_exact(4).any(|p| p[3] > 0), "{path}");
        let part = plan.part(2, 3, (size.0 - 4, size.1 - 6)).unwrap();
        let cropped = decode_rows(Cursor::new(&bytes), &part, || false).unwrap();
        for row in 0..cropped.height() {
            let from = (((row + 3) * size.0 + 2) * 4) as usize;
            let len = cropped.width() as usize * 4;
            assert_eq!(
                &cropped.data()[row as usize * len..(row as usize + 1) * len],
                &decoded.data()[from..from + len],
                "{path} row {row}"
            );
        }
        assert!(matches!(
            decode_rows(Cursor::new(&bytes), &plan, || true),
            Err(exact_raster::Refusal::Stale)
        ));
    }
}

#[cfg(windows)]
#[test]
fn windows_scaling_ignores_hidden_colour_and_keeps_valid_premultiplied_pixels() {
    let (w, h) = (16, 8);
    let raw: Vec<u8> = (0..h)
        .flat_map(|_| {
            (0..w).flat_map(|x| {
                if x < w / 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 0]
                }
            })
        })
        .collect();
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&raw)
        .unwrap();
    let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    // Exercise WIC's converter/scaler chain independently of the PNG row decoder.
    let plan = DecodePlan::new(header, (3, 2)).unwrap();
    let pixels = super::jpeg_decode::decode(Cursor::new(&bytes), &plan, || false).unwrap();
    assert!(pixels
        .data()
        .chunks_exact(4)
        .any(|p| p[3] > 0 && p[3] < 255));
    for p in pixels.data().chunks_exact(4) {
        assert_eq!(p, [p[3], 0, 0, p[3]]);
    }
}

#[cfg(windows)]
#[test]
fn windows_half_scale_averages_opaque_red_and_hidden_green() {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 0])
        .unwrap();
    let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    let plan = DecodePlan::new(header, (1, 1)).unwrap();
    let pixels = super::jpeg_decode::decode(Cursor::new(&bytes), &plan, || false).unwrap();
    for (actual, expected) in pixels.data().iter().zip([128, 0, 0, 128]) {
        assert!(actual.abs_diff(expected) <= 1, "{:?}", pixels.data());
    }
}

fn pixel(x: u32, y: u32) -> [u8; 4] {
    [(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 128]
}

pub(super) fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().unwrap();
    let mut stream = writer.stream_writer().unwrap();
    use std::io::Write;
    for y in 0..height {
        let row: Vec<_> = (0..width).flat_map(|x| pixel(x, y)).collect();
        stream.write_all(&row).unwrap();
    }
    stream.finish().unwrap();
    writer.finish().unwrap();
    bytes
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = !0u32;
    for b in kind.iter().chain(data) {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

fn header(width: u32, height: u32, interlaced: bool) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut data = Vec::from(width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[8, 6, 0, 0, u8::from(interlaced)]);
    chunk(&mut out, b"IHDR", &data);
    out
}

// A small genuine Adam7 PNG, using a stored deflate block; no image-library
// full-frame deinterlacing is involved in constructing the expected samples.
pub(super) fn adam7(width: u32, height: u32) -> Vec<u8> {
    adam7_pixels(width, height, pixel)
}

fn adam7_pixels(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut raw = Vec::new();
    for (x, y, dx, dy) in [
        (0, 0, 8, 8),
        (4, 0, 8, 8),
        (0, 4, 4, 8),
        (2, 0, 4, 4),
        (0, 2, 2, 4),
        (1, 0, 2, 2),
        (0, 1, 1, 2),
    ] {
        if x >= width {
            continue;
        }
        for y in (y..height).step_by(dy) {
            raw.push(0);
            for x in (x..width).step_by(dx) {
                raw.extend_from_slice(&pixel(x, y));
            }
        }
    }
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(u16::MAX as usize).peekable();
    while let Some(block) = blocks.next() {
        let n = block.len() as u16;
        zlib.push(u8::from(blocks.peek().is_none()));
        zlib.extend_from_slice(&n.to_le_bytes());
        zlib.extend_from_slice(&(!n).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = header(width, height, true);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn premultiplied([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    let channel = |c| (u16::from(c) * u16::from(a) / 255) as u8;
    [channel(r), channel(g), channel(b), a]
}

#[test]
fn large_source_uses_bounded_rows_and_preserves_natural_dimensions() {
    let bytes = png(4000, 2000);
    let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    let plan = DecodePlan::new(metadata, (100, 50)).unwrap();
    assert_eq!(plan.natural(), (4000, 2000));
    assert_eq!(plan.output_bytes(), 20_000);
    assert!(
        plan.peak_bytes() < 2 * 1024 * 1024,
        "no source-sized frame reservation"
    );
    let bitmap = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
    assert_eq!((bitmap.width(), bitmap.height()), (100, 50));
    let oracle = area_reference(4000, 2000, 100, 50, |x, y| premultiplied(pixel(x, y)));
    assert_eq!(bitmap.data(), oracle);
}

#[test]
fn adam7_rows_average_every_pass_without_full_frame_fallback() {
    for (w, h, dw, dh) in [
        (19, 11, 7, 5),
        (8, 8, 8, 8),
        (1, 9, 1, 4),
        (9, 1, 4, 1),
        (1, 1, 1, 1),
    ] {
        let bytes = adam7(w, h);
        let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let plan = DecodePlan::new(metadata, (dw, dh)).unwrap();
        let bitmap = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        assert_eq!(
            bitmap.data(),
            area_reference(w, h, dw, dh, |x, y| premultiplied(pixel(x, y)))
        );
        for (x, y, cw, ch) in [(0, 0, dw, 1), (dw - 1, 0, 1, dh), (dw - 1, dh - 1, 1, 1)] {
            let part = plan.part(x, y, (cw, ch)).unwrap();
            let cut = decode_rows(Cursor::new(&bytes), &part, || false).unwrap();
            for iy in 0..ch {
                for ix in 0..cw {
                    assert_eq!(cut.pixel(ix, iy), bitmap.pixel(x + ix, y + iy));
                }
            }
        }
    }
}

#[test]
fn limits_and_bad_geometry_refuse_before_decoder_allocation() {
    for (w, h) in [(0, 5), (u32::MAX, u32::MAX), (8193, 8192)] {
        let bytes = header(w, h, false);
        assert!(inspect(&mut Cursor::new(&bytes), bytes.len() as u64).is_err());
    }
    let bytes = png(8, 8);
    assert!(inspect(&mut Cursor::new(&bytes), 64 * 1024 * 1024 + 1).is_err());
    let mut over_header = header(8, 8, false);
    chunk(&mut over_header, b"tEXt", &vec![b'a'; 256 * 1024]);
    assert!(inspect(&mut Cursor::new(&over_header), over_header.len() as u64).is_err());
    let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    assert!(DecodePlan::new(metadata, (u32::MAX, 2)).is_err());
    let plan = DecodePlan::new(metadata, (4, 4)).unwrap();
    assert!(decode_rows(Cursor::new(&bytes), &plan, || true).is_err());
}

#[test]
fn cancellation_is_observed_between_rows_and_corruption_never_publishes() {
    let bytes = png(80, 40);
    let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    let plan = DecodePlan::new(metadata, (10, 5)).unwrap();
    let calls = std::cell::Cell::new(0);
    assert!(decode_rows(Cursor::new(&bytes), &plan, || {
        calls.set(calls.get() + 1);
        calls.get() > 4
    })
    .is_err());
    let mut corrupt = bytes;
    corrupt.truncate(corrupt.len() / 2);
    assert!(decode_rows(Cursor::new(&corrupt), &plan, || false).is_err());
}

#[test]
fn normalized_color_formats_match_the_full_frame_oracle() {
    for (color, depth) in [
        (png::ColorType::Rgba, png::BitDepth::Eight),
        (png::ColorType::Rgb, png::BitDepth::Eight),
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight),
        (png::ColorType::Grayscale, png::BitDepth::Eight),
        (png::ColorType::Grayscale, png::BitDepth::One),
        (png::ColorType::Indexed, png::BitDepth::One),
        (png::ColorType::Rgba, png::BitDepth::Sixteen),
    ] {
        let (w, h) = (17u32, 13u32);
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, w, h);
        encoder.set_color(color);
        encoder.set_depth(depth);
        if color == png::ColorType::Indexed {
            encoder.set_palette(vec![12, 180, 70, 211, 30, 230]);
            encoder.set_trns(vec![128, 255]);
        }
        let channels = color.samples();
        let row = (w as usize * channels * depth as usize).div_ceil(8);
        let raw: Vec<u8> = (0..row * h as usize).map(|n| (n % 251) as u8).collect();
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&raw)
            .unwrap();
        let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let (full, _, _) = decode_reference(&bytes).unwrap();
        for (dw, dh) in [(7, 5), (w, 5), (7, h), (w, h)] {
            let plan = DecodePlan::new(metadata, (dw, dh)).unwrap();
            let actual = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
            let oracle = area_reference(w, h, dw, dh, |x, y| {
                let at = ((y * w + x) * 4) as usize;
                full[at..at + 4].try_into().unwrap()
            });
            assert_eq!(actual.data(), oracle, "{color:?} {depth:?} {dw}x{dh}");
        }
    }
}

fn area_reference(
    width: u32,
    height: u32,
    out_w: u32,
    out_h: u32,
    pixel: impl Fn(u32, u32) -> [u8; 4],
) -> Vec<u8> {
    let (w, h, dw, dh) = (
        u64::from(width),
        u64::from(height),
        u64::from(out_w),
        u64::from(out_h),
    );
    let mut out = Vec::new();
    for y in 0..dh {
        let (top, bottom) = (y * h, (y + 1) * h);
        for x in 0..dw {
            let (left, right) = (x * w, (x + 1) * w);
            let mut sum = [0; 4];
            for iy in top / dh..bottom.div_ceil(dh) {
                for ix in left / dw..right.div_ceil(dw) {
                    let weight = (right.min((ix + 1) * dw) - left.max(ix * dw))
                        * (bottom.min((iy + 1) * dh) - top.max(iy * dh));
                    let p = pixel(ix as u32, iy as u32);
                    for c in 0..4 {
                        sum[c] += u64::from(p[c]) * weight;
                    }
                }
            }
            out.extend(sum.map(|v| ((v + w * h / 2) / (w * h)) as u8));
        }
    }
    out
}

#[test]
fn shrinking_averages_premultiplied_area_and_crops_the_same_pixels() {
    let (w, h) = (30, 30);
    let pixel = |x: u32, y: u32| match (x + y * 3) % 5 {
        0 => [255, 20, 10, 255],
        1 => [10, 255, 250, 0],
        2 => [230, 40, 170, 83],
        3 => [20, 100, 255, 171],
        _ => [0, 0, 0, 255],
    };
    let raw: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| pixel(x, y)))
        .collect();
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&raw)
        .unwrap();
    for (interlaced, bytes) in [(false, bytes), (true, adam7_pixels(w, h, pixel))] {
        let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let natural = DecodePlan::new(header, (w, h)).unwrap();
        for (dw, dh) in [
            (15, 15),
            (12, 12),
            (10, 10),
            (30, 12),
            (12, 30),
            (30, 30),
            (7, 11),
            (29, 29),
            (1, 1),
        ] {
            let plan = DecodePlan::new(header, (dw, dh)).unwrap();
            let actual = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
            let oracle = area_reference(w, h, dw, dh, |x, y| premultiplied(pixel(x, y)));
            assert_eq!(actual.data(), oracle, "{dw}x{dh} interlaced={interlaced}");
            assert!(actual
                .data()
                .chunks_exact(4)
                .all(|p| p[..3].iter().all(|c| *c <= p[3])));
            if (dw, dh) == (w, h) {
                let exact: Vec<u8> = raw
                    .chunks_exact(4)
                    .flat_map(|p| {
                        [
                            (u16::from(p[0]) * u16::from(p[3]) / 255) as u8,
                            (u16::from(p[1]) * u16::from(p[3]) / 255) as u8,
                            (u16::from(p[2]) * u16::from(p[3]) / 255) as u8,
                            p[3],
                        ]
                    })
                    .collect();
                assert_eq!(actual.data(), exact);
            } else {
                assert_eq!(
                    plan.cost.scratch_bytes - natural.cost.scratch_bytes,
                    u64::from(dw) * if interlaced { u64::from(dh) * 32 } else { 64 }
                );
            }
            for (x, y, cw, ch) in [
                (dw / 3, dh / 3, dw - dw / 3, dh - dh / 3),
                (dw - 1, dh - 1, 1, 1),
            ] {
                let part = plan.part(x, y, (cw, ch)).unwrap();
                let cut = decode_rows(Cursor::new(&bytes), &part, || false).unwrap();
                for iy in 0..ch {
                    for ix in 0..cw {
                        assert_eq!(cut.pixel(ix, iy), actual.pixel(x + ix, y + iy));
                    }
                }
            }
        }
    }
}

#[test]
fn fractional_area_decodes_and_crops_have_known_bytes() {
    let raw = [
        240, 0, 0, 255, 0, 255, 0, 0, 0, 0, 240, 255, 120, 60, 30, 255, 240, 120, 60, 85, 60, 120,
        240, 170,
    ];
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 3, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&raw)
        .unwrap();
    let interlaced = adam7_pixels(3, 2, |x, y| {
        let at = (y * 3 + x) as usize * 4;
        raw[at..at + 4].try_into().unwrap()
    });
    for bytes in [bytes, interlaced] {
        let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let plan = DecodePlan::new(header, (2, 1)).unwrap();
        let pixels = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        assert_eq!(pixels.data(), [133, 27, 13, 184, 27, 33, 137, 156]);
        let part = plan.part(1, 0, (1, 1)).unwrap();
        let pixels = decode_rows(Cursor::new(&bytes), &part, || false).unwrap();
        assert_eq!(pixels.data(), [27, 33, 137, 156]);
        let plan = DecodePlan::new(header, (1, 1)).unwrap();
        let pixels = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        assert_eq!(pixels.data(), [80, 30, 75, 170]);
    }
}

#[test]
fn scaled_adam7_storage_is_bounded_without_reducing_decode_size() {
    let bytes = adam7(514, 513);
    let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    let natural = DecodePlan::new(metadata, (514, 513)).unwrap();
    for (width, sums) in [(512, exact_raster::SESSION_BYTES / 4), (513, 0)] {
        let plan = DecodePlan::new(metadata, (width, 512)).unwrap();
        assert_eq!(plan.cost.scratch_bytes - natural.cost.scratch_bytes, sums);
        let pixels = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        if width == 512 {
            assert_eq!(
                pixels.data(),
                area_reference(514, 513, 512, 512, |x, y| { premultiplied(pixel(x, y)) })
            );
        } else {
            for (x, y, expected) in [
                (0, 0, [0, 0, 0, 128]),
                (256, 256, [3, 8, 18, 128]),
                (512, 511, [5, 15, 34, 128]),
            ] {
                let at = (y * width + x) as usize * 4;
                assert_eq!(&pixels.data()[at..at + 4], expected);
            }
        }
        for part in [plan, plan.part(256, 256, (1, 1)).unwrap()] {
            let key = exact_raster::RasterKey {
                source: 1,
                generation: 1,
                pixels: part.pixels,
                variant: 1,
                crop: part.crop,
            };
            let again = DecodePlan::of_key(metadata, &key).unwrap();
            assert_eq!(
                (again.pixels, again.crop, again.cost),
                (part.pixels, part.crop, part.cost)
            );
            let decoded = decode_rows(Cursor::new(&bytes), &again, || false).unwrap();
            if part.crop.whole() {
                assert_eq!(decoded.data(), pixels.data());
            } else {
                assert_eq!(decoded.pixel(0, 0), pixels.pixel(256, 256));
            }
        }
    }
    let mut bytes = header(4096, 4096, true);
    chunk(&mut bytes, b"IDAT", &[]);
    let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    for (desired, available, side) in [
        (2048., exact_raster::SESSION_BYTES, 2048),
        (2048., 16 * 1024 * 1024, 1024),
        (1000., exact_raster::SESSION_BYTES, 1024),
    ] {
        let planned = super::plan(header, (desired, desired), available).unwrap();
        assert_eq!((planned.pixels.width, planned.pixels.height), (side, side));
        assert!(planned.peak_bytes() <= available);
    }
}

fn decode_reference(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 {
        return None;
    }
    let src = &buf[..info.buffer_size()];
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    let push = |out: &mut Vec<u8>, r: u8, g: u8, b: u8, a: u8| {
        let p = |c: u8| (c as u32 * a as u32 / 255) as u8;
        out.extend_from_slice(&[p(r), p(g), p(b), a]);
    };
    match info.color_type {
        png::ColorType::Rgba => {
            for px in src.chunks_exact(4) {
                push(&mut out, px[0], px[1], px[2], px[3]);
            }
        }
        png::ColorType::Rgb => {
            for px in src.chunks_exact(3) {
                push(&mut out, px[0], px[1], px[2], 255);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for px in src.chunks_exact(2) {
                push(&mut out, px[0], px[0], px[0], px[1]);
            }
        }
        png::ColorType::Grayscale => {
            for px in src {
                push(&mut out, *px, *px, *px, 255);
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some((out, w, h))
}

/// A part of a picture is the whole decode's pixels of that part, bit for
/// bit, charged as the part; a key names it, and a part outside the decode
/// (or of a size the planner would not decode at) is refused.
#[test]
fn a_part_is_the_whole_decodes_pixels() {
    let bytes = png(200, 120);
    let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
    let whole = DecodePlan::new(header, (100, 60)).unwrap();
    let all = decode_rows(Cursor::new(&bytes), &whole, || false).unwrap();
    for (x, y, w, h) in [
        (17u32, 5u32, 40u32, 30u32),
        (0, 0, 100, 13),
        (63, 59, 37, 1),
        (99, 0, 1, 60),
    ] {
        let part = whole.part(x, y, (w, h)).unwrap();
        assert_eq!((part.pixels.width, part.pixels.height), (w, h));
        assert_eq!(part.output_bytes(), u64::from(w) * u64::from(h) * 4);
        assert_eq!(part.cost.scratch_bytes, whole.cost.scratch_bytes);
        assert_eq!((part.full(), part.natural()), (whole.pixels, (200, 120)));
        let cut = decode_rows(Cursor::new(&bytes), &part, || false).unwrap();
        for row in 0..h {
            let from = (((y + row) * 100 + x) * 4) as usize;
            assert_eq!(
                &cut.data()[(row * w * 4) as usize..((row + 1) * w * 4) as usize],
                &all.data()[from..from + (w * 4) as usize],
                "row {row} of {w}x{h} at {x},{y}"
            );
        }
        // The key that names the part gives the same plan back.
        let key = exact_raster::RasterKey {
            source: 1,
            generation: 1,
            pixels: part.pixels,
            variant: 1,
            crop: part.crop,
        };
        let again = DecodePlan::of_key(header, &key).unwrap();
        assert_eq!(
            (again.pixels, again.crop, again.cost),
            (part.pixels, part.crop, part.cost)
        );
    }
    // The whole is itself, not a part.
    assert!(whole.part(0, 0, (100, 60)).unwrap().crop.whole());
    for (x, y, w, h) in [
        (0u32, 0u32, 101u32, 60u32),
        (61, 0, 40, 60),
        (0, 31, 10, 30),
        (0, 0, 0, 5),
        (u32::MAX, 0, 2, 2),
    ] {
        assert!(whole.part(x, y, (w, h)).is_err(), "{w}x{h} at {x},{y}");
    }
    assert!(
        whole
            .part(1, 1, (5, 5))
            .unwrap()
            .part(0, 0, (2, 2))
            .is_err(),
        "a part of a part"
    );
}
