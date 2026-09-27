//! Host-owned press feedback, multiplied into scale about transform-origin.
//! The user's motion preference never removes a button's feedback (LLP 1061).
use super::*;
use exact_motion::Easing;

const DURATION: f64 = 120.;
const EASE: Easing = Easing::CubicBezier {
    x1: 0.16,
    y1: 1.,
    x2: 0.3,
    y2: 1.,
};

pub(super) struct Feedback {
    from: f32,
    to: f32,
    start: f64,
}
impl Feedback {
    pub(super) fn factor(&self, now: f64) -> f32 {
        self.from + (self.to - self.from) * EASE.progress((now - self.start) / DURATION) as f32
    }
}
impl<D: DataSource> Host<D> {
    pub(crate) fn press_feedback(&mut self, key: NodeKey, down: bool, now: f64) {
        let to = if down {
            self.kernel()
                .node_by_key(key)
                .map_or(1., |n| n.style.press_scale)
        } else {
            1.
        };
        if !to.is_finite() || to <= 0. {
            return;
        }
        let old = self.presses.get(&key);
        if old.map_or(1., |p| p.to) == to {
            return;
        }
        let from = old.map_or(1., |p| p.factor(now));
        self.presses.insert(
            key,
            Feedback {
                from,
                to,
                start: now,
            },
        );
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
            kernel.node_by_key(*key).is_some() && (p.to != 1. || now < p.start + DURATION)
        });
    }
}
