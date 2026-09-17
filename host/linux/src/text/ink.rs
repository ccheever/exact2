//! CPU-only conservative ink selection. Full shaping and GPU paint stay intact.
use super::{catalog::Catalog, Paragraph};
use cosmic_text::{CacheKey, SubpixelBin};
use std::mem::size_of;
use std::rc::{Rc, Weak};
use tiny_skia::Transform;

pub(super) const MAX_BYTES: usize = 8 * 1024 * 1024;
const ENVELOPES: usize = 256;
// Beyond this range float/integer conversion and inversion use the full path.
const COORD_LIMIT: f64 = 16_777_216.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}
impl Bounds {
    const EMPTY: Self = Self {
        x0: f64::INFINITY,
        y0: f64::INFINITY,
        x1: f64::NEG_INFINITY,
        y1: f64::NEG_INFINITY,
    };
    const ALL: Self = Self {
        x0: f64::NEG_INFINITY,
        y0: f64::NEG_INFINITY,
        x1: f64::INFINITY,
        y1: f64::INFINITY,
    };
    fn finite(self) -> bool {
        [self.x0, self.y0, self.x1, self.y1]
            .iter()
            .all(|v| v.is_finite())
    }
    fn empty(self) -> bool {
        self.x0 > self.x1 || self.y0 > self.y1
    }
    fn union(self, b: Self) -> Self {
        Self {
            x0: self.x0.min(b.x0),
            y0: self.y0.min(b.y0),
            x1: self.x1.max(b.x1),
            y1: self.y1.max(b.y1),
        }
    }
    fn translated(self, x: f64, y: f64) -> Self {
        if self.empty() {
            return self;
        }
        if !x.is_finite() || !y.is_finite() {
            return Self::ALL;
        }
        Self {
            x0: self.x0 + x,
            y0: self.y0 + y,
            x1: self.x1 + x,
            y1: self.y1 + y,
        }
    }
    fn expand(self, x: f64, y: f64) -> Self {
        Self {
            x0: self.x0 - x,
            y0: self.y0 - y,
            x1: self.x1 + x,
            y1: self.y1 + y,
        }
    }
    fn overlaps(self, b: Self) -> bool {
        !self.empty()
            && !b.empty()
            && self.x0 <= b.x1
            && self.x1 >= b.x0
            && self.y0 <= b.y1
            && self.y1 >= b.y0
    }
}

#[derive(Default)]
pub(super) struct Cache {
    catalog: Weak<()>,
    scale: u32,
    pub index: Option<Index>,
}
impl Cache {
    pub fn matches(&self, catalog: &Rc<()>, scale: f32) -> bool {
        self.scale == scale.to_bits() && self.catalog.ptr_eq(&Rc::downgrade(catalog))
    }
    pub fn reset(&mut self, catalog: &Rc<()>, scale: f32) {
        self.index = None; // Old arrays die before the new scale allocates.
        self.catalog = Rc::downgrade(catalog);
        self.scale = scale.to_bits();
    }
    pub fn bytes(&self) -> usize {
        self.index.as_ref().map_or(0, Index::bytes)
    }
}

#[derive(Clone, Copy)]
struct Line {
    source: usize,
    wrapped: usize,
}

pub(super) struct Index {
    spans: Vec<Bounds>,
    lines: Vec<Line>,
    leaves: usize,
    // Bounds on inputs to the CPU's f32 baseline arithmetic, not CSS line boxes.
    max_input: f64,
}

fn storage(count: usize, limit: usize) -> Option<(usize, usize)> {
    let leaves = count.max(1).checked_next_power_of_two()?;
    let nodes = leaves.checked_mul(2)?;
    let bytes = nodes
        .checked_mul(size_of::<Bounds>())?
        .checked_add(count.checked_mul(size_of::<Line>())?)?;
    (bytes <= limit).then_some((leaves, nodes))
}

impl Index {
    pub fn build(engine: &mut Catalog, p: &Paragraph, scale: f32) -> Option<Self> {
        Self::with_limit(engine, p, scale, MAX_BYTES)
    }
    pub(super) fn with_limit(
        engine: &mut Catalog,
        p: &Paragraph,
        scale: f32,
        limit: usize,
    ) -> Option<Self> {
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let (leaves, nodes) = storage(p.baselines.len(), limit)?;
        let mut result = Self {
            spans: Vec::new(),
            lines: Vec::new(),
            leaves,
            max_input: 0.0,
        };
        result.spans.try_reserve_exact(nodes).ok()?;
        result.lines.try_reserve_exact(p.baselines.len()).ok()?;
        if result.bytes() > limit {
            return None;
        }
        result.spans.resize(nodes, Bounds::EMPTY);
        // Bounded build scratch; no image/font ownership survives an envelope.
        let mut envelopes: Vec<(CacheKey, Bounds)> = Vec::new();
        envelopes.try_reserve_exact(ENVELOPES).ok()?;
        for (source, line) in p.layouts.iter().enumerate() {
            for (wrapped, layout) in line.iter().enumerate() {
                let n = result.lines.len();
                let baseline = *p.baselines.get(n)?;
                result.lines.push(Line { source, wrapped });
                let mut span = Bounds::EMPTY;
                for glyph in &layout.glyphs {
                    let x = glyph.x + glyph.x_offset * glyph.font_size;
                    let y = glyph.y - glyph.y_offset * glyph.font_size;
                    if !x.is_finite() || !y.is_finite() || !baseline.is_finite() {
                        span = Bounds::ALL;
                        continue;
                    }
                    result.max_input = result
                        .max_input
                        .max(f64::from(x).abs())
                        .max(f64::from(y).abs())
                        .max(f64::from(baseline).abs());
                    let mut key = glyph.physical((0.0, 0.0), scale).cache_key;
                    key.x_bin = SubpixelBin::Zero;
                    key.y_bin = SubpixelBin::Zero;
                    let envelope = match envelopes.binary_search_by_key(&key, |(k, _)| *k) {
                        Ok(i) => envelopes[i].1,
                        Err(_) => {
                            let bound = envelope(engine, key);
                            if envelopes.len() == ENVELOPES {
                                envelopes.clear();
                            }
                            let i = envelopes
                                .binary_search_by_key(&key, |(k, _)| *k)
                                .unwrap_err();
                            envelopes.insert(i, (key, bound));
                            bound
                        }
                    };
                    span = span.union(envelope.translated(
                        f64::from(x) * f64::from(scale),
                        (f64::from(baseline) + f64::from(y)) * f64::from(scale),
                    ));
                }
                result.spans[leaves + n] = span;
            }
        }
        if result.lines.len() != p.baselines.len() {
            return None;
        }
        for i in (1..leaves).rev() {
            result.spans[i] = result.spans[2 * i].union(result.spans[2 * i + 1]);
        }
        Some(result)
    }

    fn bytes(&self) -> usize {
        self.spans.capacity() * size_of::<Bounds>() + self.lines.capacity() * size_of::<Line>()
    }

    pub fn glyphs<'a>(
        &self,
        p: &'a Paragraph,
        line: usize,
    ) -> (&'a [cosmic_text::LayoutGlyph], f32) {
        let loc = self.lines[line];
        (
            &p.layouts[loc.source][loc.wrapped].glyphs,
            p.baselines[line],
        )
    }

    /// Bounds in the same pre-transform device-pixel coordinates as our spans.
    /// All uncertain arithmetic fails open. The deliberately generous error
    /// envelope includes CPU baseline mul/add rounding, quantization, integer
    /// rectangle conversion, and f32 affine arithmetic in tiny-skia.
    pub fn viewport(
        &self,
        origin: (f32, f32),
        scale: f32,
        ts: Transform,
        clip: (f32, f32, f32, f32),
    ) -> Option<Bounds> {
        let root = self.spans[1];
        if root.empty() {
            return Some(Bounds::EMPTY);
        }
        let [sx, kx, ky, sy, tx, ty] = [ts.sx, ts.kx, ts.ky, ts.sy, ts.tx, ts.ty].map(f64::from);
        let (ox, oy) = (
            f64::from(origin.0) * f64::from(scale),
            f64::from(origin.1) * f64::from(scale),
        );
        let (x, y, w, h) = (
            f64::from(clip.0),
            f64::from(clip.1),
            f64::from(clip.2),
            f64::from(clip.3),
        );
        if !root.finite()
            || ![sx, kx, ky, sy, tx, ty, ox, oy, x, y, w, h]
                .iter()
                .all(|n| n.is_finite())
        {
            return None;
        }
        if w <= 0.0 || h <= 0.0 {
            return Some(Bounds::EMPTY);
        }
        let magnitude = ox.abs()
            + oy.abs()
            + self.max_input * f64::from(scale)
            + root
                .x0
                .abs()
                .max(root.x1.abs())
                .max(root.y0.abs())
                .max(root.y1.abs())
            + 1.0;
        if magnitude > COORD_LIMIT {
            return None;
        }
        let eps = f64::from(f32::EPSILON);
        let pixel_error = 4.0 + 16.0 * eps * magnitude;
        let dx = 4.0 + 16.0 * eps * ((sx.abs() + kx.abs()) * (magnitude + pixel_error) + tx.abs());
        let dy = 4.0 + 16.0 * eps * ((ky.abs() + sy.abs()) * (magnitude + pixel_error) + ty.abs());
        let det = sx * sy - kx * ky;
        let norm = sx.abs().max(kx.abs()).max(ky.abs()).max(sy.abs());
        if !det.is_finite() || det.abs() <= norm * norm * 1e-8 {
            return None;
        }
        let mut result = Bounds::EMPTY;
        for (px, py) in [
            (x - dx, y - dy),
            (x + w + dx, y - dy),
            (x - dx, y + h + dy),
            (x + w + dx, y + h + dy),
        ] {
            let (px, py) = (px - tx, py - ty);
            let (qx, qy) = (
                (sy * px - kx * py) / det - ox,
                (sx * py - ky * px) / det - oy,
            );
            result = result.union(Bounds {
                x0: qx,
                y0: qy,
                x1: qx,
                y1: qy,
            });
        }
        result = result.expand(pixel_error, pixel_error);
        result.finite().then_some(result)
    }

    pub fn visit(&self, query: Bounds, mut draw: impl FnMut(usize)) -> usize {
        fn walk(index: &Index, node: usize, query: Bounds, draw: &mut impl FnMut(usize)) -> usize {
            if !index.spans[node].overlaps(query) {
                return 1;
            }
            if node >= index.leaves {
                let line = node - index.leaves;
                if line < index.lines.len() {
                    draw(line);
                }
                1
            } else {
                1 + walk(index, node * 2, query, draw) + walk(index, node * 2 + 1, query, draw)
            }
        }
        walk(self, 1, query, &mut draw)
    }
}

fn envelope(engine: &mut Catalog, mut key: CacheKey) -> Bounds {
    if !f32::from_bits(key.font_size_bits).is_finite() {
        return Bounds::ALL;
    }
    let mut result = Bounds::EMPTY;
    // LayoutGlyph::physical truncates the Y coordinate before CacheKey::new:
    // every finite CPU placement has y_bin=Zero. X has four phases, independent
    // of scroll origin. Tiny-skia applies transforms after this raster choice.
    // The oracle separately asserts this dependency contract for +/- fractions.
    for bin in [
        SubpixelBin::Zero,
        SubpixelBin::One,
        SubpixelBin::Two,
        SubpixelBin::Three,
    ] {
        key.x_bin = bin;
        // Uncached: extra phases must not become retained pixel backings.
        // A missing image is exactly the existing CPU renderer's no-ink case.
        if let Some(image) = engine.swash.get_image_uncached(&mut engine.fonts, key) {
            let p = image.placement;
            if p.width != 0 && p.height != 0 {
                result = result.union(Bounds {
                    x0: f64::from(p.left),
                    y0: -f64::from(p.top),
                    x1: f64::from(p.left) + f64::from(p.width),
                    y1: -f64::from(p.top) + f64::from(p.height),
                });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn array_budget_and_checked_dimensions_refuse_before_allocation() {
        assert!(storage(usize::MAX, MAX_BYTES).is_none());
        assert!(storage(200_000, MAX_BYTES).is_none());
        assert!(storage(1_000, 64).is_none());
        assert!(storage(1_000, MAX_BYTES).is_some());
    }
    #[test]
    fn unknown_span_keeps_original_order_without_hiding_known_neighbors() {
        let index = Index {
            spans: vec![Bounds::EMPTY, Bounds::ALL, Bounds::ALL, Bounds::ALL],
            lines: vec![
                Line {
                    source: 0,
                    wrapped: 0
                };
                2
            ],
            leaves: 2,
            max_input: 0.0,
        };
        let mut order = Vec::new();
        index.visit(
            Bounds {
                x0: 0.0,
                y0: 0.0,
                x1: 1.0,
                y1: 1.0,
            },
            |line| order.push(line),
        );
        assert_eq!(order, [0, 1]);
    }
}
