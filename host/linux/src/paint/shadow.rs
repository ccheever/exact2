//! CSS `box-shadow` (LLP 1064 D2, LLP 1077 D4): a list of outer and inset
//! shadows with spread. An outer one is the border box's outline, grown by
//! the spread, offset and blurred, painted only outside the border box,
//! under the box; an inset one is the padding box's outline, shrunk by the
//! spread and offset, its outside blurred inward, painted only inside the
//! padding box, over the background and under the border. The first shadow
//! is on top.
//!
//! Neither backend has a blur, and a blurred picture per node would be a
//! second raster pass. A Gaussian of a straight edge falls off as the normal
//! CDF of the distance to it, and the outline's offset curves are its
//! contours, so the shadow is a stack of offset outlines (radius grown or
//! shrunk with the distance), each filled with the alpha that brings the
//! stack to the CDF at that band: one fill per band, through the border
//! fill every backend already draws (a region inside a clip).

use super::border::{shaped_rect, BorderFill, PathOp};
use super::{Rect4, Shape};
use exact_kernel::StyleProps;

/// One shadow of the list, resolved for the appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ShadowPaint {
    /// Straight RGBA.
    color: [u8; 4],
    offset: (f32, f32),
    /// CSS's blur radius: twice the Gaussian's standard deviation.
    blur: f32,
    spread: f32,
    inset: bool,
}

/// A band is at most this wide, in points, so a pixel is never more than
/// about a point from its band's middle...
const STEP: f32 = 1.5;
/// ...up to this many bands; wider blurs step a little coarser.
const BANDS: f32 = 32.0;

impl ShadowPaint {
    /// The node's shadows, in list order (the first on top); none of them
    /// that would show nothing.
    pub fn capture(s: &StyleProps, dark: bool) -> Vec<ShadowPaint> {
        s.box_shadow
            .shadows()
            .iter()
            .filter(|b| b.offset.x.is_finite() && b.offset.y.is_finite())
            .map(|b| {
                let c = b.color.resolve(dark);
                ShadowPaint {
                    color: [c.r(), c.g(), c.b(), c.a()],
                    offset: (b.offset.x, b.offset.y),
                    blur: b.blur.max(0.0),
                    spread: if b.spread.is_finite() { b.spread } else { 0.0 },
                    inset: b.inset,
                }
            })
            .collect()
    }

    /// `base` with paint motion's geometry (offset, blur) and colour over its
    /// first shadow (LLP 1062; LLP 1077 D4: the rest change at once).
    pub fn over(
        mut base: Vec<ShadowPaint>,
        geometry: Option<exact_motion::Value>,
        color: Option<[u8; 4]>,
    ) -> Vec<ShadowPaint> {
        if base.is_empty() {
            base.push(ShadowPaint {
                color: [0; 4],
                offset: (0.0, 0.0),
                blur: 0.0,
                spread: 0.0,
                inset: false,
            });
        }
        let s = &mut base[0];
        if let Some(g) = geometry {
            s.offset = (g.x as f32, g.y as f32);
            s.blur = (g.z as f32).max(0.0);
        }
        if let Some(c) = color {
            s.color = c;
        }
        base
    }

    /// Whether it paints inside the padding box.
    pub fn inset(&self) -> bool {
        self.inset
    }

    /// The fills, outermost band first: around the border box `outer`, or
    /// for an inset shadow inside its padding box (`widths` the border's).
    pub fn fills(&self, outer: &Shape, widths: [f32; 4]) -> Vec<BorderFill> {
        if self.color[3] == 0 {
            return Vec::new();
        }
        if self.inset {
            return self.inset_fills(outer, widths);
        }
        let sigma = self.blur / 2.0;
        let (x, y, w, h) = outer.rect;
        let (dx, dy) = self.offset;
        let reach = 3.0 * sigma + dx.abs() + dy.abs() + self.spread.abs() + 1.0;
        let mut outside = Vec::new();
        shaped_rect(
            &mut outside,
            (x - reach, y - reach, w + 2.0 * reach, h + 2.0 * reach),
            [(0.0, 0.0); 4],
            None,
        );
        shaped_rect(
            &mut outside,
            outer.rect,
            outer.radii,
            outer.corners.as_ref(),
        );
        // The shape before the blur: the border box grown by the spread,
        // its radii by CSS's rule (Backgrounds 3 §7.1.1).
        let spread = self.spread;
        let base = (x - spread, y - spread, w + 2.0 * spread, h + 2.0 * spread);
        let radii = outer
            .radii
            .map(|(rx, ry)| (spread_radius(rx, spread), spread_radius(ry, spread)));
        self.bands(sigma, STEP, |edge| {
            let rect = (
                base.0 + dx - edge,
                base.1 + dy - edge,
                base.2 + 2.0 * edge,
                base.3 + 2.0 * edge,
            );
            if rect.2 <= 0.0 || rect.3 <= 0.0 {
                return None;
            }
            let mut clip = Vec::new();
            let r = radii.map(|(x, y)| ((x + edge).max(0.0), (y + edge).max(0.0)));
            shaped_rect(&mut clip, rect, r, outer.corners.as_ref());
            Some((outside.clone(), Some(clip)))
        })
    }

    /// An inset shadow: the padding box less its shrunk, offset outline,
    /// blurred inward, inside the padding box.
    fn inset_fills(&self, outer: &Shape, widths: [f32; 4]) -> Vec<BorderFill> {
        let sigma = self.blur / 2.0;
        let [t, r, b, l] = widths.map(|v| v.max(0.0));
        let (x, y, w, h) = outer.rect;
        let padding: Rect4 = (x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0));
        if padding.2 <= 0.0 || padding.3 <= 0.0 {
            return Vec::new();
        }
        let pad_radii = [
            (
                (outer.radii[0].0 - l).max(0.0),
                (outer.radii[0].1 - t).max(0.0),
            ),
            (
                (outer.radii[1].0 - r).max(0.0),
                (outer.radii[1].1 - t).max(0.0),
            ),
            (
                (outer.radii[2].0 - r).max(0.0),
                (outer.radii[2].1 - b).max(0.0),
            ),
            (
                (outer.radii[3].0 - l).max(0.0),
                (outer.radii[3].1 - b).max(0.0),
            ),
        ];
        let mut clip = Vec::new();
        shaped_rect(&mut clip, padding, pad_radii, outer.corners.as_ref());
        let (dx, dy) = self.offset;
        let s = self.spread;
        // The edge is how far the hole grows: from three deviations inside
        // the shrunk outline (the largest shadowed area) to three outside.
        // Half the outer band: an inset blur fills the whole padding box,
        // where a band's step shows against Chrome's smooth one.
        self.bands(sigma, STEP / 2.0, |edge| {
            let grow = -edge - s;
            let rect = (
                padding.0 + dx - grow,
                padding.1 + dy - grow,
                padding.2 + 2.0 * grow,
                padding.3 + 2.0 * grow,
            );
            let mut region = clip.clone();
            if rect.2 > 0.0 && rect.3 > 0.0 {
                let r = pad_radii.map(|(x, y)| ((x + grow).max(0.0), (y + grow).max(0.0)));
                shaped_rect(&mut region, rect, r, outer.corners.as_ref());
            }
            Some((region, Some(clip.clone())))
        })
    }

    /// The band stack: `shape(edge)` for each edge from three deviations out
    /// to three in, each filled with the alpha that brings the composited
    /// stack to the Gaussian's CDF at its middle.
    fn bands(
        &self,
        sigma: f32,
        width: f32,
        mut shape: impl FnMut(f32) -> Option<(Vec<PathOp>, Option<Vec<PathOp>>)>,
    ) -> Vec<BorderFill> {
        let a = self.color[3] as f32 / 255.0;
        let bands = if sigma > 0.0 {
            (6.0 * sigma / width).ceil().clamp(2.0, 2.0 * BANDS) as usize
        } else {
            1
        };
        let step = 6.0 * sigma / bands as f32;
        let mut covered = 0.0_f32;
        let mut out = Vec::with_capacity(bands);
        for band in 0..bands {
            // Band `band` spans distances (edge - step, edge] from the
            // shape's outline; its fill covers everything inside `edge`.
            let edge = 3.0 * sigma - band as f32 * step;
            let target = if band + 1 == bands {
                a
            } else {
                a * normal_cdf((step / 2.0 - edge) / sigma)
            };
            let fill = 1.0 - (1.0 - target) / (1.0 - covered);
            covered = target;
            let alpha = (fill.clamp(0.0, 1.0) * 255.0).round() as u8;
            if alpha == 0 {
                continue;
            }
            if let Some((region, clip)) = shape(edge) {
                out.push(BorderFill {
                    region,
                    clip,
                    color: [self.color[0], self.color[1], self.color[2], alpha],
                    ring: None,
                });
            }
        }
        out
    }
}

/// A corner radius grown by a shadow's spread (CSS Backgrounds 3 §7.1.1):
/// a square corner stays square, and a radius smaller than the spread grows
/// less than it, so a small radius does not balloon.
fn spread_radius(r: f32, spread: f32) -> f32 {
    if r <= 0.0 {
        return 0.0;
    }
    if spread < 0.0 {
        return (r + spread).max(0.0);
    }
    let ratio = r / spread;
    let f = if ratio >= 1.0 {
        1.0
    } else {
        1.0 + (ratio - 1.0).powi(3)
    };
    r + spread * f
}

/// Φ(z), from erfc by Abramowitz & Stegun 7.1.26 (absolute error < 1.5e-7).
fn normal_cdf(z: f32) -> f32 {
    let t = z.abs() / std::f32::consts::SQRT_2;
    let k = 1.0 / (1.0 + 0.327_591_1 * t);
    let poly = k
        * (0.254_829_6
            + k * (-0.284_496_74 + k * (1.421_413_7 + k * (-1.453_152_1 + k * 1.061_405_4))));
    let tail = 0.5 * poly * (-t * t).exp();
    if z >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{StyleId, StyleValue};

    fn shadows(text: &str) -> Vec<ShadowPaint> {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::BoxShadow, &StyleValue::Text(text.into()))
            .unwrap();
        ShadowPaint::capture(&s, false)
    }
    fn shadow(text: &str) -> Option<ShadowPaint> {
        shadows(text).into_iter().next()
    }

    #[test]
    fn none_and_transparent_paint_nothing() {
        assert_eq!(shadow("none"), None);
        let s = shadow("0 2px 4px transparent").unwrap();
        assert!(s
            .fills(&Shape::rect((0.0, 0.0, 10.0, 10.0)), [0.0; 4])
            .is_empty());
    }

    #[test]
    fn a_hard_shadow_is_one_fill_outside_the_box() {
        let s = shadow("4px 4px 0 #3a6ea5").unwrap();
        let fills = s.fills(&Shape::new((10.0, 10.0, 100.0, 50.0), [8.0; 4]), [0.0; 4]);
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].color, [0x3a, 0x6e, 0xa5, 255]);
        // The clip is the outline moved by the offset; the region is the
        // outside of the unmoved box.
        let xs: Vec<f32> = fills[0]
            .clip
            .as_ref()
            .unwrap()
            .iter()
            .flat_map(PathOp::points)
            .collect();
        assert_eq!(xs.iter().step_by(2).cloned().fold(f32::MAX, f32::min), 14.0);
    }

    #[test]
    fn a_spread_grows_the_shape_and_an_inset_one_paints_inside() {
        let list = shadows("0 0 0 6px #000, inset 0 0 0 3px #f00");
        assert_eq!(list.len(), 2);
        let outer = list[0].fills(&Shape::rect((10.0, 10.0, 100.0, 50.0)), [0.0; 4]);
        let xs: Vec<f32> = outer[0]
            .clip
            .as_ref()
            .unwrap()
            .iter()
            .flat_map(PathOp::points)
            .collect();
        assert_eq!(xs.iter().step_by(2).cloned().fold(f32::MAX, f32::min), 4.0);
        assert!(list[1].inset());
        let inner = list[1].fills(&Shape::rect((10.0, 10.0, 100.0, 50.0)), [2.0; 4]);
        assert_eq!(inner.len(), 1);
        // The clip is the padding box; the hole is it shrunk by the spread.
        let clip: Vec<f32> = inner[0]
            .clip
            .as_ref()
            .unwrap()
            .iter()
            .flat_map(PathOp::points)
            .collect();
        assert_eq!(
            clip.iter().step_by(2).cloned().fold(f32::MAX, f32::min),
            12.0
        );
        let region: Vec<f32> = inner[0].region.iter().flat_map(PathOp::points).collect();
        assert!(region.iter().step_by(2).any(|x| *x == 15.0), "{region:?}");
    }

    #[test]
    fn a_small_radius_grows_less_than_the_spread() {
        assert_eq!(spread_radius(0.0, 10.0), 0.0);
        assert_eq!(spread_radius(20.0, 10.0), 30.0);
        assert!(spread_radius(2.0, 10.0) < 12.0);
        assert_eq!(spread_radius(5.0, -10.0), 0.0);
    }

    #[test]
    fn a_blur_stacks_to_the_gaussian() {
        let s = shadow("0 0 12px rgba(0, 0, 0, 0.5)").unwrap();
        let fills = s.fills(&Shape::rect((0.0, 0.0, 100.0, 100.0)), [0.0; 4]);
        assert!(fills.len() >= 20, "{}", fills.len());
        // Composited in order, the stack reaches the colour's alpha inside.
        let total = fills
            .iter()
            .fold(0.0, |acc, f| acc + (1.0 - acc) * f.color[3] as f32 / 255.0);
        assert!((total - 0.5).abs() < 0.02, "{total}");
        // Where the stack covers a distance, it holds the Gaussian's alpha
        // there to within half a band.
        let at = |d: f32| {
            fills
                .iter()
                .filter(|f| {
                    let x = f
                        .clip
                        .as_ref()
                        .unwrap()
                        .iter()
                        .flat_map(PathOp::points)
                        .step_by(2);
                    x.fold(f32::MAX, f32::min) <= -d
                })
                .fold(0.0, |acc, f| acc + (1.0 - acc) * f.color[3] as f32 / 255.0)
        };
        for d in [-6.0, -2.0, 0.5, 3.0, 9.0] {
            let want = 0.5 * normal_cdf(-d / 6.0);
            assert!((at(d) - want).abs() < 0.03, "{d}: {} against {want}", at(d));
        }
    }

    #[test]
    fn the_cdf_is_the_normal_one() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((normal_cdf(1.0) - 0.841_344_7).abs() < 1e-5);
        assert!((normal_cdf(-2.0) - 0.022_750_1).abs() < 1e-5);
    }
}
