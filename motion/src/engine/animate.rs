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
    /// While paused, the local time it holds. NaN is an unresolved time
    /// (an inactive timeline's, LLP 1057.003 D4): the animation has no effect.
    pub hold: Option<f64>,
    /// The appearance its `light-dark()` keyframes took when it started, as
    /// a browser resolves a rule once (LLP 1062 D9).
    pub dark: bool,
    /// While it waits for the first presented frame (LLP 1003.001 D1), the
    /// engine time it began: it holds the local time it had then, and the
    /// frame moves its start by the wait.
    pub pending: Option<f64>,
    /// While pending, whether it joined a clock origin that waits for the
    /// frame too: then the frame gives it the origin's phase (LLP 1003.001
    /// D8), not a start of its own.
    pub(crate) clock_wait: bool,
}

impl AnimationPlay {
    /// Local time at engine time `now`: seconds since start, delay included.
    pub fn local(&self, now: f64) -> f64 {
        self.hold
            .unwrap_or(self.pending.unwrap_or(now) - self.start)
    }

    /// When it ends if the frame at `now` starts it (LLP 1003.001 D1).
    pub fn end_at(&self, now: f64) -> f64 {
        self.start + self.animation.end_time() + self.pending.map_or(0.0, |b| (now - b).max(0.0))
    }

    fn live(&self, now: f64) -> bool {
        self.hold.is_none() && self.local(now) <= self.animation.end_time()
    }

    /// Whether it animates a property the engine samples.
    fn sampled(&self, lowered: &[bool; Property::SLOTS], forced: bool) -> bool {
        forced
            || self
                .animation
                .keyframes
                .properties()
                .iter()
                .any(|p| !lowered[*p as usize])
    }
}

/// How a play joins its clock timeline (LLP 1055.002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Join {
    Started,
    Resumed,
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
        let now = self.sample_time();
        let pend = self.start_on_frame.then_some(now);
        // A timeline-bound play keeps its held time: the timeline, not the
        // row's play state, holds it (LLP 1057.003 D2).
        let bound = self.timeline_bound(node);
        let old = self.animations.remove(&node).unwrap_or_default();
        if old.is_empty() && animations.0.is_empty() {
            return Ok(());
        }
        // CSS Animations 2 §4.3: walk the new list from its end, pairing each
        // name with the last unmatched old animation of that name, so two
        // entries of one name are two animations and a reorder restarts none.
        let mut used = vec![false; old.len()];
        let mut plays = Vec::with_capacity(animations.0.len());
        // On a clock timeline a play that starts or resumes joins its phase
        // (LLP 1055.002 D4, D6), from now until the clock picks its start.
        let on_clock = self.animation_clock(node).is_some();
        let mut joins = Vec::with_capacity(animations.0.len());
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
                    // A pending play keeps waiting; a pause takes the
                    // local time it waits at; a resume waits at its held one
                    // (LLP 1003.001 D3). A clock's join decides for its own.
                    let (start, hold, pending) = match (prior.hold, a.paused) {
                        (hold, _) if bound => (prior.start, hold, None),
                        (None, true) => (prior.start, Some(prior.local(now)), None),
                        // A play on a clock timeline rejoins its phase
                        // (LLP 1055.002 D6); any other continues.
                        (Some(_), false) if on_clock => (now, None, None),
                        (Some(held), false) => (now - held, None, pend),
                        (hold, _) => (prior.start, hold, prior.pending),
                    };
                    let resumed = on_clock && !bound && prior.hold.is_some() && !a.paused;
                    joins.push(resumed.then_some(Join::Resumed));
                    AnimationPlay {
                        animation: a.clone(),
                        start,
                        hold,
                        dark: prior.dark,
                        clock_wait: prior.clock_wait && pending.is_some(),
                        pending,
                    }
                }
                None => {
                    joins.push(on_clock.then_some(Join::Started));
                    AnimationPlay {
                        animation: a.clone(),
                        start: now,
                        hold: a.paused.then_some(0.0),
                        dark: self.dark_of(node),
                        pending: pend.filter(|_| !a.paused && !bound && !on_clock),
                        clock_wait: false,
                    }
                }
            };
            plays.push(play);
        }
        plays.reverse();
        joins.reverse();
        // A resume from where it was held (an animation that waited for its
        // list row, LLP 1055 D13) shows what it showed: nothing to present.
        let resumed = !bound
            && old.len() == plays.len()
            && old
                .iter()
                .zip(&plays)
                .any(|(o, n)| o.hold.is_some() && n.hold.is_none())
            && old.iter().zip(&plays).all(|(o, n)| {
                o.local(now) == n.local(now)
                    && o.dark == n.dark
                    && o.animation.name == n.animation.name
                    && {
                        let mut was = o.animation.clone();
                        was.paused = n.animation.paused;
                        was == n.animation
                    }
            });
        if !resumed {
            for play in old.iter().chain(&plays) {
                for p in play.animation.keyframes.properties() {
                    self.dirty.insert((node, p));
                }
            }
        }
        if plays.is_empty() {
            self.animating.remove(&node);
        } else {
            self.animations.insert(node, plays);
            self.schedule_animations(node);
            self.seek_bound(node);
            let of = |join| {
                let at = joins.iter().enumerate().filter(|(_, j)| **j == Some(join));
                at.map(|(i, _)| i).collect::<Vec<_>>()
            };
            self.join_clock(node, &of(Join::Started), &of(Join::Resumed));
        }
        Ok(())
    }

    /// Keep the clock running for `node` while a play of it is live and
    /// sampled, after its plays or their starts change.
    pub(super) fn schedule_animations(&mut self, node: u64) {
        let (now, lowered, forced) = (
            self.sample_time(),
            self.lowered,
            self.forced.contains(&node),
        );
        let plays = self.animations.get(&node).map_or(&[][..], Vec::as_slice);
        // A pending play keeps the clock until the frame starts it, lowered
        // or not (LLP 1003.001 D1).
        if plays
            .iter()
            .any(|p| p.pending.is_some() || (p.live(now) && p.sampled(&lowered, forced)))
        {
            self.animating.insert(node);
        } else {
            self.animating.remove(&node);
        }
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
        self.lowered = [lowered; Property::SLOTS];
    }

    /// Lower only these properties' animations (Apple: the ones Core
    /// Animation plays faithfully); the engine samples the rest per frame.
    pub fn set_lowered_properties(&mut self, properties: &[Property]) {
        self.lowered = [false; Property::SLOTS];
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
        let now = self.sample_time();
        if let Some(plays) = self.animations.get(&node) {
            for play in plays {
                for p in play.animation.keyframes.properties() {
                    self.dirty.insert((node, p));
                }
            }
            if plays
                .iter()
                .any(|p| p.pending.is_some() || (p.live(now) && p.sampled(&self.lowered, sampled)))
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
        let now = self.sample_time();
        self.animations.get(&node)?.iter().rev().find_map(|play| {
            play.animation
                .sample_in(play.local(now), property, underlying, play.dark)
        })
    }

    /// Play a node's `-exact-exit-animation` as it leaves (LLP 1063), from the
    /// current clock, after the animations it already plays: as a browser
    /// appends the exit to the element's `animation` list, those keep
    /// running and the exit composites over them. An exit naming keyframes
    /// the node already plays starts again, as a second entry of that name
    /// does. Returns the clock time the last exit animation ends.
    pub fn play_exit(&mut self, node: u64, exit: &Animations) -> Result<f64, EngineError> {
        exit.validate_ending()
            .map_err(|_| EngineError::InvalidAnimation)?;
        let (now, dark) = (self.sample_time(), self.dark_of(node));
        let pending = self.start_on_frame.then_some(now);
        for a in &exit.0 {
            for p in a.keyframes.properties() {
                self.dirty.insert((node, p));
            }
        }
        let plays = self.animations.entry(node).or_default();
        plays.extend(exit.0.iter().map(|animation| AnimationPlay {
            animation: animation.clone(),
            start: now,
            hold: None,
            dark,
            pending,
            clock_wait: false,
        }));
        self.animating.insert(node);
        Ok(now + exit.end_time())
    }

    /// The host's appearance, which a keyframe's `light-dark()` colour takes
    /// when its animation starts (LLP 1062 D9): Chrome resolves the rule
    /// once, and a playing animation keeps its colours across a flip. With
    /// `playing`, those take it too, in place and keeping their start: a
    /// host's first report correcting the appearance it booted under.
    pub fn set_dark(&mut self, dark: bool, playing: bool) {
        self.dark = dark;
        if !playing {
            return;
        }
        let nodes: Vec<u64> = self
            .animations
            .keys()
            .copied()
            .filter(|n| !self.node_dark.contains_key(n))
            .collect();
        for node in nodes {
            self.redark(node, dark);
        }
    }

    /// One node's own appearance, where it differs from the host's (`None`:
    /// the host's again): a view whose appearance is not its window's (LLP
    /// 1062 D4). With `playing`, its playing animations take it in place, as
    /// [`Engine::set_dark`]'s first report does.
    pub fn set_node_dark(&mut self, node: u64, dark: Option<bool>, playing: bool) {
        match dark {
            Some(dark) => self.node_dark.insert(node, dark),
            None => self.node_dark.remove(&node),
        };
        if playing {
            self.redark(node, self.dark_of(node));
        }
    }

    fn dark_of(&self, node: u64) -> bool {
        self.node_dark.get(&node).copied().unwrap_or(self.dark)
    }

    fn redark(&mut self, node: u64, dark: bool) {
        let Some(plays) = self.animations.get_mut(&node) else {
            return;
        };
        for p in plays.iter_mut().filter(|p| p.dark != dark) {
            p.dark = dark;
            for frame in &p.animation.keyframes.0 {
                for (property, _) in &frame.dark {
                    self.dirty.insert((node, *property));
                }
            }
        }
    }

    /// Mark every property of every live sampled animation dirty, and retire
    /// nodes whose animations have all ended or paused (after this frame).
    pub(super) fn advance_animations(&mut self) {
        let now = self.sample_time();
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
                .any(|p| p.pending.is_some() || (p.live(now) && p.sampled(&lowered, forced)))
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
            .map(|p| p.end_at(self.sample_time()))
            .filter(|t| *t > self.sample_time())
            .fold(None, |acc, t| Some(acc.map_or(t, |a: f64| a.max(t))))
    }
}
