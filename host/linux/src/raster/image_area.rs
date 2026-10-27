//! Separable area and bilinear filtering of premultiplied, axis-aligned images.
use tiny_skia::{Mask, Pixmap, PixmapRef, Transform};

#[derive(Default)]
pub(super) struct Scratch {
    horizontal: Vec<Span>,
    rows: [Row; 2],
    next_row: usize,
    accumulated: Vec<[f64; 4]>,
    vertical: Vec<[f64; 4]>,
    #[cfg(test)]
    pub(super) rows_filtered: usize,
    #[cfg(test)]
    pub(super) pixels_filtered: usize,
    #[cfg(test)]
    last_source_y: Option<usize>,
}

#[derive(Default)]
struct Row {
    source_y: Option<usize>,
    pixels: Vec<[f64; 4]>,
}

struct Span {
    start: usize,
    end: usize,
    first: f64,
    last: f64,
    step: f64,
}

impl Span {
    fn new(at: usize, origin: f64, scale: f64, extent: u32, center: f32) -> Self {
        let edge = origin + f64::from(extent) * scale;
        if scale.abs() >= 0.5 {
            // tiny-skia's pixel centres and clamped bilinear taps.
            let limit = f32::from_bits((extent as f32).to_bits() - 1);
            let index = |at: f32| at.max(0.).min(limit) as usize;
            let start = index(center - 0.5);
            let last = index(center - 0.5 + 1.);
            let fraction = f64::from((center + 0.5).fract());
            return Self {
                start,
                end: last + 1,
                first: if start == last { 1. } else { 1. - fraction },
                last: fraction,
                step: 0.,
            };
        }
        let a = (at as f64 - origin) / scale;
        let b = (at as f64 + 1. - origin) / scale;
        let left = a.min(b).clamp(0., f64::from(extent));
        let right = a.max(b).clamp(0., f64::from(extent));
        let start = left.floor() as usize;
        let end = right.ceil() as usize;
        // Average only the footprint inside the image. The clip supplies coverage.
        let coverage = (at as f64 + 1.).min(origin.max(edge)) - (at as f64).max(origin.min(edge));
        let overlap = |pixel: usize| {
            let a = origin + pixel as f64 * scale;
            let b = origin + (pixel + 1) as f64 * scale;
            ((at as f64 + 1.).min(a.max(b)) - (at as f64).max(a.min(b))).max(0.)
        };
        Self {
            start,
            end,
            first: overlap(start) / coverage,
            last: overlap(end.saturating_sub(1)) / coverage,
            step: scale.abs() / coverage,
        }
    }

    fn weight(&self, at: usize) -> f64 {
        if at == self.start {
            self.first
        } else if at + 1 == self.end {
            self.last
        } else {
            self.step
        }
    }
}

impl Scratch {
    fn row(&mut self, source: PixmapRef<'_>, y: usize) -> &[[f64; 4]] {
        let slot = self.rows.iter().position(|row| row.source_y == Some(y));
        let slot = slot.unwrap_or_else(|| {
            let slot = self.next_row;
            self.next_row ^= 1;
            let row = &mut self.rows[slot];
            row.source_y = Some(y);
            let stride = source.width() as usize * 4;
            let source = &source.data()[y * stride..(y + 1) * stride];
            for (out, span) in row.pixels.iter_mut().zip(&self.horizontal) {
                *out = [0.; 4];
                for (x, pixel) in source[span.start * 4..span.end * 4]
                    .chunks_exact(4)
                    .enumerate()
                {
                    #[cfg(test)]
                    {
                        self.pixels_filtered += 1;
                    }
                    let weight = span.weight(x + span.start);
                    for c in 0..4 {
                        out[c] += f64::from(pixel[c]) * weight;
                    }
                }
            }
            #[cfg(test)]
            {
                self.rows_filtered += 1;
            }
            slot
        });
        &self.rows[slot].pixels
    }
}

pub(super) fn draw(
    target: &mut Pixmap,
    source: PixmapRef<'_>,
    transform: Transform,
    mask: Option<&Mask>,
    scratch: &mut Scratch,
) -> bool {
    if !transform.is_finite()
        || transform.kx != 0.
        || transform.ky != 0.
        || transform.sx == 0.
        || transform.sy == 0.
        || (transform.sx.abs() >= 0.5 && transform.sy.abs() >= 0.5)
        || mask.is_some_and(|m| m.width() != target.width() || m.height() != target.height())
    {
        return false;
    }
    let Some(inverse) = transform.invert().filter(Transform::is_finite) else {
        return false;
    };
    let (sx, sy) = (f64::from(transform.sx), f64::from(transform.sy));
    let (tx, ty) = (f64::from(transform.tx), f64::from(transform.ty));
    let bounds = |origin: f32, scale: f32, extent: u32, limit: u32| {
        let edge = extent as f32 * scale + origin;
        // fill_path rejects nearly empty transformed bounds before rounding,
        // using the difference of the f32 endpoints, not extent * scale.
        if edge.max(origin) - edge.min(origin) <= 1. / 4096. {
            return (0, 0);
        }
        // draw_pixmap fills a transformed rect without AA: tiny-skia 0.12's
        // LineEdge::new truncates to 26.6, then rounds with (n + 32) >> 6.
        // Both axes include the rounded start and exclude the rounded end.
        let round = |v: f32| {
            (((v.clamp(0., limit as f32) * 64.) as i64 + 32) >> 6).clamp(0, i64::from(limit))
                as usize
        };
        (round(origin.min(edge)), round(origin.max(edge)))
    };
    #[cfg(test)]
    {
        scratch.rows_filtered = 0;
        scratch.pixels_filtered = 0;
        scratch.last_source_y = None;
    }
    let (mut left, mut right) = bounds(transform.tx, transform.sx, source.width(), target.width());
    let (mut top, mut bottom) =
        bounds(transform.ty, transform.sy, source.height(), target.height());
    if right <= left || bottom <= top {
        return true;
    }
    let stride = target.width() as usize;
    if let Some(mask) = mask {
        let (mut l, mut r, mut t, mut b) = (right, left, bottom, top);
        for y in top..bottom {
            let row = &mask.data()[y * stride + left..y * stride + right];
            if let Some(first) = row.iter().position(|v| *v != 0) {
                l = l.min(left + first);
                r = r.max(left + row.iter().rposition(|v| *v != 0).unwrap() + 1);
                t = t.min(y);
                b = y + 1;
            }
        }
        if r <= l || b <= t {
            return true;
        }
        (left, right, top, bottom) = (l, r, t, b);
    }
    let width = right - left;
    scratch.horizontal.clear();
    scratch.horizontal.extend((left..right).map(|x| {
        Span::new(
            x,
            tx,
            sx,
            source.width(),
            (x as f32 + 0.5) * inverse.sx + inverse.tx,
        )
    }));
    let horizontal_first = sx.abs() < 0.5;
    let first = scratch.horizontal.first().unwrap();
    let last = scratch.horizontal.last().unwrap();
    let columns = first.start.min(last.start)..first.end.max(last.end);
    if horizontal_first {
        for row in &mut scratch.rows {
            row.source_y = None;
            row.pixels.resize(width, [0.; 4]);
        }
    } else {
        scratch.vertical.resize(columns.len(), [0.; 4]);
    }
    scratch.next_row = 0;
    let mut accumulated = std::mem::take(&mut scratch.accumulated);
    accumulated.resize(width, [0.; 4]);
    for y in top..bottom {
        let row_start = y * stride + left;
        let coverage = mask.map(|m| &m.data()[row_start..row_start + width]);
        if coverage.is_some_and(|row| row.iter().all(|v| *v == 0)) {
            continue;
        }
        let vertical = Span::new(
            y,
            ty,
            sy,
            source.height(),
            (y as f32 + 0.5) * inverse.sy + inverse.ty,
        );
        accumulated.fill([0.; 4]);
        if !horizontal_first {
            scratch.vertical.fill([0.; 4]);
        }
        // Monotone source order retains the shared boundary row for the next output row.
        for i in 0..vertical.end - vertical.start {
            let source_y = if sy > 0. {
                vertical.start + i
            } else {
                vertical.end - 1 - i
            };
            let weight = vertical.weight(source_y);
            if horizontal_first {
                for (sum, pixel) in accumulated.iter_mut().zip(scratch.row(source, source_y)) {
                    for c in 0..4 {
                        sum[c] += pixel[c] * weight;
                    }
                }
            } else {
                let start = (source_y * source.width() as usize + columns.start) * 4;
                let row = &source.data()[start..start + columns.len() * 4];
                for (sum, pixel) in scratch.vertical.iter_mut().zip(row.chunks_exact(4)) {
                    for c in 0..4 {
                        sum[c] += f64::from(pixel[c]) * weight;
                    }
                }
                #[cfg(test)]
                {
                    scratch.pixels_filtered += columns.len();
                    if scratch.last_source_y != Some(source_y) {
                        scratch.rows_filtered += 1;
                    }
                    scratch.last_source_y = Some(source_y);
                }
            }
        }
        if !horizontal_first {
            for (sum, span) in accumulated.iter_mut().zip(&scratch.horizontal) {
                for x in span.start..span.end {
                    let pixel = scratch.vertical[x - columns.start];
                    let weight = span.weight(x);
                    for c in 0..4 {
                        sum[c] += pixel[c] * weight;
                    }
                }
            }
        }
        let row = &mut target.data_mut()[row_start * 4..(row_start + width) * 4];
        for (x, (out, color)) in row.chunks_exact_mut(4).zip(&mut accumulated).enumerate() {
            let cover = coverage.map_or(255, |m| m[x]);
            if cover == 0 {
                continue;
            }
            for value in &mut *color {
                *value *= f64::from(cover) / 255.;
            }
            let inverse = 1. - (color[3] / 255.).min(1.);
            for c in 0..4 {
                out[c] = (color[c] + f64::from(out[c]) * inverse)
                    .round()
                    .clamp(0., 255.) as u8;
            }
            for c in 0..3 {
                out[c] = out[c].min(out[3]);
            }
        }
    }
    scratch.accumulated = accumulated;
    true
}
