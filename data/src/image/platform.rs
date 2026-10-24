//! A codec a host installs where the tree has none: Android's (LLP 1133.019
//! D16), whose decoder, bitmap transforms, encoders and EXIF writer are the
//! platform's Java ones, reached through the host's JNI. The host gives the
//! primitives ([`Platform`]); this drives them in the order and the search
//! every codec shares ([`super::edit`], [`super::search`]), so Android's
//! `compressImage` and `transformImage` cannot drift from Apple's.
//!
//! A decoded picture lives in the platform as a handle until it is closed,
//! so a search encodes the one decode at each trial size.
use super::edit::{self, Flip, Geometry, Metadata, Transform};
use super::{failure, search, search_lossless, Compressed, Deadline, Edit, Format, Transformed};
use std::sync::OnceLock;

/// A decoded picture, upright (its orientation applied), held by the platform.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub handle: u64,
    pub width: u32,
    pub height: u32,
    /// The source's media type, as the decoder read it.
    pub media_type: String,
    /// `srgb`, `display-p3` or `other`.
    pub color_space: String,
    /// It carries a gain map.
    pub hdr: bool,
}

/// One edit of a decoded picture, in D14's order: turned `rotate` degrees
/// clockwise about its centre into a frame of `turned` pixels, flipped in
/// that frame, cut to `crop` (`x`, `y`, `width`, `height` in it), then
/// scaled to `size` (`smooth` but for `pixelated`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub rotate: f64,
    pub flip: Option<Flip>,
    pub turned: (u32, u32),
    pub crop: (u32, u32, u32, u32),
    pub size: (u32, u32),
    pub smooth: bool,
}

impl Plan {
    /// As JSON, the platform's side of the wire.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "rotate": self.rotate,
            "flip": self.flip.map(|f| match f {
                Flip::Horizontal => "horizontal",
                Flip::Vertical => "vertical",
            }),
            "turned": [self.turned.0, self.turned.1],
            "crop": [self.crop.0, self.crop.1, self.crop.2, self.crop.3],
            "size": [self.size.0, self.size.1],
            "smooth": self.smooth,
        })
    }
}

/// What a host's codec does, each step failing with a code and its detail
/// (`undecodable`, `too-large`, `unsupported`, `UnknownError`).
pub trait Platform: Send + Sync {
    /// `source` decoded upright, refused `too-large` from its header when
    /// it holds more than `max_pixels`; converted to sRGB when `srgb`.
    fn decode(
        &self,
        source: &[u8],
        srgb: bool,
        max_pixels: u64,
    ) -> Result<Decoded, (String, String)>;
    /// The picture `handle` edited by `plan`: a new handle.
    fn edit(&self, handle: u64, plan: &Plan) -> Result<Decoded, (String, String)>;
    /// The picture `handle` at `size`, encoded as `media_type` at `quality`
    /// (0 to 1), its gain map kept only where `hdr`, and a JPEG's
    /// transparency on white.
    fn encode(
        &self,
        handle: u64,
        size: (u32, u32),
        media_type: &str,
        quality: f64,
        hdr: bool,
    ) -> Result<Vec<u8>, (String, String)>;
    /// `encoded` with `metadata`'s set of `source`'s metadata (its
    /// orientation reset, its size `size`); `None` where the platform
    /// writes none for this type.
    fn metadata(
        &self,
        source: &[u8],
        encoded: &[u8],
        media_type: &str,
        metadata: Metadata,
        size: (u32, u32),
    ) -> Option<Vec<u8>>;
    /// Let go of `handle`.
    fn close(&self, handle: u64);
    /// Whether it encodes `media_type`.
    fn encodes(&self, media_type: &str) -> bool;
}

static PLATFORM: OnceLock<Box<dyn Platform>> = OnceLock::new();

/// Install the host's codec, once a process: whether this one is installed.
pub fn install(codec: Box<dyn Platform>) -> bool {
    PLATFORM.set(codec).is_ok()
}

/// The installed codec.
pub fn installed() -> Option<&'static dyn Platform> {
    PLATFORM.get().map(|b| b.as_ref())
}

/// Closes a handle on every path out.
struct Held<'a>(&'a dyn Platform, u64);
impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.0.close(self.1);
    }
}

fn named(call: &str) -> impl Fn((String, String)) -> String + '_ {
    move |(code, detail)| edit::failure_named(call, &code, detail)
}

/// `edit` of a `width × height` picture at `size`, or `None` for no edit and
/// no size change.
fn plan_of(
    call: &str,
    (width, height): (u32, u32),
    edit: &Edit,
    size: Option<(u32, u32)>,
    smooth: bool,
) -> Result<(Option<Plan>, (u32, u32)), String> {
    let geometry = Geometry::new(call, width, height, edit)?;
    let size = size.unwrap_or_else(|| geometry.size());
    let plan = Plan {
        rotate: edit.rotate,
        flip: edit.flip,
        turned: geometry.turned,
        crop: geometry.crop,
        size,
        smooth,
    };
    Ok((
        (!edit.is_identity() || size != (width, height)).then_some(plan),
        size,
    ))
}

fn bounded(call: &str, (w, h): (u32, u32), max_pixels: u64) -> Result<(), String> {
    if u64::from(w) * u64::from(h) > max_pixels || w.max(h) > edit::MAX_SIDE {
        return Err(edit::failure_named(
            call,
            "too-large",
            format!("{w}×{h} pixels is over the output bound"),
        ));
    }
    Ok(())
}

/// `compressImage` through `codec`: one sRGB decode, the edit, then the
/// search at the crop's size (A1.2), metadata dropped and any gain map
/// tone-mapped away, as an upload is (D15).
pub fn compress(
    codec: &dyn Platform,
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    edit: &Edit,
    deadline: Deadline<'_>,
) -> Result<Compressed, String> {
    const CALL: &str = "compressImage";
    deadline.check()?;
    let decoded = codec
        .decode(source, true, deadline.max_pixels)
        .map_err(named(CALL))?;
    let mut held = vec![Held(codec, decoded.handle)];
    let upright = (decoded.width, decoded.height);
    let (plan, size) = plan_of(CALL, upright, edit, None, true)?;
    bounded(
        CALL,
        plan.map_or(upright, |p| p.turned),
        deadline.max_pixels,
    )?;
    let mut picture = decoded.handle;
    if let Some(plan) = plan {
        deadline.check()?;
        let edited = codec.edit(picture, &plan).map_err(named(CALL))?;
        picture = edited.handle;
        held.push(Held(codec, picture));
    }
    let best = match format {
        Format::Jpeg => search(size.0, size.1, max_dimension, max_bytes, |w, h, q| {
            deadline.check()?;
            (codec.encode(picture, (w, h), "image/jpeg", f64::from(q) / 100.0, false))
                .map_err(|(code, detail)| failure(&code, detail))
        })?,
        Format::Png => search_lossless(size.0, size.1, max_dimension, max_bytes, |w, h| {
            deadline.check()?;
            (codec.encode(picture, (w, h), "image/png", 1.0, false))
                .map_err(|(code, detail)| failure(&code, detail))
        })?,
    };
    drop(held);
    let name = match format {
        Format::Jpeg => "JPEG",
        Format::Png => "PNG",
    };
    best.ok_or_else(|| {
        failure(
            "unfit",
            format!("no {name} of this image fits {max_bytes} bytes"),
        )
    })
}

/// The media types a result names, as the platform's words map to them.
fn media_type(t: &str) -> &'static str {
    match t {
        "image/png" => "image/png",
        "image/webp" => "image/webp",
        "image/heic" => "image/heic",
        _ => "image/jpeg",
    }
}

/// `transformImage` through `codec`: one decode keeping the source's colour
/// and gain map, the edit at the size asked, one encode, then the metadata
/// asked where the platform writes it for the type (D15); the result says
/// what was kept.
pub fn transform(
    codec: &dyn Platform,
    source: &[u8],
    options: &Transform,
    max_bytes: u64,
    deadline: Deadline<'_>,
) -> Result<Transformed, String> {
    const CALL: &str = "transformImage";
    let wanted = match &options.media_type {
        Some(t) if !codec.encodes(t) => {
            return Err(edit::failure_named(
                CALL,
                "unsupported",
                format!("this host does not encode {t}"),
            ))
        }
        Some(t) => Some(media_type(t)),
        None => None,
    };
    deadline.check()?;
    let decoded = codec
        .decode(source, false, deadline.max_pixels)
        .map_err(named(CALL))?;
    let mut held = vec![Held(codec, decoded.handle)];
    // Unsaid, the source's own type where this host encodes it, else JPEG.
    let out_type = wanted.unwrap_or_else(|| {
        let own = media_type(&decoded.media_type);
        if own == decoded.media_type && codec.encodes(own) {
            own
        } else {
            "image/jpeg"
        }
    });
    let upright = (decoded.width, decoded.height);
    let geometry = Geometry::new(CALL, upright.0, upright.1, &options.edit)?;
    let (cw, ch) = geometry.size();
    let size = edit::resized(cw, ch, options.resize_width, options.resize_height);
    bounded(CALL, geometry.turned, deadline.max_pixels)?;
    bounded(CALL, size, deadline.max_pixels)?;
    let smooth = options.resize_quality != edit::ResizeQuality::Pixelated;
    let (plan, size) = plan_of(CALL, upright, &options.edit, Some(size), smooth)?;
    let mut picture = decoded.clone();
    if let Some(plan) = plan {
        deadline.check()?;
        picture = codec.edit(decoded.handle, &plan).map_err(named(CALL))?;
        held.push(Held(codec, picture.handle));
    }
    // A gain map rides in a JPEG; the platform drops it where an edit could
    // not keep it (a free turn draws the picture again).
    let hdr = picture.hdr && out_type == "image/jpeg";
    deadline.check()?;
    let bytes = codec
        .encode(picture.handle, size, out_type, options.quality, hdr)
        .map_err(named(CALL))?;
    drop(held);
    let (bytes, metadata) = match options.metadata {
        Metadata::None => (bytes, Metadata::None),
        kept => match codec.metadata(source, &bytes, out_type, kept, size) {
            Some(with) => (with, kept),
            None => (bytes, Metadata::None),
        },
    };
    if bytes.len() as u64 > max_bytes {
        return Err(edit::failure_named(
            CALL,
            "too-large",
            format!("{} bytes is over {max_bytes}", bytes.len()),
        ));
    }
    Ok(Transformed {
        bytes,
        width: size.0,
        height: size.1,
        media_type: out_type,
        color_space: match picture.color_space.as_str() {
            "srgb" => "srgb",
            "display-p3" => "display-p3",
            _ => "other",
        },
        hdr,
        metadata,
    })
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod tests;
