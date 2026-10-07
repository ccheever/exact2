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
use crate::transition::{Curve, Running, Transition, TransitionError, Transitions};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

mod animate;
mod clock;
mod hold;
mod path;
mod played;
pub use played::{PlayedCurve, PlayedTransition};
mod timeline;
pub use animate::AnimationPlay;
pub use hold::{HoldEnd, HoldStart, HoldToken, TransformHold};
pub use timeline::NamedTimeline;

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
    /// An `animation` row was invalid (LLP 1055 D5).
    InvalidAnimation,
}

/// Every observed property of every node has a slot (four per node by
/// receipt sync), and almost all are settled: presented at their target,
/// nothing running, no hold. A settled slot is its target alone; the rest
/// lives in a box made when the presentation leaves the target and dropped
/// when it settles again. A list's peak of nodes is then 56 bytes a
/// property here, not 104. Readers see the same values either way.
#[derive(Debug, Clone, PartialEq)]
struct Slot {
    target: Value,
    live: Option<Box<Live>>,
}

#[derive(Debug, Clone, PartialEq)]
struct Live {
    presented: Value,
    running: Option<Running>,
    owner: Option<Owner>,
}

/// Bitwise, so a settled slot hands back exactly the value it was given
/// (`-0` is not `0` here).
fn same(a: Value, b: Value) -> bool {
    [a.x, a.y, a.z, a.w]
        .iter()
        .zip([b.x, b.y, b.z, b.w])
        .all(|(a, b)| a.to_bits() == b.to_bits())
}

impl Slot {
    fn settled(target: Value) -> Slot {
        Slot { target, live: None }
    }
    fn presented(&self) -> Value {
        self.live.as_ref().map_or(self.target, |l| l.presented)
    }
    fn running(&self) -> Option<&Running> {
        self.live.as_ref()?.running.as_ref()
    }
    fn owner(&self) -> Option<Owner> {
        self.live.as_ref()?.owner
    }
    /// The box, made (presenting the target) if the slot was settled.
    fn live(&mut self) -> &mut Live {
        let target = self.target;
        self.live.get_or_insert_with(|| {
            Box::new(Live {
                presented: target,
                running: None,
                owner: None,
            })
        })
    }
    /// A settled slot gives its box back.
    fn settle(&mut self) {
        if self.live.as_ref().is_some_and(|l| {
            l.running.is_none() && l.owner.is_none() && same(l.presented, self.target)
        }) {
            self.live = None;
        }
    }
    fn set_target(&mut self, target: Value) {
        if !same(target, self.target) {
            // The presentation stays where it is.
            self.live();
        }
        self.target = target;
        self.settle();
    }
    fn set_presented(&mut self, presented: Value) {
        if self.live.is_none() && same(presented, self.target) {
            return;
        }
        self.live().presented = presented;
        self.settle();
    }
    fn set_running(&mut self, running: Option<Running>) {
        if self.live.is_none() && running.is_none() {
            return;
        }
        self.live().running = running;
        self.settle();
    }
    fn take_running(&mut self) -> Option<Running> {
        let running = self.live.as_mut()?.running.take();
        self.settle();
        running
    }
    fn set_owner(&mut self, owner: Option<Owner>) {
        if self.live.is_none() && owner.is_none() {
            return;
        }
        self.live().owner = owner;
        self.settle();
    }
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
    // Transitions a host plays itself (`play_transition`): kept for a
    // reversal's arithmetic, never sampled, dropped once ended.
    played: BTreeSet<(u64, Property)>,
    dirty: HashSet<(u64, Property), BuildHasherDefault<SlotHasher>>,
    // CSS animations per node (LLP 1055 D5), and the nodes a sampling host
    // must still advance: some animation running and not yet ended.
    animations: BTreeMap<u64, Vec<AnimationPlay>>,
    animating: BTreeSet<u64>,
    // Indexed by property; past the wire's for `Property::Layout` and `d`,
    // which no animation names and nothing lowers.
    lowered: [bool; Property::SLOTS],
    // Nodes whose animations are sampled whatever `lowered` says: a host
    // decides per node what its compositor plays faithfully (LLP 1055.000
    // D15: eligibility is per effect, not per property name).
    forced: BTreeSet<u64>,
    // Held presentations, by hold serial, for a release velocity where the
    // platform measures none (LLP 1057.001 §3). Only live holds keep one.
    held: BTreeMap<u64, crate::velocity::VelocityTracker>,
    // Each node's `layout-transition` declaration (LLP 1063): the only thing
    // that moves `Property::Layout`, so `transition: all` never covers layout.
    layout: BTreeMap<u64, Transition>,
    // The appearance a keyframe's `light-dark()` colour takes (LLP 1062 D9),
    // and the nodes whose own appearance differs from it.
    dark: bool,
    node_dark: BTreeMap<u64, bool>,
    // Drag timelines (LLP 1057.003): sources and bound consumers.
    timelines: timeline::Timelines,
    // Clock timelines (LLP 1055.002): each node's, and each one's origin.
    clocks: clock::Clocks,
    // Each path's `d`, and the two ends of its transition (LLP 1055.000
    // D15); the progress is the node's `Property::D` slot.
    paths: BTreeMap<u64, path::PathTrack>,
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

    /// Return the tables' spare room: they grow to the most nodes ever
    /// observed at once (a list mid-fling), and settle smaller. Only a table
    /// holding more than twice what it needs shrinks, so a steady one never
    /// churns.
    pub fn trim(&mut self) {
        if self.slots.capacity() > 2 * self.slots.len().max(256) {
            self.slots.shrink_to_fit();
        }
        if self.dirty.capacity() > 2 * self.dirty.len().max(256) {
            self.dirty.shrink_to_fit();
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

    /// Forget a node entirely.
    pub fn remove(&mut self, node: u64) {
        self.transitions.remove(&node);
        self.layout.remove(&node);
        self.node_dark.remove(&node);
        self.animations.remove(&node);
        self.animating.remove(&node);
        self.forced.remove(&node);
        self.forget_timelines(node);
        self.forget_clock(node);
        self.paths.remove(&node);
        // Removing a list must not scan every other node once per row, nor
        // probe every table once per property: a list row's retirement
        // removes a node per box. Its running curves are one range of the
        // ordered index; its pending frame entries (`dirty`) present nothing
        // once its slots and animations are gone (`frame` finds neither), so
        // the next frame's drain takes them.
        let running: Vec<(u64, Property)> = self
            .running
            .range((node, Property::Translate)..)
            .take_while(|key| key.0 == node)
            .copied()
            .collect();
        for key in running {
            self.running.remove(&key);
        }
        for property in Property::ALL {
            self.slots.remove(&(node, property));
        }
        self.slots.remove(&(node, Property::Layout));
        self.slots.remove(&(node, Property::D));
    }

    /// Forget only this property's target, curve, hold and pending frame.
    /// Returns whether it existed. Other properties and the node's transition
    /// declaration survive; readoption takes a new value without transitioning.
    /// No clock change occurs, and old hold tokens immediately become stale.
    pub fn remove_property(&mut self, node: u64, property: Property) -> bool {
        if property == Property::D {
            self.paths.remove(&node);
        }
        self.dirty.remove(&(node, property));
        self.running.remove(&(node, property));
        self.slots.remove(&(node, property)).is_some()
    }

    /// Whether this property is held or has a running curve, including delay.
    /// Equality with its target does not imply rest: a spring may carry velocity
    /// at zero displacement. Holds are active here but remain clock-quiescent.
    pub fn is_active(&self, node: u64, property: Property) -> bool {
        self.slots.get(&(node, property)).is_some_and(|slot| {
            matches!(slot.owner(), Some(Owner::Held(_))) || slot.running().is_some()
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
        // Layout moves only under `layout-transition`; a spring on a
        // property no spring drives as physics is its curve from rest (LLP
        // 1062 D3).
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
            self.slots.insert(key, Slot::settled(change.value));
            self.dirty.insert(key);
            return Ok(());
        };

        let after = change.value;
        // A played transition comes back to be sampled (or played again):
        // a retarget measures from its curve, as for a running one.
        self.played.remove(&key);
        if matches!(slot.owner(), Some(Owner::Held(_))) {
            slot.set_target(after);
            return Ok(());
        }
        match slot.take_running() {
            None => {
                if after == slot.target {
                    return Ok(());
                }
                slot.set_owner(None);
                let before = slot.presented();
                slot.set_target(after);
                match declaration {
                    Some(declaration) => {
                        let velocity = change.velocity.unwrap_or(Value::ZERO);
                        let running =
                            Running::start(&declaration, before, after, velocity, now, before, 1.0);
                        let presented = running.sample(now).value;
                        slot.set_running(Some(running));
                        slot.set_presented(presented);
                    }
                    None => slot.set_presented(after),
                }
            }
            Some(running) => {
                if after == running.to {
                    slot.set_running(Some(running));
                    return Ok(());
                }
                slot.set_owner(None);
                let current = running.sample(now);
                slot.set_target(after);
                let Some(declaration) = declaration.filter(|_| current.value != after) else {
                    slot.set_presented(after);
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
                let presented = next.sample(now).value;
                slot.set_running(Some(next));
                slot.set_presented(presented);
            }
        }
        if slot.running().is_some() {
            self.running.insert(key);
        }
        self.dirty.insert(key);
        Ok(())
    }

    /// A change no author made — a list's rows moved because rows above them
    /// were built or measured — taken with no transition: an idle property's
    /// target and presentation both become the value, so nothing moves on
    /// screen and nothing is left to present. A property that is running or
    /// held is observed as an ordinary change.
    pub fn observe_settled(&mut self, change: Change) -> Result<(), EngineError> {
        validate_value(change.property, change.value)?;
        let key = (change.node, change.property);
        match self.slots.get_mut(&key) {
            Some(slot) if slot.running().is_none() && slot.owner().is_none() => {
                slot.set_target(change.value);
                slot.set_presented(change.value);
                self.played.remove(&key);
                Ok(())
            }
            _ => self.observe(change),
        }
    }

    /// Move the clock to `now` and sample every running transition there.
    /// Seeking is the only operation: the result depends on `now`, never on
    /// how many calls it took to get there.
    pub fn advance(&mut self, now: f64) -> Result<(), EngineError> {
        self.validate_time(now)?;
        self.now = now;
        self.running.retain(|key| {
            let slot = self.slots.get_mut(key).expect("running slot");
            let sample = slot.running().expect("indexed curve").sample(now);
            // Done presents the target in its own encoding: the curve may
            // have run in Oklab (LLP 1100 D2).
            slot.set_presented(if sample.done {
                slot.target
            } else {
                sample.value
            });
            if sample.done {
                slot.set_running(None);
                slot.set_owner(None);
            }
            self.dirty.insert(*key);
            !sample.done
        });
        self.retire_played(now);
        self.advance_animations();
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
        // A timeline's consumers are held where its source now is, so this
        // frame carries both (LLP 1057.003 D2).
        self.seek_timelines();
        let mut dirty: Vec<_> = std::mem::take(&mut self.dirty).into_iter().collect();
        dirty.sort_unstable();
        dirty
            .into_iter()
            .filter_map(|key| {
                let slot = self.slots.get(&key);
                let underlying = slot.map(Slot::presented);
                // A sampling host paints an animation over the property's own
                // value; a running transition is above animations in the CSS
                // cascade, so it wins while it runs (LLP 1055 D5).
                let transitioning = slot.is_some_and(|s| s.running().is_some());
                let animated = (!self.lowered_for(key.0, key.1) && !transitioning)
                    .then(|| {
                        let base = underlying.or_else(|| base(key.1))?;
                        self.animated(key.0, key.1, base)
                    })
                    .flatten();
                animated.or(underlying).map(|value| Presentation {
                    node: key.0,
                    property: key.1,
                    value,
                })
            })
            .collect()
    }

    /// The running spring's identity, without allocating, sampling, or computing
    /// its settle time. After one slot lookup this only copies fixed-size fields.
    /// `None` for held, settled, unknown properties and easings. A host may lower
    /// frames only when this descriptor differs from its last playback.
    pub fn spring_descriptor(&self, node: u64, property: Property) -> Option<SpringDescriptor> {
        let running = self.slots.get(&(node, property))?.running()?;
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
        let running = self.slots.get(&(node, property))?.running()?;
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
        self.slots.get(&(node, property)).map(Slot::presented)
    }

    /// What a sampling host paints for one property now: a sampled
    /// animation over the property's own value, unless a transition runs
    /// (it wins) or the animation is lowered (the compositor plays it); else
    /// the slot's value. `frame` reports the same values as they change.
    pub fn sampled_value(&self, node: u64, property: Property) -> Option<Value> {
        let slot = self.slots.get(&(node, property));
        let underlying = slot.map(Slot::presented);
        if slot.is_some_and(|s| s.running().is_some()) || self.lowered_for(node, property) {
            return underlying;
        }
        let base = underlying.or_else(|| base(property))?;
        self.animated(node, property, base).or(underlying)
    }

    /// The current target of one property.
    pub fn target(&self, node: u64, property: Property) -> Option<Value> {
        self.slots.get(&(node, property)).map(|s| s.target)
    }

    /// Whether nothing is running.
    pub fn quiescent(&self) -> bool {
        self.running.is_empty() && self.animating.is_empty()
    }

    /// Whether anything moving changes where or how big something is —
    /// `translate`, `scale`, `rotate`, `height`, layout, SVG geometry —
    /// which a panel's full rate keeps from juddering (LLP 1061 D4). A fade
    /// or a colour change reads the same at 60 Hz, so a slow breathing
    /// opacity need not hold a 120 Hz display at 120 Hz.
    pub fn spatial(&self) -> bool {
        let spatial =
            |p: Property| !p.is_color() && !matches!(p, Property::Opacity | Property::BoxShadow);
        self.running.iter().any(|&(_, p)| spatial(p))
            || self
                .animating
                .iter()
                .flat_map(|node| &self.animations[node])
                .filter(|p| p.hold.is_none() && p.local(self.now) <= p.animation.end_time())
                .any(|p| p.animation.keyframes.properties().into_iter().any(spatial))
    }

    /// The clock time at which the last running transition or finite
    /// animation ends, or `None` when none is. An agent advances here instead
    /// of waiting; an infinite animation never settles (LLP 1055 D10).
    pub fn settle_time(&self) -> Option<f64> {
        self.running
            .iter()
            .map(|key| self.slots[key].running().expect("indexed curve").end_time())
            .chain(self.animations_settle_time())
            .fold(None, |acc, t| Some(acc.map_or(t, |a: f64| a.max(t))))
    }
}

/// What an animation samples over when the property has no value of its
/// own: its numeric initial value, or for a colour with none (paint `none`)
/// transparent, so keyframes that give both ends still play.
fn base(property: Property) -> Option<Value> {
    property
        .identity()
        .or_else(|| property.is_color().then_some(Value::ZERO))
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
        // A settled slot is its target alone.
        assert!(engine.slots.values().all(|slot| slot.live.is_none()));
        assert_eq!(std::mem::size_of::<Slot>(), 48);

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
        assert!(engine.slots[&(7, Property::Opacity)].live.is_none());
        engine.observe(change(7, 1.0)).unwrap();
        assert_eq!(engine.running.len(), 1);
        engine.remove_property(7, Property::Opacity);
        assert!(engine.running.is_empty());
    }
}
