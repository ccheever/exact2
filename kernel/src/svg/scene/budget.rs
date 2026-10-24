//! What one resolved scene may cost (LLP 1011.001 D2's bounds), the same
//! for an inline `svg` and an SVG file drawn as an image.
//!
//! A `use` copies its target, a marker is copied at each vertex of the
//! shape that names it, and a mask or pattern is resolved again for each
//! element that references it, so a file of a few kilobytes can name
//! billions of items. Dashes and offscreen surfaces cost the host in the
//! same way without being items. The resolver charges each as it makes
//! it, before the host allocates anything for it, and stops at the first
//! limit passed: its own work is bounded by the limits, not by what the
//! file names. A scene over a limit draws nothing and names the limit
//! ([`Scene::over`](super::Scene::over)); an image reports it as its
//! `error`.

use crate::svg::filter::{Filter, Op, Primitive, Transfer};
use std::cell::Cell;
use std::fmt;

/// What one scene may resolve, in units of one path segment (28 bytes,
/// up to twice that in a growing `Vec`): about 50 MB at the limit.
pub const MAX_COST: u64 = 1_000_000;

/// What one item is charged: an `Item` and its shape are some 450 bytes in
/// Rust, and each is a layer or a paint call on the host. 125,000 items
/// is the limit, past the parser's 50,000 elements: a file is refused only
/// for what it copies, or for path data in the megabytes.
pub const ITEM: u64 = 8;

/// What one character of text is charged: its glyph's outline is several
/// curves, and the host shapes and draws it.
pub const CHARACTER: u64 = 8;

/// Dashes one scene may stroke, every stroked path's together (tiny-skia
/// stops at a million in one path; each dash is a stroked outline).
pub const MAX_DASHES: f64 = 100_000.0;

/// The largest side of one island, in device pixels: Linux's cap (Apple
/// caps an island's area at the same 4,096²).
const ISLAND_SIDE: f64 = 4096.0;

/// The device scale surfaces are charged at: a phone's largest. A scene
/// does not know its host's.
const SCALE: f64 = 3.0;

/// The offscreen surfaces (filter, mask and pattern islands, and a filter's
/// working images) one scene may allocate, in bytes, summed over the
/// scene, so also what it can hold at once. `apps/svg-gallery`'s
/// `fx-huge-region`, a drop shadow and a mask whose regions are far larger
/// than the drawing, is 2.35 GB at the islands' size on Linux, and is
/// documented to draw; a few more of them, or islands nested in islands,
/// are refused. Sizing an island to what shows (QUEUE) would let this drop
/// by an order of magnitude.
pub const MAX_SURFACE: f64 = 2.5 * (1u64 << 30) as f64;

/// A filter's bytes per island pixel: the island's RGBA, and in
/// `exact-svg-raster`'s `filter::run` the source and an input in `f32`
/// RGBA (16 bytes each) beside every primitive's kept result, and the most
/// working images any one primitive holds besides.
fn filter_bytes(primitives: &[Primitive]) -> f64 {
    let scratch = primitives.iter().map(|p| match p.op {
        // The source, its blur, the moved and tinted copies, the shadow.
        Op::DropShadow(..) => 5.0,
        // Each input composited over the last.
        Op::Merge(_) => 2.0,
        // The second input's copy.
        Op::Composite(..) | Op::Blend(_) | Op::Displacement(..) => 1.0,
        // The pass along one axis.
        Op::Morphology(..) => 1.0,
        _ => 0.0,
    });
    4.0 + 16.0 * (2.0 + primitives.len() as f64 + scratch.fold(0.0, f64::max))
}

/// The filter work one scene may ask for, in steps: a step is one pass of
/// one primitive over one island pixel, and a primitive whose work per
/// pixel grows with a parameter takes one step per unit of it (below).
/// At the limit, a convolution or turbulence over an island of the largest
/// size takes some 1.3 s on the Linux host (an M-series Mac).
pub const MAX_FILTER_WORK: f64 = 1.0e9;

/// A primitive's steps per island pixel at `scale` island pixels per user
/// unit, as `exact-svg-raster` runs it.
fn steps(op: &Op, scale: f64) -> f64 {
    let radius = |r: f32| (f64::from(r.max(0.0)) * scale).min(ISLAND_SIDE);
    match op {
        // A pass of 2r + 1 samples along each axis.
        Op::Morphology(_, rx, ry) => 2.0 * (radius(*rx) + radius(*ry)) + 2.0,
        // Four channels of noise per octave.
        Op::Turbulence(_, _, octaves, ..) => TURBULENCE * f64::from((*octaves).max(1)),
        Op::Convolve(c) => c.kernel.len().max(1) as f64,
        // A 3 × 3 neighbourhood for the normal.
        Op::Lighting(_) => 9.0,
        Op::Merge(inputs) => inputs.len().max(1) as f64,
        _ => 1.0,
    }
}

/// Steps one octave of turbulence costs: four channels of gradient noise.
const TURBULENCE: f64 = 16.0;

/// The limit a scene passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// [`MAX_COST`].
    Expansion,
    /// [`MAX_DASHES`].
    Dashes,
    /// [`MAX_SURFACE`].
    Surfaces,
    /// [`MAX_FILTER_WORK`].
    FilterWork,
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Limit::Expansion => write!(
                f,
                "the SVG expands past {} elements, fewer with long paths or text (its `use`, marker, mask and pattern copies counted)",
                MAX_COST / ITEM
            ),
            Limit::Dashes => write!(f, "the SVG strokes more than {MAX_DASHES} dashes"),
            Limit::Surfaces => write!(
                f,
                "the SVG's filters, masks and patterns need more than 2.5 GiB of offscreen surfaces at a device scale of {SCALE}"
            ),
            Limit::FilterWork => write!(
                f,
                "the SVG's filters need more than {MAX_FILTER_WORK} pixel steps at a device scale of {SCALE}"
            ),
        }
    }
}

/// The running charges of one resolve.
pub(super) struct Budget {
    cost: Cell<u64>,
    dashes: Cell<f64>,
    surfaces: Cell<f64>,
    work: Cell<f64>,
    over: Cell<Option<Limit>>,
}

impl Budget {
    pub(super) fn new() -> Budget {
        Budget {
            cost: Cell::new(0),
            dashes: Cell::new(0.0),
            surfaces: Cell::new(0.0),
            work: Cell::new(0.0),
            over: Cell::new(None),
        }
    }

    /// The limit passed, if any.
    pub(super) fn over(&self) -> Option<Limit> {
        self.over.get()
    }

    fn fail(&self, limit: Limit) -> bool {
        if self.over.get().is_none() {
            self.over.set(Some(limit));
        }
        false
    }

    /// Charge `n` units of [`MAX_COST`]; `false` once over any limit.
    pub(super) fn cost(&self, n: u64) -> bool {
        if self.over.get().is_some() {
            return false;
        }
        let total = self.cost.get().saturating_add(n);
        self.cost.set(total);
        total <= MAX_COST || self.fail(Limit::Expansion)
    }

    /// Charge the dashes `pattern` (path units, even length) makes along
    /// `length` path units; `false` once over any limit.
    pub(super) fn dashes(&self, length: f64, pattern: &[f32]) -> bool {
        if pattern.is_empty() || self.over.get().is_some() {
            return self.over.get().is_none();
        }
        let interval: f64 = pattern.iter().map(|d| f64::from(*d)).sum();
        let n = length * (pattern.len() / 2) as f64 / interval;
        let total = self.dashes.get() + if n.is_nan() { f64::INFINITY } else { n };
        self.dashes.set(total);
        total <= MAX_DASHES || self.fail(Limit::Dashes)
    }

    /// Charge a pattern's whole count of dashes `n` times (a subpath
    /// restarts it, however short); `false` once over any limit.
    pub(super) fn restarts(&self, n: usize, pattern: &[f32]) -> bool {
        self.dashes((n * pattern.len() / 2) as f64, &[0.5, 0.5])
    }

    /// Charge filter primitives as made, with what each carries: a
    /// transfer table's values, a kernel's cells, a merge's inputs.
    pub(super) fn primitives(&self, made: &[Primitive]) -> bool {
        let carried = |op: &Op| match op {
            Op::ComponentTransfer(t) => t
                .iter()
                .map(|f| match f {
                    Transfer::Table(v) | Transfer::Discrete(v) => v.len(),
                    _ => 0,
                })
                .sum(),
            Op::Convolve(c) => c.kernel.len(),
            Op::Merge(inputs) => inputs.len(),
            _ => 0,
        };
        let cost: u64 = made.iter().map(|p| ITEM + carried(&p.op) as u64).sum();
        self.cost(cost)
    }

    /// Charge a surface of `w` × `h` user units drawn through `ctm` (user
    /// units to the content box's CSS pixels) at `bytes` a pixel; `false`
    /// once over any limit.
    pub(super) fn surface(&self, rect: Rect, ctm: [f32; 6], bytes: f64) -> bool {
        let (pixels, _) = island(rect, ctm);
        self.add_surface(pixels * bytes)
    }

    /// Charge `filter`'s island, its working images and its work, for an
    /// element whose user space maps to the content box by `ctm`; `false`
    /// once over any limit.
    pub(super) fn filter(&self, filter: &Filter, ctm: [f32; 6]) -> bool {
        let (pixels, scale) = island(filter.region, ctm);
        if !self.add_surface(pixels * filter_bytes(&filter.primitives)) {
            return false;
        }
        let steps: f64 = filter.primitives.iter().map(|p| steps(&p.op, scale)).sum();
        let total = self.work.get() + pixels * steps;
        self.work.set(total);
        total <= MAX_FILTER_WORK || self.fail(Limit::FilterWork)
    }

    fn add_surface(&self, bytes: f64) -> bool {
        if self.over.get().is_some() {
            return false;
        }
        let total = self.surfaces.get() + if bytes.is_nan() { 0.0 } else { bytes };
        self.surfaces.set(total);
        total <= MAX_SURFACE || self.fail(Limit::Surfaces)
    }
}

type Rect = (f32, f32, f32, f32);

/// The pixels of an island over `rect` (user units) drawn through `ctm`,
/// at the device scale: the larger of the two ways the hosts size one, by
/// the transform's uniform scale (a filter's) and by the rect's bounds
/// once drawn (a mask's, a pattern's painted area), each side at least one
/// pixel, and at most [`ISLAND_SIDE`]² in all (Linux caps each side, Apple
/// the area, so a long thin one is Apple's). And its pixels per user unit.
fn island(rect: Rect, ctm: [f32; 6]) -> (f64, f64) {
    let m = ctm.map(f64::from);
    let k = (m[0] * m[3] - m[1] * m[2]).abs().sqrt() * SCALE;
    let side = |s: f64| s.ceil().max(1.0);
    let (w, h) = (f64::from(rect.2.max(0.0)), f64::from(rect.3.max(0.0)));
    let uniform = side(w * k) * side(h * k);
    // The drawn bounds: the rect's edges through the linear part.
    let bw = (m[0] * w).abs() + (m[2] * h).abs();
    let bh = (m[1] * w).abs() + (m[3] * h).abs();
    let bounds = side(bw * SCALE) * side(bh * SCALE);
    let pixels = uniform.max(bounds).min(ISLAND_SIDE * ISLAND_SIDE);
    (
        if pixels.is_nan() {
            ISLAND_SIDE * ISLAND_SIDE
        } else {
            pixels
        },
        k,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_charge_stops_at_its_limit_and_the_first_limit_is_kept() {
        let b = Budget::new();
        assert!(b.cost(MAX_COST));
        assert!(!b.cost(1));
        assert_eq!(b.over(), Some(Limit::Expansion));
        assert!(!b.dashes(1.0, &[1.0, 1.0]), "nothing more once over");
        assert_eq!(b.over(), Some(Limit::Expansion));

        let b = Budget::new();
        // A 50,000-unit path dashed 0.5 on, 0.5 off: 50,000 dashes.
        assert!(b.dashes(50_000.0, &[0.5, 0.5]));
        assert!(b.dashes(50_000.0, &[0.5, 0.5]));
        assert!(!b.dashes(1.0, &[0.5, 0.5]));
        assert_eq!(b.over(), Some(Limit::Dashes));
        assert!(!Budget::new().dashes(f64::NAN, &[1.0, 1.0]));

        // A drop shadow and a mask, each of the largest size, as
        // `fx-huge-region` is, fit; another drop shadow does not.
        let b = Budget::new();
        let huge = [1.0e6, 0.0, 0.0, 1.0e6, 0.0, 0.0];
        let (unit, square) = ((0.0, 0.0, 1.0, 1.0), (0.0, 0.0, 100.0, 100.0));
        assert!(b.surface(unit, huge, 132.0));
        assert!(b.surface(unit, huge, 8.0));
        assert!(!b.surface(unit, huge, 132.0));
        assert_eq!(b.over(), Some(Limit::Surfaces));
        // 100 × 100 CSS pixels at the charged scale of 3: 90,000 pixels.
        let b = Budget::new();
        let one = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        assert!(b.surface(square, one, 29_826.0));
        assert!(!b.surface(square, one, 1.0));
        // A unit square skewed to a 4,096-pixel-wide band is charged by the
        // bounds a host allocates, not by its determinant of 1.
        let b = Budget::new();
        let skew = [4096.0, 4096.0, 0.0, 1.0 / 4096.0, 0.0, 0.0];
        assert_eq!(island(unit, skew).0, 4096.0 * 4096.0);
        assert!(b.surface(unit, skew, 1.0));
    }
}
