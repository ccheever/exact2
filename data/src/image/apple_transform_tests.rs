use super::super::super::{compress_edited, Format, MAX_SOURCE_PIXELS};
use super::super::{inspect, png};
use super::*;
use crate::image::edit::{parse_transform, Edit, Flip, Rect};
use serde_json::json;
use std::time::{Duration, Instant};

/// Stored 64 × 48, red over blue, EXIF orientation 6 and a GPS position:
/// upright it is 48 × 64, blue on the left.
const FIXTURE: &[u8] = include_bytes!("../../../scripts/fixtures/picker/oriented-gps.jpg");
const COLOR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../host/apple/tests/ExactKitTests/ColorFixtures"
);

fn color(name: &str) -> Vec<u8> {
    std::fs::read(format!("{COLOR}/{name}")).unwrap()
}

fn run(source: &[u8], options: serde_json::Value) -> Result<Transformed, String> {
    let options = parse_transform(&options)?;
    super::super::super::transform(
        source,
        &options,
        50_000_000,
        Deadline {
            at: Instant::now() + Duration::from_secs(60),
            check: &|| Ok(()),
            max_pixels: MAX_SOURCE_PIXELS,
            max_scratch_bytes: 512 << 20,
        },
    )
}

/// An image's pixels, decoded into sRGB, premultiplied RGBA, row by row from the top.
fn pixels(bytes: &[u8]) -> (u32, u32, Vec<[u8; 4]>) {
    let opened = Opened::new(bytes, &|| "undecodable".into()).unwrap();
    // SAFETY: `opened` is live; the context's backing is `out`, which outlives it.
    unsafe {
        let image = Owned::new(CGImageSourceCreateImageAtIndex(
            opened.source.get(),
            0,
            ptr::null(),
        ))
        .unwrap();
        let (w, h) = size(image.get());
        let mut out = vec![[0u8; 4]; (w * h) as usize];
        let space = Owned::new(CGColorSpaceCreateWithName(kCGColorSpaceSRGB)).unwrap();
        let context = Owned::new(CGBitmapContextCreate(
            out.as_mut_ptr().cast(),
            w as usize,
            h as usize,
            8,
            w as usize * 4,
            space.get(),
            ALPHA_PREMULTIPLIED_LAST,
        ))
        .unwrap();
        CGContextDrawImage(
            context.get(),
            CGRect {
                x: 0.0,
                y: 0.0,
                width: w,
                height: h,
            },
            image.get(),
        );
        (w as u32, h as u32, out)
    }
}

fn at(p: &(u32, u32, Vec<[u8; 4]>), x: u32, y: u32) -> [u8; 4] {
    p.2[(y * p.0 + x) as usize]
}

fn blue(c: [u8; 4]) -> bool {
    c[2] > 200 && c[0] < 60 && c[1] < 60
}

fn red(c: [u8; 4]) -> bool {
    c[0] > 200 && c[1] < 60 && c[2] < 60
}

/// 8 × 4 blue, with a GPS position, an Apple maker note, custom XMP
/// (`exact:Secret`), a capture date, a camera make and an IPTC copyright.
const EVERY_TAG: &[u8] = include_bytes!("../../../scripts/fixtures/picker/every-tag.jpg");

/// Which of the source's own tags an image carries.
#[derive(Debug, PartialEq)]
struct Tags {
    gps: bool,
    maker: bool,
    xmp: bool,
    date: bool,
    make: bool,
    copyright: bool,
}

fn tags(bytes: &[u8]) -> Tags {
    extern "C" {
        static kCGImagePropertyGPSDictionary: CFTypeRef;
        static kCGImagePropertyMakerAppleDictionary: CFTypeRef;
        static kCGImagePropertyExifDateTimeOriginal: CFTypeRef;
        static kCGImagePropertyTIFFMake: CFTypeRef;
        static kCGImagePropertyIPTCCopyrightNotice: CFTypeRef;
        fn CGImageMetadataCopyStringValueWithPath(
            m: CFTypeRef,
            parent: CFTypeRef,
            path: CFTypeRef,
        ) -> CFTypeRef;
    }
    let opened = Opened::new(bytes, &|| "undecodable".into()).unwrap();
    let p = opened.properties.get();
    // SAFETY: the dictionaries are live and their values borrowed; the copies are +1.
    unsafe {
        let has = |group: CFTypeRef, key: CFTypeRef| {
            let g = CFDictionaryGetValue(p, group);
            !g.is_null() && !CFDictionaryGetValue(g, key).is_null()
        };
        let meta = Owned::new(CGImageSourceCopyMetadataAtIndex(
            opened.source.get(),
            0,
            ptr::null(),
        ));
        let path = cfstring("exact:Secret").unwrap();
        Tags {
            gps: !CFDictionaryGetValue(p, kCGImagePropertyGPSDictionary).is_null(),
            maker: !CFDictionaryGetValue(p, kCGImagePropertyMakerAppleDictionary).is_null(),
            xmp: meta.is_some_and(|m| {
                Owned::new(CGImageMetadataCopyStringValueWithPath(
                    m.get(),
                    ptr::null(),
                    path.get(),
                ))
                .is_some()
            }),
            date: has(
                kCGImagePropertyExifDictionary,
                kCGImagePropertyExifDateTimeOriginal,
            ),
            make: has(kCGImagePropertyTIFFDictionary, kCGImagePropertyTIFFMake),
            copyright: has(
                kCGImagePropertyIPTCDictionary,
                kCGImagePropertyIPTCCopyrightNotice,
            ),
        }
    }
}

/// An ISO 21496-1 gain map, which only the macOS 15 and iOS 18 keys name.
fn has_gain_map(bytes: &[u8]) -> bool {
    let keys = gain_map().expect("macOS 15 or iOS 18");
    let opened = Opened::new(bytes, &|| "undecodable".into()).unwrap();
    // SAFETY: `opened` is live; the copy is +1 and released.
    Owned::new(unsafe {
        CGImageSourceCopyAuxiliaryDataInfoAtIndex(opened.source.get(), 0, keys.iso)
    })
    .is_some()
}

#[test]
fn the_edit_runs_in_one_order_on_the_upright_picture() {
    let turned = run(FIXTURE, json!({"rotate": 90})).unwrap();
    assert_eq!(
        (turned.width, turned.height, turned.media_type),
        (64, 48, "image/jpeg")
    );
    let p = pixels(&turned.bytes);
    assert_eq!((p.0, p.1), (64, 48));
    // The upright left edge, blue, turns to the top.
    assert!(
        blue(at(&p, 2, 2)) && blue(at(&p, 61, 2)),
        "{:?}",
        at(&p, 2, 2)
    );
    assert!(red(at(&p, 2, 45)), "{:?}", at(&p, 2, 45));

    let mirrored = pixels(&run(FIXTURE, json!({"flip": "horizontal"})).unwrap().bytes);
    assert_eq!((mirrored.0, mirrored.1), (48, 64));
    assert!(red(at(&mirrored, 2, 2)) && blue(at(&mirrored, 45, 2)));

    // Turn, then flip in the turned frame: the blue edge on top flips to the bottom.
    let both = pixels(
        &run(FIXTURE, json!({"rotate": 90, "flip": "vertical"}))
            .unwrap()
            .bytes,
    );
    assert!(red(at(&both, 2, 2)) && blue(at(&both, 2, 45)));

    let cropped = run(
        FIXTURE,
        json!({"crop": {"x": 0, "y": 0, "width": 24, "height": 64}}),
    )
    .unwrap();
    let c = pixels(&cropped.bytes);
    assert_eq!((c.0, c.1), (24, 64));
    assert!([(1, 1), (22, 1), (1, 62), (22, 62)]
        .iter()
        .all(|&(x, y)| blue(at(&c, x, y))));

    let resized = run(FIXTURE, json!({"rotate": 270, "resizeWidth": 32})).unwrap();
    assert_eq!((resized.width, resized.height), (32, 24));
    assert!(red(at(&pixels(&resized.bytes), 2, 2)));
}

#[test]
fn a_free_turn_is_clear_at_the_corners_where_the_type_has_alpha() {
    let opaque = png(40, 20, &[[0, 0, 255, 255]; 800].concat());
    let png = run(&opaque, json!({"rotate": 30})).unwrap();
    assert_eq!(png.media_type, "image/png", "the source's own type");
    let p = pixels(&png.bytes);
    assert_eq!((p.0, p.1), (45, 38));
    assert_eq!(at(&p, 0, 0)[3], 0);
    assert!(blue(at(&p, 22, 19)));
    let jpeg = pixels(
        &run(&opaque, json!({"rotate": 30, "type": "image/jpeg"}))
            .unwrap()
            .bytes,
    );
    assert!(
        at(&jpeg, 0, 0)[..3].iter().all(|&c| c > 245),
        "{:?}",
        at(&jpeg, 0, 0)
    );
    // A quarter turn of an opaque source leaves no corner to fill.
    let quarter = run(&opaque, json!({"rotate": 90})).unwrap();
    assert_eq!(at(&pixels(&quarter.bytes), 0, 0)[3], 255);
}

#[test]
fn metadata_is_kept_stripped_of_location_or_dropped() {
    let kept = run(FIXTURE, json!({"rotate": 90})).unwrap();
    assert_eq!(kept.metadata, Metadata::Keep);
    let read = inspect(&kept.bytes).unwrap();
    assert!(read.gps && read.exif_date, "{read:?}");
    assert_eq!(read.orientation, Some(1), "the pixels are upright now");

    let private = run(FIXTURE, json!({"metadata": "no-location"})).unwrap();
    let read = inspect(&private.bytes).unwrap();
    assert!(!read.gps && read.exif_date, "{read:?}");
    assert!(matches!(read.orientation, None | Some(1)));

    let none = run(FIXTURE, json!({"metadata": "none"})).unwrap();
    let read = inspect(&none.bytes).unwrap();
    assert!(!read.gps && !read.exif_date, "{read:?}");

    // `keep` keeps the maker note and XMP too; `no-location` writes the
    // capture's standard tags and the rights, and none of those three (D15).
    let all = |gps, maker, xmp| Tags {
        gps,
        maker,
        xmp,
        date: true,
        make: true,
        copyright: true,
    };
    assert_eq!(tags(EVERY_TAG), all(true, true, true));
    assert_eq!(
        tags(&run(EVERY_TAG, json!({"rotate": 180})).unwrap().bytes),
        all(true, true, true)
    );
    assert_eq!(
        tags(
            &run(EVERY_TAG, json!({"metadata": "no-location"}))
                .unwrap()
                .bytes
        ),
        all(false, false, false)
    );
    let nothing = Tags {
        gps: false,
        maker: false,
        xmp: false,
        date: false,
        make: false,
        copyright: false,
    };
    assert_eq!(
        tags(&run(EVERY_TAG, json!({"metadata": "none"})).unwrap().bytes),
        nothing
    );
    let compressed = super::super::super::compress(
        EVERY_TAG,
        Format::Jpeg,
        8,
        100_000,
        Instant::now() + Duration::from_secs(60),
    );
    assert_eq!(tags(&compressed.unwrap().bytes), nothing);
}

#[test]
fn compress_takes_the_edit_and_still_keeps_nothing() {
    let edit = Edit {
        rotate: 90.0,
        flip: Some(Flip::Horizontal),
        crop: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 64.0,
            height: 24.0,
        }),
    };
    let out = compress_edited(
        FIXTURE,
        Format::Jpeg,
        32,
        2_000_000,
        &edit,
        Deadline {
            at: Instant::now() + Duration::from_secs(60),
            check: &|| Ok(()),
            max_pixels: MAX_SOURCE_PIXELS,
            max_scratch_bytes: 512 << 20,
        },
    )
    .unwrap();
    // Cropped to the turned picture's blue top half, then fitted into 32.
    assert_eq!((out.width, out.height), (32, 12));
    let p = pixels(&out.bytes);
    assert!(
        blue(at(&p, 2, 2)) && blue(at(&p, 29, 9)),
        "{:?}",
        at(&p, 29, 9)
    );
    let read = inspect(&out.bytes).unwrap();
    assert!(!read.gps && !read.exif_date, "{read:?}");
}

/// Whether this ImageIO writes a gain map at all: the source's own HDR
/// decode, encoded with the gain-map request. The iOS simulator's does not
/// (iOS 27.0, 2026-10-24), so there the transform's SDR retry is what runs.
fn writes_gain_maps() -> bool {
    let keys = gain_map().expect("macOS 15 or iOS 18");
    let opened = Opened::new(&color("gainmap-iso.jpg"), &|| "undecodable".into()).unwrap();
    // SAFETY: the keys and values are immutable statics; the image is +1.
    let decode =
        dictionary(unsafe { &[(kCGImageSourceDecodeRequest, kCGImageSourceDecodeToHDR)] }).unwrap();
    let image = Owned::new(unsafe {
        CGImageSourceCreateImageAtIndex(opened.source.get(), 0, decode.get())
    })
    .unwrap();
    encode(
        image.get(),
        "public.jpeg",
        &[(keys.encode_request, keys.to_gain_map)],
        &Written::Nothing,
        1 << 20,
    )
    .is_some_and(|(bytes, _)| has_gain_map(&bytes))
}

fn hdr_headroom(bytes: &[u8]) -> f32 {
    extern "C" {
        fn CGImageGetContentHeadroom(image: CFTypeRef) -> f32;
    }
    let opened = Opened::new(bytes, &|| "undecodable".into()).unwrap();
    // SAFETY: the keys are immutable statics; the source and options remain live.
    let decode =
        dictionary(unsafe { &[(kCGImageSourceDecodeRequest, kCGImageSourceDecodeToHDR)] }).unwrap();
    let image = Owned::new(unsafe {
        CGImageSourceCreateImageAtIndex(opened.source.get(), 0, decode.get())
    })
    .unwrap();
    // SAFETY: the decoded image remains live.
    unsafe { CGImageGetContentHeadroom(image.get()) }
}

#[test]
fn a_gain_map_survives_the_edit_and_the_colour_space_is_kept() {
    let kept = writes_gain_maps();
    assert!(
        kept || cfg!(target_abi = "sim"),
        "this Mac's ImageIO writes no gain map"
    );
    for name in [
        "gainmap-iso.jpg",
        "gainmap-iso.heic",
        "gainmap-orient6.heic",
    ] {
        let source = color(name);
        let out = run(
            &source,
            json!({"rotate": 90, "metadata": if name.ends_with(".jpg") { "keep" } else { "none" }}),
        )
        .unwrap();
        // Where ImageIO cannot write one, the SDR retry runs and the result says so.
        assert_eq!((out.hdr, has_gain_map(&out.bytes)), (kept, kept), "{name}");
        if kept {
            let original = hdr_headroom(&source);
            let edited = hdr_headroom(&out.bytes);
            assert!(original > 1.0, "{name}: source must be HDR");
            assert!(
                (edited - original).abs() < 0.1,
                "{name}: HDR headroom changed from {original} to {edited}"
            );
        }
        assert_eq!(out.color_space, "display-p3", "{name}");
        assert_eq!(
            out.media_type,
            if name.ends_with(".jpg") {
                "image/jpeg"
            } else {
                "image/heic"
            }
        );
        // compressImage tone-maps to SDR and writes no gain map.
        let flat = super::super::super::compress(
            &source,
            Format::Jpeg,
            256,
            2_000_000,
            Instant::now() + Duration::from_secs(60),
        )
        .unwrap();
        assert!(!has_gain_map(&flat.bytes), "{name}");
    }
    // PNG carries no gain map: the SDR rendition, said so.
    let png = run(&color("gainmap-iso.jpg"), json!({"type": "image/png"})).unwrap();
    assert!(!png.hdr && !has_gain_map(&png.bytes));

    let wide = run(&color("p3-camera.jpg"), json!({})).unwrap();
    assert_eq!((wide.color_space, wide.hdr), ("display-p3", false));
    let turned = run(&color("p3-orient6.jpg"), json!({})).unwrap();
    assert_eq!((turned.width, turned.height), (64, 256));
    assert_eq!(run(FIXTURE, json!({})).unwrap().color_space, "srgb");
}

#[test]
fn failures_name_the_call() {
    let err = run(
        FIXTURE,
        json!({"crop": {"x": 100, "y": 0, "width": 4, "height": 4}}),
    )
    .unwrap_err();
    assert_eq!(
        err,
        "transformImage: outside: the crop misses the 48×64 image"
    );
    let err = run(b"not an image", json!({})).unwrap_err();
    assert!(err.starts_with("transformImage: undecodable: "), "{err}");
    let err = run(FIXTURE, json!({"type": "image/webp"})).unwrap_err();
    assert!(err.starts_with("transformImage: unsupported: "), "{err}");
    let options = parse_transform(&json!({})).unwrap();
    let err = super::super::super::transform(
        FIXTURE,
        &options,
        100,
        Deadline {
            at: Instant::now() + Duration::from_secs(60),
            check: &|| Ok(()),
            max_pixels: MAX_SOURCE_PIXELS,
            max_scratch_bytes: 512 << 20,
        },
    )
    .unwrap_err();
    assert!(err.starts_with("transformImage: too-large: "), "{err}");
}
