//! CSS `mask-image` on the GPU painter (LLP 1077 D2): the group the mask
//! opened (clipped to the border box) keeps only what the gradient's alpha
//! keeps, composited `DestIn` over it.

use super::{shape, Gpu};
use crate::paint::{Backend, GradientPaint, Shape};
use tiny_skia::Transform;
use vello::peniko::{BlendMode, Compose, Fill, Mix};

impl Gpu {
    pub(super) fn pop_masked(
        &mut self,
        s: &Shape,
        mask: &Result<GradientPaint, [u8; 4]>,
        ts: Transform,
    ) {
        let a = self.affine(ts);
        self.scene.push_layer(
            Fill::NonZero,
            BlendMode::new(Mix::Normal, Compose::DestIn),
            1.0,
            a,
            &shape(s),
        );
        match mask {
            Ok(gradient) => self.fill_gradient(s, gradient, ts),
            Err(color) => self.fill(s, *color, ts),
        }
        self.scene.pop_layer();
        self.pop_recorded();
    }
}
