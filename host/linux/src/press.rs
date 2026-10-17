//! Host-owned press feedback: one eased progress per pressed node, which
//! shows as its scale (multiplied into `scale` about transform-origin) and
//! its pressed paint (LLP 1061 D7: an opacity share, a background and a text
//! colour, `Host::presented` and the painter). The user's motion preference
//! never removes a button's feedback (LLP 1061).
use super::*;
use exact_motion::Easing;

const DURATION: f64 = 120.;
const EASE: Easing = Easing::CubicBezier {
    x1: 0.16,
    y1: 1.,
    x2: 0.3,
    y2: 1.,
};

/// A press's progress, 0 at rest and 1 held, eased from `from` to `to`
/// since `start`.
pub(super) struct Feedback {
    from: f32,
    to: f32,
    start: f64,
}
impl Feedback {
    pub(super) fn progress(&self, now: f64) -> f32 {
        self.from + (self.to - self.from) * EASE.progress((now - self.start) / DURATION) as f32
    }
}

/// Whether a node writes any of the pressed paint (LLP 1061 D7).
pub(crate) fn paints(style: &exact_kernel::StyleProps) -> bool {
    use exact_kernel::StyleId::{PressBackgroundColor, PressColor, PressOpacity};
    [PressOpacity, PressBackgroundColor, PressColor]
        .into_iter()
        .any(|row| style.mask.has(row))
}

/// The scale a press at `progress` shows: none for a scale no press can use.
pub(crate) fn scale(style: &exact_kernel::StyleProps, progress: f32) -> f32 {
    let to = style.press_scale;
    match to.is_finite() && to > 0. {
        true => 1. + (to - 1.) * progress,
        false => 1.,
    }
}

impl<D: DataSource> Host<D> {
    pub(crate) fn press_feedback(&mut self, key: NodeKey, down: bool, now: f64) {
        let to = if down { 1. } else { 0. };
        let old = self.presses.get(&key);
        if old.map_or(0., |p| p.to) == to {
            return;
        }
        let from = old.map_or(0., |p| p.progress(now));
        self.presses.insert(
            key,
            Feedback {
                from,
                to,
                start: now,
            },
        );
    }

    /// The press's progress on `key` now; 0 when none.
    pub(crate) fn press_progress(&self, key: NodeKey) -> f32 {
        self.presses
            .get(&key)
            .map_or(0., |f| f.progress(self.now_ms))
    }

    pub(super) fn press_settle(&self) -> Option<f64> {
        self.presses
            .values()
            .map(|p| p.start + DURATION)
            .filter(|&end| end > self.now_ms)
            .reduce(f64::max)
    }

    pub(super) fn retire_presses(&mut self) {
        let kernel = self.runner.kernel();
        let now = self.now_ms;
        self.presses.retain(|key, p| {
            kernel.node_by_key(*key).is_some() && (p.to != 0. || now < p.start + DURATION)
        });
    }
}
