//! The selection projection used by every CPU oracle and the renderer.
//! select.wgsl mirrors this scalar rule; culling is a separate operation.
use crate::Bounds;

/// Scaled length avoids overflow/underflow in squared finite components.
pub fn length(v: [f32; 3]) -> f32 {
    let scale = v.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    if scale == 0.0 {
        return 0.0;
    }
    let v = v.map(|x| x / scale);
    v.iter().map(|x| x * x).sum::<f32>().sqrt() * scale
}
#[derive(Clone, Copy)]
pub struct Projection {
    pub eye: [f32; 3],
    pub cot: f32,
    pub near: f32,
    pub height: f32,
    pub orthographic_span: Option<f32>,
}
impl Projection {
    /// `center` is the transformed bounds center; `scale` must be positive and uniform.
    pub fn projected(self, bounds: &Bounds, center: [f32; 3], scale: f32) -> f32 {
        if bounds.error == f32::MAX {
            return f32::MAX;
        }
        if bounds.error == 0.0 {
            return 0.0;
        }
        let value = if let Some(span) = self.orthographic_span {
            bounds.error * scale * self.height / span
        } else {
            let delta = std::array::from_fn(|i| center[i] - self.eye[i]);
            let distance = (length(delta) - bounds.radius * scale).max(self.near);
            bounds.error * scale / distance * (self.cot * 0.5 * self.height)
        };
        // A positive stored error stays positive even on GPUs that flush subnormals.
        // In particular all groups are above threshold zero, selecting only ORIGINAL.
        value.max(f32::MIN_POSITIVE)
    }
}
