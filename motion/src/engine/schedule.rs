//! The sampled executor's next wake, in the engine's seconds.
use super::{base, Curve, Engine};
use crate::Easing;

impl Engine {
    /// Next time a sampled value can change. The current sample time means
    /// continuous motion; a future time lets the host stop its display pump.
    /// Lowered, paused and finished animations do not request display frames.
    pub fn next_change(&self) -> Option<f64> {
        (self.links.next_change)(self)
    }

    pub(super) fn next_change_full(&self) -> Option<f64> {
        let now = self.sample_time();
        if self.clock_pending() {
            return Some(now);
        }
        let transitions = self.running.iter().filter_map(|key| {
            let run = self.slots.get(key)?.running()?;
            if run.pending.is_some() {
                return Some(now);
            }
            if now < run.start {
                return Some(run.start);
            }
            match &run.curve {
                Curve::Easing {
                    easing: Easing::Steps { count, .. },
                    duration,
                } if *duration > 0.0 => {
                    let step = ((now - run.start) / duration * *count as f64).floor() + 1.0;
                    Some((run.start + step / *count as f64 * duration).min(run.end_at(now)))
                }
                _ => Some(now),
            }
        });
        let animations = self.animating.iter().flat_map(|node| {
            self.animations[node].iter().flat_map(move |play| {
                play.animation
                    .keyframes
                    .properties()
                    .into_iter()
                    .filter_map(move |property| {
                        if self.lowered_for(*node, property) || play.hold.is_some() {
                            return None;
                        }
                        if play.pending.is_some() {
                            return Some(now);
                        }
                        let underlying = self.target(*node, property).or_else(|| base(property))?;
                        play.animation
                            .next_change(play.local(now), property, underlying, play.dark)
                            .map(|local| (play.start + local).max(now))
                    })
            })
        });
        transitions.chain(animations).reduce(f64::min)
    }
}
