//! PNG row sampling. The caller reserves `cost` before constructing a decoder.
//! Natural dimensions never depend on the sampled raster's resolution.
use exact_raster::{
    DecodeCost, Metadata, PixelSize, Refusal, MAX_ENCODED_BYTES, MAX_HEADER_BYTES,
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
}

/// `color` for a JPEG, GIF or WebP (not a PNG colour type); its pixels come
/// from `jpeg_decode`, the platform's decoder.
const JPEG: u8 = 0xFF;

impl Header {
    /// A JPEG's, GIF's or WebP's header: decoded as 8-bit RGBA by the platform
    /// (a GIF's or WebP's first frame).
    pub(super) fn jpeg(metadata: Metadata) -> Header {
        Header {
            metadata,
            depth: 8,
            channels: 4,
            color: JPEG,
            interlaced: false,
        }
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
    let mut head = [0; 33];
    input
        .read_exact(&mut head)
        .map_err(|_| Refusal::DecodeFailed)?;
    if super::jpeg_decode::is_jpeg(&head) {
        return super::jpeg_decode::inspect(input, encoded_bytes);
    }
    if let Some(header) = super::jpeg_decode::inspect_animated(&head, encoded_bytes)? {
        return Ok(header);
    }
    if &head[..8] != b"\x89PNG\r\n\x1a\n"
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
    pub pixels: PixelSize,
    pub cost: DecodeCost,
}
impl DecodePlan {
    pub fn new(header: Header, pixels: (u32, u32)) -> Result<Self, Refusal> {
        let natural = header.metadata.natural;
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
        // Android's platform decoder subsamples a JPEG by powers of two in the
        // DCT and then resamples to an exact size at full quality, which costs
        // more than the decode: plan the largest power-of-two subsample that
        // still covers the request, as an image loader's inexact decode does,
        // and let the GPU scale it where it is drawn.
        #[cfg(target_os = "android")]
        let pixels = if header.color == JPEG && std::env::var_os("EXACT_EXACT_DECODE").is_none() {
            let mut s = 1u32;
            while natural.width.div_ceil(s * 2) >= pixels.0
                && natural.height.div_ceil(s * 2) >= pixels.1
                && s < 8
            {
                s *= 2;
            }
            (natural.width.div_ceil(s), natural.height.div_ceil(s))
        } else {
            pixels
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
        })
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
            // Each destination selects one source pixel, including odd/final
            // Adam7 passes. Only the small output and one source row exist.
            let oy = (u64::from(y) * u64::from(plan.pixels.height) / u64::from(h)) as u32;
            for oy in oy.saturating_sub(1)..(oy + 2).min(plan.pixels.height) {
                let sy = sample(oy, plan.pixels.height, h);
                if sy != y {
                    continue;
                }
                for ox in 0..plan.pixels.width {
                    let sx = sample(ox, plan.pixels.width, w);
                    if sx < x0 || !(sx - x0).is_multiple_of(dx) {
                        continue;
                    }
                    let i = ((sx - x0) / dx) as usize * channels;
                    let px = &row[i..i + channels];
                    let (r, g, b, a) = match channels {
                        4 => (px[0], px[1], px[2], px[3]),
                        3 => (px[0], px[1], px[2], 255),
                        2 => (px[0], px[0], px[0], px[1]),
                        _ => (px[0], px[0], px[0], 255),
                    };
                    let at = ((u64::from(oy) * u64::from(plan.pixels.width) + u64::from(ox)) * 4)
                        as usize;
                    let premul = |v| (u32::from(v) * u32::from(a) / 255) as u8;
                    out.data_mut()[at..at + 4].copy_from_slice(&[
                        premul(r),
                        premul(g),
                        premul(b),
                        a,
                    ]);
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
    Ok(out)
}

fn sample(at: u32, output: u32, natural: u32) -> u32 {
    (((2 * u64::from(at) + 1) * u64::from(natural)) / (2 * u64::from(output))) as u32
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
