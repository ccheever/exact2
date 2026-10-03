//! Clock timelines in the engine: animations that share a phase (LLP 1055.002).
//!
//! A node whose `animation-timeline` is `clock(Name)` plays its animations on
//! the clock as any node does, from a start chosen so that every animation on
//! `Name` is in step (D4): the most recent cycle boundary on the timeline at
//! or before the moment it starts. A timeline is one number, its origin, set
//! when an animation joins it idle (D3). Nothing here runs per frame: a start
//! is chosen once, when a play starts or resumes, and every executor plays
//! from it as it plays any other start (Core Animation's `beginTime`).

use super::animate::AnimationPlay;
use super::Engine;
use crate::animation::{Animation, Direction};
use std::collections::BTreeMap;

/// Each node's clock timeline, and each timeline's origin.
#[derive(Debug, Default)]
pub(super) struct Clocks {
    of: BTreeMap<u64, String>,
    origins: BTreeMap<String, f64>,
}

impl Engine {
    /// Put `node`'s animations on the clock timeline `clock` (or with
    /// `None`, on their own starts again). Plays already running keep their
    /// start; it decides where each one that starts or resumes from now on
    /// begins.
    pub fn set_animation_clock(&mut self, node: u64, clock: Option<&str>) {
        match clock {
            Some(name) if self.clocks.of.get(&node).map(String::as_str) != Some(name) => {
                self.clocks.of.insert(node, name.to_owned());
            }
            Some(_) => {}
            None => {
                self.clocks.of.remove(&node);
            }
        }
    }

    /// The clock timeline `node`'s animations are on.
    pub fn animation_clock(&self, node: u64) -> Option<&str> {
        self.clocks.of.get(&node).map(String::as_str)
    }

    pub(super) fn forget_clock(&mut self, node: u64) {
        self.clocks.of.remove(&node);
    }

    /// Where a play of `animation` on `node` that starts (or resumes) at
    /// `now` begins: `now` off a clock timeline; on one, the last cycle
    /// boundary at or before `now` (D4). A timeline nothing live is on takes
    /// `now` as its origin first (D3), so a lone animation starts at its
    /// first keyframe. `also` is plays the caller holds outside the table
    /// (the node's own, while its row is replaced).
    pub(super) fn clock_start(
        &mut self,
        node: u64,
        animation: &Animation,
        now: f64,
        also: &[AnimationPlay],
    ) -> f64 {
        let Some(name) = self.clocks.of.get(&node) else {
            return now;
        };
        // Live: paused (it will resume on this timeline) or not yet ended.
        let live = |p: &AnimationPlay| p.hold.is_some() || p.local(now) < p.animation.end_time();
        let busy = also.iter().any(live)
            || self
                .clocks
                .of
                .iter()
                .filter(|(n, c)| *c == name && **n != node)
                .any(|(n, _)| self.animations.get(n).is_some_and(|ps| ps.iter().any(live)));
        let origin = match self.clocks.origins.get(name) {
            Some(origin) if busy => *origin,
            _ => {
                self.clocks.origins.insert(name.clone(), now);
                now
            }
        };
        // An alternating cycle is two iterations, so a joiner's first is
        // forwards and it ends on the keyframe it would end on alone.
        let alternates = matches!(
            animation.direction,
            Direction::Alternate | Direction::AlternateReverse
        );
        let period = animation.duration * if alternates { 2.0 } else { 1.0 };
        if !(period > 0.0 && period.is_finite()) {
            return now;
        }
        now - (now - origin).rem_euclid(period)
    }
}
