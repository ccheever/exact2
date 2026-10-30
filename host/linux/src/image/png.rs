//! PNG row sampling. The caller reserves `cost` before constructing a decoder.
//! Natural dimensions never depend on the sampled raster's resolution.
use exact_raster::{
    Crop, DecodeCost, Metadata, PixelSize, Refusal, MAX_ENCODED_BYTES, MAX_HEADER_BYTES,
    MAX_SOURCE_PIXELS, SESSION_BYTES,
};
use std::io::{BufReader, Read, Seek, SeekFrom};
use tiny_skia::{IntSize, Pixmap};

#[derive(Clone, Copy, Debug)]
pub(super) struct Header {
    pub metadata: Metadata,
    depth: u8,
    channels: u8,
    color: u8,
    interlaced: bool,
    #[cfg(windows)]
    pub(super) orientation: u8,
}

/// `color` for a JPEG, GIF or WebP (not a PNG colour type); its pixels come
/// from `jpeg_decode`, the platform's decoder.
const JPEG: u8 = 0xFF;
/// `color` for an SVG document (LLP 1011.001): its pixels are painted from
/// the resolved scene (`super::svg_picture`).
const SVG: u8 = 0xFE;

impl Header {
    /// Whether [`DecodePlan::new`] snaps this source's decode to a
    /// power-of-two subsample (Android's platform decoder): a request for it
    /// is then the size wanted, not a size rounded up to a bucket, which
    /// could push it past a subsample to the full size (heavy's 1600×900
    /// photos in a 433-pixel square: an 896-pixel bucket, so a full decode
    /// where a half-size one covers it).
    pub(super) fn snaps(&self) -> bool {
        // Read once: a plan is made at every poll, for every picture.
        static EXACT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        cfg!(target_os = "android")
            && self.color == JPEG
            && !*EXACT.get_or_init(|| std::env::var_os("EXACT_EXACT_DECODE").is_some())
    }

    /// A JPEG's, GIF's or WebP's header: decoded as 8-bit RGBA by the platform
    /// (a GIF's or WebP's first frame).
    pub(super) fn jpeg(metadata: Metadata) -> Header {
        Header {
            metadata,
            depth: 8,
            channels: 4,
            color: JPEG,
            interlaced: false,
            #[cfg(windows)]
            orientation: 1,
        }
    }

    /// An SVG's header: its natural size the document's (LLP 1011.001 D1),
    /// its pixels painted at any size up to the island's.
    pub(super) fn svg(metadata: Metadata) -> Header {
        Header {
            metadata,
            depth: 8,
            channels: 4,
            color: SVG,
            interlaced: false,
            #[cfg(windows)]
            orientation: 1,
        }
    }

    /// Whether the picture is an SVG document, drawn rather than decoded.
    pub(super) fn is_svg(&self) -> bool {
        self.color == SVG
    }

    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    pub(super) fn oriented(mut self, orientation: u8) -> Self {
        // WIC turns the decode by it; Android's decoders turn it themselves.
        #[cfg(windows)]
        {
            self.orientation = orientation;
        }
        if orientation >= 5 {
            std::mem::swap(
                &mut self.metadata.natural.width,
                &mut self.metadata.natural.height,
            );
        }
        self
    }
}

/// Metadata inspection allocates no image/decoder buffers and inspects at most
/// 256 KiB before IDAT. Encoded length is checked before any file buffering.
pub(super) fn inspect(
    input: &mut (impl Read + Seek),
    encoded_bytes: u64,
) -> Result<Header, Refusal> {
    if encoded_bytes > MAX_ENCODED_BYTES {
        return Err(Refusal::EncodedLimit);
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| Refusal::DecodeFailed)?;
    // As much of the first 33 bytes as there are: an SVG may be shorter.
    let mut head = [0; 33];
    let mut got = 0;
    while got < head.len() {
        match input.read(&mut head[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(_) => return Err(Refusal::DecodeFailed),
        }
    }
    let head = &head[..got];
    if super::jpeg_decode::is_jpeg(head) {
        return super::jpeg_decode::inspect(input, encoded_bytes);
    }
    if let Some(header) = super::jpeg_decode::inspect_animated(head, encoded_bytes)? {
        return Ok(header);
    }
    if super::svg_picture::could_be(head) {
        if let Some(header) = super::svg_picture::inspect(input, encoded_bytes)? {
            return Ok(header);
        }
        // The SVG read took a prefix: the chunk walk starts after the head.
        input
            .seek(SeekFrom::Start(got as u64))
            .map_err(|_| Refusal::DecodeFailed)?;
    }
    if head.len() < 33
        || &head[..8] != b"\x89PNG\r\n\x1a\n"
        || head[8..12] != 13u32.to_be_bytes()
        || &head[12..16] != b"IHDR"
        || crc(&head[12..29]) != u32::from_be_bytes(head[29..33].try_into().unwrap())
    {
        return Err(Refusal::DecodeFailed);
    }
    let width = u32::from_be_bytes(head[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(head[20..24].try_into().unwrap());
    if width == 0 || height == 0 {
        return Err(Refusal::InvalidDimensions);
    }
    if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
        return Err(Refusal::SourcePixels);
    }
    let depth = head[24];
    let channels = match head[25] {
        0 if [1, 2, 4, 8, 16].contains(&depth) => 1,
        2 if [8, 16].contains(&depth) => 3,
        3 if [1, 2, 4, 8].contains(&depth) => 1,
        4 if [8, 16].contains(&depth) => 2,
        6 if [8, 16].contains(&depth) => 4,
        _ => return Err(Refusal::DecodeFailed),
    };
    if head[26] != 0 || head[27] != 0 || head[28] > 1 {
        return Err(Refusal::DecodeFailed);
    }
    let mut at = 33u64;
    loop {
        if at + 8 > MAX_HEADER_BYTES {
            return Err(Refusal::HeaderLimit);
        }
        let mut chunk = [0; 8];
        input
            .read_exact(&mut chunk)
            .map_err(|_| Refusal::DecodeFailed)?;
        let len = u64::from(u32::from_be_bytes(chunk[..4].try_into().unwrap()));
        let end = at
            .checked_add(12)
            .and_then(|n| n.checked_add(len))
            .ok_or(Refusal::Overflow)?;
        if end > encoded_bytes {
            return Err(Refusal::DecodeFailed);
        }
        if &chunk[4..] == b"IDAT" {
            return Ok(Header {
                metadata: Metadata {
                    natural: PixelSize { width, height },
                    encoded_bytes,
                    header_bytes: at + 8,
                },
                depth,
                channels,
                color: head[25],
                interlaced: head[28] == 1,
                #[cfg(windows)]
                orientation: 1,
            });
        }
        if end > MAX_HEADER_BYTES {
            return Err(Refusal::HeaderLimit);
        }
        if &chunk[4..] == b"IEND" {
            return Err(Refusal::DecodeFailed);
        }
        input
            .seek(SeekFrom::Start(end))
            .map_err(|_| Refusal::DecodeFailed)?;
        at = end;
    }
}

fn crc(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DecodePlan {
    pub header: Header,
    /// The picture's pixels, as cached and charged.
    pub pixels: PixelSize,
    pub cost: DecodeCost,
    /// The part of the picture `pixels` are, when not all of it
    /// ([`DecodePlan::part`]).
    pub crop: Crop,
}
impl DecodePlan {
    /// Whether the platform's decoder decodes this (JPEG, GIF, WebP).
    #[cfg(target_os = "android")]
    pub(super) fn platform(&self) -> bool {
        self.header.color == JPEG
    }
    pub fn new(header: Header, pixels: (u32, u32)) -> Result<Self, Refusal> {
        let natural = header.metadata.natural;
        if header.is_svg() {
            return Self::svg(header, pixels);
        }
        if pixels.0 == 0 || pixels.1 == 0 || pixels.0 > natural.width || pixels.1 > natural.height {
            return Err(Refusal::InvalidDimensions);
        }
        let raw_row =
            (u64::from(natural.width) * u64::from(header.channels) * u64::from(header.depth))
                .div_ceil(8)
                + 1;
        // png 0.18.1: UnfilteringBuffer shifts after max(4*raw_row,128KiB),
        // retains zlib's 32KiB lookback, grows Vec geometrically, and may own
        // two filter rows. Reserve 32 rows plus 1MiB for those capacities,
        // transformed row, bounded metadata, inflater tables and BufReader.
        // `Limits` only covers SOME png allocations; it is not this ledger.
        let work_row = raw_row.max(u64::from(natural.width) * 4);
        let scratch = work_row
            .checked_mul(32)
            .and_then(|n| n.checked_add(1024 * 1024))
            .ok_or(Refusal::Overflow)?;
        // A JPEG's platform decode holds the whole file and its own row
        // buffers (two source rows of 4 bytes a pixel) beside the output.
        let scratch = if header.color == JPEG {
            header
                .metadata
                .encoded_bytes
                .checked_add(u64::from(natural.width) * 8 + 1024 * 1024)
                .ok_or(Refusal::Overflow)?
        } else {
            scratch
        };
        let scratch = scratch
            .checked_add(area_scratch(header, pixels))
            .ok_or(Refusal::Overflow)?;
        // A WIC flip/rotation may keep the scaled frame for out-of-order rows.
        #[cfg(windows)]
        let scratch = if header.orientation != 1 {
            scratch
                .checked_add(u64::from(pixels.0) * u64::from(pixels.1) * 4)
                .ok_or(Refusal::Overflow)?
        } else {
            scratch
        };
        // WIC codecs may cache a complete source frame (notably GIF/WebP).
        // Charge that backing even when the requested thumbnail is small.
        #[cfg(windows)]
        let scratch = if header.color == JPEG {
            scratch
                .checked_add(u64::from(natural.width) * u64::from(natural.height) * 4)
                .ok_or(Refusal::Overflow)?
        } else {
            scratch
        };
        // ImageIO may retain a full source decode, a sampled thumbnail and
        // CFData's copy of the encoded bytes alongside the crop's output.
        #[cfg(target_os = "macos")]
        let scratch = if header.color == JPEG {
            scratch
                .checked_add(u64::from(natural.width) * u64::from(natural.height) * 4)
                .and_then(|n| n.checked_add(u64::from(pixels.0) * u64::from(pixels.1) * 4))
                .and_then(|n| n.checked_add(header.metadata.encoded_bytes))
                .ok_or(Refusal::Overflow)?
        } else {
            scratch
        };
        // Android's platform decoder subsamples a JPEG by powers of two in the
        // DCT: the largest power-of-two subsample that still covers the
        // request is what it decodes. Kept at that size when it is near the
        // request; otherwise scaled to the request by the decoder (bilinear,
        // a ratio under 2, beside that subsample in its own buffer), as an
        // image loader scales its bitmap to the view: a cached picture two to
        // four times the size it shows at made the cache hold that many
        // fewer, and the list decode them again (heavy: 309 decodes a fling
        // against Views' 132).
        let (pixels, scratch) = if header.snaps() {
            let mut s = 1u32;
            while natural.width.div_ceil(s * 2) >= pixels.0
                && natural.height.div_ceil(s * 2) >= pixels.1
                && s < 8
            {
                s *= 2;
            }
            let decode = (natural.width.div_ceil(s), natural.height.div_ceil(s));
            let near = u64::from(decode.0) * u64::from(decode.1) * 10
                <= u64::from(pixels.0) * u64::from(pixels.1) * 13;
            if near || std::env::var_os("EXACT_NO_RESAMPLE").is_some() {
                (decode, scratch)
            } else {
                let buffer = u64::from(decode.0) * 4 * u64::from(decode.1);
                (
                    pixels,
                    scratch.checked_add(buffer).ok_or(Refusal::Overflow)?,
                )
            }
        } else {
            (pixels, scratch)
        };
        let cost = DecodeCost::checked(u64::from(pixels.0) * 4, pixels.1, scratch, 0)?;
        if cost.peak()? > SESSION_BYTES {
            return Err(Refusal::TooLarge);
        }
        Ok(Self {
            header,
            pixels: PixelSize {
                width: pixels.0,
                height: pixels.1,
            },
            cost,
            crop: Crop::default(),
        })
    }
    /// An SVG's plan (LLP 1011.001 D1): drawn at `pixels`, which may exceed
    /// the natural size, up to the island's side. The output is the
    /// painter's own pixmap; the scratch bounds the parse (the document's
    /// text, its ops and the scratch kernel's nodes: a few times the file).
    fn svg(header: Header, pixels: (u32, u32)) -> Result<Self, Refusal> {
        let side = super::svg_picture::MAX_SIDE;
        if pixels.0 == 0 || pixels.1 == 0 || pixels.0 > side || pixels.1 > side {
            return Err(Refusal::InvalidDimensions);
        }
        let scratch = header
            .metadata
            .encoded_bytes
            .checked_mul(4)
            .and_then(|n| n.checked_add(1024 * 1024))
            .ok_or(Refusal::Overflow)?;
        let cost = DecodeCost::checked(u64::from(pixels.0) * 4, pixels.1, scratch, 0)?;
        if cost.peak()? > SESSION_BYTES {
            return Err(Refusal::TooLarge);
        }
        Ok(Self {
            header,
            pixels: PixelSize {
                width: pixels.0,
                height: pixels.1,
            },
            cost,
            crop: Crop::default(),
        })
    }
    /// This plan cut to `pixels` of it from (`x`, `y`): the same decode,
    /// which keeps only that part (what a box that clips the picture
    /// shows). The scratch is the whole decode's; the output is the part.
    pub fn part(self, x: u32, y: u32, pixels: (u32, u32)) -> Result<Self, Refusal> {
        let full = self.pixels;
        if !self.crop.whole()
            || pixels.0 == 0
            || pixels.1 == 0
            || x.checked_add(pixels.0).is_none_or(|r| r > full.width)
            || y.checked_add(pixels.1).is_none_or(|b| b > full.height)
        {
            return Err(Refusal::InvalidDimensions);
        }
        if (pixels.0, pixels.1) == (full.width, full.height) {
            return Ok(self);
        }
        let cost = DecodeCost::checked(
            u64::from(pixels.0) * 4,
            pixels.1,
            self.cost.scratch_bytes,
            0,
        )?;
        Ok(Self {
            pixels: PixelSize {
                width: pixels.0,
                height: pixels.1,
            },
            cost,
            crop: Crop { x, y, full },
            ..self
        })
    }
    /// The plan a key names: its pixels, of the whole picture or a part.
    pub fn of_key(header: Header, key: &exact_raster::RasterKey) -> Result<Self, Refusal> {
        if key.crop.whole() {
            return Self::new(header, (key.pixels.width, key.pixels.height));
        }
        let full = Self::new(header, (key.crop.full.width, key.crop.full.height))?;
        // The whole decode's size is the planner's own, not any size at all.
        if full.pixels != key.crop.full {
            return Err(Refusal::InvalidDimensions);
        }
        full.part(
            key.crop.x,
            key.crop.y,
            (key.pixels.width, key.pixels.height),
        )
    }
    /// The size the picture is decoded at: `pixels`, or the whole a part is
    /// cut from.
    pub fn full(self) -> PixelSize {
        if self.crop.whole() {
            self.pixels
        } else {
            self.crop.full
        }
    }
    pub fn natural(self) -> (u32, u32) {
        (
            self.header.metadata.natural.width,
            self.header.metadata.natural.height,
        )
    }
    #[cfg(test)]
    pub fn output_bytes(self) -> u64 {
        self.cost.output_bytes
    }
    pub fn peak_bytes(self) -> u64 {
        self.cost.peak().expect("checked plan")
    }
}

fn area_scratch(header: Header, pixels: (u32, u32)) -> u64 {
    let natural = header.metadata.natural;
    if header.color == JPEG || pixels == (natural.width, natural.height) {
        return 0;
    }
    let rows = if header.interlaced { pixels.1 } else { 2 };
    let bytes = u64::from(pixels.0) * u64::from(rows) * std::mem::size_of::<[u64; 4]>() as u64;
    // Large Adam7 sums must not force a smaller decode that is enlarged to draw.
    if header.interlaced && bytes > SESSION_BYTES / 4 {
        0
    } else {
        bytes
    }
}

const PASSES: [(u32, u32, u32, u32); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

pub(super) fn decode_rows<R: Read + Seek>(
    mut input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    if plan.header.color == JPEG {
        return super::jpeg_decode::decode(input, plan, cancelled);
    }
    if cancelled() {
        return Err(Refusal::Stale);
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| Refusal::DecodeFailed)?;
    let input = BufReader::with_capacity(
        8192,
        Bounded {
            input,
            at: 0,
            end: plan.header.metadata.encoded_bytes,
        },
    );
    let mut decoder = png::Decoder::new_with_limits(
        input,
        png::Limits {
            bytes: (MAX_HEADER_BYTES + u64::from(plan.header.metadata.natural.width) * 4) as usize,
        },
    );
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|_| Refusal::DecodeFailed)?;
    let (w, h) = plan.natural();
    let info = reader.info();
    if info.width != w
        || info.height != h
        || info.interlaced != plan.header.interlaced
        || info.bit_depth as u8 != plan.header.depth
        || info.color_type as u8 != plan.header.color
        || info
            .frame_control
            .as_ref()
            .is_some_and(|f| f.width != w || f.height != h)
    {
        return Err(Refusal::DecodeFailed);
    }
    let color = reader.output_color_type().0;
    let channels = match color {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return Err(Refusal::DecodeFailed),
    };
    let size = IntSize::from_wh(plan.pixels.width, plan.pixels.height)
        .ok_or(Refusal::InvalidDimensions)?;
    let mut out = Pixmap::new(size.width(), size.height()).ok_or(Refusal::DecodeFailed)?;
    let row_size = reader.output_line_size(w).ok_or(Refusal::Overflow)?;
    let mut row = vec![0; row_size];
    let full = plan.full();
    let scaled = full != plan.header.metadata.natural;
    let mut area = (!plan.header.interlaced && scaled).then(|| AreaRows::new(plan));
    let mut interlaced = (plan.header.interlaced
        && area_scratch(plan.header, (full.width, full.height)) != 0)
        .then(|| AreaPasses::new(plan));
    let noninterlaced = [(0, 0, 1, 1)];
    let passes = if plan.header.interlaced {
        &PASSES[..]
    } else {
        &noninterlaced[..]
    };
    for (pass, &(x0, y0, dx, dy)) in passes.iter().enumerate() {
        if x0 >= w {
            continue;
        }
        for (line, y) in (y0..h).step_by(dy as usize).enumerate() {
            if cancelled() {
                return Err(Refusal::Stale);
            }
            let interlace = reader
                .read_row(&mut row)
                .map_err(|_| Refusal::DecodeFailed)?
                .ok_or(Refusal::DecodeFailed)?;
            if let png::InterlaceInfo::Adam7(actual) = interlace {
                if actual != png::Adam7Info::new(pass as u8 + 1, line as u32, w) {
                    return Err(Refusal::DecodeFailed);
                }
            }
            if let Some(area) = &mut area {
                area.push(&row, channels, y, plan, &mut out);
                continue;
            }
            if let Some(area) = &mut interlaced {
                area.push(&row, channels, (x0, y, dx), plan);
                continue;
            }
            let oy = (u64::from(y) * u64::from(full.height) / u64::from(h)) as u32;
            for oy in oy.saturating_sub(1)..(oy + 2).min(full.height) {
                if sample(oy, full.height, h) != y
                    || oy < plan.crop.y
                    || oy - plan.crop.y >= plan.pixels.height
                {
                    continue;
                }
                for ox in 0..plan.pixels.width {
                    let sx = sample(ox + plan.crop.x, full.width, w);
                    if sx < x0 || !(sx - x0).is_multiple_of(dx) {
                        continue;
                    }
                    let i = ((sx - x0) / dx) as usize * channels;
                    let at = ((oy - plan.crop.y) as usize * plan.pixels.width as usize
                        + ox as usize)
                        * 4;
                    out.data_mut()[at..at + 4]
                        .copy_from_slice(&premultiplied(&row[i..i + channels]));
                }
            }
        }
    }
    if reader
        .read_row(&mut row)
        .map_err(|_| Refusal::DecodeFailed)?
        .is_some()
    {
        return Err(Refusal::DecodeFailed);
    }
    if cancelled() {
        return Err(Refusal::Stale);
    }
    if let Some(area) = interlaced {
        area.finish(plan, &mut out);
    }
    Ok(out)
}

fn sample(at: u32, output: u32, natural: u32) -> u32 {
    (((2 * u64::from(at) + 1) * u64::from(natural)) / (2 * u64::from(output))) as u32
}

fn premultiplied(px: &[u8]) -> [u8; 4] {
    let (r, g, b, a) = match px.len() {
        4 => (px[0], px[1], px[2], px[3]),
        3 => (px[0], px[1], px[2], 255),
        2 => (px[0], px[0], px[0], px[1]),
        _ => (px[0], px[0], px[0], 255),
    };
    let premul = |v| (u32::from(v) * u32::from(a) / 255) as u8;
    [premul(r), premul(g), premul(b), a]
}

struct AreaRows {
    horizontal: Vec<[u64; 4]>,
    accumulated: Vec<[u64; 4]>,
    next_y: u32,
}

impl AreaRows {
    fn new(plan: &DecodePlan) -> Self {
        Self {
            horizontal: vec![[0; 4]; plan.pixels.width as usize],
            accumulated: vec![[0; 4]; plan.pixels.width as usize],
            next_y: plan.crop.y,
        }
    }

    fn push(&mut self, row: &[u8], channels: usize, y: u32, plan: &DecodePlan, out: &mut Pixmap) {
        let (w, h) = plan.natural();
        let full = plan.full();
        let top = u64::from(y) * u64::from(full.height);
        let bottom = top + u64::from(full.height);
        let end_y = plan.crop.y + plan.pixels.height;
        if self.next_y >= end_y || bottom <= u64::from(self.next_y) * u64::from(h) {
            return;
        }
        // Integer overlap units keep fractional boundaries and cropped decodes identical.
        let dx = u64::from(full.width);
        for (x, sum) in self.horizontal.iter_mut().enumerate() {
            let left = (x as u64 + u64::from(plan.crop.x)) * u64::from(w);
            let right = left + u64::from(w);
            *sum = [0; 4];
            for sx in left / dx..right.div_ceil(dx) {
                let weight = right.min((sx + 1) * dx) - left.max(sx * dx);
                let at = sx as usize * channels;
                let px = premultiplied(&row[at..at + channels]);
                for c in 0..4 {
                    sum[c] += u64::from(px[c]) * weight;
                }
            }
        }
        while self.next_y < end_y && u64::from(self.next_y) * u64::from(h) < bottom {
            let end = u64::from(self.next_y + 1) * u64::from(h);
            let weight = bottom.min(end) - top.max(u64::from(self.next_y) * u64::from(h));
            for (sum, row) in self.accumulated.iter_mut().zip(&self.horizontal) {
                for c in 0..4 {
                    sum[c] += row[c] * weight;
                }
            }
            if end > bottom {
                break;
            }
            let stride = plan.pixels.width as usize * 4;
            let at = (self.next_y - plan.crop.y) as usize * stride;
            let area = u64::from(w) * u64::from(h);
            for (px, sum) in out.data_mut()[at..at + stride]
                .chunks_exact_mut(4)
                .zip(&mut self.accumulated)
            {
                for c in 0..4 {
                    px[c] = ((sum[c] + area / 2) / area) as u8;
                }
                *sum = [0; 4];
            }
            self.next_y += 1;
        }
    }
}

struct AreaPasses {
    accumulated: Vec<[u64; 4]>,
}

impl AreaPasses {
    fn new(plan: &DecodePlan) -> Self {
        Self {
            accumulated: vec![[0; 4]; plan.pixels.width as usize * plan.pixels.height as usize],
        }
    }

    fn push(
        &mut self,
        row: &[u8],
        channels: usize,
        (x0, y, dx): (u32, u32, u32),
        plan: &DecodePlan,
    ) {
        let (w, h) = plan.natural();
        let (w, h) = (u64::from(w), u64::from(h));
        let full = plan.full();
        let (fw, fh) = (u64::from(full.width), u64::from(full.height));
        let (cx, cy) = (u64::from(plan.crop.x), u64::from(plan.crop.y));
        let top = u64::from(y) * fh;
        let bottom = top + fh;
        let ys = (top / h).max(cy)..bottom.div_ceil(h).min(cy + u64::from(plan.pixels.height));
        if ys.is_empty() {
            return;
        }
        // Pass order is arbitrary; each source pixel contributes its integer area once.
        for (i, x) in (u64::from(x0)..w).step_by(dx as usize).enumerate() {
            let left = x * fw;
            let right = left + fw;
            let xs = (left / w).max(cx)..right.div_ceil(w).min(cx + u64::from(plan.pixels.width));
            let pixel = premultiplied(&row[i * channels..(i + 1) * channels]);
            for oy in ys.clone() {
                let wy = bottom.min((oy + 1) * h) - top.max(oy * h);
                for ox in xs.clone() {
                    let wx = right.min((ox + 1) * w) - left.max(ox * w);
                    let at = ((oy - cy) * u64::from(plan.pixels.width) + ox - cx) as usize;
                    for (sum, value) in self.accumulated[at].iter_mut().zip(pixel) {
                        *sum += u64::from(value) * wx * wy;
                    }
                }
            }
        }
    }

    fn finish(self, plan: &DecodePlan, out: &mut Pixmap) {
        let (w, h) = plan.natural();
        let area = u64::from(w) * u64::from(h);
        for (pixel, sum) in out.data_mut().chunks_exact_mut(4).zip(self.accumulated) {
            for c in 0..4 {
                pixel[c] = ((sum[c] + area / 2) / area) as u8;
            }
        }
    }
}

// File growth, malformed seek requests and trailing chunks cannot cause a
// decoder to read beyond the encoded input admitted during metadata inspection.
struct Bounded<R> {
    input: R,
    at: u64,
    end: u64,
}
impl<R: Read> Read for Bounded<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = out.len().min(self.end.saturating_sub(self.at) as usize);
        let n = self.input.read(&mut out[..n])?;
        self.at += n as u64;
        Ok(n)
    }
}
impl<R: Seek> Seek for Bounded<R> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let at = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.at) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.end) + i128::from(n),
        };
        if !(0..=i128::from(self.end)).contains(&at) {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        self.at = self.input.seek(SeekFrom::Start(at as u64))?;
        Ok(self.at)
    }
}
