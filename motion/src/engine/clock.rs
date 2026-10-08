//! Clock timelines in the engine: animations that share a phase (LLP 1055.002).
//!
//! A node whose `animation-timeline` is `-exact-clock(Name)` plays its animations on
//! the clock as any node does, from a start chosen so that every animation on
//! `Name` is in step (D4): the most recent cycle boundary on the timeline at
//! or before the moment it starts. A timeline is one number, its origin, set
//! when an animation joins it idle (D3). Nothing here runs per frame: a start
//! is chosen once, when a play starts, resumes or moves onto a clock, and
//! every executor plays from it as it plays any other start (Core
//! Animation's `beginTime`).
//!
//! Inside a commit ([`Engine::hold_clock_joins`]) the joins wait for its end,
//! when every node's rows are applied, so the start a join takes does not
//! depend on the order the commit's nodes were applied in: a member another
//! node's row ends in the same commit is gone, a drag-bound play is not a
//! member, and a join reads the timing its row ends the commit with.

use super::Engine;
use crate::animation::{Animation, Direction};
use std::collections::{BTreeMap, BTreeSet};

/// Each node's clock timeline, each timeline's origin, and the plays that
/// join at the end of the commit.
#[derive(Debug, Default)]
pub(super) struct Clocks {
    of: BTreeMap<u64, String>,
    origins: BTreeMap<String, f64>,
    joining: BTreeMap<u64, Joining>,
    held: bool,
    // Origins set while every member waits for the first presented frame
    // (LLP 1003.001 D8), each with the time it was set: unchanged until the
    // frame moves it, and every play that joins meanwhile waits with it.
    pending: BTreeMap<String, f64>,
}

/// What of a node joins its clock.
#[derive(Debug)]
enum Joining {
    /// These plays (row indices), started or resumed by its row. A resumed
    /// one was a member all along (paused, it held the clock busy), so it
    /// keeps the origin it rejoins; a new one is not one yet.
    Plays {
        started: BTreeSet<usize>,
        resumed: BTreeSet<usize>,
    },
    /// The node itself, moved onto the clock: every play it has.
    Node,
}

impl Engine {
    /// Put `node`'s animations on the clock timeline `clock` (or with
    /// `None`, on their own starts again). Moving onto one, its running
    /// plays join it as new ones would (D4); a kept start would be a member
    /// out of phase.
    pub fn set_animation_clock(&mut self, node: u64, clock: Option<&str>) {
        match clock {
            Some(name) if self.clocks.of.get(&node).map(String::as_str) != Some(name) => {
                self.clocks.of.insert(node, name.to_owned());
                self.join_clock_node(node);
            }
            Some(_) => {}
            None => {
                self.clocks.of.remove(&node);
                self.clocks.joining.remove(&node);
            }
        }
    }

    /// The clock timeline `node`'s animations are on.
    pub fn animation_clock(&self, node: u64) -> Option<&str> {
        self.clocks.of.get(&node).map(String::as_str)
    }

    /// Hold every clock join until [`Engine::join_clocks`]: a commit's rows
    /// are applied first (`MotionSync::apply`).
    pub fn hold_clock_joins(&mut self) {
        self.clocks.held = true;
    }

    /// Join the plays that started, resumed or moved onto a clock since the
    /// joins were held (D3, D4), and stop holding them.
    pub fn join_clocks(&mut self) {
        self.clocks.held = false;
        let joining = std::mem::take(&mut self.clocks.joining);
        if joining.is_empty() {
            return;
        }
        let now = self.sample_time();
        // Whether a play of `node` at `index` joins now, and so is not yet
        // a member.
        let joins = |node: u64, index: usize| match joining.get(&node) {
            Some(Joining::Node) => true,
            Some(Joining::Plays { started, .. }) => started.contains(&index),
            None => false,
        };
        // Each joining clock's origin: kept while a member is live (paused,
        // or not yet ended; a drag-bound play follows its drag, not the
        // clock), else now.
        let names: BTreeSet<String> = joining
            .keys()
            .filter_map(|node| self.clocks.of.get(node).cloned())
            .collect();
        for name in names {
            let busy = self
                .clocks
                .of
                .iter()
                .filter(|(n, c)| **c == name && !self.timeline_bound(**n))
                .any(|(n, _)| {
                    self.animations.get(n).is_some_and(|plays| {
                        plays.iter().enumerate().any(|(i, p)| {
                            !joins(*n, i)
                                && p.pending.is_none()
                                && (p.hold.is_some() || p.local(now) < p.animation.end_time())
                        })
                    })
                });
            if self.clocks.pending.contains_key(&name) {
                continue;
            }
            if !busy || !self.clocks.origins.contains_key(&name) {
                if self.start_on_frame {
                    self.clocks.pending.insert(name.clone(), now);
                }
                self.clocks.origins.insert(name, now);
            }
        }
        for (node, what) in &joining {
            let Some(name) = self.clocks.of.get(node) else {
                continue;
            };
            let Some(origin) = self.clocks.origins.get(name).copied() else {
                continue;
            };
            let waits = self.clocks.pending.get(name).copied();
            if self.timeline_bound(*node) {
                continue;
            }
            let Some(plays) = self.animations.get_mut(node) else {
                continue;
            };
            for (i, play) in plays.iter_mut().enumerate() {
                // A held play rejoins when it resumes (D6); an ended one
                // stays ended, not restarted by the move.
                let picked = match what {
                    Joining::Node => true,
                    Joining::Plays { started, resumed } => {
                        started.contains(&i) || resumed.contains(&i)
                    }
                };
                if !picked || play.hold.is_some() || play.local(now) >= play.animation.end_time() {
                    continue;
                }
                // Joining an origin that waits: the member waits too, from
                // its own birth, and the frame gives it the origin's phase.
                play.start = match waits {
                    Some(_) => now,
                    None => boundary(&play.animation, now, origin),
                };
                play.pending = waits.map(|_| now);
                play.clock_wait = waits.is_some();
                for p in play.animation.keyframes.properties() {
                    self.dirty.insert((*node, p));
                }
            }
            self.schedule_animations(*node);
        }
    }

    /// Plays of `node` that join its clock, `started` or `resumed` (row
    /// indices): now, or at the end of the commit while joins are held.
    pub(super) fn join_clock(&mut self, node: u64, started: &[usize], resumed: &[usize]) {
        if !self.clocks.of.contains_key(&node) || started.len() + resumed.len() == 0 {
            return;
        }
        let entry = self.clocks.joining.entry(node).or_insert(Joining::Plays {
            started: BTreeSet::new(),
            resumed: BTreeSet::new(),
        });
        if let Joining::Plays {
            started: s,
            resumed: r,
        } = entry
        {
            s.extend(started);
            r.extend(resumed);
        }
        self.join_clocks_unless_held();
    }

    /// `node` moved onto its clock, or off a drag timeline while on one:
    /// every play it has joins (D4), now or at the end of the commit.
    pub(super) fn join_clock_node(&mut self, node: u64) {
        if self.clocks.of.contains_key(&node) {
            self.clocks.joining.insert(node, Joining::Node);
            self.join_clocks_unless_held();
        }
    }

    fn join_clocks_unless_held(&mut self) {
        if !self.clocks.held {
            self.join_clocks();
        }
    }

    /// The origin of the clock `node`'s plays are on, once the frame has
    /// started it (a drag-bound node follows its drag instead).
    pub(super) fn clock_origin(&self, node: u64) -> Option<f64> {
        let name = self.clocks.of.get(&node)?;
        if self.clocks.pending.contains_key(name) || self.timeline_bound(node) {
            return None;
        }
        self.clocks.origins.get(name).copied()
    }

    /// Whether a clock's origin waits for the first presented frame.
    pub(super) fn clock_pending(&self) -> bool {
        !self.clocks.pending.is_empty()
    }

    /// The frame at `frame` moves every waiting origin set at or before it
    /// (with `all`, every one) by the wait; its members move with their own
    /// pending starts.
    pub(super) fn start_clocks(&mut self, frame: f64, all: bool) {
        let origins = &mut self.clocks.origins;
        self.clocks.pending.retain(|name, begin| {
            if *begin > frame && !all {
                return true;
            }
            if let Some(origin) = origins.get_mut(name) {
                *origin += (frame - *begin).max(0.0);
            }
            false
        });
    }

    pub(super) fn forget_clock(&mut self, node: u64) {
        self.clocks.of.remove(&node);
        self.clocks.joining.remove(&node);
    }
}

/// The last cycle boundary of `origin`'s timeline at or before `now`: an
/// iteration, or two under `alternate`, so a joiner's first is forwards and
/// it ends on the keyframe it would end on alone.
pub(super) fn boundary(animation: &Animation, now: f64, origin: f64) -> f64 {
    let alternates = matches!(
        animation.direction,
        Direction::Alternate | Direction::AlternateReverse
    );
    let period = animation.duration * if alternates { 2.0 } else { 1.0 };
    if !(period > 0.0 && period.is_finite()) {
        return now;
    }
    // A join on a boundary, in float, can land a hair before it and take
    // the cycle before: one whole period early, which can end a finite
    // joiner before it shows. Within a nanosecond is on it.
    let into = (now - origin).rem_euclid(period);
    now - if period - into < 1e-9 { 0.0 } else { into }
}
