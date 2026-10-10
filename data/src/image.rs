//! `storage.fs.compressImage` (LLP 1069.002 Amendment A1): decode an image,
//! scale it to a pixel budget and encode it as a JPEG within a byte budget.
//!
//! The search is Bluesky's (`social-app` `compress.ts`), restated in A1.2
//! and kept as a pure function over an encoder ([`search`]) so every host's
//! trials can be checked against one recorded sequence. The codec is the
//! platform's: ImageIO on Apple ([`compress`]); elsewhere there is no JPEG
//! encoder in the tree and the call is refused (`unsupported`, A1.3).
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

/// What the codec made: JPEG bytes and their pixel size.
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

/// When the codec must stop starting work: checked before the decode and
/// before each trial (a trial already running finishes).
pub struct Deadline(pub Instant);

impl Deadline {
    pub fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.0 {
            return Err(failure("timeout", "the search ran past its time"));
        }
        Ok(())
    }
}

/// Decode `source`, then [`search`] with the platform's JPEG encoder,
/// starting no decode or trial at or after `deadline` (`timeout`).
/// `max_dimension` and `max_bytes` are as [`check_limits`] admits.
pub fn compress(
    source: &[u8],
    max_dimension: u32,
    max_bytes: u64,
    deadline: Instant,
) -> Result<Compressed, String> {
    check_limits(f64::from(max_dimension), max_bytes as f64)?;
    if source.len() as u64 > MAX_BYTES {
        return Err(failure(
            "too-large",
            format!("{} bytes is over {MAX_BYTES}", source.len()),
        ));
    }
    platform(source, max_dimension, max_bytes, Deadline(deadline))
}

#[cfg(target_vendor = "apple")]
fn platform(
    source: &[u8],
    max_dimension: u32,
    max_bytes: u64,
    deadline: Deadline,
) -> Result<Compressed, String> {
    apple::compress(source, max_dimension, max_bytes, deadline)
}

#[cfg(not(target_vendor = "apple"))]
fn platform(_: &[u8], _: u32, _: u64, _: Deadline) -> Result<Compressed, String> {
    Err(unsupported())
}

/// The refusal where there is no codec (A1.3).
pub fn unsupported() -> String {
    failure("unsupported", "no JPEG encoder on this host")
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
