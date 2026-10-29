//! A picture entirely inside binary rectangular clips needs only its parent mask.
use super::*;

impl Raster {
    pub(super) fn image_inside_clips(
        &self,
        width: f32,
        height: f32,
        image: Transform,
        clips: &[Shape],
        ts: Transform,
    ) -> bool {
        let Some(ink) = axis_bounds((0., 0., width, height), image) else {
            return false;
        };
        // Fractional image edges still cover only their floor/ceil pixel box.
        // A binary clip enclosing that box cannot change any sample.
        let ink = [ink[0].floor(), ink[1].floor(), ink[2].ceil(), ink[3].ceil()];
        self.width <= 8191
            && self.height <= 8191
            && clips.iter().all(|clip| {
                clip.radii == [(0., 0.); 4]
                    && clip.corners.is_none()
                    && integer_bounds(clip.rect, self.device(ts)).is_some_and(|bounds| {
                        bounds[0] <= ink[0]
                            && bounds[1] <= ink[1]
                            && bounds[2] >= ink[2]
                            && bounds[3] >= ink[3]
                    })
            })
    }
}

pub(super) fn integer_bounds(rect: Rect4, ts: Transform) -> Option<[f32; 4]> {
    axis_bounds(rect, ts).filter(|bounds| bounds.iter().all(|v| v.fract() == 0.))
}

fn axis_bounds(rect: Rect4, ts: Transform) -> Option<[f32; 4]> {
    if !ts.is_finite()
        || ts.kx != 0.
        || ts.ky != 0.
        || ts.sx <= 0.
        || ts.sy <= 0.
        || rect.2 <= 0.
        || rect.3 <= 0.
    {
        return None;
    }
    let mut corners = [
        Point::from_xy(rect.0, rect.1),
        Point::from_xy(rect.0 + rect.2, rect.1 + rect.3),
    ];
    ts.map_points(&mut corners);
    let bounds = [corners[0].x, corners[0].y, corners[1].x, corners[1].y];
    bounds
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 8191.)
        .then_some(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masked_image(
        raster: &mut Raster,
        bitmap: &Bitmap,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    ) {
        let dev = raster
            .device(ts)
            .pre_concat(Transform::from_translate(dst.0, dst.1).pre_scale(
                dst.2 / bitmap.width() as f32,
                dst.3 / bitmap.height() as f32,
            ));
        let mask = raster.mask_with(clips, ts);
        if tint.is_some() {
            raster.push_opacity(1.);
        }
        let target = raster.target.pixels().unwrap();
        target.draw_pixmap(
            0,
            0,
            bitmap.pixels(),
            &PixmapPaint {
                quality: FilterQuality::Bilinear,
                ..PixmapPaint::default()
            },
            dev,
            mask.as_ref(),
        );
        if let Some(tint) = tint {
            let mut paint = solid(tint);
            paint.blend_mode = tiny_skia::BlendMode::SourceIn;
            let rect =
                Rect::from_xywh(0., 0., target.width() as f32, target.height() as f32).unwrap();
            target.fill_rect(rect, &paint, Transform::identity(), None);
            raster.pop_opacity();
        }
    }

    #[test]
    fn image_clips_match_masks_at_dpi_with_alpha_tint_and_parent_clipping() {
        let gate = exact_raster::Gate::new();
        let session = gate.session();
        let reservation = session.reserve_allocation(32 * 24 * 4).unwrap();
        let mut pixels = Pixmap::new(32, 24).unwrap();
        for (i, p) in pixels.pixels_mut().iter_mut().enumerate() {
            *p = Color::from_rgba8((i % 256) as u8, 70, 190, (i % 256) as u8)
                .premultiply()
                .to_color_u8();
        }
        let bitmap = Arc::new(Bitmap::new(pixels, (32, 24), reservation.charge()));
        let _charge = reservation.commit(32 * 24 * 4).unwrap();
        for scale in [1., 1.25, 1.5, 2.] {
            for parent in [false, true] {
                for tint in [None, Some([173, 65, 20, 180])] {
                    for case in 0..6 {
                        let mut dst = (4., 8., 32., 24.);
                        if case == 1 {
                            dst.0 = -4.;
                        }
                        if case == 2 {
                            dst.1 += 0.25;
                        }
                        let mut clips = [Shape::rect(dst); 2];
                        if case == 3 {
                            clips[1].radii = [(4., 4.); 4];
                        }
                        if case == 4 {
                            clips[1].rect.2 -= 4.;
                        }
                        let ts = if case == 5 {
                            Transform::from_rotate(15.)
                        } else {
                            Transform::identity()
                        };
                        let mut actual = Raster::new();
                        let mut expected = Raster::new();
                        for r in [&mut actual, &mut expected] {
                            r.begin(48., 48., scale);
                            if parent {
                                r.push_clip(
                                    &Shape::new((6.25, 7.5, 22.5, 28.25), [4.; 4]),
                                    Transform::identity(),
                                );
                            }
                        }
                        let dev = actual
                            .device(ts)
                            .pre_concat(Transform::from_translate(dst.0, dst.1));
                        assert_eq!(
                            actual.image_inside_clips(32., 24., dev, &clips, ts),
                            case < 2
                        );
                        actual.image(&bitmap, dst, &clips, ts, tint);
                        masked_image(&mut expected, &bitmap, dst, &clips, ts, tint);
                        assert_eq!(
                            actual.finish().unwrap().data(),
                            expected.finish().unwrap().data(),
                            "scale={scale} parent={parent} tint={tint:?} case={case}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fractional_images_inside_binary_clips_match_masks_without_allocating_one() {
        let gate = exact_raster::Gate::new();
        let session = gate.session();
        let reservation = session.reserve_allocation(32 * 24 * 4).unwrap();
        let mut pixels = Pixmap::new(32, 24).unwrap();
        for (i, p) in pixels.pixels_mut().iter_mut().enumerate() {
            *p = Color::from_rgba8(220, (i % 256) as u8, 70, (i % 256) as u8)
                .premultiply()
                .to_color_u8();
        }
        let bitmap = Arc::new(Bitmap::new(pixels, (32, 24), reservation.charge()));
        let _charge = reservation.commit(32 * 24 * 4).unwrap();
        for scale in [1., 1.25, 1.5, 2.] {
            for parent in [false, true] {
                for tint in [None, Some([173, 65, 20, 180])] {
                    for x in [-4.5, 0.25, 4.5, 16.25] {
                        for width in [31.5, 32.25] {
                            let dst = (x, 4.375, width, 23.625);
                            let clips = [Shape::rect((0., 0., 40., 40.)); 2];
                            let mut actual = Raster::new();
                            let mut expected = Raster::new();
                            for r in [&mut actual, &mut expected] {
                                r.begin(48., 48., scale);
                                if parent {
                                    r.push_clip(
                                        &Shape::new((6.25, 7.5, 22.5, 28.25), [4.; 4]),
                                        Transform::identity(),
                                    );
                                }
                            }
                            let inside = x >= 0. && x + width <= 40.;
                            let before = actual.clip_allocations.get();
                            actual.image(&bitmap, dst, &clips, Transform::identity(), tint);
                            assert_eq!(actual.clip_allocations.get() == before, inside);
                            masked_image(
                                &mut expected,
                                &bitmap,
                                dst,
                                &clips,
                                Transform::identity(),
                                tint,
                            );
                            assert_eq!(
                                actual.finish().unwrap().data(),
                                expected.finish().unwrap().data(),
                                "scale={scale} parent={parent} tint={tint:?} dst={dst:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
