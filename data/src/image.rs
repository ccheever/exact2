//! `storage.fs.compressImage` (LLP 1069.002 Amendment A1): decode an image,
//! scale it to a pixel budget and encode it as a JPEG, or a PNG that keeps
//! its alpha (A1.11), within a byte budget; and `storage.fs.transformImage`
//! (LLP 1133.019 D14), the same codec's edit: turned, flipped, cropped and
//! sized, keeping the source's colour, HDR and metadata (D15). Both take
//! [`edit`]'s `rotate`, `flip` and `crop`.
//!
//! The JPEG search is Bluesky's (`social-app` `compress.ts`), restated in
//! A1.2 and kept as a pure function over an encoder ([`search`]) so every
//! host's trials can be checked against one recorded sequence; a PNG has no
//! quality, so it only shrinks ([`search_lossless`]). The codec is the
//! platform's: ImageIO on Apple ([`compress`]), else one a host installs
//! ([`platform`]: Android's, LLP 1133.019 D16); with neither the call is
//! refused (`unsupported`, A1.3).
use std::time::Instant;

#[cfg(target_vendor = "apple")]
mod apple;
pub mod edit;
pub mod platform;
pub use edit::{Edit, Metadata, Transform};

/// Whether this process has an image codec: Apple's, or one a host installed.
pub fn available() -> bool {
    cfg!(target_vendor = "apple") || platform::installed().is_some()
}

/// The largest `maxDimension` (A1.1).
pub const MAX_DIMENSION: u32 = 8192;
/// The largest `maxBytes` (A1.1), and the largest source file (A1.5):
/// `exact_raster::MAX_ENCODED_BYTES`.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// The most source pixels the header may declare (A1.5):
/// `exact_raster::MAX_SOURCE_PIXELS`.
pub const MAX_SOURCE_PIXELS: u64 = 64 * 1024 * 1024;
/// The most dimensions the search shrinks through (`attempts >= 4`).
pub const MAX_SHRINKS: u32 = 4;

/// What `compressImage` writes (`type`, A1.11): a JPEG unless the app asks
/// for a PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Jpeg,
    Png,
}

impl Format {
    /// The `type` option's value, or `None` for a type it does not take.
    pub fn parse(media_type: &str) -> Option<Self> {
        match media_type {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            _ => None,
        }
    }
    /// The media type the result reports.
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

/// What the codec made: encoded bytes and their pixel size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compressed {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// A failure, as `compressImage: <code>: <detail>` (A1.4): the prelude reads
/// the code from the message natively.
pub fn failure(code: &str, detail: impl std::fmt::Display) -> String {
    format!("compressImage: {code}: {detail}")
}

/// Check the two limits as A1.1 bounds them.
pub fn check_limits(max_dimension: f64, max_bytes: f64) -> Result<(u32, u64), String> {
    let integer = |n: f64, max: f64| n.fract() == 0.0 && (1.0..=max).contains(&n);
    if !integer(max_dimension, MAX_DIMENSION as f64) {
        return Err(failure(
            "invalid",
            format!("maxDimension must be an integer from 1 to {MAX_DIMENSION}"),
        ));
    }
    if !integer(max_bytes, MAX_BYTES as f64) {
        return Err(failure(
            "invalid",
            format!("maxBytes must be an integer from 1 to {MAX_BYTES}"),
        ));
    }
    Ok((max_dimension as u32, max_bytes as u64))
}

/// `containImageRes`: the size `width × height` takes inside a square of
/// `max`, never larger than itself, each side floored (and at least 1).
pub fn contain(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width <= max && height <= max {
        return (width, height);
    }
    let scale = if width > height {
        f64::from(max) / f64::from(width)
    } else {
        f64::from(max) / f64::from(height)
    };
    let side = |n: u32| ((f64::from(n) * scale).floor() as u32).max(1);
    (side(width), side(height))
}

/// Bluesky's search over an oriented `width × height` source (A1.2).
/// `encode(w, h, quality)` returns the JPEG at `quality` percent; an `Err`
/// ends the search with it. Returns the best fit, or `None`: nothing fits.
pub fn search<E>(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_bytes: u64,
    mut encode: E,
) -> Result<Option<Compressed>, String>
where
    E: FnMut(u32, u32, u32) -> Result<Vec<u8>, String>,
{
    let mut dimension = max_dimension;
    let (mut lo, mut hi) = (0u32, 101u32);
    let mut shrinks = 0;
    let mut best = None;
    while hi - lo > 1 {
        if shrinks >= MAX_SHRINKS {
            break;
        }
        let (w, h) = contain(width, height, dimension);
        // `Math.round((hi + lo) / 2)`: a half rounds up.
        let quality = (hi + lo).div_ceil(2);
        if quality <= 13 {
            lo = 0;
            hi = 101;
            shrinks += 1;
            dimension = (f64::from(dimension) * 0.8).floor() as u32;
            continue;
        }
        let bytes = encode(w, h, quality)?;
        if bytes.len() as u64 <= max_bytes {
            lo = quality;
            best = Some(Compressed {
                bytes,
                width: w,
                height: h,
            });
        } else {
            hi = quality;
        }
    }
    Ok(best)
}

/// A PNG's search (A1.11): the fitted size, then each 0.8 shrink, at most
/// [`MAX_SHRINKS`] of them, until `encode(w, h)` fits `max_bytes`. `None`:
/// nothing fits.
pub fn search_lossless<E>(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_bytes: u64,
    mut encode: E,
) -> Result<Option<Compressed>, String>
where
    E: FnMut(u32, u32) -> Result<Vec<u8>, String>,
{
    let mut dimension = max_dimension;
    for _ in 0..=MAX_SHRINKS {
        let (w, h) = contain(width, height, dimension);
        let bytes = encode(w, h)?;
        if bytes.len() as u64 <= max_bytes {
            return Ok(Some(Compressed {
                bytes,
                width: w,
                height: h,
            }));
        }
        if dimension == 1 {
            break;
        }
        dimension = ((f64::from(dimension) * 0.8).floor() as u32).max(1);
    }
    Ok(None)
}

/// When the codec must stop starting work: checked before the decode and
/// before each trial (a trial already running finishes).
pub struct Deadline<'a> {
    pub at: Instant,
    pub check: &'a dyn Fn() -> Result<(), String>,
    pub max_pixels: u64,
    pub max_scratch_bytes: u64,
}

impl Deadline<'_> {
    pub fn check(&self) -> Result<(), String> {
        (self.check)()?;
        if Instant::now() >= self.at {
            return Err(failure("timeout", "the search ran past its time"));
        }
        Ok(())
    }
}

/// Decode `source`, then search with the platform's encoder for `format`,
/// starting no decode or trial at or after `deadline` (`timeout`).
/// `max_dimension` and `max_bytes` are as [`check_limits`] admits.
pub fn compress(
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    deadline: Instant,
) -> Result<Compressed, String> {
    compress_controlled(
        source,
        format,
        max_dimension,
        max_bytes,
        deadline,
        &|| Ok(()),
        MAX_SOURCE_PIXELS,
        512 * 1024 * 1024,
    )
}

/// The native storage provider's bounded, cooperatively cancellable entry.
/// `check` is consulted before each decode/encode trial; a running native call
/// finishes before that worker's reservation is released.
/// `max_scratch_bytes` is a conservative estimate used for native-memory admission,
/// not an allocator cap on ImageIO or a bound on process RSS.
#[allow(clippy::too_many_arguments)]
pub fn compress_controlled(
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    deadline: Instant,
    check: &dyn Fn() -> Result<(), String>,
    max_pixels: u64,
    max_scratch_bytes: u64,
) -> Result<Compressed, String> {
    compress_edited(
        source,
        format,
        max_dimension,
        max_bytes,
        &Edit::default(),
        Deadline {
            at: deadline,
            check,
            max_pixels: max_pixels.min(MAX_SOURCE_PIXELS),
            max_scratch_bytes,
        },
    )
}

/// [`compress_controlled`] of `edit`'s picture (LLP 1133.019 D15): turned,
/// flipped and cropped before the search, which runs on the crop's size.
pub fn compress_edited(
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    edit: &Edit,
    deadline: Deadline<'_>,
) -> Result<Compressed, String> {
    check_limits(f64::from(max_dimension), max_bytes as f64)?;
    (deadline.check)()?;
    if source.len() as u64 > MAX_BYTES {
        return Err(failure(
            "too-large",
            format!("{} bytes is over {MAX_BYTES}", source.len()),
        ));
    }
    platform(source, format, max_dimension, max_bytes, edit, deadline)
}

#[cfg(target_vendor = "apple")]
fn platform(
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    edit: &Edit,
    deadline: Deadline<'_>,
) -> Result<Compressed, String> {
    apple::compress(source, format, max_dimension, max_bytes, edit, deadline)
}

#[cfg(not(target_vendor = "apple"))]
fn platform(
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    edit: &Edit,
    deadline: Deadline<'_>,
) -> Result<Compressed, String> {
    let codec = platform::installed().ok_or_else(unsupported)?;
    platform::compress(
        codec,
        source,
        format,
        max_dimension,
        max_bytes,
        edit,
        deadline,
    )
}

/// What `transformImage` wrote, and what of the source it kept: D14's
/// `TransformedImage` less its path and size, which the writer knows.
#[derive(Debug, Clone, PartialEq)]
pub struct Transformed {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub media_type: &'static str,
    /// `srgb`, `display-p3` or `other`.
    pub color_space: &'static str,
    pub hdr: bool,
    pub metadata: Metadata,
}

impl Transformed {
    /// The facts the result record carries beside its path, size and pixels.
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "type": self.media_type,
            "colorSpace": self.color_space,
            "hdr": self.hdr,
            "metadata": self.metadata.word(),
        })
    }
}

/// `transformImage` (LLP 1133.019 D14): one decode, the edit, one encode,
/// keeping what `options` says (D15), in at most `max_bytes`.
pub fn transform(
    source: &[u8],
    options: &Transform,
    max_bytes: u64,
    deadline: Deadline<'_>,
) -> Result<Transformed, String> {
    (deadline.check)()?;
    if source.len() as u64 > MAX_BYTES {
        return Err(edit::failure_named(
            "transformImage",
            "too-large",
            format!("{} bytes is over {MAX_BYTES}", source.len()),
        ));
    }
    transform_platform(source, options, max_bytes, deadline)
}

#[cfg(target_vendor = "apple")]
fn transform_platform(
    source: &[u8],
    options: &Transform,
    max_bytes: u64,
    deadline: Deadline<'_>,
) -> Result<Transformed, String> {
    apple::transform(source, options, max_bytes, deadline)
}

#[cfg(not(target_vendor = "apple"))]
fn transform_platform(
    source: &[u8],
    options: &Transform,
    max_bytes: u64,
    deadline: Deadline<'_>,
) -> Result<Transformed, String> {
    let codec = platform::installed().ok_or_else(|| {
        edit::failure_named(
            "transformImage",
            "unsupported",
            "no image encoder on this host",
        )
    })?;
    platform::transform(codec, source, options, max_bytes, deadline)
}

/// What an embedder's codec returns for either call: the bytes and their
/// size, and for a transform its report and the extension a `to` without
/// one gains.
#[derive(Debug, Clone, PartialEq)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub report: Option<String>,
    pub extension: Option<&'static str>,
}

/// One request to the codec as an embedder receives it (Ibex's
/// `ImageRequest`): a compression in `format` within its limits, whose
/// `options` may carry an edit; or a transform, whose `options` are
/// [`edit::parse_transform`]'s and whose output is at most `max_bytes`.
pub fn encode_request(
    source: &[u8],
    transform_call: bool,
    options: Option<&str>,
    format: Format,
    (max_dimension, max_bytes): (u32, u64),
    deadline: Deadline<'_>,
) -> Result<Encoded, String> {
    let call = if transform_call {
        "transformImage"
    } else {
        "compressImage"
    };
    let options: serde_json::Value = match options {
        Some(text) => serde_json::from_str(text)
            .map_err(|e| edit::failure_named(call, "invalid", format!("options: {e}")))?,
        None => serde_json::Value::Null,
    };
    if transform_call {
        let t = edit::parse_transform(&options)?;
        let out = transform(source, &t, max_bytes, deadline)?;
        return Ok(Encoded {
            report: Some(out.report().to_string()),
            extension: edit::extension(out.media_type),
            bytes: out.bytes,
            width: out.width,
            height: out.height,
        });
    }
    let edit = edit::parse_edit(call, &options)?;
    let out = compress_edited(source, format, max_dimension, max_bytes, &edit, deadline)?;
    Ok(Encoded {
        bytes: out.bytes,
        width: out.width,
        height: out.height,
        report: None,
        extension: None,
    })
}

/// The refusal where there is no codec (A1.3).
pub fn unsupported() -> String {
    failure("unsupported", "no image encoder on this host")
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
