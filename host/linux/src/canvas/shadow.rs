//! Outer `box-shadow`s for the reader to blur (`SHADOW`, op 40). The painter
//! otherwise draws a shadow as a stack of banded rings, each a path filled
//! inside a path clip (`paint/shadow.rs`), which HWUI rasterizes into
//! software masks on its worker threads every frame; a rounded rect drawn
//! with a blur mask filter is one analytic blur on the GPU, as Android's
//! own elevation shadows are.
use super::{Recorder, Shape};
use tiny_skia::Transform;

/// `[40, color, sigma, rect, radii×8, border box, radii×8]`.
const SHADOW: u32 = 40;

impl Recorder {
    /// `shape` blurred by `sigma` in `color`, outside `outer`; whether it was
    /// recorded (only under a scale-and-translate transform).
    pub(super) fn shadow(
        &mut self,
        shape: &Shape,
        color: [u8; 4],
        sigma: f32,
        outer: &Shape,
        ts: Transform,
    ) -> bool {
        let reach = 3.0 * sigma;
        let (x, y, w, h) = shape.rect;
        let Some(bounds) =
            super::clip::map((x - reach, y - reach, w + 2.0 * reach, h + 2.0 * reach), ts)
        else {
            return false;
        };
        if self.culled(Some(bounds)) {
            return true;
        }
        self.need(Some(bounds));
        self.transform(ts);
        self.ops.extend([SHADOW, Self::color(color)]);
        self.f(sigma);
        self.rect_radii(shape);
        self.rect_radii(outer);
        true
    }
}
