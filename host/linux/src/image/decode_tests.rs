use super::png_decode::{decode_rows, inspect, DecodePlan};
use std::io::Cursor;

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
    let n = u16::try_from(raw.len()).unwrap();
    let mut zlib = vec![0x78, 0x01, 0x01];
    zlib.extend_from_slice(&n.to_le_bytes());
    zlib.extend_from_slice(&(!n).to_le_bytes());
    zlib.extend_from_slice(&raw);
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

fn expected(width: u32, height: u32, out_w: u32, out_h: u32) -> Vec<u8> {
    (0..out_h)
        .flat_map(|y| {
            (0..out_w).flat_map(move |x| {
                let sx =
                    ((2 * u64::from(x) + 1) * u64::from(width) / (2 * u64::from(out_w))) as u32;
                let sy =
                    ((2 * u64::from(y) + 1) * u64::from(height) / (2 * u64::from(out_h))) as u32;
                let [r, g, b, a] = pixel(sx, sy);
                [r / 2, g / 2, b / 2, a]
            })
        })
        .collect()
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
    assert_eq!(bitmap.data(), expected(4000, 2000, 100, 50));
}

#[test]
fn adam7_rows_sample_every_pass_without_full_frame_fallback() {
    for (w, h, dw, dh) in [(19, 11, 7, 5), (8, 8, 8, 8), (1, 9, 1, 4)] {
        let bytes = adam7(w, h);
        let metadata = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let plan = DecodePlan::new(metadata, (dw, dh)).unwrap();
        let bitmap = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        assert_eq!(bitmap.data(), expected(w, h, dw, dh));
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
        let plan = DecodePlan::new(metadata, (7, 5)).unwrap();
        let actual = decode_rows(Cursor::new(&bytes), &plan, || false).unwrap();
        let (full, _, _) = decode_reference(&bytes).unwrap();
        let mut oracle = Vec::new();
        for y in 0..5u32 {
            for x in 0..7u32 {
                let sx = ((2 * x + 1) * w) / (2 * 7);
                let sy = ((2 * y + 1) * h) / (2 * 5);
                let i = ((sy * w + sx) * 4) as usize;
                oracle.extend_from_slice(&full[i..i + 4]);
            }
        }
        assert_eq!(actual.data(), oracle, "{color:?} {depth:?}");
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
