//! CSS animations in the engine: when each one started, and its value now.
//!
//! @ref LLP 1055 D5 (start, restart, re-timing, pause), D7 (sampled and
//! lowered executors), D10 (settle ignores infinite iterations)
//!
//! A node's `animation` row arrives with each commit, as its `transition`
//! row does. An entry whose name was not in the node's previous list starts
//! now; one that was keeps its start and takes the new timing (CSS: changing
//! a longhand re-times a running animation, it does not restart it); one that
//! left is cancelled. Pausing holds the local time and resuming continues
//! from it. A sampling host gets the animated values from
//! [`Engine::frame`](super::Engine::frame); a lowering host (Apple, whose
//! compositor runs the animation, and the web, whose browser does) reads the
//! plays and never samples them per frame.

use super::{Engine, EngineError};
use crate::animation::{Animation, Animations};
use crate::property::{Property, Value};

/// One animation playing on a node: the entry and when it started.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationPlay {
    /// The row's entry, keyframes resolved.
    pub animation: Animation,
    /// Engine time the animation started (its local time zero).
    pub start: f64,
    /// While paused, the local time it holds.
    pub hold: Option<f64>,
}

impl AnimationPlay {
    /// Local time at engine time `now`: seconds since start, delay included.
    pub fn local(&self, now: f64) -> f64 {
        self.hold.unwrap_or(now - self.start)
    }

    fn live(&self, now: f64) -> bool {
        self.hold.is_none() && self.local(now) <= self.animation.end_time()
    }

    /// Whether it animates a property the engine samples.
    fn sampled(&self, lowered: &[bool; Property::COUNT], forced: bool) -> bool {
        forced
            || self
                .animation
                .keyframes
                .properties()
                .iter()
                .any(|p| !lowered[*p as usize])
    }
}

impl Engine {
    /// Set a node's `animation` row (CSS Animations 1 §3). Validates first;
    /// on error nothing changes.
    pub fn set_animations(
        &mut self,
        node: u64,
        animations: &Animations,
    ) -> Result<(), EngineError> {
        animations
            .validate()
            .map_err(|_| EngineError::InvalidAnimation)?;
        let now = self.now;
        let old = self.animations.remove(&node).unwrap_or_default();
        if old.is_empty() && animations.0.is_empty() {
            return Ok(());
        }
        // CSS Animations 2 §4.3: walk the new list from its end, pairing each
        // name with the last unmatched old animation of that name, so two
        // entries of one name are two animations and a reorder restarts none.
        let mut used = vec![false; old.len()];
        let mut plays = Vec::with_capacity(animations.0.len());
        for a in animations.0.iter().rev() {
            let matched = old
                .iter()
                .enumerate()
                .rev()
                .find(|(i, p)| !used[*i] && p.animation.name == a.name)
                .map(|(i, _)| i);
            let play = match matched {
                Some(i) => {
                    used[i] = true;
                    let prior = &old[i];
                    let (start, hold) = match (prior.hold, a.paused) {
                        (None, true) => (prior.start, Some(now - prior.start)),
                        (Some(held), false) => (now - held, None),
                        (hold, _) => (prior.start, hold),
                    };
                    AnimationPlay {
                        animation: a.clone(),
                        start,
                        hold,
                    }
                }
                None => AnimationPlay {
                    animation: a.clone(),
                    start: now,
                    hold: a.paused.then_some(0.0),
                },
            };
            plays.push(play);
        }
        plays.reverse();
        for play in old.iter().chain(&plays) {
            for p in play.animation.keyframes.properties() {
                self.dirty.insert((node, p));
            }
        }
        if plays.is_empty() {
            self.animating.remove(&node);
        } else {
            let (lowered, forced) = (self.lowered, self.forced.contains(&node));
            if plays
                .iter()
                .any(|p| p.live(now) && p.sampled(&lowered, forced))
            {
                self.animating.insert(node);
            } else {
                self.animating.remove(&node);
            }
            self.animations.insert(node, plays);
        }
        Ok(())
    }

    /// The animations playing on a node, in row order.
    pub fn animation_plays(&self, node: u64) -> &[AnimationPlay] {
        self.animations.get(&node).map_or(&[], Vec::as_slice)
    }

    /// Nodes with animations, in node order.
    pub fn animated_nodes(&self) -> impl Iterator<Item = u64> + '_ {
        self.animations.keys().copied()
    }

    /// A lowering host (LLP 1055 D7) executes animations itself: the engine
    /// tracks their starts and pauses but never samples them into frames or
    /// keeps the clock busy for them. The web lowers every property.
    pub fn set_lowered(&mut self, lowered: bool) {
        self.lowered = [lowered; Property::COUNT];
    }

    /// Lower only these properties' animations (Apple: the ones Core
    /// Animation plays faithfully); the engine samples the rest per frame.
    pub fn set_lowered_properties(&mut self, properties: &[Property]) {
        self.lowered = [false; Property::COUNT];
        for p in properties {
            self.lowered[*p as usize] = true;
        }
    }

    /// Whether a property's animations are lowered.
    pub fn is_lowered(&self, property: Property) -> bool {
        self.lowered[property as usize]
    }

    /// Whether the host asked for this node's animations to be sampled.
    pub fn node_sampled(&self, node: u64) -> bool {
        self.forced.contains(&node)
    }

    /// Whether this node's animations of `property` are lowered: the
    /// property is, and the host has not asked to sample the node.
    pub fn lowered_for(&self, node: u64, property: Property) -> bool {
        self.lowered[property as usize] && !self.forced.contains(&node)
    }

    /// Sample (or stop sampling) every animation on `node`, whatever the
    /// lowered set says: its host's compositor cannot play this node's
    /// animations faithfully (LLP 1055.000 D15). Its properties are marked
    /// dirty so the next frame shows the switch.
    pub fn set_node_sampled(&mut self, node: u64, sampled: bool) {
        let changed = if sampled {
            self.forced.insert(node)
        } else {
            self.forced.remove(&node)
        };
        if !changed {
            return;
        }
        let now = self.now;
        if let Some(plays) = self.animations.get(&node) {
            for play in plays {
                for p in play.animation.keyframes.properties() {
                    self.dirty.insert((node, p));
                }
            }
            if plays
                .iter()
                .any(|p| p.live(now) && p.sampled(&self.lowered, sampled))
            {
                self.animating.insert(node);
            } else {
                self.animating.remove(&node);
            }
        }
    }

    /// One property's animated value over `underlying` now: the last entry
    /// that applies wins (CSS's composite order, replace); one outside its
    /// interval with no fill contributes nothing. `None` when none applies.
    pub fn animated(&self, node: u64, property: Property, underlying: Value) -> Option<Value> {
        let now = self.now;
        self.animations
            .get(&node)?
            .iter()
            .rev()
            .find_map(|play| play.animation.sample(play.local(now), property, underlying))
    }

    /// Mark every property of every live sampled animation dirty, and retire
    /// nodes whose animations have all ended or paused (after this frame).
    pub(super) fn advance_animations(&mut self) {
        let now = self.now;
        let lowered = self.lowered;
        let mut ended = Vec::new();
        for node in &self.animating {
            let plays = &self.animations[node];
            let forced = self.forced.contains(node);
            for play in plays {
                for p in play.animation.keyframes.properties() {
                    if forced || !lowered[p as usize] {
                        self.dirty.insert((*node, p));
                    }
                }
            }
            if !plays
                .iter()
                .any(|p| p.live(now) && p.sampled(&lowered, forced))
            {
                ended.push(*node);
            }
        }
        for node in ended {
            self.animating.remove(&node);
        }
    }

    /// When the last finite, running animation ends; infinite and paused ones
    /// never settle and are left out (LLP 1055 D10).
    pub(super) fn animations_settle_time(&self) -> Option<f64> {
        self.animations
            .values()
            .flatten()
            .filter(|p| p.hold.is_none() && p.animation.end_time().is_finite())
            .map(|p| p.start + p.animation.end_time())
            .filter(|t| *t > self.now)
            .fold(None, |acc, t| Some(acc.map_or(t, |a: f64| a.max(t))))
    }
}
