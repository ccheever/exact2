//! Exact full-target opaque canvas copy; every other canvas uses tiny-skia.
//! LLP 1015.001: no retained opacity promise or cache, and no epsilon geometry.
use super::*;

impl Raster {
    pub(super) fn copy_opaque_canvas(
        &mut self,
        image: &Arc<Pixmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
    ) -> bool {
        let Some(target) = &mut self.target else {
            return false;
        };
        if dst.0 != 0.
            || dst.1 != 0.
            || dst.2 <= 0.
            || dst.3 <= 0.
            || image.width() != target.width()
            || image.height() != target.height()
            || dst.2 * self.scale != target.width() as f32
            || dst.3 * self.scale != target.height() as f32
            || !ts.is_identity()
        {
            return false;
        }
        let dev = Transform::from_scale(self.scale, self.scale).pre_concat(Transform::from_scale(
            dst.2 / image.width() as f32,
            dst.3 / image.height() as f32,
        ));
        if !dev.is_identity()
            || clips.iter().any(|clip| {
                clip.rect != dst || clip.radii != [(0., 0.); 4] || clip.corners.is_some()
            })
            || self.clips.last().is_some_and(|mask| {
                mask.width() != target.width()
                    || mask.height() != target.height()
                    || mask.data().iter().any(|&a| a != 255)
            })
            || image.data().chunks_exact(4).any(|pixel| pixel[3] != 255)
        {
            return false;
        }
        // The complete proof above precedes every write, including the last alpha
        // or mask byte. Source-over of opaque bytes is exactly a replacement.
        target.data_mut().copy_from_slice(image.data());
        true
    }

    pub(super) fn raster_canvas(
        &mut self,
        image: &Arc<Pixmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
    ) {
        let (nw, nh) = (image.width() as f32, image.height() as f32);
        if nw <= 0.0 || nh <= 0.0 || dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let mask = self.mask_with(clips, ts);
        let dev = self
            .device(ts)
            .pre_concat(Transform::from_translate(dst.0, dst.1).pre_scale(dst.2 / nw, dst.3 / nh));
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        if let Some(t) = self.target.as_mut() {
            t.draw_pixmap(0, 0, image.as_ref().as_ref(), &paint, dev, mask.as_ref());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(w: u32, h: u32) -> Arc<Pixmap> {
        let mut p = Pixmap::new(w, h).unwrap();
        for (i, pixel) in p.data_mut().chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[(i * 17) as u8, (i * 41) as u8, (i * 7) as u8, 255]);
        }
        Arc::new(p)
    }
    fn raster(scale: f32) -> Raster {
        let mut r = Raster::new();
        r.begin(40., 24., scale);
        r.target
            .as_mut()
            .unwrap()
            .fill(Color::from_rgba8(53, 101, 177, 255));
        r
    }
    #[test]
    fn opaque_canvas_matches_reference_at_dpi_and_inside_opacity_layers() {
        for scale in [1., 1.25, 1.5, 2.] {
            for parent in [false, true] {
                for opacity in [None, Some(0.37)] {
                    let mut actual = raster(scale);
                    let mut reference = raster(scale);
                    let dst = (0., 0., 40., 24.);
                    let clips = [Shape::rect(dst); 2];
                    let source = image(actual.width, actual.height);
                    let before = source.data().to_vec();
                    for r in [&mut actual, &mut reference] {
                        if parent {
                            r.push_clip(&Shape::rect(dst), Transform::identity());
                        }
                        if let Some(opacity) = opacity {
                            r.push_opacity(opacity);
                        }
                    }
                    assert!(actual.copy_opaque_canvas(&source, dst, &clips, Transform::identity()));
                    reference.raster_canvas(&source, dst, &clips, Transform::identity());
                    if opacity.is_some() {
                        actual.pop_opacity();
                        reference.pop_opacity();
                    }
                    assert_eq!(
                        actual.finish().unwrap().data(),
                        reference.finish().unwrap().data(),
                        "scale={scale} parent={parent} opacity={opacity:?}"
                    );
                    assert_eq!(source.data(), before);
                }
            }
        }
    }
    #[test]
    fn opaque_canvas_refusals_are_atomic_and_fallback_pixels_match() {
        for case in 0..15 {
            let mut actual = raster(1.);
            let mut reference = raster(1.);
            let mut source = image(40, 24);
            let mut dst = (0., 0., 40., 24.);
            let mut clips = vec![Shape::rect(dst); 2];
            let mut ts = Transform::identity();
            match case {
                0..=2 => {
                    let len = source.data().len();
                    let i = match case {
                        0 => 0,
                        1 => len / 8 * 4,
                        _ => len - 4,
                    };
                    Arc::make_mut(&mut source).data_mut()[i..i + 4]
                        .copy_from_slice(&[1, 2, 3, 127]);
                }
                3 => {
                    let mut mask = Mask::new(40, 24).unwrap();
                    mask.data_mut().fill(255);
                    *mask.data_mut().last_mut().unwrap() = 254;
                    actual.clips.push(Rc::new(mask.clone()));
                    reference.clips.push(Rc::new(mask));
                }
                4 => clips[0].rect = (0., 0., 39., 24.),
                5 => clips[0] = Shape::new(dst, [2.; 4]),
                6 => ts = Transform::from_translate(0.5, 0.),
                7 => ts = Transform::from_scale(f32::from_bits(1f32.to_bits() + 1), 1.),
                8 => source = image(39, 24),
                9 => dst.0 = 1.,
                10 => dst.2 = 39.,
                11 => ts = Transform::from_scale(2., 2.),
                12 => {
                    actual = raster(1.1);
                    reference = raster(1.1);
                    source = image(actual.width, actual.height);
                }
                13 => {
                    let mut mask = Mask::new(40, 24).unwrap();
                    mask.data_mut().fill(255);
                    mask.data_mut()[0] = 0;
                    actual.clips.push(Rc::new(mask.clone()));
                    reference.clips.push(Rc::new(mask));
                }
                14 => dst.1 = -0.5,
                _ => unreachable!(),
            }
            let source_before = source.data().to_vec();
            let target_before = actual.target.as_ref().unwrap().data().to_vec();
            assert!(
                !actual.copy_opaque_canvas(&source, dst, &clips, ts),
                "case {case}"
            );
            assert_eq!(
                actual.target.as_ref().unwrap().data(),
                target_before,
                "case {case} mutated before refusal"
            );
            actual.canvas(&source, dst, &clips, ts);
            reference.raster_canvas(&source, dst, &clips, ts);
            assert_eq!(
                actual.finish().unwrap().data(),
                reference.finish().unwrap().data(),
                "case {case}"
            );
            assert_eq!(source.data(), source_before);
        }
    }
}
