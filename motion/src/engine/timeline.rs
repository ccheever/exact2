//! Drag timelines in the engine (LLP 1057.003 D2).
//!
//! A source node names a timeline its presented `translate` drives on one
//! axis; a consumer binds its `animation`s to that name over a range. The
//! consumer's plays are held (as `animation-play-state: paused` holds them)
//! at the local time the source's position gives, re-sought whenever a frame
//! is taken, so a held value and a release spring move the consumer in the
//! frame they move the source. A sampling host paints it; a lowering host
//! samples the node (its compositor's time is the clock's) or seeks its own
//! executor's copy, as the web's glue does. Names are global in phase 1;
//! CSS scopes them to the subtree (`timeline-scope`), which is phase 3 (D4).

use super::Engine;
use crate::property::Property;

/// The drag timelines, in declaration order. A page has a handful; a scan
/// is cheaper, in code and time, than a map.
#[derive(Debug, Default)]
pub(super) struct Timelines {
    /// Each source node, its timeline name, and whether it reads `x`.
    sources: Vec<(u64, String, bool)>,
    /// Each consumer node, its timeline name, and its range in points.
    bound: Vec<(u64, String, [f64; 2])>,
}

impl Engine {
    /// Declare (or with `None`, retract) the timeline `node`'s presented
    /// translate drives: its name and whether it reads the `x` axis.
    pub fn set_drag_timeline(&mut self, node: u64, source: Option<(&str, bool)>) {
        let at = self.timelines.sources.iter().position(|s| s.0 == node);
        match (source, at) {
            (Some((name, x)), Some(i)) if self.timelines.sources[i].1 == name => {
                self.timelines.sources[i].2 = x
            }
            (Some((name, x)), at) => {
                if let Some(i) = at {
                    self.timelines.sources.remove(i);
                }
                self.timelines.sources.push((node, name.to_string(), x));
            }
            (None, Some(i)) => {
                self.timelines.sources.remove(i);
            }
            (None, None) => {}
        }
    }

    /// Bind (or with `None`, unbind) `node`'s animations to a named timeline
    /// over `range`: at `range[0]` they are at their start, at `range[1]` at
    /// their end, clamped outside.
    pub fn set_animation_timeline(&mut self, node: u64, binding: Option<(&str, [f64; 2])>) {
        let at = self.timelines.bound.iter().position(|b| b.0 == node);
        match (binding, at) {
            (Some((name, range)), Some(i)) if self.timelines.bound[i].1 == name => {
                self.timelines.bound[i].2 = range;
                self.seek_timeline(i);
            }
            (Some((name, range)), at) => {
                if let Some(i) = at {
                    self.timelines.bound.remove(i);
                }
                self.timelines.bound.push((node, name.to_string(), range));
                self.seek_timeline(self.timelines.bound.len() - 1);
            }
            (None, Some(i)) => {
                self.timelines.bound.remove(i);
                // Back on the clock from where the timeline left it.
                let now = self.now;
                for play in self.animations.get_mut(&node).into_iter().flatten() {
                    if !play.animation.paused {
                        if let Some(held) = play.hold.take() {
                            play.start = now - held;
                        }
                    }
                }
            }
            (None, None) => {}
        }
    }

    /// Whether `node`'s animations follow a timeline instead of the clock.
    pub fn timeline_bound(&self, node: u64) -> bool {
        self.timelines.bound.iter().any(|b| b.0 == node)
    }

    pub(super) fn forget_timelines(&mut self, node: u64) {
        self.timelines.sources.retain(|s| s.0 != node);
        self.timelines.bound.retain(|b| b.0 != node);
    }

    /// Hold every bound consumer's plays at the time its timeline gives.
    pub(super) fn seek_timelines(&mut self) {
        for i in 0..self.timelines.bound.len() {
            self.seek_timeline(i);
        }
    }

    /// Re-seek a bound node whose plays were just set.
    pub(super) fn seek_bound(&mut self, node: u64) {
        if let Some(i) = self.timelines.bound.iter().position(|b| b.0 == node) {
            self.seek_timeline(i);
        }
    }

    fn seek_timeline(&mut self, i: usize) {
        let (node, ref name, [a, b]) = self.timelines.bound[i];
        // The source's presented translate on its axis; the last declared
        // wins. Without one the timeline is inactive and holds the start.
        let at = self
            .timelines
            .sources
            .iter()
            .rev()
            .find(|s| s.1 == *name)
            .and_then(|s| Some((self.value(s.0, Property::Translate)?, s.2)))
            .map_or(a, |(v, x)| if x { v.x } else { v.y });
        // Unclamped: outside the range the animation is before or after its
        // interval, and its fill decides, as on a CSS scroll timeline.
        let p = (at - a) / (b - a);
        let p = if p.is_finite() { p } else { 0.0 };
        let Some(plays) = self.animations.get_mut(&node) else {
            return;
        };
        let mut moved = false;
        for play in plays.iter_mut() {
            // The range spans the delay and the active interval together, as
            // Web Animations 2 converts an animation's times on a
            // progress-based timeline (Chrome agrees: the parity fixture's
            // `tl-delay`). An
            // endless animation has no end to span and holds its start, a
            // declared deviation: CSS gives it no duration and shows its end.
            let end = play.animation.end_time();
            let local = if end.is_finite() {
                p * end
            } else {
                play.animation.delay.max(0.0)
            };
            moved |= play.hold.replace(local) != Some(local);
        }
        self.animating.remove(&node);
        if !moved {
            return;
        }
        // A lowering host's executor seeks its own copy (the web's glue); only
        // what the engine samples is painted from here.
        let properties: Vec<Property> = self.animations[&node]
            .iter()
            .flat_map(|play| play.animation.keyframes.properties())
            .filter(|p| !self.lowered_for(node, *p))
            .collect();
        for property in properties {
            self.dirty.insert((node, property));
        }
    }
}
