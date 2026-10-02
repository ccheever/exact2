//! CSS `mask-image` on the CPU painter (LLP 1076 D2): the group the mask
//! opened is drawn onto the layer below through the gradient's alpha, as a
//! frame-sized mask that is empty outside the border box.

use super::Raster;
use crate::paint::{Backend, GradientPaint, Shape};
use tiny_skia::{Mask, MaskType, Pixmap, PixmapPaint, Transform};

impl Raster {
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
            self.target = Some(below);
            return;
        };
        // The mask's pixels: the gradient (or its one colour) over the
        // border box, drawn as any fill is, then read as alpha.
        let alpha_mask = Pixmap::new(layer.width(), layer.height()).and_then(|blank| {
            self.target = Some(blank);
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
        self.target = Some(below);
    }
}
