//! The engine: per-node presentation state under a seekable clock.
//!
//! @ref LLP 1002 §3 (the frame; who owns the clock); LLP 1003 §3
//!
//! The host owns time. It tells the engine what the kernel committed
//! ([`Engine::observe`], one [`Change`] per animatable row that changed),
//! advances the clock ([`Engine::advance`]), and takes the presentation values
//! to paint ([`Engine::frame`]). The engine holds no thread, no timer, and no
//! reference to the kernel: nodes are numbers the host chose.
//!
//! Because every running transition is a closed-form function of clock time,
//! `advance(t)` is a seek. A test advances to `0.3` and reads; an agent's
//! `clock` operation advances to [`Engine::settle_time`] and reads; nothing
//! ever waits. On the web none of this runs per frame — the browser is the
//! executor — but the same engine under a virtual clock is the oracle a web
//! host's output is compared against.

use crate::property::{Property, Value};
use crate::transition::{Curve, Running, TransitionError, Transitions};
use std::collections::{BTreeMap, BTreeSet};

mod hold;
pub use hold::{HoldEnd, HoldStart, HoldToken, TransformHold};

/// One animatable row's new target, as committed by the kernel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Change {
    /// The node, in the host's numbering.
    pub node: u64,
    /// Which property.
    pub property: Property,
    /// The new target (the style value after the commit).
    pub value: Value,
    /// Velocity the value is already moving at — a released gesture's — for
    /// a spring to inherit. Ignored by easings, which CSS gives no velocity.
    pub velocity: Option<Value>,
}

/// One value for the host to paint this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presentation {
    /// The node, in the host's numbering.
    pub node: u64,
    /// Which property.
    pub property: Property,
    /// The value to paint.
    pub value: Value,
}

/// Why the engine refused an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    /// `advance` was called with a time before the current one.
    ClockWentBackwards,
    /// A time or value was infinite or NaN.
    NonFinite,
    /// A scalar property carried a nonzero second component.
    InvalidValueShape,
    /// All process-local hold serials have been used; none may be reused.
    HoldSerialExhausted,
    /// A `transition` row was invalid.
    Transition(TransitionError),
}

#[derive(Debug, Clone, PartialEq)]
struct Slot {
    target: Value,
    presented: Value,
    running: Option<Running>,
    owner: Option<Owner>,
}

/// One process-unique identity follows a held property into its own return.
/// Authored replacement curves have no owner; no historical identities remain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Held(u64),
    Returning(u64),
}

/// Fixed-size identity of a running spring's complete curve. Web hosts compare
/// this before lowering frames, so a seek or unrelated input does not regenerate
/// an unchanged curve. Contains no sampled frames or derived settle duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringDescriptor {
    /// Clock time the spring starts moving, including its declared delay.
    pub start: f64,
    /// Presentation at release.
    pub from: Value,
    /// Authored target for this curve.
    pub target: Value,
    /// Release velocity in property units per second.
    pub velocity: Value,
    /// Parameters captured when this curve began.
    pub config: crate::spring::SpringConfig,
}

/// A spring in flight, restated for a host that lowers it instead of
/// sampling it per frame — the web, where the browser plays the frames
/// through `Element.animate` with linear easing (LLP 1002 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct SpringFrames {
    /// The node.
    pub node: u64,
    /// The property.
    pub property: Property,
    /// Clock time the spring starts moving (its change time plus delay).
    pub start: f64,
    /// Seconds from `start` to rest.
    pub duration: f64,
    /// Values on the 240 Hz grid, evenly spaced from `start` to `start +
    /// duration`; the first is the release value, the last the target.
    pub values: Vec<Value>,
}

/// The motion state of every node the host has told it about.
#[derive(Debug, Default)]
pub struct Engine {
    now: f64,
    transitions: BTreeMap<u64, Transitions>,
    slots: BTreeMap<(u64, Property), Slot>,
    dirty: BTreeSet<(u64, Property)>,
}

impl Engine {
    /// An engine at time zero with no nodes.
    pub fn new() -> Engine {
        Engine::default()
    }

    /// The clock.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Set a node's `transition` row. Governs changes observed from now on;
    /// a transition already running keeps its own declaration.
    pub fn set_transitions(
        &mut self,
        node: u64,
        transitions: Transitions,
    ) -> Result<(), EngineError> {
        transitions.validate().map_err(EngineError::Transition)?;
        if transitions.0.is_empty() {
            self.transitions.remove(&node);
        } else {
            self.transitions.insert(node, transitions);
        }
        Ok(())
    }

    /// Forget a node entirely.
    pub fn remove(&mut self, node: u64) {
        self.transitions.remove(&node);
        // Removing a list must not scan every other node once per row.
        for property in Property::ALL {
            self.remove_property(node, property);
        }
    }

    /// Forget only this property's target, curve, hold and pending frame.
    /// Returns whether it existed. Other properties and the node's transition
    /// declaration survive; readoption takes a new value without transitioning.
    /// No clock change occurs, and old hold tokens immediately become stale.
    pub fn remove_property(&mut self, node: u64, property: Property) -> bool {
        self.dirty.remove(&(node, property));
        self.slots.remove(&(node, property)).is_some()
    }

    /// Whether this property is held or has a running curve, including delay.
    /// Equality with its target does not imply rest: a spring may carry velocity
    /// at zero displacement. Holds are active here but remain clock-quiescent.
    pub fn is_active(&self, node: u64, property: Property) -> bool {
        self.slots.get(&(node, property)).is_some_and(|slot| {
            matches!(slot.owner, Some(Owner::Held(_))) || slot.running.is_some()
        })
    }

    /// A committed change to one animatable row. This is CSS Transitions §3:
    /// a property the engine has never seen takes its value with no
    /// transition (there is no before-change style); otherwise a matching
    /// `transition` declaration starts one from the current value, or
    /// interrupts and possibly reverses the one running.
    pub fn observe(&mut self, change: Change) -> Result<(), EngineError> {
        validate_value(change.property, change.value)?;
        if let Some(velocity) = change.velocity {
            validate_value(change.property, velocity)?;
        }
        let key = (change.node, change.property);
        let now = self.now;
        let declaration = self
            .transitions
            .get(&change.node)
            .and_then(|t| t.matching(change.property))
            .filter(|t| t.starts())
            .cloned();

        let Some(slot) = self.slots.get_mut(&key) else {
            self.slots.insert(
                key,
                Slot {
                    target: change.value,
                    presented: change.value,
                    running: None,
                    owner: None,
                },
            );
            self.dirty.insert(key);
            return Ok(());
        };

        let after = change.value;
        if matches!(slot.owner, Some(Owner::Held(_))) {
            slot.target = after;
            return Ok(());
        }
        match slot.running.take() {
            None => {
                if after == slot.target {
                    return Ok(());
                }
                slot.owner = None;
                let before = slot.presented;
                slot.target = after;
                match declaration {
                    Some(declaration) => {
                        let velocity = change.velocity.unwrap_or(Value::ZERO);
                        slot.running = Some(Running::start(
                            &declaration,
                            before,
                            after,
                            velocity,
                            now,
                            before,
                            1.0,
                        ));
                        slot.presented = slot
                            .running
                            .as_ref()
                            .map_or(before, |r| r.sample(now).value);
                    }
                    None => slot.presented = after,
                }
            }
            Some(running) => {
                if after == running.to {
                    slot.running = Some(running);
                    return Ok(());
                }
                slot.owner = None;
                let current = running.sample(now);
                slot.target = after;
                let Some(declaration) = declaration.filter(|_| current.value != after) else {
                    slot.presented = after;
                    self.dirty.insert(key);
                    return Ok(());
                };
                let inherited = change.velocity.unwrap_or(current.velocity);
                let is_easing = matches!(
                    declaration.timing,
                    crate::transition::TimingFunction::Easing(_)
                );
                let next = if is_easing && after == running.reversing_adjusted_start {
                    // CSS §3.2, the reversing case.
                    let progress = running.easing_progress(now);
                    let factor = (progress * running.reversing_shortening
                        + (1.0 - running.reversing_shortening))
                        .abs()
                        .clamp(0.0, 1.0);
                    Running::start(
                        &declaration,
                        current.value,
                        after,
                        inherited,
                        now,
                        running.to,
                        factor,
                    )
                } else {
                    Running::start(
                        &declaration,
                        current.value,
                        after,
                        inherited,
                        now,
                        current.value,
                        1.0,
                    )
                };
                slot.presented = next.sample(now).value;
                slot.running = Some(next);
            }
        }
        self.dirty.insert(key);
        Ok(())
    }

    /// Move the clock to `now` and sample every running transition there.
    /// Seeking is the only operation: the result depends on `now`, never on
    /// how many calls it took to get there.
    pub fn advance(&mut self, now: f64) -> Result<(), EngineError> {
        self.validate_time(now)?;
        self.now = now;
        for (key, slot) in self.slots.iter_mut() {
            let Some(running) = &slot.running else {
                continue;
            };
            let sample = running.sample(now);
            slot.presented = sample.value;
            if sample.done {
                slot.running = None;
                slot.owner = None;
            }
            self.dirty.insert(*key);
        }
        Ok(())
    }

    fn validate_time(&self, now: f64) -> Result<(), EngineError> {
        if !now.is_finite() {
            return Err(EngineError::NonFinite);
        }
        if now < self.now {
            return Err(EngineError::ClockWentBackwards);
        }
        Ok(())
    }

    /// The values that changed since the last frame, in node order. Taking
    /// them clears the set; a host paints exactly these.
    pub fn frame(&mut self) -> Vec<Presentation> {
        let dirty = std::mem::take(&mut self.dirty);
        dirty
            .into_iter()
            .filter_map(|key| {
                self.slots.get(&key).map(|slot| Presentation {
                    node: key.0,
                    property: key.1,
                    value: slot.presented,
                })
            })
            .collect()
    }

    /// The running spring's identity, without allocating, sampling, or computing
    /// its settle time. After one slot lookup this only copies fixed-size fields.
    /// `None` for held, settled, unknown properties and easings. A host may lower
    /// frames only when this descriptor differs from its last playback.
    pub fn spring_descriptor(&self, node: u64, property: Property) -> Option<SpringDescriptor> {
        let running = self.slots.get(&(node, property))?.running.as_ref()?;
        let Curve::Spring { config, velocity } = &running.curve else {
            return None;
        };
        Some(SpringDescriptor {
            start: running.start,
            from: running.from,
            target: running.to,
            velocity: *velocity,
            config: *config,
        })
    }

    /// The spring running on one property, lowered to frames; `None` when
    /// nothing runs there or what runs is an easing.
    pub fn spring_frames(&self, node: u64, property: Property) -> Option<SpringFrames> {
        let running = self.slots.get(&(node, property))?.running.as_ref()?;
        let (duration, values) = running.spring_frames()?;
        Some(SpringFrames {
            node,
            property,
            start: running.start,
            duration,
            values,
        })
    }

    /// The current presentation value of one property.
    pub fn value(&self, node: u64, property: Property) -> Option<Value> {
        self.slots.get(&(node, property)).map(|s| s.presented)
    }

    /// The current target of one property.
    pub fn target(&self, node: u64, property: Property) -> Option<Value> {
        self.slots.get(&(node, property)).map(|s| s.target)
    }

    /// Whether nothing is running.
    pub fn quiescent(&self) -> bool {
        self.slots.values().all(|s| s.running.is_none())
    }

    /// The clock time at which the last running transition ends, or `None`
    /// when quiescent. An agent advances here instead of waiting.
    pub fn settle_time(&self) -> Option<f64> {
        self.slots
            .values()
            .filter_map(|s| s.running.as_ref().map(Running::end_time))
            .fold(None, |acc, t| Some(acc.map_or(t, |a: f64| a.max(t))))
    }
}

fn validate_value(property: Property, value: Value) -> Result<(), EngineError> {
    if !value.is_finite() {
        return Err(EngineError::NonFinite);
    }
    if property.components() == 1 && value.y != 0.0 {
        return Err(EngineError::InvalidValueShape);
    }
    Ok(())
}
