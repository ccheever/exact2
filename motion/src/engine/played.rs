//! Transitions a host plays itself (LLP 1076 on Android, as LLP 1055 D7
//! lowers animations): the engine hands one over with its curve, presents
//! the target from then on, and stops sampling it, so nothing keeps the
//! clock busy for it; it still measures a reversal against the curve, and
//! drops it once it would have ended.
use super::*;
use crate::Easing;

/// A transition a host plays: from `from` to `to`, starting (delay
/// included) at `start` engine seconds, along `curve`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayedTransition {
    /// Value at the start.
    pub from: Value,
    /// Value at the end (the target the engine now presents).
    pub to: Value,
    /// Clock time it starts moving, seconds.
    pub start: f64,
    /// Its shape.
    pub curve: PlayedCurve,
}

/// A played transition's shape.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayedCurve {
    /// An easing over `duration` seconds.
    Easing {
        /// The timing function.
        easing: Easing,
        /// Seconds.
        duration: f64,
    },
    /// A spring as evenly spaced values over `duration` seconds (linear
    /// between them), first `from` and last `to`.
    Frames {
        /// Seconds.
        duration: f64,
        /// The values.
        values: Vec<Value>,
    },
}

impl Engine {
    /// The transitions running now, which a host may play instead.
    pub fn running_transitions(&self) -> impl Iterator<Item = (u64, Property)> + '_ {
        self.running.iter().copied()
    }

    /// Hand the transition running on `property` of `node` to the host: it
    /// is presented at its target from now on and no longer sampled. `None`
    /// when nothing runs there or a hold owns it.
    pub fn play_transition(&mut self, node: u64, property: Property) -> Option<PlayedTransition> {
        let key = (node, property);
        let at = self.sample_time();
        let slot = self.slots.get_mut(&key)?;
        if slot.owner().is_some() {
            return None;
        }
        let running = slot.running()?;
        let curve = match &running.curve {
            crate::transition::Curve::Easing { easing, duration } => PlayedCurve::Easing {
                easing: easing.clone(),
                duration: *duration,
            },
            crate::transition::Curve::Spring { .. } => {
                let (duration, values) = running.spring_frames()?;
                PlayedCurve::Frames { duration, values }
            }
        };
        // A played transition is its host's: it never waits for a frame
        // here (LLP 1003.001 D8).
        let played = PlayedTransition {
            from: running.from,
            to: running.to,
            start: running.start_at(at),
            curve,
        };
        let target = slot.target;
        slot.set_presented(target);
        if let Some(running) = slot.live.as_mut().and_then(|l| l.running.as_mut()) {
            running.start = played.start;
            running.pending = None;
        }
        self.pending.remove(&key);
        self.running.remove(&key);
        self.played.insert(key);
        self.dirty.insert(key);
        Some(played)
    }

    /// Played transitions that would have ended by `now` are dropped: their
    /// slots settle at the target they already present.
    pub(super) fn retire_played(&mut self, now: f64) {
        let slots = &mut self.slots;
        self.played.retain(|key| {
            let Some(slot) = slots.get_mut(key) else {
                return false;
            };
            let done = slot.running().is_none_or(|r| r.sample(now).done);
            if done {
                slot.set_running(None);
            }
            !done
        });
    }
}
