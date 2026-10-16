//! A covering clear is resolved only when its pixels are read or drawn into.
//! Keeping the storage private makes layer transfers resolve it too.
use crate::paint::WindowLayer;
use tiny_skia::{Color, Pixmap};

#[derive(Default)]
pub(super) struct Target {
    pixels: Option<Pixmap>,
    clear: Option<Color>,
    #[cfg(test)]
    clears: usize,
}

impl Target {
    pub(super) fn painted(pixels: Pixmap) -> Self {
        Self {
            pixels: Some(pixels),
            ..Self::default()
        }
    }

    pub(super) fn cleared(pixels: Option<Pixmap>, color: Color) -> Self {
        Self {
            pixels,
            clear: Some(color),
            #[cfg(test)]
            clears: 0,
        }
    }

    pub(super) fn clear(&mut self, color: Color) {
        self.clear = Some(color);
    }

    pub(super) fn pixels(&mut self) -> Option<&mut Pixmap> {
        if let (Some(color), Some(pixels)) = (self.clear.take(), self.pixels.as_mut()) {
            pixels.fill(color);
            #[cfg(test)]
            {
                self.clears += 1;
            }
        }
        self.pixels.as_mut()
    }

    pub(super) fn take(&mut self) -> Option<Pixmap> {
        self.pixels();
        self.pixels.take()
    }

    pub(super) fn take_layer(&mut self) -> Option<WindowLayer> {
        let pixels = self.pixels.take()?;
        Some(match self.clear.take() {
            Some(color) => {
                let c = color.premultiply().to_color_u8();
                WindowLayer::Solid {
                    color: [c.red(), c.green(), c.blue(), c.alpha()],
                    storage: pixels,
                }
            }
            None => WindowLayer::Pixels(pixels),
        })
    }

    pub(super) fn replace(&mut self, pixels: Pixmap) -> Option<Pixmap> {
        let old = self.take();
        self.pixels = Some(pixels);
        old
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn covering_fills_replace_clears_but_reads_and_transfers_resolve_them() {
        let full = Shape::rect((0., 0., 40., 24.));
        let identity = Transform::identity();
        let a = [31, 72, 109, 255];
        let b = [210, 40, 60, 255];
        let mut r = Raster::new();
        r.begin(40., 24., 1.);
        r.fill(&full, a, identity);
        r.fill(&full, b, identity);
        assert_eq!(r.target.clears, 0);
        let pixels = r.finish().unwrap();
        assert!(pixels.data().chunks_exact(4).all(|p| p == b));
        assert_eq!(r.target.clears, 1);

        // Resolve the parent's fill before an opacity layer starts.
        r.begin(40., 24., 1.);
        r.fill(&full, a, identity);
        r.push_opacity(0.5);
        r.fill(&full, b, identity);
        r.pop_opacity();
        let mut expected = Pixmap::new(40, 24).unwrap();
        expected.fill(Color::from_rgba8(a[0], a[1], a[2], a[3]));
        let mut layer = Pixmap::new(40, 24).unwrap();
        layer.fill(Color::from_rgba8(b[0], b[1], b[2], b[3]));
        expected.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &PixmapPaint {
                opacity: 0.5,
                ..Default::default()
            },
            identity,
            None,
        );
        assert_eq!(r.finish().unwrap().data(), expected.data());

        // A partial or translucent draw needs the pending background first.
        for (shape, color) in [
            (Shape::rect((3., 5., 12., 8.)), b),
            (full, [210, 40, 60, 83]),
        ] {
            r.begin(40., 24., 1.);
            r.fill(&full, a, identity);
            r.fill(&shape, color, identity);
            expected.fill(Color::from_rgba8(a[0], a[1], a[2], a[3]));
            expected.fill_path(
                &rounded_rect(&shape).unwrap(),
                &solid(color),
                FillRule::Winding,
                identity,
                None,
            );
            assert_eq!(r.finish().unwrap().data(), expected.data());
        }
    }

    #[test]
    fn reused_split_layers_clear_before_drawing_and_after_resize() {
        let full = Shape::rect((0., 0., 40., 24.));
        let identity = Transform::identity();
        let mut r = Raster::new();
        for size in [(40., 24.), (40., 24.), (20., 12.)] {
            r.begin(size.0, size.1, 1.);
            let rect = (0., 0., size.0, size.1);
            r.fill(&Shape::rect(rect), [31, 72, 109, 255], identity);
            assert!(r.split_window_canvas(rect, &[], identity));
            assert_eq!(r.target.clears, 0);
            let layer = r.take_window_underlay().unwrap();
            assert!(matches!(
                &layer,
                WindowLayer::Solid {
                    color: [31, 72, 109, 255],
                    ..
                }
            ));
            let below = layer.into_pixels();
            assert!(below
                .data()
                .chunks_exact(4)
                .all(|p| p == [31, 72, 109, 255]));
            let above = r.finish().unwrap();
            assert!(above.data().iter().all(|&v| v == 0));
            let mut dirty_above = above;
            dirty_above.fill(Color::from_rgba8(255, 20, 30, 255));
            r.recycle_window_layers(below, Some(dirty_above));
        }
        r.begin(40., 24., 1.);
        r.fill(&full, [1, 2, 3, 255], identity);
        // Beginning again abandons the previous pending clear.
        r.begin(40., 24., 1.);
        assert!(r.finish().unwrap().data().iter().all(|&v| v == 255));
        let mut transparent = Raster::transparent();
        transparent.begin(40., 24., 1.);
        assert!(transparent.finish().unwrap().data().iter().all(|&v| v == 0));
    }

    #[test]
    fn split_background_color_is_premultiplied_and_any_partial_paint_removes_it() {
        for alpha in [0, 83, 255] {
            for patch in [false, true] {
                let mut r = Raster::new();
                r.page([219, 71, 253, alpha]);
                r.begin(40., 24., 1.);
                let mut expected = Pixmap::new(40, 24).unwrap();
                expected.fill(Color::from_rgba8(219, 71, 253, alpha));
                if patch {
                    let shape = Shape::rect((4., 5., 12., 8.));
                    let color = [210, 40, 60, 83];
                    r.fill(&shape, color, Transform::identity());
                    expected.fill_path(
                        &rounded_rect(&shape).unwrap(),
                        &solid(color),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                assert!(r.split_window_canvas((0., 0., 40., 24.), &[], Transform::identity()));
                let layer = r.take_window_underlay().unwrap();
                assert_eq!(matches!(layer, WindowLayer::Pixels(_)), patch);
                assert_eq!(layer.into_pixels().data(), expected.data());
            }
        }
    }
}
