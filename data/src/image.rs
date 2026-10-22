//! `storage.fs.compressImage` (LLP 1069.002 Amendment A1): decode an image,
//! scale it to a pixel budget and encode it as a JPEG, or a PNG that keeps
//! its alpha (A1.11), within a byte budget.
//!
//! The JPEG search is Bluesky's (`social-app` `compress.ts`), restated in
//! A1.2 and kept as a pure function over an encoder ([`search`]) so every
//! host's trials can be checked against one recorded sequence; a PNG has no
//! quality, so it only shrinks ([`search_lossless`]). The codec is the
//! platform's: ImageIO on Apple ([`compress`]); elsewhere there is no encoder
//! in the tree and the call is refused (`unsupported`, A1.3).
use std::time::Instant;

#[cfg(target_vendor = "apple")]
mod apple;

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
/// `maxDimension` omitted with `type: "image/png"` (LLP 1133.006 D1, "keep
/// the size"): the image is written at its own size, and `maxBytes` refuses
/// rather than shrinks.
pub const KEEP_SIZE: u32 = 0;
/// The longest side an image keeps (the codec service's `max_dimension`):
/// one longer is `too-large`.
pub const MAX_KEPT_DIMENSION: u32 = 16_384;

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
    check_options(Format::Jpeg, max_dimension, max_bytes)
}

/// [`check_limits`] for `format`: a PNG's `max_dimension` may be
/// [`KEEP_SIZE`].
pub fn check_options(
    format: Format,
    max_dimension: f64,
    max_bytes: f64,
) -> Result<(u32, u64), String> {
    let integer = |n: f64, max: f64| n.fract() == 0.0 && (1.0..=max).contains(&n);
    let kept = format == Format::Png && max_dimension == f64::from(KEEP_SIZE);
    if !kept && !integer(max_dimension, MAX_DIMENSION as f64) {
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

/// `maxPixels` (LLP 1133.006 D1): an integer from 1 to [`MAX_SOURCE_PIXELS`].
pub fn check_pixels(max_pixels: f64) -> Result<u64, String> {
    if max_pixels.fract() != 0.0 || !(1.0..=MAX_SOURCE_PIXELS as f64).contains(&max_pixels) {
        return Err(failure(
            "invalid",
            format!("maxPixels must be an integer from 1 to {MAX_SOURCE_PIXELS}"),
        ));
    }
    Ok(max_pixels as u64)
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
    // Kept at its own size: one trial, and no shrinking.
    if max_dimension == KEEP_SIZE {
        let bytes = encode(width, height)?;
        return Ok((bytes.len() as u64 <= max_bytes).then_some(Compressed {
            bytes,
            width,
            height,
        }));
    }
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
    at: Instant,
    check: &'a dyn Fn() -> Result<(), String>,
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
    check_options(format, f64::from(max_dimension), max_bytes as f64)?;
    check()?;
    if source.len() as u64 > MAX_BYTES {
        return Err(failure(
            "too-large",
            format!("{} bytes is over {MAX_BYTES}", source.len()),
        ));
    }
    platform(
        source,
        format,
        max_dimension,
        max_bytes,
        Deadline {
            at: deadline,
            check,
            max_pixels: max_pixels.min(MAX_SOURCE_PIXELS),
            max_scratch_bytes,
        },
    )
}

#[cfg(target_vendor = "apple")]
fn platform(
    source: &[u8],
    format: Format,
    max_dimension: u32,
    max_bytes: u64,
    deadline: Deadline<'_>,
) -> Result<Compressed, String> {
    let source = still(source, deadline.max_pixels)?;
    apple::compress(&source, format, max_dimension, max_bytes, deadline)
}

/// What the codec decodes: an SVG rasterized at its own size by the host
/// (LLP 1133.006 D1, "SVG input"); an animated PNG as its first frame, which
/// ImageIO does not select ([`png_first_frame`]); else `source` itself.
pub fn still(source: &[u8], max_pixels: u64) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    use std::borrow::Cow;
    if is_svg(source) {
        let rasterize = SVG_RASTERIZER
            .get()
            .ok_or_else(|| failure("undecodable", "this host rasterizes no SVG"))?;
        return rasterize(source, max_pixels).map(Cow::Owned);
    }
    // A PNG's header names its size: past the bound it is refused before
    // ImageIO reads anything, as the web refuses it.
    if let Some((w, h)) = png_size(source) {
        if u64::from(w) * u64::from(h) > max_pixels {
            return Err(failure(
                "too-large",
                format!("{w}×{h} pixels is over {max_pixels}"),
            ));
        }
    }
    match png_first_frame(source) {
        Some(frame) => Ok(frame),
        None => Err(failure(
            "undecodable",
            "an animated PNG whose first frame is malformed",
        )),
    }
}

/// Rasterizes an SVG at its own size, as an `img` with no CSS size draws it,
/// into a PNG; `too-large` past the pixel bound, before drawing.
pub type SvgRasterizer = fn(&[u8], u64) -> Result<Vec<u8>, String>;
static SVG_RASTERIZER: std::sync::OnceLock<SvgRasterizer> = std::sync::OnceLock::new();

/// The host's SVG rasterizer (Apple: the one `img` draws with, LLP
/// 1011.001), set once at its start.
pub fn set_svg_rasterizer(rasterize: SvgRasterizer) {
    let _ = SVG_RASTERIZER.set(rasterize);
}

/// Whether `bytes` read as an SVG document: its first element, past an XML
/// declaration, comments and a doctype, is `svg`.
pub fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(4096)];
    let Ok(text) =
        std::str::from_utf8(head).or_else(|e| std::str::from_utf8(&head[..e.valid_up_to()]))
    else {
        return false;
    };
    let mut rest = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        if let Some(r) = rest.strip_prefix("<?") {
            rest = r.split_once("?>").map_or("", |(_, r)| r).trim_start();
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.split_once("-->").map_or("", |(_, r)| r).trim_start();
        } else if let Some(r) = rest.strip_prefix("<!") {
            rest = r.split_once('>').map_or("", |(_, r)| r).trim_start();
        } else {
            break;
        }
    }
    rest.strip_prefix("<svg")
        .is_some_and(|r| r.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/'))
}

/// An animated PNG as a still PNG of its first frame (LLP 1133.006 D1):
/// Chromium writes that frame, and skips a default image (the `IDAT`) that is
/// not part of the animation. The chunks before the image data, then frame
/// 0's data as `IDAT`; frame 0 covers the canvas, so the header stays. A
/// still PNG, or anything else, is itself; `None` for a malformed animation.
pub fn png_first_frame(bytes: &[u8]) -> Option<std::borrow::Cow<'_, [u8]>> {
    use std::borrow::Cow;
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) {
        return Some(Cow::Borrowed(bytes));
    }
    let mut chunks: Vec<(&[u8], &[u8])> = Vec::new();
    let mut at = SIGNATURE.len();
    while at + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
        let end = at.checked_add(12)?.checked_add(length)?;
        if end > bytes.len() {
            return None;
        }
        let kind = &bytes[at + 4..at + 8];
        chunks.push((kind, &bytes[at + 8..at + 8 + length]));
        at = end;
        if kind == b"IEND" {
            break;
        }
    }
    let Some(image) = chunks.iter().position(|(k, _)| *k == b"IDAT") else {
        return Some(Cow::Borrowed(bytes)); // no image data: the decoder refuses it
    };
    if !chunks[..image].iter().any(|(k, _)| *k == b"acTL") {
        return Some(Cow::Borrowed(bytes));
    }
    let data: Vec<&[u8]> = if chunks[..image].iter().any(|(k, _)| *k == b"fcTL") {
        chunks[image..]
            .iter()
            .take_while(|(k, _)| *k == b"IDAT")
            .map(|(_, d)| *d)
            .collect()
    } else {
        let control = chunks.iter().position(|(k, _)| *k == b"fcTL")?;
        chunks[control + 1..]
            .iter()
            .take_while(|(k, _)| *k == b"fdAT")
            .filter(|(_, d)| d.len() > 4)
            .map(|(_, d)| &d[4..])
            .collect()
    };
    if data.is_empty() {
        return None;
    }
    let mut out = SIGNATURE.to_vec();
    let mut put = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    };
    for (kind, d) in chunks[..image]
        .iter()
        .filter(|(k, _)| *k != b"acTL" && *k != b"fcTL")
    {
        put(kind, d);
    }
    for d in data {
        put(b"IDAT", d);
    }
    put(b"IEND", &[]);
    Some(Cow::Owned(out))
}

/// A PNG's width and height, from its header.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let header = bytes.strip_prefix(b"\x89PNG\r\n\x1a\n")?;
    (header.len() >= 16 && &header[4..8] == b"IHDR").then(|| {
        let n = |at: usize| {
            u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
        };
        (n(8), n(12))
    })
}

/// The PNG chunk CRC (ISO 3309).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(not(target_vendor = "apple"))]
fn platform(_: &[u8], _: Format, _: u32, _: u64, _: Deadline<'_>) -> Result<Compressed, String> {
    Err(unsupported())
}

/// The refusal where there is no codec (A1.3).
pub fn unsupported() -> String {
    failure("unsupported", "no image encoder on this host")
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
