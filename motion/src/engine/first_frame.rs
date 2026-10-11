//! Motion starts at the first frame that shows it (LLP 1003.001).
//!
//! On a host whose display drives the clock, a curve an author's commit
//! begins between frames is pending: every reading of it is at the time it
//! began, so it does not move, and the next presented frame starts it there
//! (D1, D2). The engine keeps two times (D5): `now`, the input clock, which
//! inputs, holds and releases move and check against; and `shown`, the latest
//! frame it presented. Every sample is at the later of the two.

use super::{Engine, EngineError};
use crate::property::Property;

impl Engine {
    /// The time every sample is taken at: the input clock, or the latest
    /// presented frame if that is later (LLP 1003.001 D5).
    pub fn sample_time(&self) -> f64 {
        self.now.max(self.shown)
    }

    /// Whether curves an author's commit begins wait for the first presented
    /// frame.
    pub fn starts_on_frame(&self) -> bool {
        self.start_on_frame
    }

    /// Turn the rule on or off. Off (the agent's takeover, D7), everything
    /// pending starts at `at`, or at its own begin if that is later, and the
    /// engine forgets the frames it presented: from here it samples on the
    /// agent's clock alone. Returns the nodes whose plays started, for their
    /// lowered specs.
    pub fn set_start_on_frame(&mut self, on: bool, at: f64) -> Result<Vec<u64>, EngineError> {
        let started = if on || !self.start_on_frame {
            Vec::new()
        } else {
            // Sampled where the agent's clock takes over, as Core Animation
            // is: not at a frame this forgets (D7).
            // `shown` is forgotten first, so nothing that ends between the
            // takeover and that frame is retired by the start's own sample.
            self.shown = f64::NEG_INFINITY;
            self.now = self.now.max(at);
            let started = self.start_pending(at, true)?;
            self.shown = f64::NEG_INFINITY;
            started
        };
        self.start_on_frame = on;
        Ok(started)
    }

    /// A presented frame at `frame`: raise `shown`, start every pending curve
    /// that began at or before it, and sample every curve there (D2). Returns
    /// the nodes whose plays started, for a lowering host to re-emit.
    /// Idempotent: a second call at the same frame starts only what began
    /// since.
    pub fn present_frame(&mut self, frame: f64) -> Result<Vec<u64>, EngineError> {
        self.start_pending(frame, false)
    }

    /// Start what began at or before `frame` there; with `all`, what began
    /// later too, at its own begin.
    fn start_pending(&mut self, frame: f64, all: bool) -> Result<Vec<u64>, EngineError> {
        if !frame.is_finite() {
            return Err(EngineError::NonFinite);
        }
        self.shown = self.shown.max(frame);
        let at = |begin: f64| (begin <= frame || all).then_some(frame.max(begin));
        let keys = std::mem::take(&mut self.pending);
        for key in keys {
            let Some(curve) = self
                .slots
                .get_mut(&key)
                .and_then(|slot| slot.live.as_mut())
                .and_then(|live| live.running.as_mut())
            else {
                continue;
            };
            if let Some(begin) = curve.pending {
                match at(begin) {
                    Some(t) => {
                        curve.start += t - begin;
                        curve.pending = None;
                    }
                    None => {
                        self.pending.insert(key);
                    }
                }
            }
        }
        self.start_clocks(frame, all);
        let mut started = Vec::new();
        let nodes: Vec<u64> = self.animating.iter().copied().collect();
        for node in nodes {
            let starts = self.animations.get(&node).is_some_and(|plays| {
                plays
                    .iter()
                    .any(|p| p.pending.is_some_and(|b| at(b).is_some()))
            });
            if !starts {
                continue;
            }
            let clock_origin = self.clock_origin(node);
            let Some(plays) = self.animations.get_mut(&node) else {
                continue;
            };
            let mut any = false;
            for play in plays.iter_mut() {
                if let Some((begin, t)) = play.pending.and_then(|b| Some((b, at(b)?))) {
                    // A member that joined a waiting clock origin takes its
                    // phase at the frame, from the origin that frame started
                    // (D8); any other play starts at its own frame.
                    play.start = match clock_origin.filter(|_| play.clock_wait) {
                        Some(origin) => super::clock::boundary(&play.animation, t, origin),
                        None => play.start + (t - begin),
                    };
                    play.pending = None;
                    play.clock_wait = false;
                    any = true;
                }
            }
            if any {
                for play in self.animations[&node].iter() {
                    for p in play.animation.keyframes.properties() {
                        self.dirty.insert((node, p));
                    }
                }
                self.schedule_animations(node);
                started.push(node);
            }
        }
        // Values an earlier seek cached are behind the frame: sample again.
        self.advance(self.now)?;
        Ok(started)
    }

    /// When a property's running curve starts moving (its begin plus delay),
    /// or `None` while it is pending or nothing runs (D3: a flight lands from
    /// its curve's own start).
    pub fn curve_start(&self, node: u64, property: Property) -> Option<f64> {
        let curve = self.slots.get(&(node, property))?.running()?;
        curve.pending.is_none().then_some(curve.start)
    }

    /// When a leaving node's exit ends: the latest end of its last `count`
    /// plays, the ones `play_exit` appended; `None` while any of them is
    /// pending (D3).
    pub fn exit_end(&self, node: u64, count: usize) -> Option<f64> {
        let plays = self.animations.get(&node)?;
        let exit = &plays[plays.len().saturating_sub(count)..];
        if exit.iter().any(|p| p.pending.is_some()) {
            return None;
        }
        Some(
            exit.iter()
                .map(|p| p.start + p.animation.end_time())
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }
}
