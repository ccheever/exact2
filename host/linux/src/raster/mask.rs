//! Clip intersections and CSS `mask-image` for the CPU painter (LLP 1077 D2).

use super::{reach, rounded_rect, Raster, Target};
use crate::paint::{Backend, GradientPaint, Shape};
use tiny_skia::{FillRule, Mask, MaskType, Path, Pixmap, PixmapPaint, Transform};

impl Raster {
    pub(super) fn recycle_clip_mask(&self, mask: Option<std::rc::Rc<Mask>>) {
        let Some(mask) = mask else { return };
        if mask.data().len() > super::CLIP_CACHE_BYTES || self.spare_clip.borrow().is_some() {
            return;
        }
        // Only released coverage is reusable. Active and cached masks keep
        // their own Rc owners and must remain immutable.
        if let Ok(mask) = std::rc::Rc::try_unwrap(mask) {
            *self.spare_clip.borrow_mut() = Some(mask);
        }
    }

    pub(super) fn pop_masked(
        &mut self,
        shape: &Shape,
        mask: &Result<GradientPaint, [u8; 4]>,
        ts: Transform,
    ) {
        let Some((mut below, alpha)) = self.layers.pop() else {
            return;
        };
        let Some(layer) = self.target.take() else {
            self.target = Target::painted(below);
            return;
        };
        // The mask's pixels: the gradient (or its one colour) over the
        // border box, drawn as any fill is, then read as alpha.
        let alpha_mask = Pixmap::new(layer.width(), layer.height()).and_then(|blank| {
            self.target = Target::painted(blank);
            let clips = std::mem::take(&mut self.clips);
            match mask {
                Ok(gradient) => self.fill_gradient(shape, gradient, ts),
                Err(color) => self.fill(shape, *color, ts),
            }
            self.clips = clips;
            self.target
                .take()
                .map(|p| Mask::from_pixmap(p.as_ref(), MaskType::Alpha))
        });
        let paint = PixmapPaint {
            opacity: alpha,
            ..PixmapPaint::default()
        };
        below.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &paint,
            Transform::identity(),
            alpha_mask.as_ref(),
        );
        self.target = Target::painted(below);
    }
}

impl Raster {
    pub(super) fn integer_clip(&self, shape: &Shape, ts: Transform) -> Option<tiny_skia::Rect> {
        if self.width > 8191 || self.height > 8191 || shape.rounded() || shape.corners.is_some() {
            return None;
        }
        let [left, top, right, bottom] = super::image::integer_bounds(shape.rect, self.device(ts))?;
        let parent = if self.active_clip().is_some() {
            self.rectangular_clip
                .filter(|(depth, _)| *depth == self.clips.len())?
                .1
        } else {
            tiny_skia::Rect::from_xywh(0., 0., self.width as f32, self.height as f32)?
        };
        tiny_skia::Rect::from_ltrb(
            left.max(parent.left()),
            top.max(parent.top()),
            right.min(parent.right()),
            bottom.min(parent.bottom()),
        )
    }

    pub(super) fn remember_integer_clip(&mut self, rect: Option<tiny_skia::Rect>) {
        if let Some(rect) = rect {
            self.rectangular_clip = Some((self.clips.len(), rect));
        }
    }

    /// The current clip intersected with more shapes, as a mask of its own.
    pub(super) fn mask_with(&self, shapes: &[Shape], ts: Transform) -> Option<Mask> {
        let dev = self.device(ts);
        // A clip tiny-skia cannot reach (an SVG viewport's or mask region's)
        // shows nothing.
        if shapes
            .iter()
            .filter_map(rounded_rect)
            .any(|p| !reach::within(&p, 0.0, dev))
        {
            return self.new_clip_mask();
        }
        let mut m = match self.active_clip() {
            Some(c) => {
                if let Some((depth, rect)) = self.rectangular_clip {
                    if depth == self.clips.len() {
                        let mut paths = shapes.iter().filter_map(rounded_rect);
                        if let Some(first) = paths.next() {
                            // A proven integer clip rectangle has only 0/255
                            // coverage. Build the child once and clear outside it.
                            let mut mask = self.new_clip_mask()?;
                            mask.fill_path(&first, FillRule::Winding, true, dev);
                            let width = self.width as usize;
                            let (left, top, right, bottom) = (
                                rect.left() as usize,
                                rect.top() as usize,
                                rect.right() as usize,
                                rect.bottom() as usize,
                            );
                            let data = mask.data_mut();
                            data[..top * width].fill(0);
                            data[bottom * width..].fill(0);
                            for row in data[top * width..bottom * width].chunks_exact_mut(width) {
                                row[..left].fill(0);
                                row[right..].fill(0);
                            }
                            for path in paths {
                                mask.intersect_path(&path, FillRule::Winding, true, dev);
                            }
                            return Some(mask);
                        }
                    }
                }
                let mut paths = shapes.iter().filter_map(rounded_rect);
                if let Some(first) = paths.next() {
                    let mut mask = self.new_clip_mask()?;
                    mask.fill_path(&first, FillRule::Winding, true, dev);
                    // Multiplication commutes for this first intersection. The
                    // new mask is zero outside the child's path bounds.
                    intersect(&mut mask, c, &first, dev, true);
                    for path in paths {
                        mask.intersect_path(&path, FillRule::Winding, true, dev);
                    }
                    return Some(mask);
                }
                (**c).clone()
            }
            None => {
                let mut m = self.new_clip_mask()?;
                if !self.clips.is_empty() {
                    m.data_mut().fill(255);
                    for path in shapes.iter().filter_map(rounded_rect) {
                        m.intersect_path(&path, FillRule::Winding, true, dev);
                    }
                    return Some(m);
                }
                let first = rounded_rect(shapes.first()?)?;
                m.fill_path(&first, FillRule::Winding, true, dev);
                return Some(
                    shapes[1..]
                        .iter()
                        .filter_map(rounded_rect)
                        .fold(m, |mut m, p| {
                            m.intersect_path(&p, FillRule::Winding, true, dev);
                            m
                        }),
                );
            }
        };
        for p in shapes.iter().filter_map(rounded_rect) {
            m.intersect_path(&p, FillRule::Winding, true, dev);
        }
        Some(m)
    }
}

pub(super) fn intersect_mask(mask: &mut Mask, parent: &Mask, path: &Path, dev: Transform) {
    intersect(mask, parent, path, dev, false);
}

fn intersect(mask: &mut Mask, parent: &Mask, path: &Path, dev: Transform, round: bool) {
    // A new path mask is zero outside its control bounds. Include AA slack;
    // tiny-skia tiles above 8191 pixels, where the full-mask path stays in use.
    let (width, height) = (mask.width() as usize, mask.height() as usize);
    let bounds = if width <= 8191 && height <= 8191 {
        path.clone().transform(dev).and_then(|p| {
            let b = p.bounds();
            let edges = [b.left(), b.top(), b.right(), b.bottom()];
            edges
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 8191.0)
                .then(|| {
                    [
                        (b.left().floor() - 2.0).clamp(0.0, width as f32) as usize,
                        (b.top().floor() - 2.0).clamp(0.0, height as f32) as usize,
                        (b.right().ceil() + 2.0).clamp(0.0, width as f32) as usize,
                        (b.bottom().ceil() + 2.0).clamp(0.0, height as f32) as usize,
                    ]
                })
        })
    } else {
        None
    };
    let [left, top, right, bottom] = bounds.unwrap_or([0, 0, width, height]);
    for y in top..bottom {
        let span = y * width + left..y * width + right;
        for (a, b) in mask.data_mut()[span.clone()]
            .iter_mut()
            .zip(&parent.data()[span])
        {
            *a = ((u16::from(*a) * u16::from(*b) + if round { 127 } else { 0 }) / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::FillRule;

    #[test]
    fn released_mask_storage_is_cleared_reused_and_bounded() {
        for scale in [1., 1.25, 1.5, 2.] {
            let mut r = Raster::new();
            r.begin(64., 56., scale);
            let identity = Transform::identity();
            let parent_shape = Shape::new((2.25, 4.5, 58., 48.), [5.; 4]);
            r.push_clip(&parent_shape, identity);
            let parent = r.active_clip().unwrap().clone();
            for shape in [
                Shape::new((4.25, 5.75, 28., 36.), [6.; 4]),
                Shape::rect((40., 8., 12., 12.)),
                Shape::new((10.125, 15.25, 35., 28.), [9.; 4]),
            ] {
                let mut expected = (*parent).clone();
                expected.intersect_path(
                    &rounded_rect(&shape).unwrap(),
                    FillRule::Winding,
                    true,
                    r.device(identity),
                );
                r.push_clip(&shape, identity);
                assert_eq!(r.active_clip().unwrap().data(), expected.data());
                assert_eq!(r.clip_allocations.get(), 2);
                r.pop_clip();
                assert!(r.spare_clip.borrow().is_some());
                assert_eq!(r.active_clip().unwrap().data(), parent.data());
            }
            r.pop_clip();
            assert_eq!(parent.data(), r.cached_clip.as_ref().unwrap().1.data());
            let blank = r.new_clip_mask().unwrap();
            assert!(blank.data().iter().all(|byte| *byte == 0));
            r.recycle_clip_mask(Some(std::rc::Rc::new(blank)));
            r.begin(65., 56., scale);
            assert!(r.spare_clip.borrow().is_none());
            let oversize = Mask::new(1025, 1024).unwrap();
            r.recycle_clip_mask(Some(std::rc::Rc::new(oversize)));
            assert!(r.spare_clip.borrow().is_none());
        }
    }

    #[test]
    fn integer_pane_clips_keep_fills_exact_across_nested_clips_and_pop() {
        use super::super::{solid, Color};
        for scale in [1., 1.25, 1.5, 2.] {
            for case in 0..5 {
                let mut r = Raster::new();
                r.begin(64., 56., scale);
                for (i, p) in r
                    .target
                    .pixels()
                    .unwrap()
                    .pixels_mut()
                    .iter_mut()
                    .enumerate()
                {
                    *p = Color::from_rgba8(31, (i * 37) as u8, 193, (i * 19) as u8)
                        .premultiply()
                        .to_color_u8();
                }
                let mut expected = r.target.pixels().unwrap().clone();
                let mut outer = Shape::rect((8., 12., 44., 36.));
                let mut transform = Transform::identity();
                match case {
                    1 => outer.rect.0 += 0.125,
                    2 => outer.radii = [(5., 7.); 4],
                    3 => transform = Transform::from_rotate_at(7., 30., 28.),
                    4 => transform = Transform::from_scale(-1., 1.).post_translate(64., 0.),
                    _ => {}
                }
                let mut masks = Vec::new();
                for (shape, ts) in [
                    (outer, transform),
                    (Shape::rect((16., 8., 40., 36.)), Transform::identity()),
                    (
                        Shape::new((20.25, 15.5, 30., 28.), [4.; 4]),
                        Transform::identity(),
                    ),
                ] {
                    let mut mask = masks.last().cloned().unwrap_or_else(|| {
                        let mut mask = Mask::new(r.width, r.height).unwrap();
                        mask.data_mut().fill(255);
                        mask
                    });
                    mask.intersect_path(
                        &rounded_rect(&shape).unwrap(),
                        FillRule::Winding,
                        true,
                        r.device(ts),
                    );
                    masks.push(mask);
                    r.push_clip(&shape, ts);
                    assert_eq!(
                        r.active_clip().unwrap().data(),
                        masks.last().unwrap().data()
                    );
                    if case == 0 && masks.len() < 3 {
                        assert_eq!(r.rectangular_clip.unwrap().0, r.clips.len());
                    }
                    for alpha in [0, 1, 127, 254, 255] {
                        for bounds in [
                            (-4., 0., 72., 60.),
                            (20., 24., 12., 8.),
                            (17., 17., 1., 1.),
                            (56., 48., 4., 4.),
                        ] {
                            let fill = Shape::rect(bounds);
                            let color = [83, 215, 151, alpha];
                            r.fill(&fill, color, Transform::identity());
                            expected.fill_path(
                                &rounded_rect(&fill).unwrap(),
                                &solid(color),
                                FillRule::Winding,
                                r.device(Transform::identity()),
                                masks.last(),
                            );
                            assert_eq!(
                                r.target.pixels().unwrap().data(),
                                expected.data(),
                                "scale={scale} case={case} depth={} alpha={alpha} fill={bounds:?}",
                                masks.len()
                            );
                        }
                    }
                }
                while !masks.is_empty() {
                    masks.pop();
                    r.pop_clip();
                    let fill = Shape::rect((0.25, 0.5, 63., 55.));
                    r.fill(&fill, [117, 43, 201, 255], Transform::identity());
                    expected.fill_path(
                        &rounded_rect(&fill).unwrap(),
                        &solid([117, 43, 201, 255]),
                        FillRule::Winding,
                        r.device(Transform::identity()),
                        masks.last(),
                    );
                    assert_eq!(
                        r.target.pixels().unwrap().data(),
                        expected.data(),
                        "pop scale={scale} case={case} depth={}",
                        masks.len()
                    );
                }
            }
        }
    }

    #[test]
    fn bounded_child_intersection_preserves_parent_coverage_and_order() {
        for scale in [1., 1.25, 1.5, 2.] {
            for transform in [
                Transform::identity(),
                Transform::from_translate(-3.75, 5.125),
                Transform::from_rotate_at(17., 25., 25.),
            ] {
                let mut r = Raster::new();
                r.begin(60., 60., scale);
                let mut parent = Shape::rect((2.25, 1.125, 45.5, 42.75));
                parent.radii = [(8.25, 11.5); 4];
                r.push_clip(&parent, Transform::identity());
                let mut child = Shape::rect((4.5, -2.25, 38.125, 32.75));
                child.radii = [(7.125, 9.25); 4];
                for shapes in [
                    vec![],
                    vec![child],
                    vec![child, parent],
                    vec![Shape::rect((0., 0., 0., 0.)), child, parent],
                    vec![Shape::rect((100., 100., 10., 10.))],
                ] {
                    let mut reference = (**r.active_clip().unwrap()).clone();
                    for path in shapes.iter().filter_map(super::super::rounded_rect) {
                        reference.intersect_path(
                            &path,
                            FillRule::Winding,
                            true,
                            r.device(transform),
                        );
                    }
                    let actual = r.mask_with(&shapes, transform).unwrap();
                    assert_eq!(
                        actual.data(),
                        reference.data(),
                        "scale={scale} transform={transform:?} shapes={shapes:?}"
                    );
                }
            }
        }
    }
}
