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

use crate::animation::{Animation, AnimationError, Animations, PlayState};
use crate::property::{Property, Value};
use crate::transition::{Curve, Running, Transition, TransitionError, Transitions};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

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
    /// An `animation` row was invalid.
    Animation(AnimationError),
}

/// A keyframe animation in play on one node. Its start is the clock when
/// its style first applied (CSS Animations §3); while paused, `paused` holds
/// the local time it stopped at.
#[derive(Debug, Clone, PartialEq)]
struct Playing {
    animation: Animation,
    start: f64,
    paused: Option<f64>,
    // The appearance its `light-dark()` keyframes took when it started, as
    // a browser resolves them once (LLP 1062 D9).
    dark: bool,
}

impl Playing {
    fn local(&self, now: f64) -> f64 {
        self.paused.unwrap_or(now - self.start)
    }

    /// Whether its values can still change as the clock moves.
    fn live(&self, now: f64) -> bool {
        self.paused.is_none() && self.local(now) < self.animation.end_time()
    }
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

// Engine keys are host-allocated integers, never text. Frame order belongs
// to `dirty`, not the target lookup table.
#[derive(Debug, Default)]
struct SlotHasher(u64);
impl Hasher for SlotHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_u64(u64::from(*byte));
        }
    }
    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// The motion state of every node the host has told it about.
#[derive(Debug, Default)]
pub struct Engine {
    now: f64,
    transitions: BTreeMap<u64, Transitions>,
    slots: HashMap<(u64, Property), Slot, BuildHasherDefault<SlotHasher>>,
    // Observed targets provide CSS's before-change style, but only live curves
    // need a clock. Holds and settled slots never enter this index.
    running: BTreeSet<(u64, Property)>,
    dirty: HashSet<(u64, Property), BuildHasherDefault<SlotHasher>>,
    // Keyframe animations overlay the slots' values; `animating` indexes the
    // nodes whose overlay still moves with the clock.
    animations: BTreeMap<u64, Vec<Playing>>,
    animating: BTreeSet<u64>,
    // Each node's `layout-transition` declaration (LLP 1063): the only thing
    // that moves `Property::Layout`, so `transition: all` never covers layout.
    layout: BTreeMap<u64, Transition>,
    // The appearance a keyframe's `light-dark()` colour takes (LLP 1062 D9),
    // and the nodes whose own appearance differs from it.
    dark: bool,
    node_dark: BTreeMap<u64, bool>,
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
        let Some(list) = self.animations.get_mut(&node) else {
            return;
        };
        for p in list.iter_mut().filter(|p| p.dark != dark) {
            p.dark = dark;
            for block in &p.animation.keyframes.blocks {
                for (property, _) in &block.dark {
                    self.dirty.insert((node, *property));
                }
            }
        }
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

    /// Set a node's `layout-transition` row (LLP 1063): the last declaration
    /// that covers every property governs [`Property::Layout`] changes
    /// observed from now on. One that names a property governs nothing, as
    /// `transition: opacity 1s` does not move a box.
    pub fn set_layout_transition(
        &mut self,
        node: u64,
        transitions: &Transitions,
    ) -> Result<(), EngineError> {
        transitions.validate().map_err(EngineError::Transition)?;
        match transitions.matching(Property::Layout) {
            Some(declaration) => self.layout.insert(node, declaration.clone()),
            None => self.layout.remove(&node),
        };
        Ok(())
    }

    /// Set a node's `animation` row, at the current clock. CSS Animations
    /// §3: an animation already playing under the same keyframes keeps its
    /// start time, and new durations, delays or counts apply as if it had
    /// always had them; a new one starts now; one no longer listed stops,
    /// and the property shows its un-animated value again. An unchanged row
    /// changes nothing, so re-rendering never restarts an animation.
    pub fn set_animations(&mut self, node: u64, list: Animations) -> Result<(), EngineError> {
        list.validate().map_err(EngineError::Animation)?;
        let now = self.now;
        let old = self.animations.remove(&node).unwrap_or_default();
        let mut unmatched: Vec<Option<&Playing>> = old.iter().map(Some).collect();
        let next: Vec<Playing> = list
            .0
            .into_iter()
            .map(|animation| {
                let paused = animation.play_state == PlayState::Paused;
                let same = unmatched
                    .iter_mut()
                    .find(|p| p.is_some_and(|p| p.animation.keyframes == animation.keyframes))
                    .and_then(Option::take);
                match same {
                    Some(p) => {
                        let local = p.local(now);
                        Playing {
                            animation,
                            start: if p.paused.is_some() && !paused {
                                now - local
                            } else {
                                p.start
                            },
                            paused: paused.then_some(local),
                            dark: p.dark,
                        }
                    }
                    None => Playing {
                        animation,
                        start: now,
                        paused: paused.then_some(0.0),
                        dark: self.dark_of(node),
                    },
                }
            })
            .collect();
        if next == old {
            if !old.is_empty() {
                self.animations.insert(node, old);
            }
            return Ok(());
        }
        for property in Property::ALL {
            if old
                .iter()
                .chain(&next)
                .any(|p| p.animation.keyframes.affects(property))
            {
                self.dirty.insert((node, property));
            }
        }
        if next.iter().any(|p| p.live(now)) {
            self.animating.insert(node);
        } else {
            self.animating.remove(&node);
        }
        if !next.is_empty() {
            self.animations.insert(node, next);
        }
        Ok(())
    }

    /// The value a property shows: its transition-level value with every
    /// animation on the node composited over it, in list order.
    fn shown(&self, node: u64, property: Property, base: Value) -> Value {
        let Some(list) = self.animations.get(&node) else {
            return base;
        };
        list.iter().fold(base, |under, p| {
            p.animation
                .progress(p.local(self.now))
                .and_then(|progress| p.animation.value(property, progress, under, p.dark))
                .unwrap_or(under)
        })
    }

    /// Forget a node entirely.
    pub fn remove(&mut self, node: u64) {
        self.node_dark.remove(&node);
        self.transitions.remove(&node);
        self.layout.remove(&node);
        self.animations.remove(&node);
        self.animating.remove(&node);
        // Removing a list must not scan every other node once per row.
        for property in Property::ALL {
            self.remove_property(node, property);
        }
        self.remove_property(node, Property::Layout);
    }

    /// Replay a node's animations from the current clock: its `exit-animation`
    /// as it leaves (LLP 1063), which restarts even when it names the
    /// keyframes an entry animation was already playing. The end is the
    /// clock time the last of them ends, infinite for an endless one.
    pub fn restart_animations(&mut self, node: u64, list: Animations) -> Result<f64, EngineError> {
        self.set_animations(node, Animations::NONE)?;
        let end = list.end_time();
        self.set_animations(node, list)?;
        Ok(self.now + end)
    }

    /// Forget only this property's target, curve, hold and pending frame.
    /// Returns whether it existed. Other properties and the node's transition
    /// declaration survive; readoption takes a new value without transitioning.
    /// No clock change occurs, and old hold tokens immediately become stale.
    pub fn remove_property(&mut self, node: u64, property: Property) -> bool {
        self.dirty.remove(&(node, property));
        self.running.remove(&(node, property));
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
        let declaration = if change.property == Property::Layout {
            self.layout.get(&change.node)
        } else {
            self.transitions
                .get(&change.node)
                .and_then(|t| t.matching(change.property))
        }
        .filter(|t| t.starts())
        .map(|t| t.governing(change.property));

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
                    self.running.remove(&key);
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
        if slot.running.is_some() {
            self.running.insert(key);
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
        self.running.retain(|key| {
            let slot = self.slots.get_mut(key).expect("running slot");
            let running = slot.running.as_ref().expect("indexed curve");
            let sample = running.sample(now);
            slot.presented = sample.value;
            if sample.done {
                slot.running = None;
                slot.owner = None;
            }
            self.dirty.insert(*key);
            !sample.done
        });
        let (animations, dirty) = (&self.animations, &mut self.dirty);
        self.animating.retain(|node| {
            let list = &animations[node];
            for property in Property::ALL {
                if list.iter().any(|p| p.animation.keyframes.affects(property)) {
                    dirty.insert((*node, property));
                }
            }
            list.iter().any(|p| p.live(now))
        });
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
        let mut dirty: Vec<_> = std::mem::take(&mut self.dirty).into_iter().collect();
        dirty.sort_unstable();
        dirty
            .into_iter()
            .filter_map(|key| {
                self.slots.get(&key).map(|slot| Presentation {
                    node: key.0,
                    property: key.1,
                    value: self.shown(key.0, key.1, slot.presented),
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

    /// The current presentation value of one property, animations included.
    pub fn value(&self, node: u64, property: Property) -> Option<Value> {
        self.slots
            .get(&(node, property))
            .map(|s| self.shown(node, property, s.presented))
    }

    /// The current target of one property.
    pub fn target(&self, node: u64, property: Property) -> Option<Value> {
        self.slots.get(&(node, property)).map(|s| s.target)
    }

    /// Whether nothing is running: no transition, and no animation whose
    /// value still moves with the clock. An `infinite` animation keeps a
    /// host's frames running for as long as it plays.
    pub fn quiescent(&self) -> bool {
        self.running.is_empty() && self.animating.is_empty()
    }

    /// The clock time at which the last running transition or finite
    /// animation ends, or `None` when there is none. An agent advances here
    /// instead of waiting. An `infinite` animation never ends and is not
    /// waited for, as the web's `clock settle` skips an endless one.
    pub fn settle_time(&self) -> Option<f64> {
        let transitions = self.running.iter().map(|key| {
            self.slots[key]
                .running
                .as_ref()
                .expect("indexed curve")
                .end_time()
        });
        let animations = self
            .animating
            .iter()
            .flat_map(|node| &self.animations[node])
            .filter(|p| p.live(self.now))
            .map(|p| p.start + p.animation.end_time())
            .filter(|t| t.is_finite());
        transitions
            .chain(animations)
            .fold(None, |acc, t| Some(acc.map_or(t, |a: f64| a.max(t))))
    }
}

fn validate_value(property: Property, value: Value) -> Result<(), EngineError> {
    if !value.is_finite() {
        return Err(EngineError::NonFinite);
    }
    if !value.fits(property) {
        return Err(EngineError::InvalidValueShape);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Easing, TimingFunction, Transition, TransitionProperty};

    #[test]
    fn the_clock_index_contains_curves_only_and_retires_them() {
        let mut engine = Engine::new();
        let change = |node, value| Change {
            node,
            property: Property::Opacity,
            value: Value::scalar(value),
            velocity: None,
        };
        for node in 0..4096 {
            engine.observe(change(node, 1.0)).unwrap();
        }
        engine.frame();
        // advance and settle_time iterate this index, never the idle slots.
        assert!(engine.running.is_empty());
        engine.advance(1.0).unwrap();
        assert!(engine.frame().is_empty());
        assert_eq!(engine.slots.len(), 4096);
        assert_eq!(engine.settle_time(), None);

        engine
            .set_transitions(
                7,
                Transitions(vec![Transition::new(
                    TransitionProperty::All,
                    1.0,
                    TimingFunction::Easing(Easing::Linear),
                )]),
            )
            .unwrap();
        engine.observe(change(7, 0.0)).unwrap();
        assert_eq!(engine.running.len(), 1);
        engine.advance(1.5).unwrap();
        assert_eq!(engine.value(7, Property::Opacity), Some(Value::scalar(0.5)));
        let held = engine
            .begin_hold(7, Property::Opacity, 1.5, None)
            .unwrap()
            .unwrap();
        assert!(engine.running.is_empty());
        engine.end_hold(held.token, 1.5, HoldEnd::Cancel).unwrap();
        assert_eq!(engine.running.len(), 1);
        engine.advance(engine.settle_time().unwrap()).unwrap();
        assert!(engine.running.is_empty());
        engine.observe(change(7, 1.0)).unwrap();
        assert_eq!(engine.running.len(), 1);
        engine.remove_property(7, Property::Opacity);
        assert!(engine.running.is_empty());
    }
}
