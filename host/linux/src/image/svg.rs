//! An SVG file as an image on Linux (LLP 1011.001 D4): sniffed by its bytes,
//! its natural size the root tag's (D1), planned at the box's own pixels
//! rather than under the natural size, parsed and resolved by
//! `exact-svg-file` on the decode worker, and painted as an island is
//! (`Painter::svg_picture`): on the worker when the picture has no text,
//! on the paint thread, where the session's fonts are, when it has (D3).
use super::png_decode::{DecodePlan, Header};
use crate::text::Shared;
use exact_kernel::svg::Scene;
use exact_kernel::ObjectFit;
use exact_raster::{DecodePermit, Metadata, PixelSize, Refusal, ResidentBytes, MAX_HEADER_BYTES};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use tiny_skia::Pixmap;

/// The largest side a picture is drawn at, in device pixels: the island's.
pub(super) const MAX_SIDE: u32 = 4096;

/// What a decode of this picture asks beyond its plan: how it fits its
/// box, and the device pixels per point, which with the plan's pixels
/// gives the CSS size the scene resolves at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Ask {
    pub fit: ObjectFit,
    pub scale: f32,
}

impl Default for Ask {
    fn default() -> Self {
        Ask {
            fit: ObjectFit::Fill,
            scale: 1.0,
        }
    }
}

/// Whether the first bytes could begin an XML document (a BOM, white space,
/// then `<`): what earns the prefix read, which a raster's header never
/// pays (metadata inspection allocates nothing for one).
pub(super) fn could_be(head: &[u8]) -> bool {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    head.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b'<')
}

/// The header of an SVG document, from a prefix of at most
/// [`MAX_HEADER_BYTES`]: its natural size in CSS pixels taken as layout
/// units, rounded up to whole pixels. `Ok(None)` when the bytes are not an
/// SVG's.
pub(super) fn inspect(
    input: &mut (impl Read + Seek),
    encoded_bytes: u64,
) -> Result<Option<Header>, Refusal> {
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| Refusal::DecodeFailed)?;
    let want = encoded_bytes.min(MAX_HEADER_BYTES) as usize;
    let mut prefix = vec![0; want];
    let mut read = 0;
    while read < want {
        match input.read(&mut prefix[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(_) => return Err(Refusal::DecodeFailed),
        }
    }
    prefix.truncate(read);
    if !exact_svg_file::sniff(&prefix) {
        return Ok(None);
    }
    // An SVG whose root tag is past the prefix has no readable size.
    let Some(natural) = exact_svg_file::natural(&prefix) else {
        return Err(Refusal::HeaderLimit);
    };
    let (w, h) = natural.size();
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(Refusal::InvalidDimensions);
    }
    let (width, height) = (w.ceil() as u32, h.ceil() as u32);
    if u64::from(width) * u64::from(height) > exact_raster::MAX_SOURCE_PIXELS {
        return Err(Refusal::SourcePixels);
    }
    Ok(Some(Header::svg(Metadata {
        natural: PixelSize { width, height },
        encoded_bytes,
        header_bytes: read as u64,
    })))
}

/// The pixels a picture is drawn at for a box of `desired` device pixels
/// (D1): under `fill` the box's own two axes; under the other fits the
/// natural ratio at the size `object-fit` places it, `none` at the natural
/// size in device pixels; a box not laid out yet at the natural size.
/// Each side at most [`MAX_SIDE`]; the budget halves it as a raster's plan.
pub(super) fn plan(
    header: Header,
    desired: (f32, f32),
    ask: Ask,
    available: u64,
) -> Result<DecodePlan, Refusal> {
    let natural = header.metadata.natural;
    let (nw, nh) = (natural.width as f32, natural.height as f32);
    let laid_out = desired.0 > 0.0 && desired.1 > 0.0;
    let (w, h) = match ask.fit {
        _ if !laid_out => (nw * ask.scale, nh * ask.scale),
        ObjectFit::Fill => desired,
        ObjectFit::None => (nw * ask.scale, nh * ask.scale),
        ObjectFit::Contain | ObjectFit::ScaleDown | ObjectFit::Cover => {
            let (sx, sy) = (desired.0 / nw, desired.1 / nh);
            let s = match ask.fit {
                ObjectFit::Cover => sx.max(sy),
                ObjectFit::ScaleDown => sx.min(sy).min(ask.scale),
                _ => sx.min(sy),
            };
            (nw * s, nh * s)
        }
    };
    let mut pixels = (
        (w.ceil().max(1.0) as u32).min(MAX_SIDE),
        (h.ceil().max(1.0) as u32).min(MAX_SIDE),
    );
    loop {
        let candidate = DecodePlan::new(header, pixels);
        if candidate
            .as_ref()
            .is_ok_and(|p| p.peak_bytes() <= available)
            || pixels == (1, 1)
        {
            return candidate;
        }
        pixels = (pixels.0.div_ceil(2), pixels.1.div_ceil(2));
    }
}

/// Whether a box's new size wants another picture: either axis past a 25%
/// band of the drawn one (under `fill` a ratio change is a different
/// picture), as a raster's re-ask is judged on its longest side.
pub(super) fn resize_changes(
    header: Header,
    ask: Ask,
    previous: (f32, f32),
    next: (f32, f32),
) -> bool {
    if previous == next {
        return false;
    }
    let (Ok(before), Ok(after)) = (
        plan(header, previous, ask, exact_raster::SESSION_BYTES),
        plan(header, next, ask, exact_raster::SESSION_BYTES),
    ) else {
        return false;
    };
    let past =
        |b: u32, a: u32| u64::from(a) * 4 > u64::from(b) * 5 || u64::from(a) * 2 < u64::from(b);
    past(before.pixels.width, after.pixels.width) || past(before.pixels.height, after.pixels.height)
}

/// A picture parsed and resolved on the worker, waiting for the paint
/// thread to draw it (D3: its text takes the session's fonts).
pub(super) struct Pending {
    pub permit: DecodePermit,
    pub scene: Scene,
    pub css: (f32, f32),
    pub scale: f32,
    pub natural: (u32, u32),
    pub source: Arc<super::workers::SourceOwner>,
}

/// What the worker made of a picture.
pub(super) enum Decoded {
    /// Painted, on the worker.
    Pixels(Pixmap),
    /// Resolved; the paint thread paints it.
    Later(Scene),
}

/// Parse `bytes` and resolve the scene at the CSS size the plan's pixels
/// are at `scale`, then paint it here unless it has text.
pub(super) fn decode(
    name: &str,
    bytes: &[u8],
    plan: &DecodePlan,
    scale: f32,
    cancelled: impl Fn() -> bool,
) -> Result<Decoded, Refusal> {
    // (A link's query, a CDN's signature, is not logged: the source's, or
    // one a diagnostic quotes.)
    let log = |line: String| eprintln!("{}", super::remote::without_queries(&line));
    let text = std::str::from_utf8(bytes).map_err(|_| {
        log(format!("exact: image `{name}`: not UTF-8 text"));
        Refusal::DecodeFailed
    })?;
    let loaded = exact_svg_file::load(text).map_err(|e| {
        log(format!("exact: image `{name}`: {e}"));
        Refusal::DecodeFailed
    })?;
    for warning in &loaded.document.warnings {
        log(format!("exact: image `{name}`: {warning}"));
    }
    if cancelled() {
        return Err(Refusal::Stale);
    }
    let css = css_size(plan.pixels, scale);
    let scene = loaded.scene(css.0, css.1);
    if let Some(limit) = scene.over {
        log(format!("exact: image `{name}`: {limit}"));
        return Err(Refusal::DecodeFailed);
    }
    if loaded.document.text {
        return Ok(Decoded::Later(scene));
    }
    WORKER_TEXT
        .with(|engine| paint(engine.clone(), &scene, css, scale, plan.pixels))
        .map(Decoded::Pixels)
}

/// The CSS size a plan's pixels are at `scale` device pixels per point.
pub(super) fn css_size(pixels: PixelSize, scale: f32) -> (f32, f32) {
    (pixels.width as f32 / scale, pixels.height as f32 / scale)
}

thread_local! {
    /// A worker's own text engine: a painter needs one, and a picture
    /// without text asks it for nothing.
    static WORKER_TEXT: Shared = crate::text::TextEngine::shared();
}

/// `scene` painted into `pixels`, exactly.
pub(super) fn paint(
    text: Shared,
    scene: &Scene,
    css: (f32, f32),
    scale: f32,
    pixels: PixelSize,
) -> Result<Pixmap, Refusal> {
    let drawn = crate::paint::Painter::svg_picture(text, scene, css, scale).map_err(|e| {
        let line = format!("exact: an SVG image did not draw: {e}");
        eprintln!("{}", super::remote::without_queries(&line));
        Refusal::DecodeFailed
    })?;
    if (drawn.width(), drawn.height()) != (pixels.width, pixels.height) {
        return Err(Refusal::ActualExceedsReservation);
    }
    Ok(drawn)
}

/// Finish a pending picture on the paint thread: paint it with the
/// session's text engine and complete its permit as the worker completes a
/// raster's.
pub(super) fn finish(text: &Shared, pending: Pending) {
    let Pending {
        permit,
        scene,
        css,
        scale,
        natural,
        source,
    } = pending;
    if permit.is_cancelled() {
        permit.fail();
        return;
    }
    let pixels = permit.key().pixels;
    // This charge precedes the painter's allocation of the picture.
    let charge = permit.allocation_charge();
    match paint(text.clone(), &scene, css, scale, pixels) {
        Ok(drawn) => {
            let bitmap = Arc::new(super::Bitmap::from_source(
                drawn.into(),
                natural,
                exact_raster::Crop::default(),
                charge,
                source,
            ));
            let bytes = bitmap.bytes();
            let _ = permit.complete(
                bitmap,
                ResidentBytes {
                    output: bytes,
                    copy: 0,
                },
            );
        }
        Err(_) => permit.fail(),
    }
}
