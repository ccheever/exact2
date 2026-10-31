//! Deadlines for the sampled executor. Values still come from `sample_in`.
//! @ref LLP 1149.001 M2b — a step's held value needs no display callbacks.

use super::{Animation, Direction, Phase};
use crate::{Easing, Property, StepPosition, Value};

impl Animation {
    /// Next local time at which this property needs sampling. `Some(t)` means
    /// continuous motion now; a later time is a step, keyframe or phase edge.
    /// A paused or finished animation needs no wake. The executor compares
    /// the sample with its last presentation before painting an edge.
    pub fn next_change(
        &self,
        t: f64,
        property: Property,
        underlying: Value,
        dark: bool,
    ) -> Option<f64> {
        if self.paused || !t.is_finite() || self.keyframes.0.is_empty() {
            return None;
        }
        match self.phase(t) {
            Phase::After => return None,
            Phase::Before => return Some(self.delay.min(self.end_time()).max(0.0)),
            Phase::Active => {}
        }
        let overall = (t - self.delay) / self.duration;
        let iteration = overall.floor();
        let odd = (iteration as i64).rem_euclid(2) == 1;
        let forwards = match self.direction {
            Direction::Normal => true,
            Direction::Reverse => false,
            Direction::Alternate => !odd,
            Direction::AlternateReverse => odd,
        };
        let progress = self.directed_progress(t)?;
        let track = self.keyframes.track(property, underlying, dark);
        let mut interval = track.len().saturating_sub(2);
        for (i, pair) in track.windows(2).enumerate() {
            if pair[0].0 <= progress && progress < pair[1].0 {
                interval = i;
                break;
            }
        }
        let (a, b) = (&track[interval], &track[interval + 1]);
        let span = b.0 - a.0;
        let edge = if a.2 == b.2 || span <= 0.0 {
            if forwards {
                b.0
            } else {
                a.0
            }
        } else {
            let easing = a.1.unwrap_or(&self.easing);
            match easing {
                Easing::Steps { count, position } => {
                    let local = (progress - a.0) / span;
                    let n = *count as f64;
                    let step = if forwards {
                        (local * n).floor() + 1.0
                    } else {
                        // At a reverse boundary the closed-form sampler still
                        // holds the upper step. Cross it before sleeping again.
                        let step = (local * n).floor();
                        if *position == StepPosition::JumpNone {
                            // jump-none has no jump at progress 1, including
                            // the start of a reverse or alternate iteration.
                            step.min(n - 1.0)
                        } else {
                            step
                        }
                    };
                    a.0 + (step / n).clamp(0.0, 1.0) * span
                }
                _ => return Some(t),
            }
        };
        // Reverse traversal changes a step just below its boundary, because
        // `sample_in` uses the upper value at the boundary itself.
        let simple = if forwards { edge } else { 1.0 - edge };
        let boundary = self.delay + (iteration + simple) * self.duration;
        // Cross after converting to time: adding the iteration can erase an
        // ULP in progress. The iteration's end itself is already a phase edge.
        let next = if !forwards && edge > 0.0 {
            boundary.next_up()
        } else {
            boundary
        }
        .min(self.end_time());
        // A just-later instant can round back to `t` when a host converts to
        // milliseconds. It then requests a display frame instead of sleeping.
        Some(if next > t { next } else { t.next_up() })
    }
}
