//! The JS target's motion capability (LLP 1071 §7): springs, holds and a
//! pan's release velocity from the one `exact_motion` Engine every host runs.
//!
//! The page (`host/web-js/motion.js`) is the kernel seam here: it tells the
//! engine each registered node's `transition` row and its four compositor
//! targets as commits change them (the kernel's `motion_sync`), moves holds
//! for the web host's own `motion-glue.js`, and plays what [`Motion::lower`]
//! lowers, as `host/web/src/motion.rs`'s `Springs` does for the wasm host:
//! nothing here runs per frame; a spring is compiled to frames once.
//!
//! Nodes are the runtime's view ids. Time is seconds on the page's clock.

use exact_motion::{
    Change, Engine, EngineError, HoldEnd, HoldToken, Property, SpringDescriptor, Transitions,
    Value, VelocityTracker,
};
use std::collections::BTreeMap;

/// The compositor properties the seam feeds, in the page's order.
pub const TARGETS: [Property; 4] = [
    Property::Translate,
    Property::Scale,
    Property::Rotate,
    Property::Opacity,
];

/// The engine, the page's live holds by serial, the springs it plays, and
/// one pan contact's samples.
pub struct Motion {
    engine: Engine,
    holds: BTreeMap<u64, HoldToken>,
    playing: BTreeMap<(u64, Property), SpringDescriptor>,
    pan: Option<(u32, VelocityTracker)>,
}

impl Default for Motion {
    fn default() -> Self {
        Motion::new()
    }
}

impl Motion {
    /// Empty, at clock zero; the browser runs CSS transitions and
    /// animations itself (LLP 1055 D7), so the engine lowers only springs.
    pub fn new() -> Motion {
        let mut engine = Engine::new();
        engine.set_lowered(true);
        Motion {
            engine,
            holds: BTreeMap::new(),
            playing: BTreeMap::new(),
            pan: None,
        }
    }

    fn seek(&mut self, now: f64) -> Result<(), EngineError> {
        self.engine.advance(now.max(self.engine.now()))
    }

    /// A node's `transition` row as CSS text; text the grammar refuses is
    /// no transition, as an invalid CSS declaration is.
    pub fn transitions(&mut self, node: u64, text: &str) -> bool {
        let parsed = Transitions::parse(text);
        let ok = parsed.is_ok();
        self.engine
            .set_transitions(node, parsed.unwrap_or_default())
            .is_ok()
            && ok
    }

    /// A node's targets after a commit, at `now`: translate x and y, scale,
    /// rotate (degrees), opacity. The first observation of a node is taken
    /// as it is (nothing transitions), as the kernel's `adopt` does.
    pub fn observe(&mut self, node: u64, values: [f64; 5], now: f64) -> Result<(), EngineError> {
        self.seek(now)?;
        let [x, y, scale, rotate, opacity] = values;
        let targets = [
            Value::new(x, y),
            Value::scalar(scale),
            Value::scalar(rotate),
            Value::scalar(opacity),
        ];
        for (property, value) in TARGETS.into_iter().zip(targets) {
            self.engine.observe(Change {
                node,
                property,
                value,
                velocity: None,
            })?;
        }
        Ok(())
    }

    /// A node left the tree: forgotten, with its springs.
    pub fn remove(&mut self, node: u64) {
        self.engine.remove(node);
        self.playing.retain(|(n, _), _| *n != node);
        self.holds.retain(|_, token| token.node() != node);
    }

    fn token(&self, serial: u64) -> Option<HoldToken> {
        self.holds
            .get(&serial)
            .copied()
            .filter(|t| self.engine.has_hold(*t))
    }

    /// Capture a presented value; the hold's serial and the value it took,
    /// or `None` for a node the engine doesn't know.
    pub fn begin(
        &mut self,
        node: u64,
        property: Property,
        presented: Value,
        now: f64,
    ) -> Result<Option<(u64, Value)>, EngineError> {
        if self.engine.value(node, property).is_none() {
            return Ok(None);
        }
        let Some(start) = self
            .engine
            .begin_hold(node, property, now, Some(presented))?
        else {
            return Ok(None);
        };
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        self.holds.insert(start.token.serial(), start.token);
        // A curve crossing its target is cancelled on takeover too.
        self.playing.remove(&(node, property));
        Ok(Some((start.token.serial(), start.value)))
    }

    /// Move a hold; `false` for a stale one.
    pub fn update(&mut self, serial: u64, value: Value, now: f64) -> Result<bool, EngineError> {
        match self.token(serial) {
            Some(token) => self.engine.update_hold(token, now, value),
            None => Ok(false),
        }
    }

    /// Record what a constrained display shows for a hold (LLP 1057.001 §3).
    pub fn track(&mut self, serial: u64, now: f64, shown: Value) -> bool {
        self.token(serial)
            .is_some_and(|token| self.engine.track_hold(token, now, shown))
    }

    /// The engine's velocity over a hold's values, where the platform
    /// measures none (LLP 1057.001 §3).
    pub fn measured(&self, serial: u64, now: f64) -> Value {
        self.token(serial)
            .and_then(|token| self.engine.hold_velocity(token, now))
            .filter(|v| v.x.is_finite() && v.y.is_finite())
            .unwrap_or(Value::ZERO)
    }

    /// Release (a velocity) or cancel (`None`) a hold, to the newest target.
    pub fn end(
        &mut self,
        serial: u64,
        velocity: Option<Value>,
        now: f64,
    ) -> Result<bool, EngineError> {
        let Some(token) = self.token(serial) else {
            return Ok(false);
        };
        let end = velocity.map_or(HoldEnd::Cancel, |velocity| HoldEnd::Release { velocity });
        let accepted = self.engine.end_hold(token, now, end)?;
        if accepted {
            self.holds.remove(&serial);
        }
        Ok(accepted)
    }

    /// Whether a hold is live.
    pub fn live(&self, serial: u64) -> bool {
        self.token(serial).is_some()
    }

    /// A live hold's node and property.
    pub fn held(&self, serial: u64) -> Option<(u64, Property)> {
        self.token(serial).map(|t| (t.node(), t.property()))
    }

    /// When the last spring in flight ends, seconds.
    pub fn settle_time(&self) -> Option<f64> {
        self.engine.settle_time()
    }

    /// Seek to `now` and lower what changed, as the wasm host's batch ops
    /// (`Springs::lower_current`, `Host::emit_lowered`): a spring that
    /// started is its frames, one that stopped short of its target is
    /// cancelled. Each op is numbers in `out`: `[1, node, property, at,
    /// delay, duration, n, x0, y0, …]` (seconds) or `[2, node, property]`.
    pub fn lower(&mut self, now: f64, out: &mut Vec<f64>) -> Result<(), EngineError> {
        self.seek(now)?;
        let now = self.engine.now();
        let index = |p: Property| TARGETS.iter().position(|t| *t == p).unwrap_or(4) as f64;
        for p in self.engine.frame() {
            let key = (p.node, p.property);
            if self.engine.is_held(p.node, p.property) {
                self.playing.remove(&key);
                continue;
            }
            match self.engine.spring_descriptor(p.node, p.property) {
                Some(descriptor) => {
                    if self.playing.get(&key) == Some(&descriptor) {
                        continue;
                    }
                    let Some(frames) = self.engine.spring_frames(p.node, p.property) else {
                        continue;
                    };
                    self.playing.insert(key, descriptor);
                    out.extend([
                        1.0,
                        p.node as f64,
                        index(p.property),
                        now,
                        (frames.start - now).max(0.0),
                        frames.duration,
                        frames.values.len() as f64,
                    ]);
                    for v in &frames.values {
                        out.extend([v.x, v.y]);
                    }
                }
                None => {
                    if let Some(previous) = self.playing.remove(&key) {
                        if p.value != previous.target {
                            out.extend([2.0, p.node as f64, index(p.property)]);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// A pan's pointer sample at `ms`, the event's own time (LLP 1057 §10.6,
    /// `host/web/src/pan_velocity.rs`); `first` begins the contact on `view`.
    pub fn pan_sample(&mut self, view: u32, first: bool, x: f64, y: f64, ms: f64) {
        if first || self.pan.as_ref().is_none_or(|(v, _)| *v != view) {
            self.pan = Some((view, VelocityTracker::new()));
        }
        if let Some((_, tracker)) = self.pan.as_mut() {
            tracker.push(ms / 1000.0, Value::new(x, y));
        }
    }

    /// The contact's release velocity at `ms`, CSS pixels per second, then
    /// forgotten; zero for another view or too few samples.
    pub fn pan_release(&mut self, view: u32, ms: f64) -> Value {
        let v = self
            .pan
            .take()
            .filter(|(v, _)| *v == view)
            .map_or(Value::ZERO, |(_, t)| t.estimate(ms / 1000.0));
        if v.x.is_finite() && v.y.is_finite() {
            v
        } else {
            Value::ZERO
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod abi;

#[cfg(test)]
mod tests;
