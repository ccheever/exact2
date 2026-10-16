//! Large rounded panels have a rectangular interior with full coverage. Blend
//! that interior with SSE2; let tiny-skia rasterize the curved end strips.
use super::*;

impl Raster {
    pub(super) fn fill_solid_middle(
        &mut self,
        shape: &Shape,
        color: [u8; 4],
        path: &Path,
        dev: Transform,
    ) -> bool {
        if self.active_clip().is_some()
            || shape.corners.is_some()
            || !shape.rounded()
            || !dev.is_finite()
            || dev.kx != 0.
            || dev.ky != 0.
            || dev.sx <= 0.
            || dev.sy <= 0.
            || self.width > 8191
            || self.height > 8191
        {
            return false;
        }
        let (x, y, w, h) = shape.rect;
        if ![x, y, w, h].iter().all(|v| v.is_finite())
            || !shape.radii.iter().all(|&(rx, ry)| {
                rx.is_finite()
                    && ry.is_finite()
                    && rx >= 0.
                    && ry >= 0.
                    && rx <= w / 2.
                    && ry <= h / 2.
            })
        {
            return false;
        }
        let left = x * dev.sx + dev.tx;
        let right = (x + w) * dev.sx + dev.tx;
        let top = y * dev.sy + dev.ty;
        let bottom = (y + h) * dev.sy + dev.ty;
        if ![left, right, top, bottom]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 8191.)
            || left.fract() != 0.
            || right.fract() != 0.
        {
            return false;
        }
        let [tl, tr, br, bl] = shape.radii;
        // Stay away from curve endpoints and their antialiasing footprint.
        let start = (top + tl.1.max(tr.1) * dev.sy + 2.).ceil().max(0.) as u32;
        let end = (bottom - bl.1.max(br.1) * dev.sy - 2.)
            .floor()
            .max(0.)
            .min(self.height as f32) as u32;
        let left = left.max(0.).min(self.width as f32) as usize;
        let right = right.max(0.).min(self.width as f32) as usize;
        if end <= start || end - start < 16 || right.saturating_sub(left) < 256 {
            return false;
        }
        let Some(path) = path.clone().transform(dev) else {
            return false;
        };
        // Cancel the integer interior with an opposite-winding contour. Keep
        // every original curve coordinate unchanged, including its AA rounding.
        let mut edges = PathBuilder::new();
        edges.push_path(&path);
        edges.move_to(left as f32, start as f32);
        edges.line_to(left as f32, end as f32);
        edges.line_to(right as f32, end as f32);
        edges.line_to(right as f32, start as f32);
        edges.close();
        let Some(edges) = edges.finish() else {
            return false;
        };
        let Some(target) = self.target.as_mut() else {
            return false;
        };
        target.fill_path(
            &edges,
            &solid(color),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        let c = Color::from_rgba8(color[0], color[1], color[2], color[3]).premultiply();
        let rgba = [c.red(), c.green(), c.blue(), c.alpha()].map(|v| (v * 255. + 0.5) as u16);
        let stride = self.width as usize * 4;
        for row in target.data_mut()[start as usize * stride..end as usize * stride]
            .chunks_exact_mut(stride)
        {
            blend(&mut row[left * 4..right * 4], rgba);
        }
        true
    }
}

fn blend(bytes: &mut [u8], rgba: [u16; 4]) {
    let inverse = 255 - rgba[3];
    let offset = {
        use std::arch::x86_64::*;
        let mut offset = 0;
        // SSE2 is baseline on x86-64. Unaligned loads/stores stay within each
        // complete 16-byte chunk; all products and sums fit in unsigned u16.
        #[allow(unsafe_code)]
        unsafe {
            let zero = _mm_setzero_si128();
            let inv = _mm_set1_epi16(inverse as i16);
            let rounding = _mm_set1_epi16(255);
            let src = _mm_setr_epi16(
                rgba[0] as i16,
                rgba[1] as i16,
                rgba[2] as i16,
                rgba[3] as i16,
                rgba[0] as i16,
                rgba[1] as i16,
                rgba[2] as i16,
                rgba[3] as i16,
            );
            while offset + 16 <= bytes.len() {
                let address = bytes.as_mut_ptr().add(offset).cast::<__m128i>();
                let dst = _mm_loadu_si128(address);
                let lo = _mm_unpacklo_epi8(dst, zero);
                let hi = _mm_unpackhi_epi8(dst, zero);
                let over = |v| {
                    _mm_add_epi16(
                        src,
                        _mm_srli_epi16::<8>(_mm_add_epi16(_mm_mullo_epi16(v, inv), rounding)),
                    )
                };
                _mm_storeu_si128(address, _mm_packus_epi16(over(lo), over(hi)));
                offset += 16;
            }
        }
        offset
    };
    for pixel in bytes[offset..].chunks_exact_mut(4) {
        for i in 0..4 {
            // tiny-skia lowp source-over uses ceil(product / 256), not / 255.
            pixel[i] = (rgba[i] + ((pixel[i] as u16 * inverse + 255) >> 8)) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_panel_matches_reference_and_refusals_do_not_write() {
        let mut r = Raster::new();
        r.begin(5120., 1440., 1.);
        let mut shape = Shape::rect((16., 16., 5088., 161.));
        shape.radii = [(8., 8.); 4];
        let color = [20, 25, 29, 232];
        let mut expected = r.target.as_ref().unwrap().clone();
        expected.fill_path(
            &rounded_rect(&shape).unwrap(),
            &solid(color),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        r.fill(&shape, color, Transform::identity());
        assert!(r.target.as_ref().unwrap().data() == expected.data());

        for case in 0..7 {
            let mut r = Raster::new();
            r.begin(420., 200., 1.);
            let mut shape = Shape::rect((4., 16., 408., 120.));
            shape.radii = [(8., 8.); 4];
            let mut dev = Transform::identity();
            match case {
                0 => r.push_clip(&Shape::rect((8., 8., 200., 100.)), dev),
                1 => dev = Transform::from_rotate(5.),
                2 => dev = Transform::from_scale(-1., 1.),
                3 => dev = Transform::from_translate(0.25, 0.),
                4 => shape.radii[0].1 = 80.,
                5 => shape.rect.2 = 100.,
                6 => shape.rect.1 = 300.,
                _ => unreachable!(),
            }
            let previous = r.target.as_ref().unwrap().data().to_vec();
            let path = rounded_rect(&shape).unwrap();
            assert!(
                !r.fill_solid_middle(&shape, color, &path, dev),
                "case={case}"
            );
            assert!(r.target.as_ref().unwrap().data() == previous);
        }
    }

    #[test]
    fn blend_matches_tiny_skia_with_unaligned_tails_and_all_alpha_values() {
        for alpha in 0..=255 {
            let color = [219, 71, 253, alpha];
            let c = Color::from_rgba8(color[0], color[1], color[2], alpha).premultiply();
            let rgba = [c.red(), c.green(), c.blue(), c.alpha()].map(|v| (v * 255. + 0.5) as u16);
            for count in [1, 3, 4, 5, 17, 31] {
                let mut reference = Pixmap::new(count, 1).unwrap();
                for (i, pixel) in reference.pixels_mut().iter_mut().enumerate() {
                    *pixel = Color::from_rgba8(11, 252, 139, (i * 31) as u8)
                        .premultiply()
                        .to_color_u8();
                }
                let mut actual = vec![193; reference.data().len() + 6];
                actual[3..3 + reference.data().len()].copy_from_slice(reference.data());
                blend(&mut actual[3..3 + reference.data().len()], rgba);
                reference.fill_path(
                    &PathBuilder::from_rect(Rect::from_xywh(0., 0., count as f32, 1.).unwrap()),
                    &solid(color),
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
                assert_eq!(&actual[3..3 + reference.data().len()], reference.data());
                assert_eq!(&actual[..3], &[193; 3]);
                assert_eq!(&actual[actual.len() - 3..], &[193; 3]);
            }
        }
    }

    #[test]
    fn rounded_middle_matches_reference_over_translucent_pixels() {
        for scale in [1., 1.25, 1.5, 2.] {
            for y in [-12.25, 0., 16., 17.75123, 110.] {
                for alpha in [0, 1, 83, 127, 232, 254, 255] {
                    let mut r = Raster::new();
                    r.begin(420., 200., scale);
                    let target = r.target.as_mut().unwrap();
                    for (i, p) in target.pixels_mut().iter_mut().enumerate() {
                        *p = Color::from_rgba8(
                            (i * 17) as u8,
                            (i * 41) as u8,
                            (i * 7) as u8,
                            (i * 3) as u8,
                        )
                        .premultiply()
                        .to_color_u8();
                    }
                    let mut expected = target.clone();
                    let mut shape = Shape::rect((4., y, 408., 120.));
                    shape.radii = [(8., 9.), (17., 13.), (12., 19.), (6., 7.)];
                    let path = rounded_rect(&shape).unwrap();
                    let color = [20, 25, 29, alpha];
                    let dev = r.device(Transform::identity());
                    expected.fill_path(&path, &solid(color), FillRule::Winding, dev, None);
                    assert!(r.fill_solid_middle(&shape, color, &path, dev));
                    assert!(
                        r.target.as_ref().unwrap().data() == expected.data(),
                        "scale={scale} y={y} alpha={alpha}"
                    );
                }
            }
        }
    }
}
