//! Pixel-aligned square borders need no scan conversion of their empty centre.
use super::*;

impl Raster {
    pub(super) fn fill_square_border(
        &mut self,
        part: &BorderFill,
        path: &Path,
        dev: Transform,
    ) -> bool {
        let Some(((outer, outer_radii), (inner, inner_radii))) = part.ring else {
            return false;
        };
        if part.clip.is_some()
            || part.ring_clip.is_some()
            || self.active_clip().is_some()
            || outer_radii != [(0., 0.); 4]
            || inner_radii != [(0., 0.); 4]
            || self.width > 8191
            || self.height > 8191
            || !dev.is_finite()
            || dev.kx != 0.
            || dev.ky != 0.
            || dev.sx <= 0.
            || dev.sy <= 0.
        {
            return false;
        }
        let bounds = |(x, y, w, h): Rect4| {
            [
                x * dev.sx + dev.tx,
                y * dev.sy + dev.ty,
                (x + w) * dev.sx + dev.tx,
                (y + h) * dev.sy + dev.ty,
            ]
        };
        let a = bounds(outer);
        let b = bounds(inner);
        if a.iter()
            .chain(b.iter())
            .any(|v| !v.is_finite() || v.abs() > 8191. || v.fract() != 0.)
            || b[0] < a[0]
            || b[1] < a[1]
            || b[2] > a[2]
            || b[3] > a[3]
            || b[0] > b[2]
            || b[1] > b[3]
        {
            return false;
        }
        let Some(target) = self.target.path_pixels(path, dev) else {
            return false;
        };
        // Integer shared edges have no partial coverage or overlapping ink.
        for [left, top, right, bottom] in [
            [a[0], a[1], a[2], b[1]],
            [a[0], b[3], a[2], a[3]],
            [a[0], b[1], b[0], b[3]],
            [b[2], b[1], a[2], b[3]],
        ] {
            if let Some(rect) = Rect::from_ltrb(left, top, right, bottom) {
                target.fill_rect(rect, &solid(part.color), Transform::identity(), None);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::border::border_fills;

    #[test]
    fn square_border_pixels_match_rings_with_alpha_clipping_and_dpi() {
        for scale in [1., 1.25, 1.5, 2.] {
            for widths in [[4.; 4], [0., 0., 4., 0.], [8., 4., 12., 0.], [20.; 4]] {
                for alpha in [0, 83, 255] {
                    for origin in [(-8., -4.), (4., 8.), (32., 24.)] {
                        let shape = Shape::rect((origin.0, origin.1, 40., 40.));
                        for part in border_fills(&shape, widths, [[17, 71, 183, alpha]; 4]) {
                            let mut actual = Raster::new();
                            actual.begin(60., 60., scale);
                            let mut expected = Raster::new();
                            expected.begin(60., 60., scale);
                            let path = tiny_path(&part.region).unwrap();
                            let dev = actual.device(Transform::identity());
                            assert!(actual.fill_square_border(&part, &path, dev));
                            expected.target.pixels().unwrap().fill_path(
                                &path,
                                &solid(part.color),
                                FillRule::EvenOdd,
                                dev,
                                None,
                            );
                            assert_eq!(
                                actual.finish().unwrap().data(),
                                expected.finish().unwrap().data(),
                                "scale={scale} widths={widths:?} origin={origin:?} alpha={alpha}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fractional_curved_transformed_and_clipped_borders_keep_path_painting() {
        for case in 0..5 {
            let mut r = Raster::new();
            r.begin(60., 60., 1.);
            let mut shape = Shape::rect((4., 4., 40., 40.));
            let mut dev = Transform::identity();
            match case {
                0 => shape.rect.0 += 0.25,
                1 => shape.radii = [(4., 4.); 4],
                2 => dev = Transform::from_rotate(15.),
                3 => r.push_clip(&Shape::rect((0., 0., 10., 10.)), dev),
                _ => dev = Transform::from_scale(-1., 1.),
            }
            let part = border_fills(&shape, [2.; 4], [[10, 20, 30, 255]; 4]).remove(0);
            let path = tiny_path(&part.region).unwrap();
            assert!(!r.fill_square_border(&part, &path, dev));
        }
    }
}
