//! The installed-codec driver over a recording fake: the plan each edit
//! hands the platform, the sizes the search encodes, what a transform
//! keeps, and that every handle is closed.
use super::*;
use crate::image::edit::{parse_edit, parse_transform};
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A "source" is `fake <w> <h> <type> <colour> <hdr>`; an encode is as many
/// bytes as its pixels times its quality, so the search has a curve.
#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<String>>,
    open: Mutex<Vec<u64>>,
    next: Mutex<u64>,
    /// What `metadata` writes, else `None`.
    writes_metadata: bool,
}

impl Fake {
    fn handle(&self) -> u64 {
        let mut n = self.next.lock().unwrap();
        *n += 1;
        self.open.lock().unwrap().push(*n);
        *n
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn say(&self, line: String) {
        self.calls.lock().unwrap().push(line);
    }
}

impl Platform for Fake {
    fn decode(
        &self,
        source: &[u8],
        srgb: bool,
        max_pixels: u64,
    ) -> Result<Decoded, (String, String)> {
        let text = String::from_utf8_lossy(source);
        let words: Vec<&str> = text.split(' ').collect();
        if words.first() != Some(&"fake") {
            return Err(("undecodable".into(), "not an image".into()));
        }
        let (w, h): (u32, u32) = (words[1].parse().unwrap(), words[2].parse().unwrap());
        if u64::from(w) * u64::from(h) > max_pixels {
            return Err(("too-large".into(), "header".into()));
        }
        self.say(format!("decode srgb={srgb}"));
        Ok(Decoded {
            handle: self.handle(),
            width: w,
            height: h,
            media_type: words[3].into(),
            color_space: if srgb { "srgb".into() } else { words[4].into() },
            hdr: !srgb && words[5] == "1",
        })
    }
    fn edit(&self, handle: u64, plan: &Plan) -> Result<Decoded, (String, String)> {
        self.say(format!("edit {handle} {}", plan.json()));
        Ok(Decoded {
            handle: self.handle(),
            width: plan.size.0,
            height: plan.size.1,
            media_type: "image/jpeg".into(),
            color_space: "display-p3".into(),
            hdr: plan.rotate % 90.0 == 0.0,
        })
    }
    fn encode(
        &self,
        handle: u64,
        size: (u32, u32),
        media_type: &str,
        quality: f64,
        hdr: bool,
    ) -> Result<Vec<u8>, (String, String)> {
        self.say(format!(
            "encode {handle} {}x{} {media_type} {quality} hdr={hdr}",
            size.0, size.1
        ));
        let n = (f64::from(size.0 * size.1) * quality).ceil() as usize;
        Ok(vec![0; n.max(1)])
    }
    fn metadata(
        &self,
        _: &[u8],
        encoded: &[u8],
        media_type: &str,
        metadata: Metadata,
        size: (u32, u32),
    ) -> Option<Vec<u8>> {
        self.say(format!(
            "metadata {media_type} {} {}x{}",
            metadata.word(),
            size.0,
            size.1
        ));
        self.writes_metadata.then(|| [encoded, b"exif"].concat())
    }
    fn close(&self, handle: u64) {
        self.open.lock().unwrap().retain(|h| *h != handle);
    }
    fn encodes(&self, media_type: &str) -> bool {
        matches!(media_type, "image/jpeg" | "image/png" | "image/webp")
    }
}

fn deadline() -> Deadline<'static> {
    Deadline {
        at: Instant::now() + Duration::from_secs(10),
        check: &|| Ok(()),
        max_pixels: crate::image::MAX_SOURCE_PIXELS,
        max_scratch_bytes: 512 << 20,
    }
}

const PORTRAIT: &[u8] = b"fake 48 64 image/jpeg display-p3 1";

#[test]
fn a_quarter_turn_and_crop_hand_the_platform_one_plan_then_one_encode() {
    let fake = Fake {
        writes_metadata: true,
        ..Fake::default()
    };
    let options = parse_transform(
        &json!({"rotate": 90, "crop": {"x": 0, "y": 0, "width": 32, "height": 48}}),
    )
    .unwrap();
    let out = transform(&fake, PORTRAIT, &options, 1 << 20, deadline()).unwrap();
    // The turned frame is 64×48; the crop is its left half.
    assert_eq!(
        fake.calls(),
        [
            "decode srgb=false".to_string(),
            format!(
                "edit 1 {}",
                json!({"rotate": 90.0, "flip": null, "turned": [64, 48], "crop": [0, 0, 32, 48], "size": [32, 48], "smooth": true})
            ),
            "encode 2 32x48 image/jpeg 0.9 hdr=true".into(),
            "metadata image/jpeg keep 32x48".into(),
        ]
    );
    assert_eq!(
        (out.width, out.height, out.media_type),
        (32, 48, "image/jpeg")
    );
    assert_eq!(
        (out.color_space, out.hdr, out.metadata),
        ("display-p3", true, Metadata::Keep)
    );
    assert!(out.bytes.ends_with(b"exif"));
    assert!(fake.open.lock().unwrap().is_empty(), "every handle closed");
}

#[test]
fn an_untouched_transform_encodes_the_decode_and_says_what_it_could_not_keep() {
    let fake = Fake::default();
    let options = parse_transform(&json!({"type": "image/png"})).unwrap();
    let out = transform(&fake, PORTRAIT, &options, 1 << 20, deadline()).unwrap();
    assert_eq!(
        fake.calls(),
        [
            "decode srgb=false",
            "encode 1 48x64 image/png 0.9 hdr=false",
            "metadata image/png keep 48x64",
        ]
    );
    // A PNG carries no gain map, and this platform wrote no metadata for it.
    assert_eq!(
        (out.hdr, out.metadata, out.media_type),
        (false, Metadata::None, "image/png")
    );
    assert!(fake.open.lock().unwrap().is_empty());
}

#[test]
fn a_free_turn_and_a_size_are_one_edit_and_the_type_unsaid_is_the_sources() {
    let fake = Fake::default();
    let options = parse_transform(
        &json!({"rotate": 30, "resizeWidth": 20, "resizeQuality": "pixelated", "metadata": "none"}),
    )
    .unwrap();
    let out = transform(
        &fake,
        b"fake 40 40 image/webp srgb 0",
        &options,
        1 << 20,
        deadline(),
    )
    .unwrap();
    let calls = fake.calls();
    assert!(calls[1].contains("\"turned\":[55,55]"), "{calls:?}");
    assert!(
        calls[1].contains("\"size\":[20,20]") && calls[1].contains("\"smooth\":false"),
        "{calls:?}"
    );
    assert_eq!(calls[2], "encode 2 20x20 image/webp 0.9 hdr=false");
    assert_eq!(calls.len(), 3, "no metadata asked for `none`");
    assert_eq!(
        (out.media_type, out.metadata),
        ("image/webp", Metadata::None)
    );
}

#[test]
fn a_type_the_platform_cannot_encode_is_refused_before_the_decode() {
    let fake = Fake::default();
    let options = parse_transform(&json!({"type": "image/heic"})).unwrap();
    let e = transform(&fake, PORTRAIT, &options, 1 << 20, deadline()).unwrap_err();
    assert!(e.starts_with("transformImage: unsupported:"), "{e}");
    assert!(fake.calls().is_empty());
    let e = transform(
        &fake,
        b"nope",
        &parse_transform(&json!({})).unwrap(),
        1 << 20,
        deadline(),
    )
    .unwrap_err();
    assert!(e.starts_with("transformImage: undecodable:"), "{e}");
}

#[test]
fn a_crop_outside_and_an_output_too_large_close_what_was_decoded() {
    let fake = Fake::default();
    let options =
        parse_transform(&json!({"crop": {"x": 100, "y": 0, "width": 4, "height": 4}})).unwrap();
    let e = transform(&fake, PORTRAIT, &options, 1 << 20, deadline()).unwrap_err();
    assert!(e.starts_with("transformImage: outside:"), "{e}");
    let options = parse_transform(&json!({"resizeWidth": 16384, "resizeHeight": 16384})).unwrap();
    let e = transform(&fake, PORTRAIT, &options, 1 << 20, deadline()).unwrap_err();
    assert!(e.starts_with("transformImage: too-large:"), "{e}");
    let e = transform(
        &fake,
        PORTRAIT,
        &parse_transform(&json!({})).unwrap(),
        10,
        deadline(),
    )
    .unwrap_err();
    assert!(e.starts_with("transformImage: too-large:"), "{e}");
    assert!(fake.open.lock().unwrap().is_empty());
}

#[test]
fn compress_decodes_srgb_edits_once_and_searches_on_the_crop() {
    let fake = Fake::default();
    let edit = parse_edit(
        "compressImage",
        &json!({"rotate": 90, "crop": {"x": 0, "y": 0, "width": 32, "height": 48}}),
    )
    .unwrap();
    let out = compress(&fake, PORTRAIT, Format::Jpeg, 8192, 1000, &edit, deadline()).unwrap();
    let calls = fake.calls();
    assert_eq!(calls[0], "decode srgb=true");
    assert!(calls[1].starts_with("edit 1 "), "{calls:?}");
    assert_eq!(calls.iter().filter(|c| c.starts_with("edit")).count(), 1);
    // Every trial encodes the edit, without its gain map, at most 32×48.
    let trials: Vec<&String> = calls.iter().filter(|c| c.starts_with("encode")).collect();
    assert!(trials
        .iter()
        .all(|c| c.starts_with("encode 2 32x48 image/jpeg") && c.ends_with("hdr=false")));
    // The same sequence the shared search records: 1,536 pixels, a 1,000-byte budget.
    let expected = search(32, 48, 8192, 1000, |w, h, q| {
        Ok(vec![
            0;
            (f64::from(w * h) * f64::from(q) / 100.0).ceil()
                as usize
        ])
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        (out.width, out.height, out.bytes.len()),
        (expected.width, expected.height, expected.bytes.len())
    );
    assert!(fake.open.lock().unwrap().is_empty());
    let e = compress(
        &fake,
        PORTRAIT,
        Format::Png,
        8192,
        1,
        &Edit::default(),
        deadline(),
    )
    .unwrap_err();
    assert!(e.starts_with("compressImage: unfit:"), "{e}");
}
