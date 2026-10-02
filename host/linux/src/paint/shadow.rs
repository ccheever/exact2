//! CSS `box-shadow`, one outer shadow (LLP 1064 D2): the border box's outline,
//! offset and blurred, painted only outside the border box, under the box.
//!
//! Neither backend has a blur, and a blurred picture per node would be a
//! second raster pass. A Gaussian of a straight edge falls off as the normal
//! CDF of the distance to it, and the outline's offset curves are its
//! contours, so the shadow is a stack of offset outlines (radius grown or
//! shrunk with the distance), each filled with the alpha that brings the
//! stack to the CDF at that band: one fill per band, through the border
//! fill every backend already draws (a region inside a clip).

use super::border::{rounded_rect, shaped_rect, BorderFill, PathOp};
use super::Shape;
use exact_kernel::StyleProps;

/// The rows' shadow, resolved for the appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ShadowPaint {
    /// Straight RGBA, the opacity row folded into alpha.
    color: [u8; 4],
    offset: (f32, f32),
    /// CSS's blur radius: twice the Gaussian's standard deviation.
    blur: f32,
}

/// A band is at most this wide, in points, so a pixel is never more than
/// about a point from its band's middle...
const STEP: f32 = 1.5;
/// ...up to this many bands; wider blurs step a little coarser.
const BANDS: f32 = 32.0;

impl ShadowPaint {
    /// The node's shadow, or none when nothing would show.
    pub fn capture(s: &StyleProps, dark: bool) -> Option<ShadowPaint> {
        let c = s.shadow_color.resolve(dark);
        let alpha = (c.a() as f32 * s.shadow_opacity.clamp(0.0, 1.0)).round();
        (alpha >= 1.0 && s.shadow_offset.x.is_finite() && s.shadow_offset.y.is_finite()).then(
            || ShadowPaint {
                color: [c.r(), c.g(), c.b(), alpha as u8],
                offset: (s.shadow_offset.x, s.shadow_offset.y),
                blur: s.shadow_radius.max(0.0),
            },
        )
    }

    /// `base` with paint motion's geometry (offset, blur) and colour over it
    /// (LLP 1062); none when nothing would show.
    pub fn over(
        base: Option<ShadowPaint>,
        geometry: Option<exact_motion::Value>,
        color: Option<[u8; 4]>,
    ) -> Option<ShadowPaint> {
        let mut s = base.unwrap_or(ShadowPaint {
            color: [0; 4],
            offset: (0.0, 0.0),
            blur: 0.0,
        });
        if let Some(g) = geometry {
            s.offset = (g.x as f32, g.y as f32);
            s.blur = (g.z as f32).max(0.0);
        }
        if let Some(c) = color {
            s.color = c;
        }
        (s.color[3] >= 1).then_some(s)
    }

    /// The fills, outermost band first, around the border box `outer`.
    pub fn fills(&self, outer: &Shape) -> Vec<BorderFill> {
        let sigma = self.blur / 2.0;
        let (x, y, w, h) = outer.rect;
        let (dx, dy) = self.offset;
        let reach = 3.0 * sigma + dx.abs() + dy.abs() + 1.0;
        let mut outside = Vec::new();
        rounded_rect(
            &mut outside,
            (x - reach, y - reach, w + 2.0 * reach, h + 2.0 * reach),
            [(0.0, 0.0); 4],
        );
        shaped_rect(
            &mut outside,
            outer.rect,
            outer.radii,
            outer.corners.as_ref(),
        );
        let a = self.color[3] as f32 / 255.0;
        let bands = if sigma > 0.0 {
            (6.0 * sigma / STEP).ceil().clamp(2.0, BANDS) as usize
        } else {
            1
        };
        let step = 6.0 * sigma / bands as f32;
        let mut covered = 0.0_f32;
        let mut out = Vec::with_capacity(bands);
        for band in 0..bands {
            // Band `band` spans distances (edge - step, edge] from the
            // offset outline; its fill covers everything inside `edge`.
            let edge = 3.0 * sigma - band as f32 * step;
            let target = if band + 1 == bands {
                a
            } else {
                a * normal_cdf((step / 2.0 - edge) / sigma)
            };
            let fill = 1.0 - (1.0 - target) / (1.0 - covered);
            covered = target;
            let rect = (x + dx - edge, y + dy - edge, w + 2.0 * edge, h + 2.0 * edge);
            let alpha = (fill.clamp(0.0, 1.0) * 255.0).round() as u8;
            if rect.2 <= 0.0 || rect.3 <= 0.0 || alpha == 0 {
                continue;
            }
            let mut clip: Vec<PathOp> = Vec::new();
            let radii = outer
                .radii
                .map(|(x, y)| ((x + edge).max(0.0), (y + edge).max(0.0)));
            shaped_rect(&mut clip, rect, radii, outer.corners.as_ref());
            out.push(BorderFill {
                region: outside.clone(),
                clip: Some(clip),
                color: [self.color[0], self.color[1], self.color[2], alpha],
            });
        }
        out
    }
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

    fn shadow(text: &str) -> Option<ShadowPaint> {
        let mut s = StyleProps::default();
        for row in [
            StyleId::ShadowColor,
            StyleId::ShadowOffset,
            StyleId::ShadowRadius,
            StyleId::ShadowOpacity,
        ] {
            s.set_dynamic(row, &StyleValue::Text(text.into())).unwrap();
        }
        ShadowPaint::capture(&s, false)
    }

    #[test]
    fn none_and_transparent_paint_nothing() {
        assert_eq!(shadow("none"), None);
        assert_eq!(shadow("0 2px 4px transparent"), None);
        assert!(StyleProps::default().shadow_opacity == 0.0);
    }

    #[test]
    fn a_hard_shadow_is_one_fill_outside_the_box() {
        let s = shadow("4px 4px 0 #3a6ea5").unwrap();
        let fills = s.fills(&Shape::new((10.0, 10.0, 100.0, 50.0), [8.0; 4]));
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
    fn a_blur_stacks_to_the_gaussian() {
        let s = shadow("0 0 12px rgba(0, 0, 0, 0.5)").unwrap();
        let fills = s.fills(&Shape::rect((0.0, 0.0, 100.0, 100.0)));
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
